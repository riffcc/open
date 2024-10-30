use signals_rs::gsp::{NetworkOptimizer, OptimizationMetrics, HexTopology};
use signals_rs::hex::Position;
use signals_rs::common::NodeId;
use std::time::{Duration, SystemTime};

#[test]
fn gsp_3700_hex_density_optimization() {
    let optimizer = NetworkOptimizer::new();
    let topology = HexTopology::new();
    
    // Create an uneven hex grid distribution
    let nodes = vec![
        // Dense cluster
        (NodeId::generate(), Position::new(0, 0, 0)),
        (NodeId::generate(), Position::new(1, -1, 0)),
        (NodeId::generate(), Position::new(1, 0, -1)),
        (NodeId::generate(), Position::new(0, 1, -1)),
        (NodeId::generate(), Position::new(0, -1, 1)),
        
        // Sparse region
        (NodeId::generate(), Position::new(3, -2, -1)),
        (NodeId::generate(), Position::new(4, -2, -2)),
    ];
    
    for (id, pos) in &nodes {
        topology.add_node(*id, *pos);
    }
    
    // Calculate density optimization
    let optimizations = optimizer.optimize_density(&topology);
    
    // Verify suggested movements balance the grid
    for (id, new_pos) in optimizations {
        let old_pos = nodes.iter()
            .find(|(nid, _)| nid == &id)
            .unwrap().1;
            
        let old_density = topology.calculate_density(&old_pos);
        let new_density = topology.calculate_density(&new_pos);
        
        assert!(new_density < old_density,
            "Optimization should reduce high density regions");
        assert!(new_pos.is_valid_hex_position(),
            "Optimized positions should be valid hex coordinates");
    }
}

#[test]
fn gsp_3701_coherence_based_positioning() {
    let optimizer = NetworkOptimizer::new();
    let metrics = OptimizationMetrics::new();
    
    // Create nodes with varying coherence scores
    let nodes = vec![
        (NodeId::generate(), Position::new(1, -1, 0), 0.9),  // High coherence
        (NodeId::generate(), Position::new(1, 0, -1), 0.8),
        (NodeId::generate(), Position::new(0, 1, -1), 0.7),
        (NodeId::generate(), Position::new(-1, 1, 0), 0.4),  // Low coherence
    ];
    
    for (id, pos, coherence) in &nodes {
        metrics.record_node_coherence(*id, *coherence);
        metrics.record_node_position(*id, *pos);
    }
    
    // Optimize positions based on coherence
    let optimized = optimizer.optimize_for_coherence(&metrics);
    
    // Verify high coherence nodes maintain more stable positions
    for (id, new_pos) in &optimized {
        let (_, old_pos, coherence) = nodes.iter()
            .find(|(nid, _, _)| nid == id)
            .unwrap();
            
        let movement = old_pos.hex_distance(&new_pos);
        
        if *coherence > 0.7 {
            assert!(movement <= 1,
                "High coherence nodes should move minimally");
        }
    }
}

#[test]
fn gsp_3702_path_optimization() {
    let optimizer = NetworkOptimizer::new();
    let topology = HexTopology::new();
    
    // Create a suboptimal path configuration
    let path = vec![
        Position::new(0, 0, 0),
        Position::new(1, -1, 0),
        Position::new(2, -1, -1),
        Position::new(3, -1, -2),  // Suboptimal bend
        Position::new(4, -2, -2),
    ];
    
    for (i, pos) in path.iter().enumerate() {
        topology.add_node(NodeId::generate(), *pos);
        if i > 0 {
            topology.add_path_segment(&path[i-1], pos);
        }
    }
    
    // Optimize path
    let optimized = optimizer.optimize_path(&topology, &path[0], &path[4]);
    
    // Verify optimized path properties
    assert!(optimized.len() <= path.len(),
        "Optimized path should not be longer");
        
    for window in optimized.windows(2) {
        assert_eq!(window[0].hex_distance(&window[1]), 1,
            "Optimized path should maintain hex grid connectivity");
    }
    
    // Calculate path metrics
    let original_length: u64 = path.windows(2)
        .map(|w| w[0].hex_distance(&w[1]) as u64)
        .sum();
    let optimized_length: u64 = optimized.windows(2)
        .map(|w| w[0].hex_distance(&w[1]) as u64)
        .sum();
        
    assert!(optimized_length <= original_length,
        "Optimized path should be more efficient");
}

#[test]
fn gsp_3703_load_balancing() {
    let optimizer = NetworkOptimizer::new();
    let metrics = OptimizationMetrics::new();
    
    // Create hex grid with uneven load
    let nodes = vec![
        // Overloaded nodes
        (Position::new(0, 0, 0), 0.9),
        (Position::new(1, -1, 0), 0.85),
        // Normal load
        (Position::new(1, 0, -1), 0.5),
        (Position::new(0, 1, -1), 0.45),
        // Underutilized
        (Position::new(-1, 1, 0), 0.2),
        (Position::new(-1, 0, 1), 0.15),
    ];
    
    for (pos, load) in &nodes {
        metrics.record_position_load(*pos, *load);
    }
    
    // Calculate load balancing adjustments
    let adjustments = optimizer.balance_load(&metrics);
    
    // Verify load distribution
    let mut new_loads = metrics.clone();
    for (from, to) in &adjustments {
        new_loads.transfer_load(from, to, 0.2);
    }
    
    let final_loads: Vec<f64> = nodes.iter()
        .map(|(pos, _)| new_loads.get_position_load(pos))
        .collect();
        
    let load_variance = calculate_variance(&final_loads);
    assert!(load_variance < 0.1,
        "Load should be more evenly distributed");
}

#[test]
fn gsp_3704_stability_preservation() {
    let optimizer = NetworkOptimizer::new();
    let metrics = OptimizationMetrics::new();
    
    // Create stable hex formation
    let stable_hex = vec![
        Position::new(0, 0, 0),
        Position::new(1, -1, 0),
        Position::new(1, 0, -1),
        Position::new(0, 1, -1),
        Position::new(-1, 1, 0),
        Position::new(-1, 0, 1),
        Position::new(0, -1, 1),
    ];
    
    // Record stability metrics
    for pos in &stable_hex {
        metrics.record_position_stability(*pos, 0.95);
    }
    
    // Add some unstable outliers
    let unstable = vec![
        Position::new(2, -2, 0),
        Position::new(2, -1, -1),
    ];
    
    for pos in &unstable {
        metrics.record_position_stability(*pos, 0.3);
    }
    
    // Optimize positions
    let adjustments = optimizer.optimize_positions(&metrics);
    
    // Verify stable formation is preserved
    for pos in &stable_hex {
        assert!(!adjustments.iter().any(|(from, _)| from == pos),
            "Stable positions should not be disrupted");
    }
}

// Helper function to calculate variance
fn calculate_variance(values: &[f64]) -> f64 {
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    values.iter()
        .map(|x| (x - mean).powi(2))
        .sum::<f64>() / values.len() as f64
} 