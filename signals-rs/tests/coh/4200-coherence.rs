use signals_rs::gsp::{CoherenceManager, NodeState, CoherenceMetrics};
use signals_rs::hex::Position;
use signals_rs::common::NodeId;
use std::time::{Duration, SystemTime};

#[test]
fn gsp_4200_track_node_introduction() {
    let coherence = CoherenceManager::new();
    let metrics = CoherenceMetrics::new();
    
    // Create a chain of node introductions
    let nodes = vec![
        (NodeId::generate(), None),              // Original node
        (NodeId::generate(), Some(0)),           // Introduced by first node
        (NodeId::generate(), Some(1)),           // Introduced by second node
        (NodeId::generate(), Some(0)),           // Another from first node
    ];
    
    // Record introductions
    for (idx, (node_id, introducer)) in nodes.iter().enumerate() {
        if let Some(intro_idx) = introducer {
            let introducer_id = nodes[*intro_idx].0;
            coherence.record_node_introduction(*node_id, introducer_id);
        }
        metrics.record_introduction_event(*node_id, SystemTime::now());
    }
    
    // Verify introduction tracking
    let first_node = nodes[0].0;
    let introduction_count = coherence.get_introduction_count(first_node);
    assert_eq!(introduction_count, 2,
        "Should track correct number of introductions");
        
    // Verify introduction patterns
    let patterns = coherence.analyze_introduction_patterns();
    assert!(patterns.is_healthy(),
        "Introduction patterns should be within normal bounds");
}

#[test]
fn gsp_4201_sponsor_relationship_tracking() {
    let coherence = CoherenceManager::new();
    
    // Create sponsor relationships
    let sponsor = NodeId::generate();
    let sponsored_nodes = vec![
        (NodeId::generate(), 0.9),  // Good behavior
        (NodeId::generate(), 0.7),  // Moderate behavior
        (NodeId::generate(), 0.3),  // Poor behavior
    ];
    
    // Record sponsored node behaviors
    for (node_id, behavior_score) in &sponsored_nodes {
        coherence.record_node_introduction(*node_id, sponsor);
        coherence.record_node_behavior(*node_id, *behavior_score);
    }
    
    // Calculate sponsor responsibility
    let sponsor_score = coherence.calculate_sponsor_score(sponsor);
    
    // Verify sponsor impact
    assert!(sponsor_score < 0.9,
        "Sponsor score should be affected by poor behavior of sponsored nodes");
    
    // Verify responsibility chain
    let chain = coherence.get_responsibility_chain(sponsored_nodes[2].0);
    assert_eq!(chain[0], sponsor,
        "Responsibility chain should track back to sponsor");
}

#[test]
fn gsp_4202_coherence_persistence() {
    let coherence = CoherenceManager::new();
    let node_id = NodeId::generate();
    
    // Record series of coherence updates
    let updates = vec![
        (0.9, "initial high coherence"),
        (0.7, "minor violation"),
        (0.4, "major violation"),
        (0.5, "partial recovery"),
    ];
    
    for (value, reason) in updates {
        coherence.update_node_coherence(node_id, value, reason);
    }
    
    // Verify persistence
    let history = coherence.get_coherence_history(node_id);
    assert_eq!(history.len(), updates.len(),
        "Should maintain complete coherence history");
        
    // Verify state consistency
    let current = coherence.get_current_coherence(node_id);
    assert_eq!(current, 0.5,
        "Current coherence should reflect latest update");
}

#[test]
fn gsp_4203_route_coherence_updates() {
    let coherence = CoherenceManager::new();
    let metrics = CoherenceMetrics::new();
    
    // Create network with varying coherence levels
    let nodes = vec![
        (Position::new(0, 0, 0), 0.9),    // High coherence
        (Position::new(1, -1, 0), 0.7),
        (Position::new(1, 0, -1), 0.4),   // Low coherence
        (Position::new(0, 1, -1), 0.8),
    ];
    
    for (pos, coh) in &nodes {
        let id = NodeId::generate();
        coherence.record_node_state(id, NodeState {
            position: *pos,
            coherence: *coh,
            last_seen: SystemTime::now(),
        });
    }
    
    // Calculate route preferences
    let routes = coherence.calculate_route_preferences();
    
    // Verify coherence affects routing
    for (start, end) in routes.iter() {
        let path = routes.get_path(start, end).unwrap();
        let path_coherence: f64 = path.iter()
            .map(|id| coherence.get_current_coherence(*id))
            .sum::<f64>() / path.len() as f64;
            
        assert!(path_coherence >= 0.6,
            "Routes should prefer higher coherence paths");
    }
}

#[test]
fn gsp_4204_coherence_based_filtering() {
    let coherence = CoherenceManager::new();
    
    // Create nodes with varying coherence
    let nodes = vec![
        (NodeId::generate(), 0.9),
        (NodeId::generate(), 0.7),
        (NodeId::generate(), 0.3),
        (NodeId::generate(), 0.1),
    ];
    
    for (id, coh) in &nodes {
        coherence.set_node_coherence(*id, *coh);
    }
    
    // Test different coherence thresholds
    let thresholds = vec![0.8, 0.5, 0.2];
    
    for threshold in thresholds {
        let filtered = coherence.filter_network_view(threshold);
        
        // Verify filtering
        for (id, coh) in &nodes {
            let included = filtered.contains(id);
            assert_eq!(included, *coh >= threshold,
                "Network view should filter based on coherence threshold");
        }
    }
} 