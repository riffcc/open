use signals_rs::gsp::{CoherenceManager, NetworkManager, NodeState, CoherenceMetrics};
use signals_rs::hex::Position;
use signals_rs::common::NodeId;
use signals_rs::routing::RoutingTable;
use std::time::{Duration, SystemTime};

#[test]
fn gsp_4600_routing_coherence_integration() {
    let coherence = CoherenceManager::new();
    let network = NetworkManager::new();
    let metrics = CoherenceMetrics::new();
    
    // Create network with varying coherence levels
    let nodes = vec![
        (Position::new(0, 0, 0), 0.9),    // High coherence
        (Position::new(1, -1, 0), 0.7),
        (Position::new(1, 0, -1), 0.3),   // Low coherence
        (Position::new(0, 1, -1), 0.8),
    ];
    
    for (pos, coh) in &nodes {
        let id = NodeId::generate();
        network.add_node(id, *pos);
        coherence.set_node_coherence(id, *coh);
    }
    
    // Build routing tables
    let mut routing = RoutingTable::new();
    routing.rebuild_with_coherence(&network, &coherence);
    
    // Verify routing decisions
    for (start_pos, _) in &nodes {
        let start_id = network.get_node_at(start_pos).unwrap();
        
        for (end_pos, _) in &nodes {
            if start_pos != end_pos {
                let end_id = network.get_node_at(end_pos).unwrap();
                let path = routing.get_path(start_id, end_id);
                
                if let Some(route) = path {
                    // Verify path avoids low coherence nodes
                    for hop in route.path() {
                        let hop_coherence = coherence.get_current_coherence(*hop);
                        assert!(hop_coherence >= 0.5,
                            "Routes should avoid low coherence nodes");
                    }
                }
            }
        }
    }
}

#[test]
fn gsp_4601_network_view_consistency() {
    let coherence = CoherenceManager::new();
    let network = NetworkManager::new();
    let metrics = CoherenceMetrics::new();
    
    // Create initial network state
    let mut nodes = Vec::new();
    for i in 0..10 {
        let id = NodeId::generate();
        let pos = Position::new(i, -i, 0);
        nodes.push((id, pos));
        network.add_node(id, pos);
        coherence.set_node_coherence(id, 0.8);
    }
    
    // Apply series of updates
    let updates = vec![
        (nodes[0].0, 0.3),  // Coherence drop
        (nodes[1].0, 0.9),  // Coherence increase
        (nodes[2].0, 0.0),  // Complete defederation
    ];
    
    for (node_id, new_coherence) in updates {
        coherence.update_node_coherence(node_id, new_coherence, "test update");
        
        // Verify network view remains consistent
        let view = network.get_network_view(&coherence);
        
        // Check all nodes see same coherence-filtered view
        for (id, _) in &nodes {
            let local_view = network.get_local_view(*id, &coherence);
            assert_eq!(view.coherence_filtered(), local_view.coherence_filtered(),
                "Network views should be consistent across nodes");
        }
    }
}

#[test]
fn gsp_4602_partition_handling() {
    let coherence = CoherenceManager::new();
    let network = NetworkManager::new();
    let metrics = CoherenceMetrics::new();
    
    // Create two connected network regions
    let mut region_a = Vec::new();
    let mut region_b = Vec::new();
    
    // Populate regions
    for i in 0..5 {
        let id = NodeId::generate();
        let pos = Position::new(i, -i, 0);
        region_a.push((id, pos));
        network.add_node(id, pos);
        coherence.set_node_coherence(id, 0.8);
    }
    
    for i in 0..5 {
        let id = NodeId::generate();
        let pos = Position::new(i+6, -(i+6), 0);
        region_b.push((id, pos));
        network.add_node(id, pos);
        coherence.set_node_coherence(id, 0.8);
    }
    
    // Create bridge nodes
    let bridge_a = NodeId::generate();
    let bridge_b = NodeId::generate();
    network.add_node(bridge_a, Position::new(5, -5, 0));
    network.add_node(bridge_b, Position::new(6, -6, 0));
    
    // Simulate partition by defederating bridge nodes
    coherence.set_node_coherence(bridge_a, 0.0);
    coherence.set_node_coherence(bridge_b, 0.0);
    
    // Verify partition handling
    let partition_info = network.analyze_partitions(&coherence);
    assert_eq!(partition_info.partition_count(), 2,
        "Should detect network partition");
        
    // Check partition recovery
    coherence.set_node_coherence(bridge_a, 0.8);
    let recovered = network.analyze_partitions(&coherence);
    assert_eq!(recovered.partition_count(), 1,
        "Should detect partition recovery");
}

#[test]
fn gsp_4603_network_scaling() {
    let coherence = CoherenceManager::new();
    let network = NetworkManager::new();
    let metrics = CoherenceMetrics::new();
    
    // Create large network
    let node_count = 1000;
    let mut nodes = Vec::new();
    
    for i in 0..node_count {
        let id = NodeId::generate();
        let x = i as i64 / 10;
        let y = -(i as i64 % 10);
        let pos = Position::new(x, y, -x-y);
        nodes.push((id, pos));
        network.add_node(id, pos);
        coherence.set_node_coherence(id, 0.8);
    }
    
    // Measure performance metrics
    let start = SystemTime::now();
    
    // Test coherence updates
    for (id, _) in nodes.iter().take(100) {
        coherence.update_node_coherence(*id, 0.7, "test update");
    }
    
    // Test routing updates
    let mut routing = RoutingTable::new();
    routing.rebuild_with_coherence(&network, &coherence);
    
    let duration = SystemTime::now().duration_since(start).unwrap();
    
    // Verify scaling properties
    assert!(duration < Duration::from_secs(1),
        "Operations should scale reasonably with network size");
    
    metrics.record_scaling_metrics(node_count, duration);
}

#[test]
fn gsp_4604_extreme_coherence_recovery() {
    let coherence = CoherenceManager::new();
    let network = NetworkManager::new();
    let metrics = CoherenceMetrics::new();
    
    // Create network
    let mut nodes = Vec::new();
    for i in 0..20 {
        let id = NodeId::generate();
        let pos = Position::new(i, -i, 0);
        nodes.push((id, pos));
        network.add_node(id, pos);
        coherence.set_node_coherence(id, 0.8);
    }
    
    // Create extreme coherence event
    for (id, _) in &nodes {
        coherence.set_node_coherence(*id, 0.0);
    }
    
    // Simulate recovery process
    for (id, _) in &nodes {
        // Gradual coherence restoration
        for i in 1..=10 {
            let new_coherence = i as f64 * 0.1;
            coherence.update_node_coherence(*id, new_coherence, "recovery");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    
    // Verify network recovery
    let final_state = network.analyze_health(&coherence);
    assert!(final_state.is_healthy(),
        "Network should recover from extreme coherence event");
    
    // Check recovery metrics
    let recovery_stats = metrics.analyze_recovery_process();
    assert!(recovery_stats.is_stable(),
        "Recovery process should result in stable network");
} 