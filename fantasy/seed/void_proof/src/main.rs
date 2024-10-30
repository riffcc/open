use std::cell::UnsafeCell;
use std::io::{self, Write};

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
}

fn main() {
    let memory = UnstableMemory::new();
    let mut total_transitions = 0;
    let mut ordered_patterns = 0;
    let mut last_states = Vec::with_capacity(3);

    println!("Void Proof Interactive Terminal");
    println!("Press ENTER to trigger a transition");
    println!("Press 'q' to quit\n");

    loop {
        print!("\rTransitions: {} | Ordered Patterns: {} | Order Ratio: {:.2}% > ", 
            total_transitions, 
            ordered_patterns,
            if total_transitions > 0 {
                (ordered_patterns as f64 / total_transitions as f64) * 100.0
            } else {
                0.0
            });
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();

        if input.trim() == "q" {
            break;
        }

        unsafe {
            let before = *memory.state.get();
            memory.transition();
            let after = *memory.state.get();
            
            // Track last 3 states for pattern detection
            last_states.push(after);
            if last_states.len() > 3 {
                last_states.remove(0);
            }

            // Check for patterns (3 identical states in a row)
            if last_states.len() == 3 
                && last_states.iter().all(|&x| x == last_states[0]) {
                ordered_patterns += 1;
            }

            total_transitions += 1;

            // Visual representation of the transition
            println!("\n{:?} -> {:?}", before, after);
        }
    }
}