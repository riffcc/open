use signals_rs::gsp::{GossipProtocol, FailureDetector, NodeStatus};
use signals_rs::common::NodeId;
use std::time::{Duration, SystemTime};

#[test]
fn gsp_3100_rapid_failure_detection() {
    let detector = FailureDetector::new(Duration::from_secs(5)); // 5s timeout
    let target = NodeId::generate();
    
    // Record initial heartbeat
    detector.record_heartbeat(target).await?;
    assert_eq!(detector.node_status(target), NodeStatus::Alive);
    
    // Fast forward time
    detector.advance_time(Duration::from_secs(6)).await?;
    
    // Should detect failure quickly
    let detection_time = SystemTime::now();
    assert_eq!(detector.node_status(target), NodeStatus::Failed);
    
    let elapsed = SystemTime::now().duration_since(detection_time)
        .expect("Time should not go backwards");
    assert!(elapsed < Duration::from_millis(100),
        "Failure detection should be near-instant");
}

#[test]
fn gsp_3101_partition_detection() {
    let node = GossipProtocol::new(NodeId::generate());
    let partition_monitor = node.partition_monitor();
    
    // Set up neighbor groups
    let group_a: Vec<_> = (0..5).map(|_| NodeId::generate()).collect();
    let group_b: Vec<_> = (0..5).map(|_| NodeId::generate()).collect();
    
    // Simulate partition: group A can reach each other but not group B
    for &id in &group_a {
        node.record_reachable(id).await?;
    }
    for &id in &group_b {
        node.record_unreachable(id).await?;
    }
    
    let partitions = partition_monitor.detect_partitions().await?;
    assert_eq!(partitions.len(), 2, "Should detect network partition");
    
    // Verify partition healing detection
    for &id in &group_b {
        node.record_reachable(id).await?;
    }
    
    assert!(partition_monitor.is_healed().await?,
        "Should detect partition healing");
}

#[test]
fn gsp_3102_failure_propagation() {
    let node = GossipProtocol::new(NodeId::generate());
    let failed_node = NodeId::generate();
    let update_rx = node.status_updates();
    
    // Add some neighbors
    let neighbors: Vec<_> = (0..3).map(|_| NodeId::generate()).collect();
    for &id in &neighbors {
        node.add_neighbor(id, 0.9).await?;
    }
    
    // Detect and propagate failure
    node.detect_failure(failed_node).await?;
    
    // Verify all neighbors receive failure notification
    let mut notified = vec![];
    for _ in 0..neighbors.len() {
        if let Ok(update) = update_rx.recv_timeout(Duration::from_secs(1)) {
            notified.push(update.recipient);
        }
    }
    
    assert_eq!(notified.len(), neighbors.len(),
        "All neighbors should be notified of failure");
}

#[test]
fn gsp_3103_flap_detection() {
    let detector = FailureDetector::new(Duration::from_secs(5));
    let flappy_node = NodeId::generate();
    
    // Simulate rapid status changes
    for _ in 0..10 {
        detector.record_heartbeat(flappy_node).await?;
        detector.advance_time(Duration::from_secs(6)).await?;
        detector.record_heartbeat(flappy_node).await?;
        detector.advance_time(Duration::from_secs(1)).await?;
    }
    
    assert!(detector.is_flapping(flappy_node),
        "Should detect flapping node");
    
    // Verify increased failure threshold for flappy nodes
    detector.record_heartbeat(flappy_node).await?;
    detector.advance_time(Duration::from_secs(6)).await?;
    
    assert_eq!(detector.node_status(flappy_node), NodeStatus::Alive,
        "Should be more tolerant of flappy node timeouts");
}

#[test]
fn gsp_3104_coordinated_recovery() {
    let node = GossipProtocol::new(NodeId::generate());
    let failed_node = NodeId::generate();
    
    // Set up recovery monitoring
    let recovery = node.coordinate_recovery(failed_node);
    let status_rx = node.status_updates();
    
    // Simulate neighbors reporting recovery
    let neighbors: Vec<_> = (0..5).map(|_| NodeId::generate()).collect();
    for &id in &neighbors {
        node.report_node_recovered(failed_node, id).await?;
    }
    
    // Wait for recovery consensus
    let recovered = recovery.await?;
    assert!(recovered, "Should achieve recovery consensus");
    
    // Verify status update
    let update = status_rx.recv_timeout(Duration::from_secs(1))
        .expect("Should receive status update");
    assert_eq!(update.node, failed_node);
    assert_eq!(update.status, NodeStatus::Recovered);
} 