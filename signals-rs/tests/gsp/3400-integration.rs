use signals_rs::gsp::{GossipProtocol, GossipState, SecurityManager, UpdateContent};
use signals_rs::hex::Position;
use signals_rs::common::NodeId;
use signals_rs::crypto::SigningKey;
use std::time::{Duration, SystemTime};

/// Helper to create a hex cluster with a center and 6 surrounding nodes
fn create_hex_cluster(center_pos: Position) -> Vec<(NodeId, GossipProtocol, Position)> {
    let mut nodes = Vec::new();
    
    // Center node
    nodes.push((
        NodeId::generate(),
        GossipProtocol::new(NodeId::generate(), SecurityManager::new()),
        center_pos
    ));
    
    // Six surrounding nodes in hex pattern
    let hex_offsets = [
        (1, -1, 0),   // East
        (1, 0, -1),   // Southeast
        (0, 1, -1),   // Southwest
        (-1, 1, 0),   // West
        (-1, 0, 1),   // Northwest
        (0, -1, 1),   // Northeast
    ];
    
    for (dx, dy, dz) in hex_offsets {
        let pos = Position::new(
            center_pos.x + dx,
            center_pos.y + dy,
            center_pos.z + dz
        );
        nodes.push((
            NodeId::generate(),
            GossipProtocol::new(NodeId::generate(), SecurityManager::new()),
            pos
        ));
    }
    
    // Connect nodes according to hex grid adjacency
    for i in 0..nodes.len() {
        for j in 0..nodes.len() {
            if i != j && nodes[i].2.hex_distance(&nodes[j].2) == 1 {
                nodes[i].1.add_neighbor(nodes[j].0, 0.9).await?;
            }
        }
    }
    
    nodes
}

#[test]
fn gsp_3400_hex_gossip_propagation() {
    let cluster = create_hex_cluster(Position::new(0, 0, 0));
    
    // Initiate state change at center node
    let update = UpdateContent::Position(Position::new(1, -1, 0));
    cluster[0].1.broadcast_update(update.clone()).await?;
    
    // Let gossip propagate through hex grid
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Verify propagation follows hex distance pattern
    for (_, node, pos) in &cluster {
        let received = node.received_updates().await?;
        let update_time = received.iter()
            .find(|u| u.content == update)
            .expect("Update should have propagated")
            .timestamp;
            
        let center_distance = pos.hex_distance(&Position::new(0, 0, 0));
        let propagation_delay = update_time.duration_since(SystemTime::now())?;
        
        // Updates should arrive faster to closer nodes
        assert!(propagation_delay <= Duration::from_millis(50 * center_distance as u64),
            "Gossip propagation should follow hex distance");
    }
}

#[test]
fn gsp_3401_hex_partition_recovery() {
    // Create two hex clusters with a gap between them
    let cluster_a = create_hex_cluster(Position::new(-3, 2, 1));
    let cluster_b = create_hex_cluster(Position::new(3, -2, -1));
    
    // Broadcast different updates in each cluster
    let update_a = UpdateContent::Position(Position::new(-4, 2, 2));
    let update_b = UpdateContent::Position(Position::new(4, -2, -2));
    
    cluster_a[0].1.broadcast_update(update_a.clone()).await?;
    cluster_b[0].1.broadcast_update(update_b.clone()).await?;
    
    // Let clusters converge internally
    tokio::time::sleep(Duration::from_millis(200)).await;
    
    // Bridge the clusters by connecting nearest edge nodes
    let bridge_a = cluster_a.iter()
        .max_by_key(|(_, _, pos)| pos.x)
        .unwrap();
    let bridge_b = cluster_b.iter()
        .min_by_key(|(_, _, pos)| pos.x)
        .unwrap();
        
    bridge_a.1.add_neighbor(bridge_b.0, 0.9).await?;
    bridge_b.1.add_neighbor(bridge_a.0, 0.9).await?;
    
    // Let updates propagate across bridge
    tokio::time::sleep(Duration::from_millis(300)).await;
    
    // Verify both updates reached all nodes
    for cluster in [cluster_a, cluster_b] {
        for (_, node, _) in cluster {
            let received = node.received_updates().await?;
            assert!(received.iter().any(|u| u.content == update_a),
                "Update A should reach all nodes");
            assert!(received.iter().any(|u| u.content == update_b),
                "Update B should reach all nodes");
        }
    }
}

#[test]
fn gsp_3402_hex_concurrent_updates() {
    let cluster = create_hex_cluster(Position::new(0, 0, 0));
    
    // Launch concurrent updates from multiple nodes
    let mut update_futures = Vec::new();
    
    for (i, (_, node, pos)) in cluster.iter().enumerate() {
        let update = UpdateContent::Position(Position::new(
            pos.x + (i as i64),
            pos.y - (i as i64),
            pos.z
        ));
        update_futures.push(node.broadcast_update(update));
    }
    
    // Wait for all updates to complete
    futures::future::join_all(update_futures).await?;
    
    // Verify all nodes received all updates in consistent order
    let mut last_order = None;
    for (_, node, _) in &cluster {
        let received = node.received_updates().await?;
        let order: Vec<_> = received.iter()
            .map(|u| u.content.clone())
            .collect();
            
        if let Some(last) = last_order {
            assert_eq!(order, last, "Update order should be consistent across nodes");
        }
        last_order = Some(order);
    }
} 