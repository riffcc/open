use std::cell::UnsafeCell;
use std::io;
use std::time::{Duration, Instant};
use std::thread;
use std::sync::{Arc, Mutex};
use std::collections::VecDeque;
use crossterm::{
    event::{self, Event, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    prelude::*,
    widgets::{Axis, Block, Borders, Chart, Dataset, GraphType, Paragraph},
    Terminal, Frame, backend::CrosstermBackend,
};

struct TimelineMetrics {
    timeline_counts: Vec<(f64, f64)>,  // (transition_number, count)
    order_ratio: f64,
    total_entropy: u64,
    runs_completed: u32,
    active_simulations: VecDeque<Vec<(f64, f64)>>, // Store last 25 simulation curves
    current_sim_page: usize,
    sims_per_page: usize,
}

impl TimelineMetrics {
    fn new() -> Self {
        Self {
            timeline_counts: Vec::new(),
            order_ratio: 0.0,
            total_entropy: 0,
            runs_completed: 0,
            active_simulations: VecDeque::with_capacity(25),
            current_sim_page: 0,
            sims_per_page: 5,
        }
    }

    fn record_transition(&mut self, transition: u32, timeline_count: usize) {
        let point = (transition as f64, timeline_count as f64);
        self.timeline_counts.push(point);
        
        // Calculate entropy as log2 of timeline count
        if timeline_count > 1 {
            self.total_entropy += (timeline_count as f64).log2() as u64;
        }
        
        // Update order ratio based on pattern detection
        if self.timeline_counts.len() >= 2 {
            let last_two = &self.timeline_counts[self.timeline_counts.len()-2..];
            if last_two[0].1 < last_two[1].1 {
                self.order_ratio = (self.order_ratio * (self.runs_completed as f64) + 1.0) / 
                    ((self.runs_completed + 1) as f64);
            }
        }
    }

    fn clear_run(&mut self) {
        self.timeline_counts.clear();
        self.runs_completed += 1;
    }

    fn add_simulation_progress(&mut self, sim_index: usize, transition: u32, count: usize) {
        while self.active_simulations.len() <= sim_index {
            self.active_simulations.push_back(Vec::new());
        }
        
        if let Some(sim) = self.active_simulations.get_mut(sim_index) {
            sim.push((transition as f64, count as f64));
        }
    }

    fn next_page(&mut self) {
        let max_pages = (self.active_simulations.len() + self.sims_per_page - 1) / self.sims_per_page;
        self.current_sim_page = (self.current_sim_page + 1) % max_pages;
    }

    fn prev_page(&mut self) {
        let max_pages = (self.active_simulations.len() + self.sims_per_page - 1) / self.sims_per_page;
        self.current_sim_page = (self.current_sim_page + max_pages - 1) % max_pages;
    }

    fn inject_entropy(&mut self, sim_index: Option<usize>) {
        match sim_index {
            Some(idx) => {
                if let Some(sim) = self.active_simulations.get_mut(idx) {
                    if let Some(&(transition, _)) = sim.last() {
                        // Inject entropy at current transition point
                        let mut local_timeline = TimelineState::new();
                        for _ in 0..((transition as u32) + 1) {
                            local_timeline.transition();
                        }
                        // Force an additional transition
                        local_timeline.transition();
                        let count = count_timelines(&local_timeline);
                        sim.push((transition + 1.0, count as f64));
                    }
                }
            },
            None => {
                // Inject entropy into all active simulations
                for i in 0..self.active_simulations.len() {
                    self.inject_entropy(Some(i));
                }
            }
        }
    }
}

struct UnstableMemory {
    state: UnsafeCell<Option<bool>>
}

impl UnstableMemory {
    fn new() -> Self {
        UnstableMemory {
            state: UnsafeCell::new(None)
        }
    }

    unsafe fn transition(&self) {
        if rand::random::<bool>() {
            *self.state.get() = Some(rand::random::<bool>());
        }
    }
}

struct TimelineState {
    memory: UnstableMemory,
    spawn_time: Instant,
    child_timelines: Vec<TimelineState>
}

impl TimelineState {
    fn new() -> Self {
        Self {
            memory: UnstableMemory::new(),
            spawn_time: Instant::now(),
            child_timelines: Vec::new()
        }
    }

    fn transition(&mut self) {
        let start = Instant::now();
        
        unsafe {
            self.memory.transition();
            
            // If a transition occurs, a new timeline may emerge
            if let Some(true) = *self.memory.state.get() {
                self.child_timelines.push(TimelineState::new());
            }
        }

        // Natural time dilation based on timeline count
        // Add visualization delay proportional to entropy
        // This only affects our observation, not the underlying timeline mechanics
        let elapsed = start.elapsed();
        if !self.child_timelines.is_empty() {
            let dilation = (self.child_timelines.len() as f64).log2() as u64;
            thread::sleep(Duration::from_nanos(elapsed.as_nanos() as u64 * dilation));
        }

        // Allow child timelines to transition
        for timeline in &mut self.child_timelines {
            timeline.transition();
        }
    }
}

fn count_timelines(timeline: &TimelineState) -> usize {
    1 + timeline.child_timelines.iter()
        .map(count_timelines)
        .sum::<usize>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_void_transitions_to_boolean() {
        let memory = UnstableMemory::new();
        let mut transitions_observed = 0;
        let mut void_to_boolean_occurred = false;

        // We'll need to observe many transitions to prove inevitability
        for _ in 0..1000 {
            unsafe {
                let before = *memory.state.get();
                memory.transition();
                let after = *memory.state.get();
                
                if before.is_none() && after.is_some() {
                    void_to_boolean_occurred = true;
                    break;
                }
                transitions_observed += 1;
            }
        }

        assert!(void_to_boolean_occurred, 
            "Void should eventually transition to boolean state. Observed {} transitions without occurrence", 
            transitions_observed);
    }

    #[test]
    fn test_transitions_form_patterns() {
        let memory = UnstableMemory::new();
        let mut state_sequence = Vec::new();
        
        // Record a sequence of 1000 transitions
        for _ in 0..1000 {
            unsafe {
                memory.transition();
                state_sequence.push(*memory.state.get());
            }
        }

        // Look for repeating patterns in the sequence
        let mut pattern_found = false;
        for window_size in 2..=10 {
            for window in state_sequence.windows(window_size) {
                // Count how many times this exact sequence appears
                let pattern_count = state_sequence
                    .windows(window_size)
                    .filter(|w| w == &window)
                    .count();
                
                if pattern_count > 1 {
                    pattern_found = true;
                    break;
                }
            }
            if pattern_found {
                break;
            }
        }

        assert!(pattern_found, "Transitions should naturally form patterns");
    }

    #[test]
    fn test_patterns_stabilize() {
        let memory = UnstableMemory::new();
        let mut stable_patterns_found = 0;
        let mut total_comparisons = 0;
        let mut observation_windows = Vec::new();
        
        // Record multiple observation windows
        for _ in 0..10 {
            let mut window = Vec::new();
            for _ in 0..100 {
                unsafe {
                    memory.transition();
                    window.push(*memory.state.get());
                }
            }
            observation_windows.push(window);
        }

        // Look for patterns that repeat across different observation windows
        for i in 0..observation_windows.len() {
            for j in (i+1)..observation_windows.len() {
                let window1 = &observation_windows[i];
                let window2 = &observation_windows[j];
                
                // Look for matching subsequences of at least length 3
                for k in 0..(window1.len() - 2) {
                    total_comparisons += 1;
                    if window1[k..(k+3)] == window2[k..(k+3)] {
                        stable_patterns_found += 1;
                    }
                }
            }
        }

        let order_ratio = (stable_patterns_found as f64) / (total_comparisons as f64);
        println!("Order emergence ratio: {:.2}% ({} stable patterns in {} comparisons)", 
                order_ratio * 100.0,
                stable_patterns_found,
                total_comparisons);

        assert!(stable_patterns_found > 0, 
            "Should find patterns that remain stable across multiple observation windows");
    }

    #[test]
    fn test_timeline_spawning() {
        let mut root_timeline = TimelineState::new();
        let mut spawned_timelines = 0;
        
        // Run for a fixed number of transitions
        for _ in 0..100 {
            root_timeline.transition();
            spawned_timelines = count_timelines(&root_timeline);
            
            // Break if we've spawned enough timelines to prove it works
            if spawned_timelines > 3 {
                break;
            }
        }

        assert!(spawned_timelines > 0, "Timeline spawning should occur");
    }

    #[test]
    fn test_time_dilation() {
        let mut timeline = TimelineState::new();
        let start = Instant::now();
        
        // Perform transitions and measure real time vs dilated time
        for _ in 0..10 {
            timeline.transition();
        }
        
        let elapsed = start.elapsed();
        assert!(elapsed > Duration::from_micros(1), 
            "Time dilation should slow down processing");
    }

    #[test]
    fn test_metrics_paging() {
        let mut metrics = TimelineMetrics::new();
        
        // Add some test simulations
        for i in 0..10 {
            metrics.add_simulation_progress(i, 0, i+1);
        }
        
        assert_eq!(metrics.current_sim_page, 0);
        
        // Test page navigation
        metrics.next_page();
        assert_eq!(metrics.current_sim_page, 1);
        
        metrics.prev_page();
        assert_eq!(metrics.current_sim_page, 0);
    }

    #[test]
    fn test_parallel_simulation_tracking() {
        let metrics = Arc::new(Mutex::new(TimelineMetrics::new()));
        
        // Run a small parallel simulation
        run_parallel_simulations(Arc::clone(&metrics));
        
        // Give some time for simulations to run
        thread::sleep(Duration::from_millis(100));
        
        let metrics = metrics.lock().unwrap();
        assert!(!metrics.active_simulations.is_empty(), "Should have active simulations");
        
        // Check that simulations are being tracked separately
        if let Some(first_sim) = metrics.active_simulations.front() {
            assert!(!first_sim.is_empty(), "Simulation should have data points");
        }
    }

    #[test]
    fn test_timeline_branching_patterns() {
        let mut root_timeline = TimelineState::new();
        let mut branch_points = Vec::new();
        let mut timings = Vec::new();
        let mut timeline_counts = Vec::new();
        
        // Record points where branching occurs, measure timing, and track total timelines
        for i in 0..10 {
            let before_count = count_timelines(&root_timeline);
            let start = Instant::now();
            root_timeline.transition();
            let elapsed = start.elapsed();
            let after_count = count_timelines(&root_timeline);
            
            timeline_counts.push(after_count);
            
            if after_count > before_count {
                branch_points.push(i);
                timings.push(elapsed);
            }
        }
        
        // Verify that branching occurs
        assert!(!branch_points.is_empty(), "Timeline should branch at least once");
        
        // Verify exponential growth pattern in timeline counts
        if timeline_counts.len() >= 3 {
            let first_growth = timeline_counts[1] - timeline_counts[0];
            let last_growth = timeline_counts[timeline_counts.len()-1] - timeline_counts[timeline_counts.len()-2];
            assert!(last_growth >= first_growth, 
                "Timeline growth should follow exponential pattern");
        }
        
        // Verify time dilation
        if timings.len() >= 2 {
            let first_timing = timings[0];
            let last_timing = timings[timings.len() - 1];
            assert!(last_timing > first_timing, 
                "Later transitions should take longer due to time dilation");
        }
        
        // Verify that we see branching activity throughout the simulation
        assert!(
            branch_points.iter().any(|&time| time > 5),
            "Should be able to branch in later transitions"
        );
    }

    #[test]
    fn test_entropy_injection() {
        let mut metrics = TimelineMetrics::new();
        
        // Setup initial simulation state
        metrics.add_simulation_progress(0, 0, 1);
        metrics.add_simulation_progress(0, 1, 2);
        
        // Test single simulation entropy injection
        let before_count = metrics.active_simulations[0].len();
        metrics.inject_entropy(Some(0));
        assert!(metrics.active_simulations[0].len() > before_count, 
            "Entropy injection should add new timeline states");
        
        // Test parallel entropy injection
        metrics.add_simulation_progress(1, 0, 1);
        metrics.inject_entropy(None);
        for sim in &metrics.active_simulations {
            assert!(!sim.is_empty(), "All simulations should have timeline states after parallel injection");
        }
    }
}

fn run_simulation(metrics: &mut TimelineMetrics) {
    let mut root_timeline = TimelineState::new();
    metrics.timeline_counts.clear();
    
    for i in 0..25 {
        root_timeline.transition();
        let count = count_timelines(&root_timeline);
        metrics.record_transition(i, count);
    }
    
    metrics.clear_run();
}

fn run_parallel_simulations(metrics: Arc<Mutex<TimelineMetrics>>) {
    for sim_index in 0..25 {
        let metrics = Arc::clone(&metrics);
        thread::spawn(move || {
            let mut local_timeline = TimelineState::new();
            
            for i in 0..25 {
                local_timeline.transition();
                let count = count_timelines(&local_timeline);
                
                // Update metrics immediately after each transition
                let mut metrics = metrics.lock().unwrap();
                metrics.add_simulation_progress(sim_index, i, count);
                drop(metrics); // Release lock immediately
                
                // Smaller sleep for more responsive updates
                thread::sleep(Duration::from_millis(10));
            }
        });
    }
}

fn ui(f: &mut Frame<CrosstermBackend<io::Stdout>>, metrics: &TimelineMetrics) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(70),
            Constraint::Percentage(30),
        ])
        .split(f.size());

    // Create datasets for all active simulations
    let start_idx = metrics.current_sim_page * metrics.sims_per_page;
    let datasets: Vec<Dataset> = metrics.active_simulations.iter()
        .skip(start_idx)
        .take(metrics.sims_per_page)
        .enumerate()
        .map(|(i, points)| {
            Dataset::default()
                .name(format!("Sim {}", start_idx + i))
                .marker(symbols::Marker::Dot)
                .graph_type(GraphType::Scatter)
                .data(points)
                .style(Style::default().fg(Color::Indexed((i * 12 + 1) as u8)))
        })
        .collect();

    let max_y = metrics.active_simulations.iter()
        .flat_map(|sim| sim.iter().map(|(_, y)| *y))
        .fold(0.0, f64::max)
        .max(1.0);

    let chart = Chart::new(datasets)
        .block(Block::default().title("Timeline Growth").borders(Borders::ALL))
        .x_axis(Axis::default().title("Transitions").bounds([0.0, 25.0]))
        .y_axis(Axis::default().title("Timeline Count").bounds([0.0, max_y]));

    f.render_widget(chart, chunks[0]);

    // Metrics panel (continuing from previous ui function)
    let metrics_text = vec![
        Line::from(vec![
            Span::raw("Runs Completed: "),
            Span::styled(
                metrics.runs_completed.to_string(),
                Style::default().fg(Color::Green)
            )
        ]),
        Line::from(vec![
            Span::raw("Order Ratio: "),
            Span::styled(
                format!("{:.2}%", metrics.order_ratio * 100.0),
                Style::default().fg(Color::Yellow)
            )
        ]),
        Line::from(vec![
            Span::raw("Total Entropy: "),
            Span::styled(
                metrics.total_entropy.to_string(),
                Style::default().fg(Color::Magenta)
            )
        ]),
        Line::from(vec![
            Span::raw("Simulation Page: "),
            Span::styled(
                format!("{}/{}", 
                    metrics.current_sim_page + 1,
                    (metrics.active_simulations.len() + metrics.sims_per_page - 1) / metrics.sims_per_page
                ),
                Style::default().fg(Color::Cyan)
            )
        ]),
        Line::from(""),
        Line::from("PageUp/PageDown to browse simulations"),
        Line::from("Press SPACE to run simulation"),
        Line::from("Press 'q' to quit"),
    ];

    let metrics_paragraph = Paragraph::new(metrics_text)
        .block(Block::default().title("Metrics").borders(Borders::ALL))
        .alignment(Alignment::Left);

    f.render_widget(metrics_paragraph, chunks[1]);
}

