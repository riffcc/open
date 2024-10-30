use signals_rs::gsp::{CoherenceManager, NetworkManager, NodeState};
use signals_rs::common::{NodeId, Position};
use std::time::{Duration, SystemTime};

#[test]
fn gsp_3000_basic_gossip_propagation() {
    let network = NetworkManager::new();
    let coherence = CoherenceManager::new();
    
    // Create initial network state
    let nodes = vec![
        Position::new(0, 0, 0),
        Position::new(1, -1, 0),
        Position::new(1, 0, -1),
        Position::new(0, 1, -1),
    ];
    
    for pos in &nodes {
        let id = NodeId::generate();
        network.add_node(id, *pos);
        coherence.set_node_coherence(id, 0.8);
    }
    
    // Test gossip propagation
    let message = "test_gossip_message";
    let source = network.get_node_at(&nodes[0]).unwrap();
    let propagation = network.propagate_gossip(source, message).await?;
    
    assert!(propagation.reached_ratio() > 0.8, 
        "Gossip should reach most nodes");
}

#[test]
fn gsp_3001_coherence_based_propagation() {
    let network = NetworkManager::new();
    let coherence = CoherenceManager::new();
    
    // Create network with varying coherence
    let nodes = vec![
        (Position::new(0, 0, 0), 0.9),
        (Position::new(1, -1, 0), 0.7),
        (Position::new(1, 0, -1), 0.3),
        (Position::new(0, 1, -1), 0.8),
    ];
    
    for (pos, coh) in &nodes {
        let id = NodeId::generate();
        network.add_node(id, *pos);
        coherence.set_node_coherence(id, *coh);
    }
    
    let message = "test_message";
    let source = network.get_node_at(&nodes[0].0).unwrap();
    let propagation = network.propagate_with_coherence(source, message, &coherence).await?;
    
    assert!(propagation.low_coherence_reached_ratio() < 0.5,
        "Low coherence nodes should receive fewer messages");
}

#[test]
fn gsp_3002_gossip_validation() {
    let network = NetworkManager::new();
    let coherence = CoherenceManager::new();
    
    let source = NodeId::generate();
    network.add_node(source, Position::new(0, 0, 0));
    coherence.set_node_coherence(source, 0.8);
    
    // Test valid message
    let valid = network.validate_gossip(source, "valid_message").await?;
    assert!(valid.is_ok(), "Valid messages should be accepted");
    
    // Test invalid message
    coherence.set_node_coherence(source, 0.2);
    let invalid = network.validate_gossip(source, "spam_message").await?;
    assert!(invalid.is_err(), "Messages from low coherence nodes should be rejected");
}

#[test]
fn gsp_3003_network_partition_handling() {
    let network = NetworkManager::new();
    let coherence = CoherenceManager::new();
    
    // Create partitioned network
    let partition_a = vec![
        Position::new(0, 0, 0),
        Position::new(1, -1, 0),
    ];
    
    let partition_b = vec![
        Position::new(3, -2, -1),
        Position::new(4, -2, -2),
    ];
    
    for pos in partition_a.iter().chain(partition_b.iter()) {
        let id = NodeId::generate();
        network.add_node(id, *pos);
        coherence.set_node_coherence(id, 0.8);
    }
    
    let source = network.get_node_at(&partition_a[0]).unwrap();
    let propagation = network.propagate_gossip(source, "test").await?;
    
    assert!(propagation.partition_aware(),
        "Gossip should detect network partitions");
}

#[test]
fn gsp_3004_gossip_performance() {
    let network = NetworkManager::new();
    let coherence = CoherenceManager::new();
    
    // Create large network
    let node_count = 1000;
    for i in 0..node_count {
        let id = NodeId::generate();
        let x = i as i64 / 10;
        let y = -(i as i64 % 10);
        let pos = Position::new(x, y, -x-y);
        network.add_node(id, pos);
        coherence.set_node_coherence(id, 0.8);
    }
    
    let start = SystemTime::now();
    let source = NodeId::generate();
    network.propagate_gossip(source, "scale_test").await?;
    let duration = SystemTime::now().duration_since(start).unwrap();
    
    assert!(duration < Duration::from_secs(1),
        "Gossip should scale efficiently");
} 