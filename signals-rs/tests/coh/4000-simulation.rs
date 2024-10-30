use signals_rs::gsp::{NetworkSimulator, SimConfig, SimMetrics, HexGrid};
use signals_rs::hex::Position;
use signals_rs::common::NodeId;
use std::time::{Duration, SystemTime};

#[test]
fn gsp_4000_hex_network_growth() {
    let mut sim = NetworkSimulator::new(SimConfig {
        initial_nodes: 7,  // Center + 6 neighbors
        growth_rate: 0.1,  // 10% growth per tick
        max_nodes: 100,
        tick_duration: Duration::from_secs(1),
        ..Default::default()
    });
    
    let metrics = SimMetrics::new();
    
    // Run growth simulation
    for _ in 0..50 {
        sim.tick().await?;
        
        // Record metrics
        let grid = sim.current_grid();
        metrics.record_grid_state(&grid);
        
        // Verify hex properties are maintained
        for node in grid.nodes() {
            let neighbors = grid.hex_neighbors(node.position());
            assert!(neighbors.len() <= 6,
                "Nodes should maintain hex grid constraints");
            
            for n in neighbors {
                assert_eq!(node.position().hex_distance(&n), 1,
                    "Neighbors should be exactly one hex step away");
            }
        }
    }
    
    // Verify growth patterns
    let growth_stats = metrics.analyze_growth();
    assert!(growth_stats.maintains_hex_structure(),
        "Network should maintain hex structure while growing");
    assert!(growth_stats.is_balanced(),
        "Growth should be relatively balanced across regions");
}

#[test]
fn gsp_4001_network_stress_test() {
    let mut sim = NetworkSimulator::new(SimConfig {
        initial_nodes: 19,  // Two rings of hex cells
        message_rate: 100.0,  // Messages per second
        failure_rate: 0.01,   // 1% chance of node failure per tick
        tick_duration: Duration::from_millis(100),
        ..Default::default()
    });
    
    let metrics = SimMetrics::new();
    
    // Run stress test
    for _ in 0..100 {
        sim.tick().await?;
        
        // Inject random failures
        if fastrand::f64() < 0.01 {
            sim.inject_node_failure(NodeId::generate());
        }
        
        // Record metrics
        metrics.record_network_state(sim.current_state());
    }
    
    // Analyze network resilience
    let resilience = metrics.analyze_resilience();
    assert!(resilience.message_delivery_rate() > 0.95,
        "Network should maintain high message delivery under stress");
    assert!(resilience.recovery_time() < Duration::from_secs(5),
        "Network should recover quickly from failures");
}

#[test]
fn gsp_4002_partition_recovery_simulation() {
    let mut sim = NetworkSimulator::new(SimConfig {
        initial_nodes: 37,  // Three rings of hex cells
        partition_probability: 0.1,
        healing_rate: 0.2,
        ..Default::default()
    });
    
    let metrics = SimMetrics::new();
    
    // Create initial partition
    let center = Position::new(0, 0, 0);
    sim.create_partition(
        |pos| pos.x > 0,  // Split east/west
        Duration::from_secs(5)
    );
    
    // Run recovery simulation
    let mut partition_healed = false;
    for tick in 0..100 {
        sim.tick().await?;
        metrics.record_tick_state(&sim);
        
        if !partition_healed && metrics.is_network_unified() {
            partition_healed = true;
            metrics.record_healing_time(tick);
        }
    }
    
    // Verify recovery properties
    let recovery = metrics.analyze_recovery();
    assert!(partition_healed, "Network should eventually heal");
    assert!(recovery.maintains_hex_structure(),
        "Recovery should maintain hex grid properties");
    assert!(recovery.is_efficiently_healed(),
        "Healing should be reasonably efficient");
}

#[test]
fn gsp_4003_load_balancing_simulation() {
    let mut sim = NetworkSimulator::new(SimConfig {
        initial_nodes: 19,
        load_variance: 0.3,  // 30% random load variance
        balancing_threshold: 0.2,  // Balance when >20% difference
        ..Default::default()
    });
    
    let metrics = SimMetrics::new();
    
    // Create initial uneven load
    let center_region = HexGrid::region_from_center(
        Position::new(0, 0, 0),
        1  // One ring
    );
    sim.apply_load_multiplier(&center_region, 2.0);  // Double load in center
    
    // Run balancing simulation
    for _ in 0..50 {
        sim.tick().await?;
        metrics.record_load_state(&sim);
        
        // Verify hex structure maintained
        let grid = sim.current_grid();
        for node in grid.nodes() {
            assert!(grid.hex_neighbors(node.position()).len() <= 6,
                "Load balancing should maintain hex structure");
        }
    }
    
    // Verify load distribution
    let distribution = metrics.analyze_load_distribution();
    assert!(distribution.variance() < 0.3,
        "Load should be reasonably balanced");
    assert!(distribution.maintains_locality(),
        "Balancing should respect hex grid locality");
}

#[test]
fn gsp_4004_convergence_simulation() {
    let mut sim = NetworkSimulator::new(SimConfig {
        initial_nodes: 37,
        convergence_threshold: 0.95,
        ..Default::default()
    });
    
    let metrics = SimMetrics::new();
    
    // Introduce conflicting states
    let states = vec![
        (Position::new(0, 0, 0), "state_a"),
        (Position::new(2, -1, -1), "state_b"),
        (Position::new(-2, 1, 1), "state_c"),
    ];
    
    for (pos, state) in states {
        sim.inject_state(pos, state);
    }
    
    // Run convergence simulation
    let mut converged = false;
    for tick in 0..200 {
        sim.tick().await?;
        metrics.record_convergence_state(&sim);
        
        if !converged && metrics.is_converged() {
            converged = true;
            metrics.record_convergence_time(tick);
        }
    }
    
    // Verify convergence properties
    let convergence = metrics.analyze_convergence();
    assert!(converged, "Network should eventually converge");
    assert!(convergence.is_consistent(),
        "Convergence should reach consistent state");
    assert!(convergence.follows_hex_pattern(),
        "Convergence should follow hex grid pattern");
} 