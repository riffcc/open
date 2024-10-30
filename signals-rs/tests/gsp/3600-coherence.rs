use signals_rs::gsp::{CoherenceManager, NodeState, CoherenceMetrics};
use signals_rs::hex::Position;
use signals_rs::common::NodeId;
use std::time::{Duration, SystemTime};

#[test]
fn gsp_3600_hex_stability_scoring() {
    let coherence = CoherenceManager::new(Position::new(0, 0, 0));
    
    // Create a set of nodes in hex grid positions
    let nodes = vec![
        // Stable inner hex
        (NodeId::generate(), Position::new(1, -1, 0), 100), // East
        (NodeId::generate(), Position::new(1, 0, -1), 95),  // Southeast
        (NodeId::generate(), Position::new(0, 1, -1), 90),  // Southwest
        
        // Less stable outer hex
        (NodeId::generate(), Position::new(2, -1, -1), 70),
        (NodeId::generate(), Position::new(2, -2, 0), 65),
        (NodeId::generate(), Position::new(1, -2, 1), 60),
    ];
    
    // Record stability metrics
    for (id, pos, uptime) in &nodes {
        coherence.record_node_state(*id, NodeState {
            position: *pos,
            uptime: Duration::from_secs(*uptime),
            last_seen: SystemTime::now(),
            ..Default::default()
        });
    }
    
    // Calculate coherence scores
    let scores = coherence.calculate_hex_scores();
    
    // Verify inner hex has higher coherence
    let inner_avg = scores.iter()
        .filter(|(pos, _)| pos.hex_distance(&Position::new(0, 0, 0)) == 1)
        .map(|(_, score)| score)
        .sum::<f64>() / 3.0;
        
    let outer_avg = scores.iter()
        .filter(|(pos, _)| pos.hex_distance(&Position::new(0, 0, 0)) == 2)
        .map(|(_, score)| score)
        .sum::<f64>() / 3.0;
        
    assert!(inner_avg > outer_avg,
        "Inner hex should have higher coherence");
}

#[test]
fn gsp_3601_coherence_propagation() {
    let coherence = CoherenceManager::new(Position::new(0, 0, 0));
    let metrics = CoherenceMetrics::new();
    
    // Set up a chain of hex positions
    let hex_chain = vec![
        Position::new(1, -1, 0),   // d=1
        Position::new(2, -1, -1),  // d=2
        Position::new(3, -2, -1),  // d=3
    ];
    
    // Record coherence values along chain
    for (i, pos) in hex_chain.iter().enumerate() {
        let node_id = NodeId::generate();
        coherence.record_node_state(node_id, NodeState {
            position: *pos,
            coherence: 1.0 - (i as f64 * 0.2), // Decreasing coherence
            last_seen: SystemTime::now(),
            ..Default::default()
        });
    }
    
    // Verify coherence propagation
    for window in hex_chain.windows(2) {
        let from_score = coherence.get_position_score(&window[0]);
        let to_score = coherence.get_position_score(&window[1]);
        
        assert!(from_score >= to_score,
            "Coherence should decrease with hex distance");
        assert!(to_score >= from_score * 0.7,
            "Coherence drop should be gradual across hex grid");
    }
}

#[test]
fn gsp_3602_coherence_recovery() {
    let coherence = CoherenceManager::new(Position::new(0, 0, 0));
    
    // Set up a hex cluster with low coherence
    let cluster_pos = vec![
        Position::new(1, -1, 0),
        Position::new(1, 0, -1),
        Position::new(0, 1, -1),
    ];
    
    for pos in &cluster_pos {
        let node_id = NodeId::generate();
        coherence.record_node_state(node_id, NodeState {
            position: *pos,
            coherence: 0.3, // Low initial coherence
            last_seen: SystemTime::now(),
            ..Default::default()
        });
    }
    
    // Simulate stability improvements
    for _ in 0..10 {
        for pos in &cluster_pos {
            let node_id = NodeId::generate();
            coherence.record_node_state(node_id, NodeState {
                position: *pos,
                coherence: 0.8, // Improved stability
                last_seen: SystemTime::now(),
                ..Default::default()
            });
        }
        coherence.update_metrics().await?;
    }
    
    // Verify coherence recovery
    for pos in &cluster_pos {
        let score = coherence.get_position_score(pos);
        assert!(score > 0.7,
            "Coherence should recover with sustained stability");
    }
}

#[test]
fn gsp_3603_hex_boundary_effects() {
    let coherence = CoherenceManager::new(Position::new(0, 0, 0));
    
    // Set up nodes at hex grid boundary
    let boundary_nodes = vec![
        // Inner nodes (full hex neighborhood)
        (Position::new(1, -1, 0), 0.9),
        (Position::new(1, 0, -1), 0.9),
        // Boundary nodes (partial hex neighborhood)
        (Position::new(3, -2, -1), 0.9),
        (Position::new(3, -1, -2), 0.9),
    ];
    
    for (pos, coh) in &boundary_nodes {
        let node_id = NodeId::generate();
        coherence.record_node_state(node_id, NodeState {
            position: *pos,
            coherence: *coh,
            last_seen: SystemTime::now(),
            ..Default::default()
        });
    }
    
    // Calculate boundary effects
    let scores = coherence.calculate_hex_scores();
    
    // Verify boundary coherence adjustment
    for (pos, _) in boundary_nodes {
        let score = scores.get(&pos).unwrap();
        let neighbors = coherence.count_hex_neighbors(&pos);
        
        if neighbors < 6 {
            assert!(*score < 0.9,
                "Boundary nodes should have reduced coherence");
        }
    }
}

#[test]
fn gsp_3604_temporal_coherence() {
    let coherence = CoherenceManager::new(Position::new(0, 0, 0));
    let node_id = NodeId::generate();
    let pos = Position::new(1, -1, 0);
    
    // Record state changes over time
    let timestamps: Vec<_> = (0..5)
        .map(|i| SystemTime::now() + Duration::from_secs(i * 60))
        .collect();
    
    // Simulate position stability then movement
    for &time in &timestamps[0..3] {
        coherence.record_node_state(node_id, NodeState {
            position: pos,
            last_seen: time,
            ..Default::default()
        });
    }
    
    // Simulate position changes
    for &time in &timestamps[3..] {
        coherence.record_node_state(node_id, NodeState {
            position: Position::new(
                pos.x + (time.duration_since(timestamps[0]).unwrap().as_secs() % 2) as i64,
                pos.y,
                pos.z
            ),
            last_seen: time,
            ..Default::default()
        });
    }
    
    let temporal_score = coherence.calculate_temporal_coherence(&pos);
    assert!(temporal_score < 0.8,
        "Temporal coherence should decrease with position changes");
} 