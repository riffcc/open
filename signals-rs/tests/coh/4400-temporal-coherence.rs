use signals_rs::gsp::{CoherenceManager, NodeState, CoherenceMetrics, TimeWindow};
use signals_rs::hex::Position;
use signals_rs::common::NodeId;
use std::time::{Duration, SystemTime};

#[test]
fn gsp_4400_coherence_history_tracking() {
    let coherence = CoherenceManager::new();
    let node_id = NodeId::generate();
    
    // Record coherence changes over time
    let updates = vec![
        (0.9, Duration::from_secs(0)),
        (0.8, Duration::from_secs(100)),
        (0.6, Duration::from_secs(200)),
        (0.7, Duration::from_secs(300)),
        (0.8, Duration::from_secs(400)),
    ];
    
    let base_time = SystemTime::now();
    for (value, offset) in updates {
        let timestamp = base_time + offset;
        coherence.record_coherence_update(node_id, value, timestamp);
    }
    
    // Verify history accuracy
    let history = coherence.get_coherence_history(node_id);
    assert_eq!(history.len(), updates.len(),
        "Should maintain complete history");
    
    // Verify temporal ordering
    let mut last_time = SystemTime::UNIX_EPOCH;
    for entry in history {
        assert!(entry.timestamp > last_time,
            "History should maintain temporal order");
        last_time = entry.timestamp;
    }
}

#[test]
fn gsp_4401_gradual_coherence_changes() {
    let coherence = CoherenceManager::new();
    let metrics = CoherenceMetrics::new();
    
    let node_id = NodeId::generate();
    coherence.set_node_coherence(node_id, 0.5); // Start at neutral
    
    // Simulate gradual improvement
    for i in 0..10 {
        metrics.record_positive_behavior(node_id);
        coherence.apply_gradual_change(node_id, 0.05);
        
        let current = coherence.get_current_coherence(node_id);
        assert!(current <= 0.5 + (i + 1) as f64 * 0.05,
            "Coherence should increase gradually");
    }
    
    // Simulate gradual degradation
    for i in 0..10 {
        metrics.record_negative_behavior(node_id);
        coherence.apply_gradual_change(node_id, -0.05);
        
        let current = coherence.get_current_coherence(node_id);
        assert!(current >= 0.5 - (i + 1) as f64 * 0.05,
            "Coherence should decrease gradually");
    }
}

#[test]
fn gsp_4402_severe_violation_handling() {
    let coherence = CoherenceManager::new();
    let node_id = NodeId::generate();
    
    // Set initial good standing
    coherence.set_node_coherence(node_id, 0.9);
    
    // Record severe violation
    coherence.record_severe_violation(node_id, "attempted network partition");
    
    let post_violation = coherence.get_current_coherence(node_id);
    assert!(post_violation < 0.3,
        "Severe violations should cause immediate large coherence drop");
    
    // Verify violation record
    let violations = coherence.get_severe_violations(node_id);
    assert_eq!(violations.len(), 1,
        "Should track severe violations");
    
    // Verify recovery restrictions
    let max_recoverable = coherence.get_max_recoverable_coherence(node_id);
    assert!(max_recoverable < 0.7,
        "Severe violations should limit maximum recoverable coherence");
}

#[test]
fn gsp_4403_temporal_consistency_checks() {
    let coherence = CoherenceManager::new();
    let metrics = CoherenceMetrics::new();
    
    let node_id = NodeId::generate();
    let base_time = SystemTime::now();
    
    // Record temporally consistent updates
    for i in 0..5 {
        let time = base_time + Duration::from_secs(i * 60);
        coherence.record_node_state(node_id, NodeState {
            position: Position::new(0, 0, 0),
            last_seen: time,
            ..Default::default()
        });
    }
    
    // Verify temporal consistency
    assert!(coherence.check_temporal_consistency(node_id),
        "Updates should maintain temporal consistency");
    
    // Attempt backdated update
    let result = coherence.record_node_state(node_id, NodeState {
        position: Position::new(0, 0, 0),
        last_seen: base_time - Duration::from_secs(60),
        ..Default::default()
    });
    
    assert!(result.is_err(),
        "Should reject backdated updates");
}

#[test]
fn gsp_4404_coherence_data_pruning() {
    let coherence = CoherenceManager::new();
    let metrics = CoherenceMetrics::new();
    
    let node_id = NodeId::generate();
    let base_time = SystemTime::now();
    
    // Create old coherence data
    for i in 0..100 {
        let time = base_time - Duration::from_secs(i * 3600);
        coherence.record_coherence_update(node_id, 0.8, time);
    }
    
    // Set pruning window
    let window = TimeWindow::new(Duration::from_secs(24 * 3600)); // 24 hours
    
    // Perform pruning
    let removed = coherence.prune_old_data(window);
    
    // Verify pruning results
    assert!(removed > 0, "Should remove outdated entries");
    
    // Verify remaining data
    let history = coherence.get_coherence_history(node_id);
    for entry in history {
        assert!(entry.timestamp > base_time - window.duration(),
            "Remaining entries should be within time window");
    }
    
    // Verify metrics maintained
    let stats = metrics.calculate_coherence_stats(node_id);
    assert!(stats.is_complete(),
        "Should maintain statistical validity after pruning");
} 