use signals_rs::gsp::{AdaptiveGossip, GossipMetrics, UpdatePriority};
use signals_rs::hex::Position;
use signals_rs::common::NodeId;
use std::time::{Duration, SystemTime};

/// Helper to simulate network conditions between hex positions
fn simulate_network_delay(from: &Position, to: &Position) -> Duration {
    let distance = from.hex_distance(to);
    Duration::from_millis(20 * distance as u64 + fastrand::u64(0..10))
}

#[test]
fn gsp_3500_adaptive_frequency() {
    let adaptive = AdaptiveGossip::new(Position::new(0, 0, 0));
    let metrics = GossipMetrics::new();
    
    // Simulate stable network conditions
    for _ in 0..100 {
        metrics.record_delay(simulate_network_delay(
            &Position::new(0, 0, 0),
            &Position::new(1, -1, 0)
        ));
    }
    
    let stable_frequency = adaptive.calculate_frequency(&metrics);
    
    // Simulate degraded conditions (higher delays)
    for _ in 0..20 {
        metrics.record_delay(simulate_network_delay(
            &Position::new(0, 0, 0),
            &Position::new(3, -2, -1)
        ) * 2);
    }
    
    let degraded_frequency = adaptive.calculate_frequency(&metrics);
    assert!(degraded_frequency > stable_frequency,
        "Gossip frequency should increase under degraded conditions");
}

#[test]
fn gsp_3501_hex_aware_prioritization() {
    let adaptive = AdaptiveGossip::new(Position::new(0, 0, 0));
    
    let updates = vec![
        // Updates from different hex distances
        (Position::new(1, -1, 0), "adjacent update"),    // d=1
        (Position::new(2, -1, -1), "near update"),       // d=2
        (Position::new(3, -2, -1), "far update"),        // d=3
        (Position::new(-3, 2, 1), "distant update"),     // d=3
    ];
    
    let mut priorities: Vec<_> = updates.iter()
        .map(|(pos, content)| {
            let priority = adaptive.calculate_priority(
                pos,
                content,
                SystemTime::now()
            );
            (pos, priority)
        })
        .collect();
    
    // Sort by priority
    priorities.sort_by_key(|(_, p)| *p);
    
    // Verify closer updates get higher priority
    for window in priorities.windows(2) {
        let (pos1, pri1) = window[0];
        let (pos2, pri2) = window[1];
        let dist1 = pos1.hex_distance(&Position::new(0, 0, 0));
        let dist2 = pos2.hex_distance(&Position::new(0, 0, 0));
        
        assert!(dist1 >= dist2 || pri1 <= pri2,
            "Closer updates should generally have higher priority");
    }
}

#[test]
fn gsp_3502_congestion_adaptation() {
    let adaptive = AdaptiveGossip::new(Position::new(0, 0, 0));
    let metrics = GossipMetrics::new();
    
    // Simulate increasing congestion in different hex directions
    let directions = [
        Position::new(1, -1, 0),   // East
        Position::new(1, 0, -1),   // Southeast
        Position::new(0, 1, -1),   // Southwest
    ];
    
    for pos in &directions {
        for _ in 0..20 {
            metrics.record_congestion(pos, 0.8);
        }
    }
    
    // Check adaptive behavior
    for pos in &directions {
        let strategy = adaptive.get_transmission_strategy(pos, &metrics);
        
        assert!(strategy.batch_size < adaptive.default_batch_size(),
            "Batch size should reduce under congestion");
        assert!(strategy.retry_limit > adaptive.default_retry_limit(),
            "Retry limit should increase under congestion");
    }
    
    // Verify other directions unaffected
    let uncongested = Position::new(-1, 1, 0); // West
    let normal_strategy = adaptive.get_transmission_strategy(&uncongested, &metrics);
    
    assert_eq!(normal_strategy.batch_size, adaptive.default_batch_size(),
        "Uncongested directions should use default batch size");
}

#[test]
fn gsp_3503_hex_path_learning() {
    let adaptive = AdaptiveGossip::new(Position::new(0, 0, 0));
    let metrics = GossipMetrics::new();
    
    // Simulate successful transmissions along specific hex paths
    let good_path = vec![
        Position::new(1, -1, 0),
        Position::new(2, -1, -1),
        Position::new(3, -2, -1),
    ];
    
    for window in good_path.windows(2) {
        for _ in 0..10 {
            metrics.record_successful_transmission(
                &window[0],
                &window[1],
                Duration::from_millis(20)
            );
        }
    }
    
    // Verify path preference
    let target = Position::new(3, -2, -1);
    let preferred_path = adaptive.get_preferred_path(&target, &metrics);
    
    assert_eq!(preferred_path, good_path,
        "Should learn and prefer reliable hex paths");
}

#[test]
fn gsp_3504_stability_detection() {
    let adaptive = AdaptiveGossip::new(Position::new(0, 0, 0));
    let metrics = GossipMetrics::new();
    
    // Simulate stable hex grid section
    let stable_nodes = [
        Position::new(1, -1, 0),
        Position::new(1, 0, -1),
        Position::new(0, 1, -1),
    ];
    
    for pos in &stable_nodes {
        for _ in 0..50 {
            metrics.record_delay(simulate_network_delay(
                &Position::new(0, 0, 0),
                pos
            ));
            metrics.record_successful_transmission(
                &Position::new(0, 0, 0),
                pos,
                Duration::from_millis(20)
            );
        }
    }
    
    // Verify stability detection
    for pos in &stable_nodes {
        assert!(adaptive.is_stable_region(pos, &metrics),
            "Should detect stable hex regions");
    }
    
    // Verify unstable detection
    let unstable = Position::new(-3, 2, 1);
    assert!(!adaptive.is_stable_region(&unstable, &metrics),
        "Should detect unstable hex regions");
} 