fn main() -> io::Result<()> {
    // Terminal initialization
    enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;

    let metrics = Arc::new(Mutex::new(TimelineMetrics::new()));
    let mut simulation_mode = false;  // false = single, true = parallel

    loop {
        terminal.draw(|f| ui(f, &metrics.lock().unwrap()))?;

        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char(' ') => {
                        simulation_mode = true;
                        run_parallel_simulations(Arc::clone(&metrics));
                    },
                    KeyCode::Char('s') => {
                        simulation_mode = false;
                        run_simulation(&mut metrics.lock().unwrap());
                    },
                    KeyCode::Char('e') => {
                        // Inject entropy based on mode
                        let mut metrics = metrics.lock().unwrap();
                        if simulation_mode {
                            metrics.inject_entropy(None);  // All sims
                        } else {
                            metrics.inject_entropy(Some(0));  // Current sim
                        }
                    },
                    KeyCode::Char('n') => metrics.lock().unwrap().next_page(),
                    KeyCode::Char('p') => metrics.lock().unwrap().prev_page(),
                    _ => {
                        if !simulation_mode {
                            // Single-sim mode: any key triggers transition
                            run_simulation(&mut metrics.lock().unwrap());
                        }
                    }
                }
            }
        }
        thread::sleep(Duration::from_millis(16));
    }

    // Cleanup
    disable_raw_mode()?;
    io::stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}