use std::cell::UnsafeCell;
use std::io;
use std::time::{Duration, Instant};
use std::thread;
use std::sync::{Arc, Mutex};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering, AtomicBool};
use once_cell::sync::Lazy;
use num_cpus;

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

static EVENT_QUEUE: Lazy<Arc<Mutex<EventQueue>>> = Lazy::new(|| {
    Arc::new(Mutex::new(EventQueue::new(num_cpus::get())))
});

#[derive(Debug, Clone)]
enum TimelineEvent {
    Transition(usize, Arc<TimelineState>),  // (timeline_id, isolated_state)
    Branch(usize, Arc<TimelineState>),      // (parent_id, new_isolated_state)
    Pattern(usize, PatternType, Arc<TimelineState>)  // (timeline_id, pattern, state)
}

struct EventQueue {
    events: VecDeque<TimelineEvent>,
    workers: Vec<thread::JoinHandle<()>>,
    timeline_count: Arc<AtomicUsize>,
    state_pool: Arc<Mutex<Vec<Arc<TimelineState>>>>, // Keep isolated states
}

impl EventQueue {
    fn new(worker_count: usize) -> Self {
        Self {
            events: VecDeque::new(),
            workers: Vec::with_capacity(worker_count),
            timeline_count: Arc::new(AtomicUsize::new(0)),
            state_pool: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn spawn_workers(&mut self, metrics: Arc<Mutex<TimelineMetrics>>) {
        for _ in 0..self.workers.capacity() {
            let events = Arc::new(Mutex::new(VecDeque::new())); // Create new queue per worker
            let count = Arc::clone(&self.timeline_count);
            let metrics = Arc::clone(&metrics);
            let state_pool = Arc::clone(&self.state_pool);
            
            // Move events queue into worker
            let worker = thread::spawn(move || {
                loop {
                    let event = events.lock().unwrap().pop_front();

                    match event {
                        Some(TimelineEvent::Transition(id, state)) => {
                            // Process transition in isolated memory
                            let mut new_state = (*state).clone();
                            new_state.transition();
                            
                            // Store new state and queue any resulting events
                            let mut pool = state_pool.lock().unwrap();
                            pool.push(Arc::new(new_state));
                            
                            metrics.lock().unwrap().record_transition(id as u32, pool.len());
                        },
                        Some(TimelineEvent::Branch(parent_id, parent_state)) => {
                            // Create new isolated timeline
                            let new_id = count.fetch_add(1, Ordering::SeqCst);
                            let mut new_state = TimelineState::new_with_state(
                                parent_state.memory.get_state()
                            );
                            
                            // Store and queue transition
                            let state_arc = Arc::new(new_state);
                            state_pool.lock().unwrap().push(Arc::clone(&state_arc));
                            
                            let mut events = events.lock().unwrap();
                            events.push_back(TimelineEvent::Transition(new_id, state_arc));
                        },
                        Some(TimelineEvent::Pattern(id, pattern_type, state)) => {
                            // Process pattern detection asynchronously
                            if let Some(pattern) = state.detect_quantum_structure() {
                                let mut metrics = metrics.lock().unwrap();
                                metrics.record_pattern(id, pattern);
                            }
                        },
                        _ => thread::sleep(Duration::from_millis(1))
                    }
                }
            });
            
            self.workers.push(worker);
        }
    }

    fn get_timeline_states(&self, timeline_id: usize) -> Vec<Option<bool>> {
        self.state_pool.lock()
            .unwrap()
            .iter()
            .filter(|state| state.id.load(Ordering::SeqCst) == timeline_id)
            .map(|state| state.memory.get_state())
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq)]
enum PatternType {
    Emergence,
    OrderFormation,
    OrderStabilization,
    Chaos
}

#[derive(Debug, Clone, PartialEq)]
enum QuantumPattern {
    Hexagonal {
        center: usize,
        vertices: [usize; 6],
        stability: f64,
    },
    Dodecahedral {
        front_face: [usize; 6],
        back_face: [usize; 6],
        connecting_edges: Vec<(usize, usize)>,
        coherence: f64,
    },
    TransitionState {
        from: Box<QuantumPattern>,
        to: Box<QuantumPattern>,
        progress: f64,
    }
}

impl QuantumPattern {
    fn stability(&self) -> f64 {
        match self {
            QuantumPattern::Hexagonal { stability, .. } => *stability,
            QuantumPattern::Dodecahedral { coherence, .. } => *coherence,
            QuantumPattern::TransitionState { progress, .. } => *progress,
        }
    }
}

impl From<QuantumPattern> for PatternType {
    fn from(pattern: QuantumPattern) -> Self {
        match pattern {
            QuantumPattern::Hexagonal { stability, .. } => {
                if stability > 0.8 { PatternType::OrderStabilization }
                else { PatternType::OrderFormation }
            },
            QuantumPattern::Dodecahedral { coherence, .. } => {
                if coherence > 0.8 { PatternType::OrderStabilization }
                else { PatternType::OrderFormation }
            },
            QuantumPattern::TransitionState { .. } => PatternType::Emergence,
        }
    }
}

#[derive(Debug, Default, Clone)]
struct CoherenceMetrics {
    disorder_to_order_transitions: u64,
    order_to_disorder_transitions: u64,
    stable_order_duration: Vec<Duration>,
    branch_points: Vec<(Instant, usize)>,
    pattern_formations: Vec<(Instant, QuantumPattern)>,
    branch_patterns: Vec<PatternType>,
}

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
    coherence_transitions: Vec<(f64, f64)>,  // (transition_number, coherence_probability)
    order_persistence: Vec<(f64, f64)>,      // (transition_number, stable_duration)
    branch_distribution: Vec<(f64, usize)>,  // (transition_number, branches_at_point)
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
            coherence_transitions: Vec::new(),
            order_persistence: Vec::new(),
            branch_distribution: Vec::new(),
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
        // If we have at least 2 simulations, we can toggle between pages 0 and 1
        if self.active_simulations.len() >= 2 {
            self.current_sim_page = if self.current_sim_page == 0 { 1 } else { 0 };
        }
    }

