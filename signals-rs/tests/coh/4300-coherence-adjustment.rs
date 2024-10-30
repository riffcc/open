use signals_rs::gsp::{CoherenceManager, NodeState, CoherenceMetrics};
use signals_rs::hex::Position;
use signals_rs::common::NodeId;
use std::time::{Duration, SystemTime};

#[test]
fn gsp_4300_position_consistency_rewards() {
    let coherence = CoherenceManager::new();
    let node_id = NodeId::generate();
    let start_pos = Position::new(0, 0, 0);
    
    // Record consistent position updates
    let timestamps: Vec<_> = (0..10)
        .map(|i| SystemTime::now() + Duration::from_secs(i * 60))
        .collect();
        
    for time in &timestamps {
        coherence.record_node_state(node_id, NodeState {
            position: start_pos,
            last_seen: *time,
            ..Default::default()
        });
    }
    
    let initial_coherence = coherence.get_current_coherence(node_id);
    coherence.reward_position_consistency(node_id, Duration::from_secs(600));
    let final_coherence = coherence.get_current_coherence(node_id);
    
    assert!(final_coherence > initial_coherence,
        "Consistent position updates should increase coherence");
}

#[test]
fn gsp_4301_route_sharing_validation() {
    let coherence = CoherenceManager::new();
    let metrics = CoherenceMetrics::new();
    
    // Create a set of valid routes
    let routes = vec![
        (Position::new(0, 0, 0), Position::new(1, -1, 0)),
        (Position::new(1, -1, 0), Position::new(2, -1, -1)),
        (Position::new(2, -1, -1), Position::new(2, -2, 0)),
    ];
    
    let node_id = NodeId::generate();
    
    // Record valid route sharing
    for (from, to) in &routes {
        coherence.record_route_share(node_id, *from, *to);
        metrics.validate_route(*from, *to, true);
    }
    
    let initial_coherence = coherence.get_current_coherence(node_id);
    coherence.reward_valid_routes(node_id);
    let final_coherence = coherence.get_current_coherence(node_id);
    
    assert!(final_coherence > initial_coherence,
        "Valid route sharing should increase coherence");
}

#[test]
fn gsp_4302_message_routing_success() {
    let coherence = CoherenceManager::new();
    let metrics = CoherenceMetrics::new();
    
    let node_id = NodeId::generate();
    let start_coherence = 0.5;
    coherence.set_node_coherence(node_id, start_coherence);
    
    // Simulate successful message routing
    for _ in 0..100 {
        metrics.record_message_delivery(node_id, true);
    }
    
    // Calculate success rate and adjust coherence
    let success_rate = metrics.get_delivery_success_rate(node_id);
    coherence.reward_message_delivery(node_id, success_rate);
    
    let final_coherence = coherence.get_current_coherence(node_id);
    assert!(final_coherence > start_coherence,
        "Successful message routing should increase coherence");
    
    // Verify proportional reward
    let coherence_gain = final_coherence - start_coherence;
    assert!(coherence_gain.is_proportional_to(success_rate),
        "Coherence increase should be proportional to success rate");
}

#[test]
fn gsp_4303_invalid_update_penalties() {
    let coherence = CoherenceManager::new();
    let node_id = NodeId::generate();
    
    // Set initial coherence
    let start_coherence = 0.8;
    coherence.set_node_coherence(node_id, start_coherence);
    
    // Record invalid updates
    let invalid_updates = vec![
        "invalid position format",
        "malformed route data",
        "inconsistent neighbor list",
        "timestamp violation",
    ];
    
    for reason in invalid_updates {
        coherence.record_invalid_update(node_id, reason);
    }
    
    let final_coherence = coherence.get_current_coherence(node_id);
    assert!(final_coherence < start_coherence,
        "Invalid updates should decrease coherence");
    
    // Verify penalty severity
    let violations = coherence.get_violation_count(node_id);
    assert!(violations == invalid_updates.len(),
        "Should track all violations");
}

#[test]
fn gsp_4304_problematic_introduction_handling() {
    let coherence = CoherenceManager::new();
    
    // Create chain of introductions
    let introducer = NodeId::generate();
    let problem_nodes: Vec<_> = (0..5)
        .map(|_| NodeId::generate())
        .collect();
    
    // Record introductions
    for node_id in &problem_nodes {
        coherence.record_node_introduction(*node_id, introducer);
    }
    
    // Simulate problematic behavior
    for node_id in &problem_nodes {
        coherence.record_node_violation(*node_id, "spam behavior");
        coherence.record_node_violation(*node_id, "invalid updates");
    }
    
    // Check introducer penalty
    let initial_coherence = coherence.get_current_coherence(introducer);
    coherence.process_introduction_violations(&problem_nodes);
    let final_coherence = coherence.get_current_coherence(introducer);
    
    assert!(final_coherence < initial_coherence,
        "Introducing problematic nodes should decrease coherence");
    
    // Verify introduction tracking
    let intro_stats = coherence.get_introduction_stats(introducer);
    assert!(intro_stats.problem_ratio() > 0.8,
        "Should track high ratio of problematic introductions");
} 