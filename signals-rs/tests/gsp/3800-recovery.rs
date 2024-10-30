use signals_rs::gsp::{RecoveryManager, RecoveryMetrics, HexRegion};
use signals_rs::hex::Position;
use signals_rs::common::NodeId;
use std::time::{Duration, SystemTime};

#[test]
fn gsp_3800_hex_region_recovery() {
    let recovery = RecoveryManager::new();
    let metrics = RecoveryMetrics::new();
    
    // Define a hex region that's experiencing issues
    let problem_region = HexRegion {
        center: Position::new(0, 0, 0),
        radius: 2,  // Affects 2 rings of hex cells
    };
    
    // Record node failures in the region
    let failed_nodes = vec![
        (NodeId::generate(), Position::new(1, -1, 0)),
        (NodeId::generate(), Position::new(1, 0, -1)),
        (NodeId::generate(), Position::new(0, 1, -1)),
    ];
    
    for (id, pos) in &failed_nodes {
        metrics.record_node_failure(*id, *pos, SystemTime::now());
    }
    
    // Generate recovery plan
    let plan = recovery.generate_recovery_plan(&problem_region, &metrics);
    
    // Verify recovery properties
    assert!(plan.replacement_positions.len() >= failed_nodes.len(),
        "Should provide enough replacement positions");
        
    for pos in &plan.replacement_positions {
        // Verify positions are valid hex coordinates
        assert!(pos.is_valid_hex_position(),
            "Recovery positions must be valid hex coordinates");
            
        // Verify positions maintain hex grid spacing
        for other_pos in &plan.replacement_positions {
            if pos != other_pos {
                assert!(pos.hex_distance(other_pos) >= 1,
                    "Recovery positions must maintain minimum hex spacing");
            }
        }
    }
}

#[test]
fn gsp_3801_cascading_failure_prevention() {
    let recovery = RecoveryManager::new();
    let metrics = RecoveryMetrics::new();
    
    // Simulate cascading failures in hex pattern
    let failure_sequence = vec![
        (Position::new(0, 0, 0), SystemTime::now()),
        (Position::new(1, -1, 0), SystemTime::now() + Duration::from_secs(1)),
        (Position::new(1, 0, -1), SystemTime::now() + Duration::from_secs(2)),
    ];
    
    for (pos, time) in &failure_sequence {
        metrics.record_position_failure(*pos, *time);
    }
    
    // Check cascade detection
    let cascade = recovery.detect_cascade_pattern(&metrics);
    assert!(cascade.is_some(), "Should detect cascading failure pattern");
    
    // Generate preventive measures
    let prevention = recovery.generate_prevention_plan(&cascade.unwrap());
    
    // Verify prevention strategy
    assert!(!prevention.reinforcement_positions.is_empty(),
        "Should suggest reinforcement positions");
        
    // Verify reinforcements form protective hex pattern
    for pos in &prevention.reinforcement_positions {
        let at_risk = failure_sequence.iter()
            .filter(|(fail_pos, _)| fail_pos.hex_distance(pos) == 1)
            .count();
        assert!(at_risk > 0,
            "Reinforcements should be adjacent to at-risk positions");
    }
}

#[test]
fn gsp_3802_stability_restoration() {
    let recovery = RecoveryManager::new();
    let metrics = RecoveryMetrics::new();
    
    // Create unstable hex region
    let unstable_positions = vec![
        Position::new(0, 0, 0),
        Position::new(1, -1, 0),
        Position::new(1, 0, -1),
    ];
    
    for pos in &unstable_positions {
        metrics.record_position_instability(*pos, 0.7); // High instability
    }
    
    // Generate stability plan
    let plan = recovery.generate_stability_plan(&metrics);
    
    // Verify stabilization strategy
    assert!(plan.stabilization_actions.len() >= unstable_positions.len(),
        "Should have actions for all unstable positions");
        
    // Verify hex grid integrity is maintained
    let mut final_positions = unstable_positions.clone();
    final_positions.extend(plan.new_support_positions.iter());
    
    for pos in &final_positions {
        let neighbors = final_positions.iter()
            .filter(|&p| p != pos && p.hex_distance(pos) == 1)
            .count();
        assert!(neighbors >= 2,
            "Stabilization should maintain hex connectivity");
    }
}

#[test]
fn gsp_3803_recovery_prioritization() {
    let recovery = RecoveryManager::new();
    let metrics = RecoveryMetrics::new();
    
    // Record various issues in hex grid
    let issues = vec![
        // Critical path failure
        (Position::new(0, 0, 0), 0.9, SystemTime::now()),
        // Moderate instability
        (Position::new(1, -1, 0), 0.6, SystemTime::now()),
        // Minor issue
        (Position::new(2, -1, -1), 0.3, SystemTime::now()),
    ];
    
    for (pos, severity, time) in &issues {
        metrics.record_position_issue(*pos, *severity, *time);
    }
    
    // Generate prioritized recovery
    let priorities = recovery.prioritize_recovery(&metrics);
    
    // Verify prioritization
    let mut last_priority = f64::MAX;
    for (pos, priority) in priorities {
        let issue = issues.iter()
            .find(|(p, _, _)| p == &pos)
            .unwrap();
        
        assert!(priority <= last_priority,
            "Issues should be ordered by priority");
        assert!((priority - issue.1).abs() < 0.2,
            "Priority should correlate with severity");
        
        last_priority = priority;
    }
}

#[test]
fn gsp_3804_hex_pattern_restoration() {
    let recovery = RecoveryManager::new();
    
    // Define ideal hex pattern
    let ideal_pattern = vec![
        Position::new(0, 0, 0),    // Center
        Position::new(1, -1, 0),   // East
        Position::new(1, 0, -1),   // Southeast
        Position::new(0, 1, -1),   // Southwest
        Position::new(-1, 1, 0),   // West
        Position::new(-1, 0, 1),   // Northwest
        Position::new(0, -1, 1),   // Northeast
    ];
    
    // Create damaged pattern
    let mut damaged_pattern = ideal_pattern.clone();
    damaged_pattern.remove(2); // Remove one position
    damaged_pattern.push(Position::new(2, -1, -1)); // Add misplaced position
    
    // Generate restoration plan
    let plan = recovery.restore_hex_pattern(&damaged_pattern, &ideal_pattern);
    
    // Verify restoration
    assert_eq!(plan.positions_to_add.len(), 1,
        "Should add missing position");
    assert_eq!(plan.positions_to_remove.len(), 1,
        "Should remove misplaced position");
        
    // Apply restoration
    let mut restored = damaged_pattern.clone();
    restored.extend(plan.positions_to_add.iter());
    restored.retain(|p| !plan.positions_to_remove.contains(p));
    
    // Verify hex properties
    for pos in &restored {
        let neighbors = restored.iter()
            .filter(|&p| p != pos && p.hex_distance(pos) == 1)
            .count();
        assert_eq!(neighbors, if pos == &Position::new(0, 0, 0) { 6 } else { 2..=3 },
            "Restored pattern should maintain hex properties");
    }
} 