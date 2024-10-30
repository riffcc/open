use signals_rs::gsp::{GossipProtocol, GossipUpdate, UpdateContent};
use signals_rs::common::{NodeId, Timestamp};
use signals_rs::hex::Position;
use std::time::{Duration, SystemTime};

#[test]
fn gsp_3000_trusted_path_propagation() {
    let node = GossipProtocol::new(NodeId::generate());
    
    // Set up trusted and untrusted neighbors
    let trusted = NodeId::generate();
    let untrusted = NodeId::generate();
    
    node.add_neighbor(trusted, 0.8).await?; // High trust
    node.add_neighbor(untrusted, 0.2).await?; // Low trust
    
    // Create gossip update
    let update = GossipUpdate {
        source_id: NodeId::generate(),
        timestamp: SystemTime::now(),
        content: UpdateContent::Position(Position::new(1, -1, 0)),
    };
    
    // Track update propagation
    let propagation = node.track_update_propagation(update.clone());
    node.process_gossip(update).await?;
    
    let recipients = propagation.await?;
    
    assert!(recipients.contains(&trusted),
        "Update should propagate to trusted neighbor");
    assert!(!recipients.contains(&untrusted),
        "Update should not propagate to untrusted neighbor");
}

#[test]
fn gsp_3001_adaptive_frequency() {
    let node = GossipProtocol::new(NodeId::generate());
    let monitor = node.frequency_monitor();
    
    // Simulate stable network
    for _ in 0..10 {
        node.record_successful_gossip().await?;
    }
    
    let stable_frequency = monitor.current_frequency();
    
    // Simulate network instability
    for _ in 0..5 {
        node.record_failed_gossip().await?;
    }
    
    let unstable_frequency = monitor.current_frequency();
    assert!(unstable_frequency > stable_frequency,
        "Gossip frequency should increase during instability");
}

#[test]
fn gsp_3002_complete_state_updates() {
    let node = GossipProtocol::new(NodeId::generate());
    
    // Create comprehensive update
    let update = GossipUpdate {
        source_id: NodeId::generate(),
        timestamp: SystemTime::now(),
        content: UpdateContent::CompleteState {
            position: Position::new(1, -1, 0),
            neighbors: vec![NodeId::generate(), NodeId::generate()],
            routes: vec![(NodeId::generate(), NodeId::generate())],
        },
    };
    
    // Process update
    node.process_gossip(update.clone()).await?;
    
    // Verify all state components updated
    let state = node.current_state();
    assert_eq!(state.position, update.content.position().unwrap());
    assert_eq!(state.neighbors.len(), update.content.neighbors().unwrap().len());
    assert_eq!(state.routes.len(), update.content.routes().unwrap().len());
}

#[test]
fn gsp_3003_serialization_integrity() {
    let node = GossipProtocol::new(NodeId::generate());
    
    // Create complex update
    let original = GossipUpdate {
        source_id: NodeId::generate(),
        timestamp: SystemTime::now(),
        content: UpdateContent::MultiUpdate(vec![
            UpdateContent::Position(Position::new(1, -1, 0)),
            UpdateContent::NeighborList(vec![NodeId::generate()]),
            UpdateContent::RouteChange {
                target: NodeId::generate(),
                next_hop: Some(NodeId::generate()),
            },
        ]),
    };
    
    // Serialize and deserialize
    let serialized = bincode::serialize(&original)
        .expect("Serialization failed");
    let deserialized: GossipUpdate = bincode::deserialize(&serialized)
        .expect("Deserialization failed");
    
    assert_eq!(original, deserialized,
        "Update should maintain integrity through serialization");
}

#[test]
fn gsp_3004_large_update_chunking() {
    let node = GossipProtocol::new(NodeId::generate());
    
    // Create large update
    let large_update = GossipUpdate {
        source_id: NodeId::generate(),
        timestamp: SystemTime::now(),
        content: UpdateContent::NeighborList(
            (0..1000).map(|_| NodeId::generate()).collect()
        ),
    };
    
    // Track chunked transmission
    let chunks = node.prepare_transmission(large_update).await?;
    
    assert!(chunks.len() > 1, "Large update should be chunked");
    assert!(chunks.iter().all(|c| c.len() <= node.max_chunk_size()),
        "Chunks should respect size limit");
    
    // Verify reassembly
    let reassembled = node.reassemble_chunks(chunks).await?;
    assert_eq!(reassembled.content, large_update.content,
        "Update should reassemble correctly");
} 