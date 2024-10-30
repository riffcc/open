use signals_rs::hex::{Position, NodeState, RelativePosition};
use signals_rs::common::{NodeId, Timestamp};
use std::time::{Duration, SystemTime};

#[test]
fn hex_1300_temporal_position_tracking() {
    let node = NodeState::new(NodeId::generate());
    let neighbor_id = NodeId::generate();
    
    // Add neighbor with initial relative position
    let rel_pos = RelativePosition {
        dx: 1, dy: -1, dz: 0,
        last_seen: SystemTime::now(),
    };
    
    node.add_neighbor_position(neighbor_id, rel_pos.clone()).await?;
    
    // Verify temporal data is tracked
    let stored = node.get_neighbor_position(neighbor_id)
        .expect("Should have neighbor position");
    
    assert_eq!(stored.dx, rel_pos.dx);
    assert_eq!(stored.dy, rel_pos.dy);
    assert_eq!(stored.dz, rel_pos.dz);
    assert!(SystemTime::now()
        .duration_since(stored.last_seen)
        .unwrap() < Duration::from_secs(1));
}

#[test]
fn hex_1301_stale_neighbor_detection() {
    let node = NodeState::new(NodeId::generate());
    let neighbor_id = NodeId::generate();
    
    // Add neighbor with old timestamp
    let stale_time = SystemTime::now() - Duration::from_secs(3600); // 1 hour old
    let rel_pos = RelativePosition {
        dx: 1, dy: -1, dz: 0,
        last_seen: stale_time,
    };
    
    node.add_neighbor_position(neighbor_id, rel_pos).await?;
    
    // Verify stale detection
    assert!(node.is_neighbor_stale(neighbor_id),
        "Should detect stale neighbor data");
    
    // Verify cleanup
    node.prune_stale_neighbors().await?;
    assert!(node.get_neighbor_position(neighbor_id).is_none(),
        "Stale neighbor should be removed");
}

#[test]
fn hex_1302_position_update_consistency() {
    let node = NodeState::new(NodeId::generate());
    let neighbor_id = NodeId::generate();
    
    // Set up initial positions
    node.update_position(Position::new(0,0,0)).await?;
    node.add_neighbor_position(neighbor_id, RelativePosition {
        dx: 1, dy: -1, dz: 0,
        last_seen: SystemTime::now(),
    }).await?;
    
    // Update node position
    node.update_position(Position::new(1,0,-1)).await?;
    
    // Verify relative positions updated correctly
    let updated = node.get_neighbor_position(neighbor_id)
        .expect("Should have neighbor position");
    
    assert_eq!(updated.dx, 0); // Relative position should adjust
    assert_eq!(updated.dy, -1);
    assert_eq!(updated.dz, 1);
}

#[test]
fn hex_1303_neighbor_change_triggers() {
    let node = NodeState::new(NodeId::generate());
    let neighbor_id = NodeId::generate();
    
    // Set up update monitoring
    let update_rx = node.position_updates();
    
    // Add neighbor with position
    node.add_neighbor_position(neighbor_id, RelativePosition {
        dx: 1, dy: -1, dz: 0,
        last_seen: SystemTime::now(),
    }).await?;
    
    // Update neighbor position
    let new_rel_pos = RelativePosition {
        dx: 2, dy: -1, dz: -1,
        last_seen: SystemTime::now(),
    };
    node.update_neighbor_position(neighbor_id, new_rel_pos).await?;
    
    // Verify local update triggered
    let update = update_rx.recv_timeout(Duration::from_secs(1))
        .expect("Should receive position update");
    assert!(update.triggered_by_neighbor);
}

#[test]
fn hex_1304_invalid_update_rejection() {
    let node = NodeState::new(NodeId::generate());
    let neighbor_id = NodeId::generate();
    
    // Try to add invalid relative position
    let invalid_pos = RelativePosition {
        dx: 1, dy: 1, dz: 1, // Invalid: sum != 0
        last_seen: SystemTime::now(),
    };
    
    let result = node.add_neighbor_position(neighbor_id, invalid_pos).await;
    assert!(result.is_err(), "Should reject invalid relative position");
    
    // Verify no position was stored
    assert!(node.get_neighbor_position(neighbor_id).is_none(),
        "Invalid position should not be stored");
} 