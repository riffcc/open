use signals_rs::nbr::{RouteTable, Route, RouteUpdate, PathMetrics};
use signals_rs::common::NodeId;
use std::time::{Duration, SystemTime};

#[test]
fn nbr_2200_maintain_consistent_state() {
    let table = RouteTable::new(NodeId::generate());
    let target = NodeId::generate();
    let next_hop = NodeId::generate();
    
    // Add initial route
    let route = Route {
        target,
        next_hop,
        metrics: PathMetrics {
            distance: 2,
            coherence: 0.8,
            timestamp: SystemTime::now(),
        }
    };
    
    table.add_route(route.clone()).await?;
    
    // Verify route state
    let stored = table.get_route(target)
        .expect("Route should exist");
    
    assert_eq!(stored.next_hop, next_hop);
    assert_eq!(stored.metrics.distance, 2);
    assert!(table.has_route_to(target));
}

#[test]
fn nbr_2201_propagate_route_updates() {
    let table = RouteTable::new(NodeId::generate());
    let update_rx = table.route_updates();
    
    let target = NodeId::generate();
    let next_hop = NodeId::generate();
    
    // Add route and verify update propagation
    table.add_route(Route {
        target,
        next_hop,
        metrics: PathMetrics::default(),
    }).await?;
    
    let update = update_rx.recv_timeout(Duration::from_secs(1))
        .expect("Should receive route update");
    
    assert_eq!(update.target, target);
    assert_eq!(update.next_hop, next_hop);
    assert_eq!(update.action, RouteUpdate::Add);
}

#[test]
fn nbr_2202_detect_prevent_cycles() {
    let table = RouteTable::new(NodeId::generate());
    let node_a = NodeId::generate();
    let node_b = NodeId::generate();
    let target = NodeId::generate();
    
    // Try to create a routing cycle
    table.add_route(Route {
        target: node_b,
        next_hop: node_a,
        metrics: PathMetrics::default(),
    }).await?;
    
    table.add_route(Route {
        target,
        next_hop: node_b,
        metrics: PathMetrics::default(),
    }).await?;
    
    // Attempt to create cycle
    let result = table.add_route(Route {
        target: node_a,
        next_hop: target,
        metrics: PathMetrics::default(),
    }).await;
    
    assert!(result.is_err(), "Should reject routes that create cycles");
}

#[test]
fn nbr_2203_handle_concurrent_updates() {
    let table = RouteTable::new(NodeId::generate());
    let target = NodeId::generate();
    
    // Attempt concurrent route updates
    let update1 = table.add_route(Route {
        target,
        next_hop: NodeId::generate(),
        metrics: PathMetrics {
            distance: 2,
            coherence: 0.8,
            timestamp: SystemTime::now(),
        }
    });
    
    let update2 = table.add_route(Route {
        target,
        next_hop: NodeId::generate(),
        metrics: PathMetrics {
            distance: 3,
            coherence: 0.9,
            timestamp: SystemTime::now(),
        }
    });
    
    let (result1, result2) = futures::join!(update1, update2);
    assert!(result1.is_ok() || result2.is_ok(),
        "At least one update should succeed");
    
    // Verify table remains consistent
    let final_route = table.get_route(target)
        .expect("Should have final route");
    assert!(final_route.metrics.is_valid());
}

#[test]
fn nbr_2204_cleanup_obsolete_routes() {
    let table = RouteTable::new(NodeId::generate());
    let target = NodeId::generate();
    
    // Add route with old timestamp
    table.add_route(Route {
        target,
        next_hop: NodeId::generate(),
        metrics: PathMetrics {
            distance: 2,
            coherence: 0.8,
            timestamp: SystemTime::now() - Duration::from_secs(3600),
        }
    }).await?;
    
    // Run cleanup
    table.cleanup_obsolete_routes().await?;
    
    assert!(!table.has_route_to(target),
        "Obsolete route should be removed");
    
    // Verify new routes can still be added
    let new_route = Route {
        target,
        next_hop: NodeId::generate(),
        metrics: PathMetrics::default(),
    };
    
    assert!(table.add_route(new_route).await.is_ok(),
        "Should accept new routes after cleanup");
} 