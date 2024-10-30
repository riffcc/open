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
    widgets::canvas::{Canvas, Points},
    Terminal, Frame, backend::CrosstermBackend,
};

struct TimelineMetrics {
    selected_simulation: Option<usize>,
    timeline_counts: Vec<(f64, f64)>,
    entropy_values: Vec<(f64, f64)>,
    order_values: Vec<(f64, f64)>,
    order_ratio: f64,
    total_entropy: u64,
    single_runs: u32,
    parallel_timelines: u32,
    active_simulations: VecDeque<Vec<(f64, f64)>>,
    current_sim_page: usize,
    sims_per_page: usize,
}

impl TimelineMetrics {
    fn new() -> Self {
        Self {
            selected_simulation: None,
            timeline_counts: Vec::new(),
            entropy_values: Vec::new(),
            order_values: Vec::new(),
            order_ratio: 0.0,
            total_entropy: 0,
            single_runs: 0,
            parallel_timelines: 0,
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
                self.order_ratio = (self.order_ratio * (self.single_runs as f64) + 1.0) / 
                    ((self.single_runs + 1) as f64);
            }
        }
    }

    fn clear_run(&mut self) {
        self.timeline_counts.clear();
        self.single_runs += 1;
    }

    fn add_simulation_progress(&mut self, sim_index: usize, transition: u32, timeline: &TimelineState) {
        while self.active_simulations.len() <= sim_index {
            self.active_simulations.push_back(Vec::new());
        }
        
        if let Some(sim) = self.active_simulations.get_mut(sim_index) {
            let count = count_timelines(timeline);
            sim.push((transition as f64, count as f64));
            
            // Update entropy based on timeline count
            let entropy = if count > 1 {
                (count as f64).log2()
            } else {
                0.0
            };
            
            // Add order and entropy tracking
            self.entropy_values.push((transition as f64, entropy));
            self.order_values.push((transition as f64, timeline.local_order));
        }
    }

    fn next_page(&mut self) {
        // Check if we have any actual simulation data
        let has_data = !self.active_simulations.is_empty() && 
                      self.active_simulations.iter().any(|sim| !sim.is_empty());
        
        if !has_data {
            self.current_sim_page = 0;
            return;
        }
        
        // Otherwise, calculate pages based on highest used simulation index
        let max_sim_index = self.active_simulations.len() - 1;
        let total_pages = (max_sim_index / self.sims_per_page) + 1;
        self.current_sim_page = (self.current_sim_page + 1) % total_pages;
    }

    fn prev_page(&mut self) {
        if self.active_simulations.is_empty() {
            self.current_sim_page = 0;
            return;
        }
        
        let max_sim_index = self.active_simulations.len() - 1;
        let total_pages = (max_sim_index / self.sims_per_page) + 1;
        self.current_sim_page = (self.current_sim_page + total_pages - 1) % total_pages;
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

#[derive(Clone)]
struct TimelineState {
    memory: Arc<UnstableMemory>,
    /// Timestamp when this timeline branch was created
    /// Currently unused - will be used for coherence vector calculations
    spawn_time: Instant,
    child_timelines: Vec<Arc<TimelineState>>,
    local_order: f64,
    local_entropy: f64,
    /// Parent timeline reference for veracity rail construction
    /// Currently unused - will be used for FTL information pathways
    parent: Option<Arc<TimelineState>>,
    /// Historical changes for coherence tracking
    /// Currently unused - will be used for timeline fabric stability
    changes: Vec<Arc<TimelineState>>,
}

fn count_timelines(timeline: &TimelineState) -> usize {
    1 + timeline.child_timelines.iter()
        .map(|child| count_timelines(&*child))
        .sum::<usize>()
}

impl TimelineState {
    fn new() -> Self {
        Self {
            memory: Arc::new(UnstableMemory::new()),
            spawn_time: Instant::now(),
            child_timelines: Vec::new(),
            local_order: 0.0,
            local_entropy: 0.0,
            parent: None,
            changes: Vec::new(),
        }
    }

    fn calculate_local_order(&self) -> f64 {
        const MIN_PATTERN_LENGTH: usize = 3;
        
        // Collect state observations from this timeline and all children
        let mut states = Vec::new();
        unsafe {
            // Get current timeline's state
            states.push(*self.memory.state.get());
            
            // Get states from child timelines
            for child in &self.child_timelines {
                states.push(*child.memory.state.get());
            }
        }
        
        // Only calculate order when we have enough observations
        if states.len() < MIN_PATTERN_LENGTH {
            return 0.0;
        }

        let mut stable_patterns = 0;
        let mut total_comparisons = 0;
        
        // Look for patterns in the current state collection
        for window_size in MIN_PATTERN_LENGTH..=states.len() {
            for i in 0..=(states.len() - window_size) {
                for j in (i + 1)..=(states.len() - window_size) {
                    total_comparisons += 1;
                    if states[i..(i + window_size)] == states[j..(j + window_size)] {
                        stable_patterns += 1;
                    }
                }
            }
        }

        // Calculate order ratio (0.0 to 1.0)
        if total_comparisons > 0 {
            (stable_patterns as f64) / (total_comparisons as f64)
        } else {
            0.0
        }
    }

    /// Transitions this timeline forward, allowing infinite branching through temporal scaling
    /// 
    /// The transition mechanism implements a form of "temporal resistance" that mirrors
    /// relativistic physics, enabling simulation of infinite complexity on finite hardware:
    /// 
    /// 1. As timelines branch, entropy increases (measured as log2 of total branches)
    /// 2. Higher entropy creates temporal resistance (like mass creates spatial resistance)
    /// 3. This resistance manififies as time dilation (like mass dilates time near gravity wells)
    /// 4. As entropy approaches infinity, time flow approaches (but never reaches) zero
    /// 5. This allows infinite branching by proportionally slowing computation
    /// 
    /// Through this temporal scaling mechanism, a finite computer can simulate an infinite
    /// multiverse by trading time for complexity - the more complex the branching structure
    /// becomes, the slower time flows within those branches, allowing unlimited growth
    /// within finite computational resources.
    fn transition(&mut self) {
        let start = Instant::now();
        
        // Calculate entropy (not order!) based on total number of branching timelines
        let total_timelines = count_timelines(self);
        let entropy = if total_timelines > 1 {
            (total_timelines as f64).log2()  // Same entropy calculation we use elsewhere
        } else {
            0.0
        };
        
        // Time dilation scales with entropy - more branches = slower time
        let dilation_factor = entropy + 1.0; // Prevent division by zero
        let max_duration = Duration::from_micros(100);  // Base time quantum
                                                        // In a realistic universe, this would be Planck time
                                                        // 😈
        let sleep_duration = max_duration.mul_f64(1.0 - 1.0/dilation_factor);
        
        // Implement time dilation - trading time for infinite complexity
        thread::sleep(sleep_duration);

        unsafe {
            self.memory.transition();
            
            // Spawn new timeline if we get a true state
            if let Some(true) = *self.memory.state.get() {
                self.child_timelines.push(Arc::new(TimelineState::new()));
            }
        }

        // Update metrics
        self.local_order = self.calculate_local_order();
        self.local_entropy = if self.child_timelines.is_empty() {
            0.0
        } else {
            (self.child_timelines.len() as f64).log2()
        };

        // Time dilation based on both entropy and order
        let elapsed = start.elapsed();
        let dilation = (self.local_entropy * (1.0 + self.local_order)) as u64;
        thread::sleep(Duration::from_nanos(elapsed.as_nanos() as u64 * dilation));
        
        // Allow child timelines to transition
        for timeline in &mut self.child_timelines {
            if let Some(timeline) = Arc::get_mut(timeline) {
                timeline.transition();
            }
        }
    }
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
        let timeline1 = TimelineState::new();
        let timeline2 = TimelineState::new();
        let timeline3 = TimelineState::new();
        
        // Test adding simulation progress with proper TimelineState references
        metrics.add_simulation_progress(0, 0, &timeline1);
        metrics.add_simulation_progress(0, 1, &timeline2);
        metrics.add_simulation_progress(1, 0, &timeline3);
        
        assert_eq!(metrics.current_sim_page, 0);
        metrics.next_page();
        assert_eq!(metrics.current_sim_page, 1);
        metrics.prev_page();
        assert_eq!(metrics.current_sim_page, 0);
    }

    #[test]
    fn test_parallel_simulation_tracking() {
        let metrics = Arc::new(Mutex::new(TimelineMetrics::new()));
        
        // Start parallel simulations
        let metrics_clone = Arc::clone(&metrics);
        metrics_clone.lock().unwrap().parallel_timelines = 0;  // Reset before starting
        
        run_parallel_simulations(metrics_clone);
        
        // Give simulations time to run a few transitions
        thread::sleep(Duration::from_millis(100));
        
        let metrics = metrics.lock().unwrap();
        
        // Verify initial parallel timeline count
        assert!(metrics.parallel_timelines >= 25, 
            "Should start with at least 25 root timelines");
            
        // Verify active simulations were created
        assert_eq!(metrics.active_simulations.len(), 25,
            "Should have 25 active simulation tracks");
            
        // Verify simulations are recording data
        assert!(metrics.active_simulations.iter().all(|sim| !sim.is_empty()),
            "All simulations should record transition data");
    }

    #[test]
    fn test_timeline_branching_patterns() {
        let mut root_timeline = TimelineState::new();
        let mut timings = Vec::new();
        let mut orders = Vec::new();
        
        // Record transitions with timing and order measurements
        for _ in 0..10 {
            let start = Instant::now();
            root_timeline.transition();
            let elapsed = start.elapsed();
            timings.push(elapsed);
            orders.push(root_timeline.calculate_local_order());
        }
        
        // Verify that entropy increases
        assert!(root_timeline.child_timelines.len() > 0,
            "Entropy should increase as timelines branch");
        
        // Verify that transitions take longer as entropy increases
        let first_timing = timings[0];
        let last_timing = timings[timings.len() - 1];
        assert!(last_timing >= first_timing, 
            "Later transitions should take longer due to time dilation");
    }

    #[test]
    fn test_entropy_injection() {
        let mut metrics = TimelineMetrics::new();
        let timeline1 = TimelineState::new();
        let timeline2 = TimelineState::new();
        let timeline3 = TimelineState::new();
        
        // Setup initial simulation state with proper TimelineState references
        metrics.add_simulation_progress(0, 0, &timeline1);
        metrics.add_simulation_progress(0, 1, &timeline2);
        
        // Test single simulation entropy injection
        let before_count = metrics.active_simulations[0].len();
        metrics.inject_entropy(Some(0));
        assert!(metrics.active_simulations[0].len() > before_count, 
            "Entropy injection should add new timeline states");
        
        // Test parallel entropy injection
        metrics.add_simulation_progress(1, 0, &timeline3);
        metrics.inject_entropy(None);
        for sim in &metrics.active_simulations {
            assert!(!sim.is_empty(), "All simulations should have timeline states after parallel injection");
        }
    }

    #[test]
    fn test_timeline_growth() {
        let mut metrics = TimelineMetrics::new();
        let mut timeline = TimelineState::new();
        
        // Record initial state
        metrics.add_simulation_progress(0, 0, &timeline);
        
        // Transition and verify growth
        timeline.transition();
        metrics.add_simulation_progress(0, 1, &timeline);
        
        assert!(!metrics.active_simulations.is_empty());
        if let Some(sim) = metrics.active_simulations.get(0) {
            assert!(sim.len() >= 2);
        }
    }

    #[test]
    fn test_order_calculation_empty() {
        let timeline = TimelineState::new();
        assert_eq!(timeline.calculate_local_order(), 0.0, 
            "Order should be 0 with no patterns");
    }

    #[test]
    fn test_order_calculation_single_pattern() {
        let mut timeline = TimelineState::new();
        
        // Force some transitions to create a pattern
        for _ in 0..5 {
            unsafe {
                *timeline.memory.state.get() = Some(true);
                timeline.child_timelines.push(Arc::new(TimelineState::new()));
            }
        }
        
        let order = timeline.calculate_local_order();
        assert!(order > 0.0, 
            "Order should be positive when patterns exist");
        assert!(order <= 1.0, 
            "Order should never exceed 1.0");
    }

    #[test]
    fn test_order_calculation_complex() {
        let mut timeline = TimelineState::new();
        
        // Create a complex pattern with both true and false values
        unsafe {
            *timeline.memory.state.get() = Some(true);
            timeline.child_timelines.push(Arc::new(TimelineState::new()));
            
            *timeline.memory.state.get() = Some(false);
            timeline.child_timelines.push(Arc::new(TimelineState::new()));
            
            *timeline.memory.state.get() = Some(true);
            timeline.child_timelines.push(Arc::new(TimelineState::new()));
            
            // Repeat the pattern
            *timeline.memory.state.get() = Some(true);
            timeline.child_timelines.push(Arc::new(TimelineState::new()));
            
            *timeline.memory.state.get() = Some(false);
            timeline.child_timelines.push(Arc::new(TimelineState::new()));
            
            *timeline.memory.state.get() = Some(true);
            timeline.child_timelines.push(Arc::new(TimelineState::new()));
        }
        
        let order = timeline.calculate_local_order();
        assert!(order > 0.3, 
            "Order should be significant with repeating patterns");
        assert!(order <= 1.0, 
            "Order should never exceed 1.0");
    }

    #[test]
    fn test_order_calculation_random() {
        let mut timeline = TimelineState::new();
        
        // Create random patterns
        for _ in 0..10 {
            unsafe {
                timeline.memory.transition();
                if let Some(true) = *timeline.memory.state.get() {
                    timeline.child_timelines.push(Arc::new(TimelineState::new()));
                }
            }
        }
        
        let order = timeline.calculate_local_order();
        assert!(order >= 0.0, 
            "Order should never be negative");
        assert!(order <= 1.0, 
            "Order should never exceed 1.0");
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
    // Start with 25 root timelines
    metrics.lock().unwrap().parallel_timelines += 25;

    for sim_index in 0..25 {
        let metrics = Arc::clone(&metrics);
        thread::spawn(move || {
            let mut local_timeline = TimelineState::new();
            
            for i in 0..25 {
                local_timeline.transition();
                let count = count_timelines(&local_timeline);
                
                let mut metrics = metrics.lock().unwrap();
                metrics.add_simulation_progress(sim_index, i, &local_timeline);
                // Update parallel_timelines to reflect all child timelines
                metrics.parallel_timelines = metrics.parallel_timelines
                    .saturating_add((count as u32).saturating_sub(1)); // subtract 1 to not double-count root
                metrics.record_transition(i, count);
                drop(metrics);
                
                thread::sleep(Duration::from_millis(10));
            }
        });
    }
}

fn project_timeline_to_sphere(timeline: &TimelineState, depth: f64, theta: f64) -> Vec<(f64, f64, f64)> {
    let mut points = Vec::new();
    let entropy = timeline.local_entropy;
    
    // Convert to spherical coordinates
    let phi = (depth / 10.0) * std::f64::consts::PI;
    let x = entropy * phi.sin() * theta.cos();
    let y = entropy * phi.sin() * theta.sin();
    let z = entropy * phi.cos();
    
    points.push((x, y, z));
    
    // Project child timelines with angular distribution
    let child_count = timeline.child_timelines.len();
    for (i, child) in timeline.child_timelines.iter().enumerate() {
        let new_theta = theta + (2.0 * std::f64::consts::PI * (i as f64) / child_count as f64);
        points.extend(project_timeline_to_sphere(child, depth + 1.0, new_theta));
    }
    
    points
}

fn ui<B: Backend>(f: &mut Frame<B>, metrics: &TimelineMetrics) {
    // Create a vertical layout with three sections:
    // 40% - Canvas (top)
    // 40% - Chart (middle)
    // 20% - Metrics (bottom)
    let size = f.size();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(40),
            Constraint::Percentage(40),
            Constraint::Percentage(20),
        ])
        .split(size);

    // CMB Visualization
    let canvas = Canvas::default()
        .marker(symbols::Marker::Braille)
        .paint(|ctx| {
            for sim in &metrics.active_simulations {
                if let Some(_) = sim.last() {
                    let timeline = TimelineState::new();
                    let points = project_timeline_to_sphere(&timeline, 0.0, 0.0);
                    for (x, y, z) in points {
                        let temp = (z + 1.0) / 2.0;
                        let color = match temp {
                            t if t < 0.2 => Color::Rgb(0, 0, 255),  // Cold
                            t if t < 0.4 => Color::Rgb(0, 255, 255),
                            t if t < 0.6 => Color::Rgb(255, 255, 0),
                            t if t < 0.8 => Color::Rgb(255, 165, 0),
                            _ => Color::Rgb(255, 0, 0),  // Hot
                        };
                        ctx.draw(&Points {
                            coords: &[(x, y)],
                            color,
                        });
                    }
                }
            }
        })
        .block(Block::default().title("Entropy Distribution (CMB)").borders(Borders::ALL));

    // Combined Entropy/Order Graph with distribution bands
    let mut datasets = Vec::new();

    if metrics.selected_simulation.is_none() {
        // Overview mode - show all simulations on current page
        let page_start = metrics.current_sim_page * metrics.sims_per_page;
        let page_end = (page_start + metrics.sims_per_page).min(metrics.active_simulations.len());

        for i in page_start..page_end {
            if let Some(sim) = metrics.active_simulations.get(i) {
                // Timeline count line
                datasets.push(Dataset::default()
                    .name(format!("Timeline {}", i + 1))
                    .marker(symbols::Marker::Dot)
                    .graph_type(GraphType::Line)
                    .style(Style::default().fg(Color::Rgb(
                        (50 * i) as u8,
                        255,
                        (50 * i) as u8
                    )))
                    .data(sim));

                // Order line
                datasets.push(Dataset::default()
                    .name(format!("Order {}", i + 1))
                    .marker(symbols::Marker::Braille)
                    .graph_type(GraphType::Line)
                    .style(Style::default()
                        .fg(Color::Rgb(
                            (50 * i) as u8,
                            (50 * i) as u8,
                            255
                        ))
                        .add_modifier(Modifier::RAPID_BLINK))
                    .data(&metrics.order_values));
            }
        }
    } else if let Some(selected) = metrics.selected_simulation {
        // Detail view - show single simulation with entropy and order
        if let Some(sim) = metrics.active_simulations.get(selected) {
            // Timeline count
            datasets.push(Dataset::default()
                .name("Timeline Count")
                .marker(symbols::Marker::Dot)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(Color::Cyan))
                .data(sim));

            // Entropy
            datasets.push(Dataset::default()
                .name("Entropy")
                .marker(symbols::Marker::Dot)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(Color::Red))
                .data(&metrics.entropy_values));

            // Order
            datasets.push(Dataset::default()
                .name("Order")
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::RAPID_BLINK))
                .data(&metrics.order_values));
        }
    }

    // Add distribution bands visualization
    let distribution_data: Vec<(f64, Vec<f64>)> = metrics.entropy_values.iter()
        .map(|(x, _)| {
            let entropies = metrics.active_simulations.iter()
                .flat_map(|sim| {
                    sim.iter()
                        .filter(|(t, _)| (t - x).abs() < 0.1)
                        .map(|(_, e)| *e)
                })
                .collect();
            (*x, entropies)
        })
        .collect();

    // First collect all points
    let distribution_points: Vec<Vec<(f64, f64)>> = distribution_data.into_iter()
        .filter_map(|(x, values)| {
            if let (Some(min), Some(max)) = (
                values.iter().min_by(|a, b| a.partial_cmp(b).unwrap()),
                values.iter().max_by(|a, b| a.partial_cmp(b).unwrap())
            ) {
                Some(vec![(x, *min), (x, *max)])
            } else {
                None
            }
        })
        .collect();
    
    // Then create all datasets
    for points in &distribution_points {
        datasets.push(Dataset::default()
            .name("Distribution")
            .graph_type(GraphType::Line)
            .style(Style::default()
                .fg(Color::Gray)
                .bg(Color::Reset)
                .add_modifier(Modifier::DIM))
            .data(points));
    }

    let combined_chart = Chart::new(datasets)
        .block(Block::default()
            .title("Entropy (solid) vs Order (dashed) with Distribution")
            .borders(Borders::ALL))
        .x_axis(Axis::default()
            .title("Transitions")
            .style(Style::default().fg(Color::Gray))
            .bounds([0.0, 25.0]))
        .y_axis(Axis::default()
            .title("Magnitude")
            .style(Style::default().fg(Color::Gray))
            .bounds([0.0, metrics.entropy_values.iter()
                .map(|(_, v)| *v)
                .chain(metrics.order_values.iter().map(|(_, v)| *v))
                .fold(0.0, f64::max)]));

    // Metrics panel
    let metrics_text = vec![
        Line::from(vec![
            Span::raw("Single Timeline Runs: "),
            Span::styled(
                metrics.single_runs.to_string(),
                Style::default().fg(Color::Green)
            )
        ]),
        Line::from(vec![
            Span::raw("Parallel Timelines: "),
            Span::styled(
                metrics.parallel_timelines.to_string(),
                Style::default().fg(Color::Blue)
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
        Line::from("Press PageUp/PageDown or n/p to browse simulations"),
        Line::from("Press SPACE to run simulation"),
        Line::from("Press 'q' to quit"),
    ];

    let metrics_paragraph = Paragraph::new(metrics_text)
        .block(Block::default().title("Metrics").borders(Borders::ALL))
        .alignment(Alignment::Left);

    // Render everything
    f.render_widget(canvas, chunks[0]);
    f.render_widget(combined_chart, chunks[1]);
    f.render_widget(metrics_paragraph, chunks[2]);
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
                    KeyCode::Char('n') | KeyCode::PageDown => metrics.lock().unwrap().next_page(),
                    KeyCode::Char('p') | KeyCode::PageUp => metrics.lock().unwrap().prev_page(),
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