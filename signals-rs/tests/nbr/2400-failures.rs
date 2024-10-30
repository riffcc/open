use signals_rs::nbr::{RouteTable, Route, PathMetrics, LatencyStats};
use signals_rs::common::{NodeId, NetworkError};
use std::time::{Duration, SystemTime};
use std::collections::HashSet;

#[test]
fn nbr_2400_handle_disconnections() {
    let table = RouteTable::new(NodeId::generate());
    let update_rx = table.route_updates();
    let disconnected = NodeId::generate();
    
    // Add routes through soon-to-disconnect node
    let routes = vec![
        Route {
            target: NodeId::generate(),
            next_hop: disconnected,
            metrics: PathMetrics::default(),
        },
        Route {
            target: NodeId::generate(),
            next_hop: disconnected,
            metrics: PathMetrics::default(),
        }
    ];
    
    for route in routes {
        table.add_route(route).await?;
    }
    
    // Simulate node disconnection
    table.handle_node_disconnect(disconnected).await?;
    
    // Verify all affected routes are updated
    let mut affected = HashSet::new();
    while let Ok(update) = update_rx.try_recv() {
        affected.insert(update.target);
    }
    
    assert!(!affected.is_empty(), "Routes should be updated on disconnection");
    assert!(!table.has_routes_through(disconnected), 
        "No routes should remain through disconnected node");
}

#[test]
fn nbr_2401_handle_partitions() {
    let table = RouteTable::new(NodeId::generate());
    let partition_nodes: Vec<NodeId> = (0..3).map(|_| NodeId::generate()).collect();
    
    // Add routes that will be partitioned
    for node in &partition_nodes {
        table.add_route(Route {
            target: NodeId::generate(),
            next_hop: *node,
            metrics: PathMetrics::default(),
        }).await?;
    }
    
    // Simulate network partition
    table.handle_partition_event(partition_nodes.clone()).await?;
    
    // Verify partition handling
    for node in partition_nodes {
        assert!(!table.has_routes_through(node),
            "Partitioned routes should be removed");
    }
    
    assert!(table.is_partition_detected(), 
        "Partition state should be tracked");
}

#[test]
fn nbr_2402_recover_from_corruption() {
    let table = RouteTable::new(NodeId::generate());
    let target = NodeId::generate();
    
    // Add some valid routes
    let valid_routes = vec![
        Route {
            target,
            next_hop: NodeId::generate(),
            metrics: PathMetrics::default(),
        },
        Route {
            target,
            next_hop: NodeId::generate(),
            metrics: PathMetrics::default(),
        }
    ];
    
    for route in valid_routes {
        table.add_route(route).await?;
    }
    
    // Simulate corruption by forcing invalid state
    table.simulate_corruption().await?;
    
    // Trigger recovery
    table.recover_from_corruption().await?;
    
    // Verify table recovered to valid state
    assert!(table.verify_integrity().await?.is_ok(),
        "Table should recover to valid state");
    assert!(table.has_routes_to(target),
        "Valid routes should be preserved");
}

#[test]
fn nbr_2403_reject_invalid_updates() {
    let table = RouteTable::new(NodeId::generate());
    let target = NodeId::generate();
    
    // Try various invalid updates
    let invalid_cases = vec![
        Route {
            target,
            next_hop: target, // Self-referential
            metrics: PathMetrics::default(),
        },
        Route {
            target: NodeId::generate(),
            next_hop: NodeId::generate(),
            metrics: PathMetrics { // Invalid metrics
                latency: LatencyStats {
                    average: Duration::from_secs(1000), // Unreasonable latency
                    ..Default::default()
                },
                ..Default::default()
            },
        }
    ];
    
    for invalid in invalid_cases {
        let result = table.add_route(invalid).await;
        assert!(result.is_err(), "Should reject invalid route updates");
        assert!(matches!(result, Err(NetworkError::InvalidRoute(_))),
            "Should return appropriate error type");
    }
}

#[test]
fn nbr_2404_maintain_consistency() {
    let table = RouteTable::new(NodeId::generate());
    let target = NodeId::generate();
    let update_rx = table.route_updates();
    
    // Set up initial routes
    let routes: Vec<Route> = (0..5).map(|_| Route {
        target,
        next_hop: NodeId::generate(),
        metrics: PathMetrics::default(),
    }).collect();
    
    // Simulate concurrent failures and updates
    let mut handles = vec![];
    
    // Add routes
    for route in routes {
        handles.push(tokio::spawn(table.add_route(route)));
    }
    
    // Simulate failures
    handles.push(tokio::spawn(table.handle_node_disconnect(NodeId::generate())));
    handles.push(tokio::spawn(table.handle_partition_event(vec![NodeId::generate()])));
    
    // Wait for all operations
    futures::future::join_all(handles).await;
    
    // Verify consistency
    assert!(table.verify_integrity().await?.is_ok(),
        "Table should maintain consistency during concurrent failures");
    
    // Check update notifications were properly sent
    let updates: Vec<_> = update_rx.try_iter().collect();
    assert!(!updates.is_empty(), "Updates should be propagated");
    assert!(updates.iter().all(|u| u.is_valid()),
        "All updates should be valid");
} 