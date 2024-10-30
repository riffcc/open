use signals_rs::nbr::{NeighborTable, RelativePosition};
use signals_rs::common::{NodeId, Timestamp};
use std::time::{Duration, SystemTime};

#[test]
fn nbr_2000_track_relative_positions() {
    let table = NeighborTable::new();
    let neighbor_id = NodeId::generate();
    
    let rel_pos = RelativePosition {
        dx: 1, dy: -1, dz: 0,
        last_seen: SystemTime::now(),
    };
    
    table.add_neighbor(neighbor_id, rel_pos.clone()).await?;
    
    let stored = table.get_neighbor(neighbor_id)
        .expect("Should have neighbor entry");
    
    assert_eq!(stored.position, rel_pos);
    assert!(table.has_neighbor(neighbor_id));
}

#[test]
fn nbr_2001_timestamp_validation() {
    let table = NeighborTable::new();
    let neighbor_id = NodeId::generate();
    
    // Add neighbor with current timestamp
    let now = SystemTime::now();
    table.add_neighbor(neighbor_id, RelativePosition {
        dx: 1, dy: -1, dz: 0,
        last_seen: now,
    }).await?;
    
    // Try to update with older timestamp
    let old_time = now - Duration::from_secs(60);
    let result = table.update_neighbor(neighbor_id, RelativePosition {
        dx: 2, dy: -1, dz: -1,
        last_seen: old_time,
    }).await;
    
    assert!(result.is_err(), "Should reject updates with older timestamps");
}

#[test]
fn nbr_2002_prune_stale_neighbors() {
    let table = NeighborTable::new();
    let stale_id = NodeId::generate();
    let fresh_id = NodeId::generate();
    
    // Add stale neighbor
    table.add_neighbor(stale_id, RelativePosition {
        dx: 1, dy: -1, dz: 0,
        last_seen: SystemTime::now() - Duration::from_secs(3600),
    }).await?;
    
    // Add fresh neighbor
    table.add_neighbor(fresh_id, RelativePosition {
        dx: -1, dy: 1, dz: 0,
        last_seen: SystemTime::now(),
    }).await?;
    
    table.prune_stale_entries().await?;
    
    assert!(!table.has_neighbor(stale_id), "Stale neighbor should be removed");
    assert!(table.has_neighbor(fresh_id), "Fresh neighbor should remain");
}

#[test]
fn nbr_2003_persist_across_restart() {
    let node_id = NodeId::generate();
    let neighbor_id = NodeId::generate();
    
    // Create and populate table
    let table1 = NeighborTable::new(node_id);
    table1.add_neighbor(neighbor_id, RelativePosition {
        dx: 1, dy: -1, dz: 0,
        last_seen: SystemTime::now(),
    }).await?;
    
    // Create new instance and verify data loads
    let table2 = NeighborTable::new(node_id);
    assert!(table2.has_neighbor(neighbor_id), 
        "Neighbor table should persist across restarts");
}

#[test]
fn nbr_2004_concurrent_updates() {
    let table = NeighborTable::new();
    let neighbor_id = NodeId::generate();
    
    // Simulate concurrent updates
    let update1 = table.update_neighbor(neighbor_id, RelativePosition {
        dx: 1, dy: -1, dz: 0,
        last_seen: SystemTime::now(),
    });
    
    let update2 = table.update_neighbor(neighbor_id, RelativePosition {
        dx: -1, dy: 1, dz: 0,
        last_seen: SystemTime::now(),
    });
    
    let (result1, result2) = futures::join!(update1, update2);
    assert!(result1.is_ok() || result2.is_ok(), 
        "At least one update should succeed");
    
    // Verify table is in consistent state
    let final_pos = table.get_neighbor(neighbor_id)
        .expect("Should have final position");
    assert!(final_pos.position.is_valid());
} 