use signals_rs::gsp::{GossipState, StateManager, StateUpdate, StateSnapshot};
use signals_rs::common::NodeId;
use signals_rs::hex::Position;
use std::time::{Duration, SystemTime};

#[test]
fn gsp_3200_state_convergence() {
    let manager = StateManager::new(NodeId::generate());
    let nodes: Vec<_> = (0..5).map(|_| NodeId::generate()).collect();
    
    // Initialize different states
    for &node in &nodes {
        manager.add_node_state(node, GossipState {
            position: Position::new(1, -1, 0),
            neighbors: vec![nodes[0]], // Different neighbor lists
            last_update: SystemTime::now(),
            ..Default::default()
        }).await?;
    }
    
    // Let states converge through gossip
    manager.run_gossip_rounds(5).await?;
    
    // Verify state convergence
    let states: Vec<_> = nodes.iter()
        .map(|&id| manager.get_node_state(id))
        .collect::<Result<Vec<_>>>()?;
    
    for window in states.windows(2) {
        assert_eq!(window[0].neighbors, window[1].neighbors,
            "Node states should converge");
    }
}

#[test]
fn gsp_3201_conflict_resolution() {
    let manager = StateManager::new(NodeId::generate());
    let node = NodeId::generate();
    
    // Create conflicting updates with different timestamps
    let update1 = StateUpdate {
        node,
        position: Position::new(1, -1, 0),
        timestamp: SystemTime::now(),
    };
    
    let update2 = StateUpdate {
        node,
        position: Position::new(-1, 1, 0),
        timestamp: SystemTime::now() + Duration::from_secs(1),
    };
    
    // Apply updates out of order
    manager.apply_update(update2.clone()).await?;
    manager.apply_update(update1).await?;
    
    let final_state = manager.get_node_state(node)?;
    assert_eq!(final_state.position, update2.position,
        "Should keep newer state update");
}

#[test]
fn gsp_3202_snapshot_consistency() {
    let manager = StateManager::new(NodeId::generate());
    let snapshot_monitor = manager.snapshot_monitor();
    
    // Create initial state
    let nodes: Vec<_> = (0..10).map(|_| NodeId::generate()).collect();
    for &node in &nodes {
        manager.add_node_state(node, GossipState::default()).await?;
    }
    
    // Take snapshot during updates
    let snapshot_future = manager.take_snapshot();
    
    // Apply concurrent updates
    for &node in &nodes {
        manager.apply_update(StateUpdate {
            node,
            position: Position::new(1, -1, 0),
            timestamp: SystemTime::now(),
        }).await?;
    }
    
    let snapshot = snapshot_future.await?;
    
    // Verify snapshot consistency
    assert!(snapshot_monitor.is_consistent(&snapshot),
        "Snapshot should be internally consistent");
    
    // Verify snapshot isolation
    let current_state = manager.get_current_state()?;
    assert_ne!(snapshot, current_state,
        "Snapshot should be isolated from concurrent updates");
}

#[test]
fn gsp_3203_state_pruning() {
    let manager = StateManager::new(NodeId::generate());
    
    // Add states with old timestamps
    let old_nodes: Vec<_> = (0..5).map(|_| NodeId::generate()).collect();
    for &node in &old_nodes {
        manager.add_node_state(node, GossipState {
            last_update: SystemTime::now() - Duration::from_secs(3600),
            ..Default::default()
        }).await?;
    }
    
    // Add current states
    let current_nodes: Vec<_> = (0..5).map(|_| NodeId::generate()).collect();
    for &node in &current_nodes {
        manager.add_node_state(node, GossipState {
            last_update: SystemTime::now(),
            ..Default::default()
        }).await?;
    }
    
    // Run pruning
    let pruned = manager.prune_old_states().await?;
    
    assert_eq!(pruned.len(), old_nodes.len(),
        "Should prune exactly the old states");
    
    for node in current_nodes {
        assert!(manager.has_node_state(node),
            "Current states should be preserved");
    }
}

#[test]
fn gsp_3204_state_verification() {
    let manager = StateManager::new(NodeId::generate());
    let verifier = manager.state_verifier();
    
    // Create valid and invalid states
    let valid_state = GossipState {
        position: Position::new(1, -1, 0),
        neighbors: vec![NodeId::generate()],
        last_update: SystemTime::now(),
        ..Default::default()
    };
    
    let invalid_state = GossipState {
        position: Position::new(1, 1, 1), // Invalid position
        neighbors: vec![],
        last_update: SystemTime::now(),
        ..Default::default()
    };
    
    assert!(verifier.verify_state(&valid_state).is_ok(),
        "Should accept valid state");
    assert!(verifier.verify_state(&invalid_state).is_err(),
        "Should reject invalid state");
    
    // Verify state transitions
    let transition = verifier.verify_transition(&valid_state, &valid_state);
    assert!(transition.is_ok(),
        "Should allow valid state transitions");
} 