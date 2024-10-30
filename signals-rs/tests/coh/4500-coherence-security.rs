use signals_rs::gsp::{CoherenceManager, NodeState, CoherenceMetrics};
use signals_rs::hex::Position;
use signals_rs::common::NodeId;
use signals_rs::security::{SecurityMetrics, AttackDetector};
use std::time::{Duration, SystemTime};

#[test]
fn gsp_4500_prevent_coherence_manipulation() {
    let coherence = CoherenceManager::new();
    let security = SecurityMetrics::new();
    
    let attacker = NodeId::generate();
    let target = NodeId::generate();
    
    // Attempt rapid coherence changes
    for _ in 0..100 {
        coherence.attempt_update(attacker, target, 1.0);
        coherence.attempt_update(attacker, target, 0.0);
    }
    
    // Verify manipulation prevention
    let changes = security.analyze_coherence_changes(target);
    assert!(changes.is_within_normal_bounds(),
        "Should prevent rapid coherence manipulation");
        
    // Check rate limiting
    let updates = coherence.get_recent_updates(attacker, target);
    assert!(updates.len() < 10,
        "Should rate limit coherence updates");
}

#[test]
fn gsp_4501_coherence_data_privacy() {
    let coherence = CoherenceManager::new();
    let metrics = CoherenceMetrics::new();
    
    // Create network of nodes with varying coherence
    let nodes: Vec<_> = (0..10).map(|_| NodeId::generate()).collect();
    
    for node in &nodes {
        for target in &nodes {
            if node != target {
                let value = fastrand::f64();
                coherence.set_node_coherence(*node, *target, value);
            }
        }
    }
    
    // Verify privacy boundaries
    for node in &nodes {
        let visible = coherence.get_visible_coherence_data(*node);
        
        // Node should only see its own ratings and ratings about it
        for (rater, target, _) in visible {
            assert!(rater == *node || target == *node,
                "Nodes should only see relevant coherence data");
        }
    }
}

#[test]
fn gsp_4502_coherence_update_authenticity() {
    let coherence = CoherenceManager::new();
    let security = SecurityMetrics::new();
    
    let node_a = NodeId::generate();
    let node_b = NodeId::generate();
    
    // Attempt unauthorized updates
    let result = coherence.forge_update(node_a, node_b, 0.0);
    assert!(result.is_err(),
        "Should reject unauthorized coherence updates");
        
    // Verify update signatures
    let valid_update = coherence.create_signed_update(node_a, node_b, 0.8);
    assert!(coherence.verify_update_signature(&valid_update),
        "Should verify authentic coherence updates");
        
    // Check update authenticity tracking
    security.record_update_attempt(valid_update);
    assert!(security.all_updates_authentic(),
        "Should maintain update authenticity records");
}

#[test]
fn gsp_4503_malicious_pattern_detection() {
    let coherence = CoherenceManager::new();
    let detector = AttackDetector::new();
    
    // Create suspicious coherence patterns
    let malicious = NodeId::generate();
    let targets: Vec<_> = (0..20).map(|_| NodeId::generate()).collect();
    
    // Pattern 1: Mass defederation
    for target in &targets {
        coherence.set_node_coherence(malicious, *target, -1.0);
    }
    
    // Pattern 2: Coordinated attacks
    let attackers: Vec<_> = (0..5).map(|_| NodeId::generate()).collect();
    let victim = NodeId::generate();
    
    for attacker in &attackers {
        coherence.set_node_coherence(*attacker, victim, -1.0);
    }
    
    // Verify detection
    assert!(detector.detect_mass_defederation(malicious),
        "Should detect mass defederation attempts");
        
    assert!(detector.detect_coordinated_attack(&attackers, victim),
        "Should detect coordinated coherence attacks");
}

#[test]
fn gsp_4504_gaming_prevention() {
    let coherence = CoherenceManager::new();
    let metrics = CoherenceMetrics::new();
    
    let node_a = NodeId::generate();
    let node_b = NodeId::generate();
    
    // Attempt gaming patterns
    let patterns = vec![
        // Oscillating updates
        (0.0, 1.0, Duration::from_secs(1)),
        (1.0, 0.0, Duration::from_secs(2)),
        // Gradual manipulation
        (0.1, 0.2, Duration::from_secs(10)),
        (0.2, 0.3, Duration::from_secs(20)),
        // Sudden jumps
        (0.3, 0.9, Duration::from_secs(30)),
    ];
    
    for (start, end, delay) in patterns {
        coherence.set_node_coherence(node_a, node_b, start);
        std::thread::sleep(delay);
        coherence.set_node_coherence(node_a, node_b, end);
    }
    
    // Verify gaming detection
    let behavior = metrics.analyze_update_patterns(node_a);
    assert!(behavior.has_suspicious_patterns(),
        "Should detect coherence gaming attempts");
        
    // Check penalties
    let final_coherence = coherence.get_current_coherence(node_a);
    assert!(final_coherence < 0.5,
        "Gaming attempts should result in coherence penalties");
} 