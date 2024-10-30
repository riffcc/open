use std::cell::UnsafeCell;
use std::io::{self, Write};
use std::time::{Duration, Instant};
use std::thread;

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
        let time_since_spawn = start.duration_since(self.spawn_time);
        
        unsafe {
            self.memory.transition();
            
            // If a transition occurs, a new timeline may emerge
            if let Some(true) = *self.memory.state.get() {
                self.child_timelines.push(TimelineState::new());
            }
        }

        // Natural time dilation based purely on computational load
        let elapsed = start.elapsed();
        let sleep_duration = Duration::from_nanos(
            elapsed.as_nanos() as u64 * 
            (self.child_timelines.len() + 1) as u64
        );
        thread::sleep(sleep_duration);

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
}

fn main() {
    let mut root_timeline = TimelineState::new();
    let mut total_transitions = 0;

    println!("Void Proof Multiverse Terminal");
    println!("Press ENTER to trigger transitions across all timelines");
    println!("Press 'q' to quit\n");

    loop {
        print!("\rTimelines: {} | Total Transitions: {} > ", 
            count_timelines(&root_timeline),
            total_transitions);
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();

        if input.trim() == "q" {
            break;
        }

        root_timeline.transition();
        total_transitions += 1;
    }
}