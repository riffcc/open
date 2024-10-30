use signals_rs::gsp::{MetricsCollector, NetworkMetrics, HexMetrics};
use signals_rs::hex::Position;
use signals_rs::common::NodeId;
use std::time::{Duration, SystemTime};

#[test]
fn gsp_3900_hex_coverage_metrics() {
    let collector = MetricsCollector::new();
    
    // Create a hex grid with some gaps
    let nodes = vec![
        // Complete hex cell
        Position::new(0, 0, 0),    // Center
        Position::new(1, -1, 0),   // East
        Position::new(1, 0, -1),   // Southeast
        Position::new(0, 1, -1),   // Southwest
        Position::new(-1, 1, 0),   // West
        Position::new(-1, 0, 1),   // Northwest
        Position::new(0, -1, 1),   // Northeast
        
        // Partial hex cell
        Position::new(2, -1, -1),  // Center
        Position::new(3, -2, -1),  // East
        Position::new(2, 0, -2),   // Southeast
    ];
    
    for pos in &nodes {
        collector.record_node_position(NodeId::generate(), *pos);
    }
    
    let coverage = collector.calculate_hex_coverage();
    
    // Verify coverage metrics
    assert_eq!(coverage.complete_cells, 1,
        "Should detect one complete hex cell");
    assert_eq!(coverage.partial_cells, 1,
        "Should detect one partial hex cell");
    assert!(coverage.coverage_ratio() < 1.0,
        "Coverage ratio should reflect gaps");
        
    // Verify neighbor counts
    for pos in &nodes {
        let neighbors = coverage.neighbor_count(pos);
        if pos.hex_distance(&Position::new(0, 0, 0)) <= 1 {
            assert_eq!(neighbors, 6, "Complete hex should have 6 neighbors");
        } else {
            assert!(neighbors < 6, "Partial hex should have fewer neighbors");
        }
    }
}

#[test]
fn gsp_3901_hex_coherence_metrics() {
    let collector = MetricsCollector::new();
    let metrics = NetworkMetrics::new();
    
    // Record node movements in hex grid
    let node_id = NodeId::generate();
    let movements = vec![
        // Stable period
        (Position::new(0, 0, 0), Duration::from_secs(0)),
        (Position::new(0, 0, 0), Duration::from_secs(10)),
        (Position::new(0, 0, 0), Duration::from_secs(20)),
        
        // Movement period
        (Position::new(1, -1, 0), Duration::from_secs(30)),
        (Position::new(1, 0, -1), Duration::from_secs(40)),
        (Position::new(2, -1, -1), Duration::from_secs(50)),
    ];
    
    for (pos, time) in movements {
        collector.record_node_movement(node_id, pos, SystemTime::now() + time);
    }
    
    let coherence = collector.calculate_movement_coherence();
    
    // Verify coherence metrics
    assert!(coherence.stability_periods.len() >= 1,
        "Should detect stable period");
    assert!(coherence.movement_periods.len() >= 1,
        "Should detect movement period");
        
    // Verify hex grid properties of movements
    for window in movements.windows(2) {
        let distance = window[0].0.hex_distance(&window[1].0);
        metrics.record_movement_distance(distance);
    }
    
    assert!(metrics.average_movement_distance() <= 2.0,
        "Movements should follow hex grid constraints");
}

#[test]
fn gsp_3902_network_flow_metrics() {
    let collector = MetricsCollector::new();
    let hex_metrics = HexMetrics::new();
    
    // Record message flows between hex positions
    let flows = vec![
        // Direct neighbors
        (Position::new(0, 0, 0), Position::new(1, -1, 0), 100),
        (Position::new(0, 0, 0), Position::new(0, 1, -1), 80),
        
        // Two steps away
        (Position::new(0, 0, 0), Position::new(2, -1, -1), 50),
        
        // Three steps away
        (Position::new(0, 0, 0), Position::new(3, -2, -1), 20),
    ];
    
    for (from, to, count) in flows {
        for _ in 0..count {
            collector.record_message_flow(from, to);
        }
    }
    
    let flow_metrics = collector.calculate_flow_metrics();
    
    // Verify flow patterns
    for distance in 1..=3 {
        let flows_at_distance: Vec<_> = flow_metrics.flows.iter()
            .filter(|((from, to), _)| from.hex_distance(to) == distance)
            .collect();
            
        let avg_flow = flows_at_distance.iter()
            .map(|(_, count)| count)
            .sum::<u64>() as f64 / flows_at_distance.len() as f64;
            
        hex_metrics.record_flow_at_distance(distance, avg_flow);
    }
    
    // Verify flow decreases with hex distance
    let flows_by_distance = hex_metrics.flows_by_distance();
    for window in flows_by_distance.windows(2) {
        assert!(window[0] >= window[1],
            "Flow should decrease with hex distance");
    }
}

#[test]
fn gsp_3903_load_distribution_metrics() {
    let collector = MetricsCollector::new();
    
    // Create hex grid with varying loads
    let loads = vec![
        // Center hex
        (Position::new(0, 0, 0), 100),    // High load
        (Position::new(1, -1, 0), 90),
        (Position::new(1, 0, -1), 85),
        
        // Outer hex
        (Position::new(2, -1, -1), 60),
        (Position::new(2, -2, 0), 55),
        (Position::new(1, -2, 1), 50),    // Lower load
    ];
    
    for (pos, load) in &loads {
        collector.record_position_load(*pos, *load as f64);
    }
    
    let distribution = collector.calculate_load_distribution();
    
    // Verify load distribution properties
    assert!(distribution.load_gradient() > 0.0,
        "Should detect load gradient from center");
        
    // Verify hex distance correlation
    let center = Position::new(0, 0, 0);
    for (pos, load) in &loads {
        let distance = pos.hex_distance(&center);
        let expected_load = 100.0 - (distance as f64 * 20.0);
        assert!((load as f64 - expected_load).abs() < 30.0,
            "Load should roughly correlate with hex distance");
    }
}

#[test]
fn gsp_3904_temporal_stability_metrics() {
    let collector = MetricsCollector::new();
    
    // Record stability metrics over time for hex regions
    let regions = vec![
        // Stable hex region
        (HexRegion {
            center: Position::new(0, 0, 0),
            radius: 1
        }, 0.9),
        
        // Less stable region
        (HexRegion {
            center: Position::new(3, -2, -1),
            radius: 1
        }, 0.6),
    ];
    
    // Record metrics over time
    for hour in 0..24 {
        for (region, base_stability) in &regions {
            let stability = base_stability + (fastrand::f64() * 0.1 - 0.05);
            collector.record_region_stability(
                region.clone(),
                stability,
                SystemTime::now() + Duration::from_secs(hour * 3600)
            );
        }
    }
    
    let stability = collector.calculate_temporal_stability();
    
    // Verify stability metrics
    for (region, base_stability) in regions {
        let region_stability = stability.get_region_stability(&region);
        assert!((region_stability - base_stability).abs() < 0.15,
            "Temporal stability should be close to base stability");
            
        let variance = stability.get_region_variance(&region);
        assert!(variance < 0.1,
            "Stability variance should be small");
    }
} 