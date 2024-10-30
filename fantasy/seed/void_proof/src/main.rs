use std::cell::UnsafeCell;

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
}