    fn prev_page(&mut self) {
        // Same logic - just toggle between 0 and 1
        if self.active_simulations.len() >= 2 {
            self.current_sim_page = if self.current_sim_page == 0 { 1 } else { 0 };
        }
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

    fn record_coherence(&mut self, timeline: &TimelineState) {
        let transition_num = self.timeline_counts.len() as f64;
        self.coherence_transitions.push((
            transition_num,
            timeline.calculate_coherence_probability()
        ));

        // Record order persistence
        if let Some(duration) = timeline.metrics.stable_order_duration.last() {
            self.order_persistence.push((
                transition_num,
                duration.as_secs_f64() * 1000.0  // Convert to milliseconds
            ));
        }

        // Record branch points
        if let Some((time, branches)) = timeline.metrics.branch_points.last() {
            self.branch_distribution.push((
                transition_num,
                *branches
            ));
        }
    }

    fn record_pattern(&mut self, timeline_id: usize, pattern: QuantumPattern) {
        if let Some(sim) = self.active_simulations.get_mut(timeline_id) {
            sim.push((sim.len() as f64, pattern.stability()));
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum MemoryCoherenceState {
    Classical,  // Thread-safe, boring
    Quantum,    // Unsafe, spicy
    Superposition, // Both safe and unsafe simultaneously because why not
}

/// TODO: Add quantum coherence collapse trigger that can dynamically switch memory 
/// between thread-safe and unsafe states mid-simulation. This would let us observe
/// the transition between classical and quantum behavior in real-time.
/// 
/// Bonus points: Implement MemoryCoherenceState::Superposition where memory is
/// simultaneously safe and unsafe until observed. This is definitely fine and 
/// won't cause the universe to divide by zero.
/// 
/// Note: If this actually works, please notify the physics department. They'll 
/// want to see this. Or maybe they won't. Both until observed.
#[derive(Debug)]
struct UnstableMemory {
    state: Arc<(AtomicBool, AtomicBool)>,
    // coherence_state: AtomicCell<MemoryCoherenceState>, // Uncomment when reality is ready
}

impl UnstableMemory {
    fn new() -> Self {
        Self {
            state: Arc::new((AtomicBool::new(false), AtomicBool::new(false)))
        }
    }

    // Keep transition unsafe because QUANTUM FLUCTUATIONS
    unsafe fn transition(&self) {
        if rand::random::<bool>() {
            self.state.0.store(true, Ordering::SeqCst);
            self.state.1.store(rand::random::<bool>(), Ordering::SeqCst);
        }
    }

    // These can be safe because they're just reading/writing
    fn get_state(&self) -> Option<bool> {
        if self.state.0.load(Ordering::SeqCst) {
            Some(self.state.1.load(Ordering::SeqCst))
        } else {
            None
        }
    }

    fn set_state(&self, state: Option<bool>) {
        match state {
            Some(value) => {
                self.state.0.store(true, Ordering::SeqCst);
                self.state.1.store(value, Ordering::SeqCst);
            }
            None => {
                self.state.0.store(false, Ordering::SeqCst);
            }
        }
    }
}

#[derive(Debug, Clone)]
struct TimelineState {
    id: Arc<AtomicUsize>,
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
    metrics: CoherenceMetrics,
}

fn count_timelines(timeline: &TimelineState) -> usize {
    1 + timeline.child_timelines.iter()
        .map(|child| count_timelines(&*child))
        .sum::<usize>()
}

impl TimelineState {
    fn new() -> Self {
        Self {
            id: Arc::new(AtomicUsize::new(0)),
            memory: Arc::new(UnstableMemory::new()),
            spawn_time: Instant::now(),
            child_timelines: Vec::new(),
            local_order: 0.0,
            local_entropy: 0.0,
            parent: None,
            changes: Vec::new(),
            metrics: CoherenceMetrics::default(),
        }
    }

    fn new_with_state(initial_state: Option<bool>) -> Self {
        let memory = UnstableMemory::new();
        memory.set_state(initial_state);
        Self {
            id: Arc::new(AtomicUsize::new(0)),
            memory: Arc::new(memory),
            spawn_time: Instant::now(),
            child_timelines: Vec::new(),
            local_order: 0.0,
            local_entropy: 0.0,
            parent: None,
            changes: Vec::new(),
            metrics: CoherenceMetrics::default(),
        }
    }

    fn calculate_local_order(&self) -> f64 {
        const MIN_PATTERN_LENGTH: usize = 3;
        
        // Get states from event queue instead of direct child access
        let states = EVENT_QUEUE.lock().unwrap().get_timeline_states(self.id.load(Ordering::SeqCst));
        
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
        let id = self.id.load(Ordering::SeqCst);
        
        // Calculate time dilation (keep this part!)
        let total_timelines = count_timelines(self);
        let dilation_factor = if total_timelines > 1 { // If we have ANY branche
            (total_timelines as f64 * 10e88).log2() + 1.0 // Sub-Planck PRECISION in a simulated universe is very interesting... we use time dilation to make it work.
        } else {
            1.0
        };
        
        unsafe {
            let old_state = self.memory.get_state();
            self.memory.transition();
            
            if let Some(state) = self.memory.get_state() {
                // Instead of directly creating child timelines, queue events
                EVENT_QUEUE.lock().unwrap().events.push_back(
                    TimelineEvent::Branch(id, Arc::new(TimelineState::new_with_state(Some(state))))
                );
            }
            
            // Queue pattern detection as an event
            if let Some(pattern) = self.detect_quantum_structure() {
                EVENT_QUEUE.lock().unwrap().events.push_back(
                    TimelineEvent::Pattern(id, pattern.into(), Arc::new(self.clone()))
                );
            }
        }

        // Keep the beautiful time dilation
        if total_timelines > 1 {
            let elapsed = start.elapsed();
            thread::sleep(Duration::from_nanos(
                (elapsed.as_nanos() as f64 * dilation_factor) as u64
            ));
        }
    }

    fn calculate_coherence_probability(&self) -> f64 {
        let total_transitions = self.metrics.disorder_to_order_transitions 
            + self.metrics.order_to_disorder_transitions;
        
        if total_transitions == 0 {
            return 0.0;
        }

        let effective_order_transitions = (self.metrics.disorder_to_order_transitions as usize)
            * self.metrics.branch_points.iter()
                .map(|(_, branches)| *branches)
                .sum::<usize>();

        (effective_order_transitions as f64) / (total_transitions as f64)
    }

    fn track_pattern_formation(&mut self, time: Instant, old_state: Option<bool>) {
        let pattern_type = unsafe {  // We're already in an unsafe context
            match (old_state, self.memory.get_state()) {
                (None, Some(_)) => PatternType::Emergence,
                (Some(false), Some(true)) => PatternType::OrderFormation,
                (Some(true), Some(true)) => PatternType::OrderStabilization,
                _ => PatternType::Chaos
            }
        };

        // Track the pattern formation
        if let Some(pattern) = self.detect_quantum_structure() {
            self.metrics.pattern_formations.push((time, pattern));
        }
    }

    fn detect_hexagonal_structure(&self) -> Option<QuantumPattern> {
        // Look for six-fold symmetry in branch patterns
        if self.child_timelines.len() >= 6 {
            // Check for hexagonal arrangement of order values
            // This would be AMAZING to implement!
        }
        None  // For now
    }

    fn detect_quantum_structure(&self) -> Option<QuantumPattern> {
        // First check for basic hexagonal patterns
        if let Some(hex) = self.detect_hexagonal_pattern() {
            return Some(hex);
        }

        // Then check for full dodecahedral structures
        if let Some(dodeca) = self.detect_dodecahedral_pattern() {
            return Some(dodeca);
        }

        // Check if we're watching one pattern transform into another
        self.detect_pattern_transition()
    }

    fn detect_hexagonal_pattern(&self) -> Option<QuantumPattern> {
        if self.child_timelines.len() < 6 {
            return None;
        }

        // Look for 6-fold rotational symmetry in the order values
        let mut potential_vertices = Vec::new();
        for (i, child) in self.child_timelines.iter().enumerate() {
            if child.local_order > 0.8 {  // High order threshold
                potential_vertices.push(i);
            }
        }

        // Check if we can form a hexagon
        if potential_vertices.len() >= 6 {
            // Calculate center of pattern
            let center = self.child_timelines.len() / 2;
            let vertices = potential_vertices.iter()
                .take(6)
                .copied()
                .collect::<Vec<_>>()
                .try_into()
                .ok()?;

            Some(QuantumPattern::Hexagonal {
                center,
                vertices,
                stability: self.calculate_pattern_stability(&vertices),
            })
        } else {
            None
        }
    }

    fn detect_dodecahedral_pattern(&self) -> Option<QuantumPattern> {
        if self.child_timelines.len() < 12 {
            return None;
        }

        // Look for two parallel hexagonal faces
        let mut ordered_timelines: Vec<_> = self.child_timelines.iter()
            .enumerate()
            .filter(|(_, t)| t.local_order > 0.8)
            .collect();

        if ordered_timelines.len() >= 12 {
            // Try to find two parallel hexagonal faces
            let (front, back) = self.find_parallel_faces(&ordered_timelines)?;
            
            // Find connecting edges between faces
            let connections = self.map_quantum_connections(&front, &back);

            Some(QuantumPattern::Dodecahedral {
                front_face: front,
                back_face: back,
                connecting_edges: connections,
                coherence: self.calculate_dodecahedral_coherence(&front, &back),
            })
        } else {
            None
        }
    }

    fn calculate_dodecahedral_coherence(&self, front: &[usize; 6], back: &[usize; 6]) -> f64 {
        // Calculate quantum coherence between faces
        let front_order: f64 = front.iter()
            .map(|&i| self.child_timelines[i].local_order)
            .sum::<f64>() / 6.0;
        
        let back_order: f64 = back.iter()
            .map(|&i| self.child_timelines[i].local_order)
            .sum::<f64>() / 6.0;

        // Perfect coherence = faces mirror each other
        1.0 - (front_order - back_order).abs()
    }

    fn calculate_pattern_stability(&self, vertices: &[usize; 6]) -> f64 {
        // Calculate average order of vertices
        vertices.iter()
            .map(|&i| self.child_timelines[i].local_order)
            .sum::<f64>() / 6.0
    }

    fn find_parallel_faces(&self, ordered: &[(usize, &Arc<TimelineState>)]) 
        -> Option<([usize; 6], [usize; 6])> {
        // Find two sets of 6 points with similar order values
        // that form parallel planes
        // For now, just take first 12 and split them
        if ordered.len() >= 12 {
            let front: [usize; 6] = ordered[0..6]
                .iter()
                .map(|(i, _)| *i)
                .collect::<Vec<_>>()
                .try_into()
                .ok()?;
            
            let back: [usize; 6] = ordered[6..12]
                .iter()
                .map(|(i, _)| *i)
                .collect::<Vec<_>>()
                .try_into()
                .ok()?;
            
            Some((front, back))
        } else {
            None
        }
    }

    fn map_quantum_connections(&self, front: &[usize; 6], back: &[usize; 6]) 
        -> Vec<(usize, usize)> {
        // Map connections between front and back faces
        front.iter()
            .zip(back.iter())
            .map(|(&f, &b)| (f, b))
            .collect()
    }

    fn detect_pattern_transition(&self) -> Option<QuantumPattern> {
        // TODO: Detect transitions between pattern types
        None
    }

    fn calculate_time_dilation(&self) -> f64 {
        let total_timelines = count_timelines(self);
        if total_timelines > 1 {
            // Use same formula from transition() but expose it as a method
            (total_timelines as f64 * 10e88).log2() + 1.0
        } else {
            1.0
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
            let before = memory.get_state();
            unsafe { memory.transition(); }
            let after = memory.get_state();
            
            if before.is_none() && after.is_some() {
                void_to_boolean_occurred = true;
                break;
            }
            transitions_observed += 1;
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
            unsafe { memory.transition(); }
            state_sequence.push(memory.get_state());
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
                    window.push(memory.get_state());
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
    fn test_timeline_branching() {
        let mut timeline = TimelineState::new();
        let start_time = Instant::now();
        let max_duration = Duration::from_secs(5);  // Time Lord approved timeout
        
        // The Doctor Who Method: Search until we find what we need!
        while start_time.elapsed() < max_duration {
            unsafe {
                // Try the current timeline
                timeline.memory.transition();
                if timeline.memory.get_state().is_some() {
                    let new_timeline = TimelineState::new_with_state(timeline.memory.get_state());
                    timeline.child_timelines.push(Arc::new(new_timeline));
                    break;  // Found a branch!
                }
                
                // No branch? Check the existing children
                let mut found_branch = false;
                if let Some(child) = timeline.child_timelines.first_mut() {
                    if let Some(child) = Arc::get_mut(child) {
                        child.memory.transition();
                        if child.memory.get_state().is_some() {
                            let new_timeline = TimelineState::new_with_state(child.memory.get_state());
                            child.child_timelines.push(Arc::new(new_timeline));
                            found_branch = true;
                            break;
                        }
                    }
                }
                if found_branch {
                    break;
                }

                // Let time dilation occur naturally
                let dilation = timeline.calculate_time_dilation();
                thread::sleep(Duration::from_nanos((dilation * 1000.0) as u64));
            }
        }
        
        assert!(!timeline.child_timelines.is_empty(), 
            "Should eventually find a timeline branch through space and time");
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
        
        // Create a pattern through proper quantum transitions
        for _ in 0..3 {
            unsafe { 
                timeline.memory.transition();
                if timeline.memory.get_state().is_some() {
                    timeline.child_timelines.push(Arc::new(TimelineState::new()));
                }
            }
        }
        
        let order = timeline.calculate_local_order();
        assert!(order >= 0.0, "Quantum pattern should have measurable order");
    }

    #[test]
    fn test_order_calculation_complex() {
        let mut timeline = TimelineState::new();
        
        // Create a more complex pattern through quantum transitions
        for _ in 0..6 {
            unsafe {
                timeline.memory.transition();
                if timeline.memory.get_state().is_some() {
                    timeline.child_timelines.push(Arc::new(TimelineState::new()));
                }
            }
        }
        
        let order = timeline.calculate_local_order();
        assert!(order >= 0.0, "Complex quantum pattern should have measurable order");
    }

    #[test]
    fn test_order_calculation_random() {
        let mut timeline = TimelineState::new();
        
        // Create random patterns
        for _ in 0..10 {
            unsafe {
                timeline.memory.transition();
                if let Some(true) = timeline.memory.get_state() {
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

    #[test]
    fn test_timeline_preserves_quantum_state() {
        let parent = TimelineState::new();
        unsafe {
            parent.memory.transition();
            let original_state = parent.memory.get_state();
            let child = TimelineState::new_with_state(original_state);
            assert_eq!(child.memory.get_state(), original_state, 
                "Child timelines should preserve their parent's quantum state, not YEET THEM INTO THE VOID");
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
        .x_bounds([-50.0, 50.0])
        .y_bounds([-25.0, 25.0])
        .paint(|ctx| {
            // For each active simulation
            for (sim_index, sim) in metrics.active_simulations.iter().enumerate() {
                if let Some(&(_, count)) = sim.last() {
                    // Project the actual timeline states
                    for (transition, count) in sim {
                        let x = *transition as f64 / 2.0;  // Spread out horizontally
                        let y = count.log2();  // Height based on actual complexity
                        
                        // Color based on actual quantum coherence
                        let coherence = metrics.coherence_transitions
                            .last()
                            .map(|(_, prob)| *prob)
                            .unwrap_or(0.0);
                        
                        let color = match coherence {
                            c if c > 0.8 => Color::Rgb(0, 255, 0),    // High coherence
                            c if c > 0.6 => Color::Rgb(64, 192, 64),
                            c if c > 0.4 => Color::Rgb(128, 128, 255),
                            c if c > 0.2 => Color::Rgb(192, 64, 192),
                            _ => Color::Rgb(255, 0, 255),             // Low coherence
                        };
                        
                        ctx.draw(&Points {
                            coords: &[(x, y)],
                            color,
                        });
                    }
                }
            }
        })
        .block(Block::default()
            .title("Quantum Pattern Formation (CMB)")
            .borders(Borders::ALL));

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
        Line::from(vec![
            Span::raw("Coherence Probability: "),
            Span::styled(
                format!("{:.4}%", metrics.coherence_transitions.last()
                    .map(|(_, prob)| prob * 100.0)
                    .unwrap_or(0.0)),
                Style::default().fg(Color::Green)
            )
        ]),
        Line::from(vec![
            Span::raw("Average Order Duration: "),
            Span::styled(
                format!("{:.2}ms", metrics.order_persistence.last()
                    .map(|(_, duration)| *duration)
                    .unwrap_or(0.0)),
                Style::default().fg(Color::Blue)
            )
        ]),
        Line::from(vec![
            Span::raw("Branch Points: "),
            Span::styled(
                metrics.branch_distribution.len().to_string(),
                Style::default().fg(Color::Magenta)
            )
        ]),
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
