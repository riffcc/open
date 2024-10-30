use signals_rs::nbr::{PathDiscovery, Route, PathMetrics};
use signals_rs::hex::Position;
use signals_rs::common::NodeId;
use std::time::Duration;

#[test]
fn nbr_2100_optimize_hex_distance() {
    let discovery = PathDiscovery::new(NodeId::generate());
    let target = NodeId::generate();
    
    // Set up a network topology with multiple possible paths
    let topology = vec![
        // (node_id, position)
        (NodeId::generate(), Position::new(1, -1, 0)),  // Path A
        (NodeId::generate(), Position::new(1, 0, -1)),  // Path B
        (NodeId::generate(), Position::new(2, -1, -1)), // Longer path
    ];
    
    for (id, pos) in &topology {
        discovery.add_node(*id, pos.clone()).await?;
    }
    
    // Find path to target
    let path = discovery.find_path(target).await?;
    
    // Verify chosen path minimizes hex distance
    let path_distance = discovery.calculate_path_distance(&path);
    for node in topology.iter() {
        let alt_path = discovery.calculate_path_through(node.0, target);
        assert!(path_distance <= discovery.calculate_path_distance(&alt_path),
            "Selected path should have minimal hex distance");
    }
}

#[test]
fn nbr_2101_neighbor_disconnect_updates() {
    let discovery = PathDiscovery::new(NodeId::generate());
    let target = NodeId::generate();
    let intermediate = NodeId::generate();
    
    // Set up initial path through intermediate node
    discovery.add_node(intermediate, Position::new(1, -1, 0)).await?;
    discovery.add_node(target, Position::new(2, -1, -1)).await?;
    
    let initial_path = discovery.find_path(target).await?;
    assert!(initial_path.nodes.contains(&intermediate),
        "Initial path should use intermediate node");
    
    // Simulate intermediate node disconnection
    discovery.handle_node_disconnect(intermediate).await?;
    
    // Verify path is updated
    let new_path = discovery.find_path(target).await?;
    assert!(!new_path.nodes.contains(&intermediate),
        "Updated path should not use disconnected node");
}

#[test]
fn nbr_2102_route_table_updates() {
    let discovery = PathDiscovery::new(NodeId::generate());
    let target = NodeId::generate();
    
    // Set up initial route
    let initial_next_hop = NodeId::generate();
    discovery.add_node(initial_next_hop, Position::new(1, -1, 0)).await?;
    discovery.add_node(target, Position::new(2, -1, -1)).await?;
    
    let route_updates = discovery.route_updates();
    
    // Add better path
    let better_hop = NodeId::generate();
    discovery.add_node(better_hop, Position::new(1, 0, -1)).await?;
    
    // Verify route table update
    let update = route_updates.recv_timeout(Duration::from_secs(1))
        .expect("Should receive route update");
    
    assert_eq!(update.target, target);
    assert_eq!(update.next_hop, better_hop);
}

#[test]
fn nbr_2103_respect_coherence() {
    let discovery = PathDiscovery::new(NodeId::generate());
    let target = NodeId::generate();
    let low_coherence_node = NodeId::generate();
    let high_coherence_node = NodeId::generate();
    
    // Add nodes with different coherence ratings
    discovery.add_node_with_coherence(low_coherence_node, 
        Position::new(1, -1, 0), 0.2).await?;
    discovery.add_node_with_coherence(high_coherence_node,
        Position::new(1, 0, -1), 0.9).await?;
    
    let path = discovery.find_path(target).await?;
    
    assert!(path.nodes.contains(&high_coherence_node));
    assert!(!path.nodes.contains(&low_coherence_node),
        "Path should prefer high coherence nodes");
}

#[test]
fn nbr_2104_reject_invalid_paths() {
    let discovery = PathDiscovery::new(NodeId::generate());
    let target = NodeId::generate();
    
    // Try to add invalid path (disconnected nodes)
    let result = discovery.add_path(Route {
        target,
        nodes: vec![NodeId::generate(), NodeId::generate()],
        metrics: PathMetrics::default(),
    }).await;
    
    assert!(result.is_err(), "Should reject invalid path");
    
    // Try to add path with cycle
    let node = NodeId::generate();
    let result = discovery.add_path(Route {
        target,
        nodes: vec![node, NodeId::generate(), node],
        metrics: PathMetrics::default(),
    }).await;
    
    assert!(result.is_err(), "Should reject path with cycles");
} 