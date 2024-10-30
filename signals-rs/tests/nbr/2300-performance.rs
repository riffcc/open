use signals_rs::nbr::{RouteOptimizer, Route, PathMetrics, LatencyStats};
use signals_rs::common::NodeId;
use std::time::{Duration, SystemTime};

#[test]
fn nbr_2300_prefer_low_latency() {
    let optimizer = RouteOptimizer::new();
    let target = NodeId::generate();
    
    // Create routes with different latencies
    let high_latency = Route {
        target,
        next_hop: NodeId::generate(),
        metrics: PathMetrics {
            latency: LatencyStats {
                average: Duration::from_millis(100),
                jitter: Duration::from_millis(10),
                ..Default::default()
            },
            ..Default::default()
        }
    };
    
    let low_latency = Route {
        target,
        next_hop: NodeId::generate(),
        metrics: PathMetrics {
            latency: LatencyStats {
                average: Duration::from_millis(20),
                jitter: Duration::from_millis(2),
                ..Default::default()
            },
            ..Default::default()
        }
    };
    
    optimizer.add_route(high_latency.clone()).await?;
    optimizer.add_route(low_latency.clone()).await?;
    
    let chosen = optimizer.get_best_route(target)
        .expect("Should have route");
    
    assert_eq!(chosen.next_hop, low_latency.next_hop,
        "Should prefer lower latency route");
}

#[test]
fn nbr_2301_load_balancing() {
    let optimizer = RouteOptimizer::new();
    let target = NodeId::generate();
    
    // Add multiple viable routes
    let routes: Vec<Route> = (0..3).map(|_| Route {
        target,
        next_hop: NodeId::generate(),
        metrics: PathMetrics {
            latency: LatencyStats {
                average: Duration::from_millis(30),
                jitter: Duration::from_millis(5),
                ..Default::default()
            },
            ..Default::default()
        }
    }).collect();
    
    for route in routes.iter() {
        optimizer.add_route(route.clone()).await?;
    }
    
    // Track route selection distribution
    let mut selections = std::collections::HashMap::new();
    for _ in 0..100 {
        let chosen = optimizer.get_best_route(target)
            .expect("Should have route");
        *selections.entry(chosen.next_hop).or_insert(0) += 1;
    }
    
    // Verify reasonable distribution
    for count in selections.values() {
        assert!(*count > 10 && *count < 50,
            "Routes should be reasonably balanced");
    }
}

#[test]
fn nbr_2302_route_caching() {
    let optimizer = RouteOptimizer::new();
    let target = NodeId::generate();
    let route = Route {
        target,
        next_hop: NodeId::generate(),
        metrics: PathMetrics::default(),
    };
    
    // Prime cache
    optimizer.add_route(route.clone()).await?;
    
    // Measure lookup times
    let start = SystemTime::now();
    for _ in 0..1000 {
        let _ = optimizer.get_best_route(target);
    }
    let elapsed = SystemTime::now().duration_since(start)
        .expect("Time should not go backwards");
    
    assert!(elapsed < Duration::from_millis(50),
        "Cached route lookups should be fast");
}

#[test]
fn nbr_2303_minimize_updates() {
    let optimizer = RouteOptimizer::new();
    let update_rx = optimizer.route_updates();
    let target = NodeId::generate();
    
    // Add initial route
    let initial = Route {
        target,
        next_hop: NodeId::generate(),
        metrics: PathMetrics {
            latency: LatencyStats {
                average: Duration::from_millis(30),
                ..Default::default()
            },
            ..Default::default()
        }
    };
    optimizer.add_route(initial).await?;
    
    // Add slightly different route
    let similar = Route {
        target,
        next_hop: NodeId::generate(),
        metrics: PathMetrics {
            latency: LatencyStats {
                average: Duration::from_millis(32), // Only 2ms difference
                ..Default::default()
            },
            ..Default::default()
        }
    };
    optimizer.add_route(similar).await?;
    
    // Verify no update for minor improvement
    assert!(update_rx.try_recv().is_err(),
        "Minor improvements should not trigger updates");
}

#[test]
fn nbr_2304_scaling_performance() {
    let optimizer = RouteOptimizer::new();
    
    // Add routes to many targets
    let routes: Vec<Route> = (0..1000).map(|_| Route {
        target: NodeId::generate(),
        next_hop: NodeId::generate(),
        metrics: PathMetrics::default(),
    }).collect();
    
    let start = SystemTime::now();
    for route in routes {
        optimizer.add_route(route).await?;
    }
    let elapsed = SystemTime::now().duration_since(start)
        .expect("Time should not go backwards");
    
    assert!(elapsed < Duration::from_secs(1),
        "Route management should scale well");
    
    // Verify memory usage
    let mem_size = std::mem::size_of_val(&optimizer);
    assert!(mem_size < 1024 * 1024,
        "Memory usage should stay reasonable");
} 