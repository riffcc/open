use signals_rs::hex::{Position, NodeState};
use signals_rs::common::NodeId;
use std::time::{Duration, SystemTime};

#[test]
fn hex_1200_initial_position() {
    let node = NodeState::new(NodeId::generate());
    assert_eq!(node.position(), &Position::new(0,0,0),
        "New node should start at origin position");
}

#[test]
fn hex_1201_position_persistence() {
    let node_id = NodeId::generate();
    let position = Position::new(1,-1,0);
    
    // Write position to Iroh
    let node = NodeState::new(node_id);
    node.update_position(position.clone()).await?;
    
    // Create new node instance and verify position loads
    let node2 = NodeState::new(node_id);
    assert_eq!(node2.position(), &position,
        "Position should persist across node restarts");
}

#[test]
fn hex_1202_neighbor_notifications() {
    let node = NodeState::new(NodeId::generate());
    let neighbor_id = NodeId::generate();
    
    // Add neighbor and set up notification monitoring
    node.add_neighbor(neighbor_id).await?;
    let notification_rx = node.position_updates();
    
    // Update position
    let new_pos = Position::new(1,-1,0);
    node.update_position(new_pos.clone()).await?;
    
    // Verify neighbor was notified
    let update = notification_rx.recv_timeout(Duration::from_secs(1))
        .expect("Should receive position update notification");
    assert_eq!(update.position, new_pos);
    assert_eq!(update.node_id, node.id());
}

#[test]
fn hex_1203_concurrent_updates() {
    let node_id = NodeId::generate();
    let node1 = NodeState::new(node_id);
    let node2 = NodeState::new(node_id);
    
    // Attempt concurrent position updates
    let pos1 = Position::new(1,-1,0);
    let pos2 = Position::new(-1,1,0);
    
    let update1 = node1.update_position(pos1.clone());
    let update2 = node2.update_position(pos2.clone());
    
    // Let both updates complete
    futures::join!(update1, update2);
    
    // Verify consistent final state
    let final_pos = node1.position();
    assert_eq!(node2.position(), final_pos,
        "Position should be consistent across instances");
}

#[test]
fn hex_1204_temporal_ordering() {
    let node = NodeState::new(NodeId::generate());
    let positions = vec![
        Position::new(0,0,0),
        Position::new(1,-1,0),
        Position::new(1,0,-1),
    ];
    
    // Record update times
    let mut update_times = Vec::new();
    
    for pos in positions {
        let start = SystemTime::now();
        node.update_position(pos).await?;
        update_times.push((start, pos));
    }
    
    // Verify history maintains temporal order
    let history = node.position_history();
    for window in history.windows(2) {
        assert!(window[0].timestamp <= window[1].timestamp,
            "Position updates should maintain temporal ordering");
    }
} 