use signals_rs::nbr::{RouteTable, Route, RouteUpdate, PathMetrics, Position};
use signals_rs::common::NodeId;
use std::time::{Duration, SystemTime};

#[test]
fn nbr_2200_track_relative_positions() {
    let table = RouteTable::new(NodeId::generate());
    let neighbor_id = NodeId::generate();
    
    let rel_pos = RelativePosition {
        dx: 1, dy: -1, dz: 0,
        last_seen: SystemTime::now(),
    };
    
    table.add_neighbor_position(neighbor_id, rel_pos.clone()).await?;
    
    let stored = table.get_neighbor_position(neighbor_id)
        .expect("Should have neighbor position");
    assert_eq!(stored, rel_pos);
}

#[test]
fn nbr_2201_convert_positions() {
    let table = RouteTable::new(NodeId::generate());
    let neighbor_id = NodeId::generate();
    
    // Add absolute position
    let abs_pos = Position::new(2, -1, -1);
    table.add_neighbor_absolute(neighbor_id, abs_pos).await?;
    
    // Verify relative conversion
    let rel_pos = table.get_neighbor_position(neighbor_id)
        .expect("Should have relative position");
    assert_eq!(rel_pos.dx, 2);
    assert_eq!(rel_pos.dy, -1);
    assert_eq!(rel_pos.dz, -1);
    
    // Convert back to absolute
    let abs_again = table.get_neighbor_absolute(neighbor_id)
        .expect("Should convert to absolute");
    assert_eq!(abs_again, abs_pos);
}

#[test]
fn nbr_2202_handle_position_updates() {
    let table = RouteTable::new(NodeId::generate());
    let neighbor_id = NodeId::generate();
    let update_rx = table.position_updates();
    
    // Add initial position
    table.add_neighbor_position(neighbor_id, RelativePosition {
        dx: 1, dy: -1, dz: 0,
        last_seen: SystemTime::now(),
    }).await?;
    
    // Update position
    let new_pos = RelativePosition {
        dx: 2, dy: -1, dz: -1,
        last_seen: SystemTime::now(),
    };
    table.update_neighbor_position(neighbor_id, new_pos.clone()).await?;
    
    // Verify update was received
    let update = update_rx.recv_timeout(Duration::from_secs(1))
        .expect("Should receive position update");
    assert_eq!(update.neighbor_id, neighbor_id);
    assert_eq!(update.new_position, new_pos);
}

#[test]
fn nbr_2203_maintain_position_constraints() {
    let table = RouteTable::new(NodeId::generate());
    let neighbor_id = NodeId::generate();
    
    // Try invalid positions
    let invalid_pos = RelativePosition {
        dx: 1, dy: 1, dz: 1, // Sum != 0
        last_seen: SystemTime::now(),
    };
    
    let result = table.add_neighbor_position(neighbor_id, invalid_pos).await;
    assert!(result.is_err(), "Should reject invalid positions");
    
    // Verify valid positions work
    let valid_pos = RelativePosition {
        dx: 1, dy: -1, dz: 0,
        last_seen: SystemTime::now(),
    };
    assert!(table.add_neighbor_position(neighbor_id, valid_pos).await.is_ok());
}

#[test]
fn nbr_2204_position_change_notifications() {
    let table = RouteTable::new(NodeId::generate());
    let neighbor_id = NodeId::generate();
    let notify_rx = table.position_notifications();
    
    // Add position and make changes
    let positions = vec![
        RelativePosition::new(1, -1, 0),
        RelativePosition::new(2, -1, -1),
        RelativePosition::new(1, 0, -1),
    ];
    
    for pos in positions {
        table.update_neighbor_position(neighbor_id, pos.clone()).await?;
        let notification = notify_rx.recv_timeout(Duration::from_secs(1))
            .expect("Should receive notification");
        assert_eq!(notification.neighbor_id, neighbor_id);
        assert_eq!(notification.position, pos);
    }
} 