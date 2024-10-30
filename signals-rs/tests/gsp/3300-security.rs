use signals_rs::gsp::{SecurityManager, GossipUpdate, UpdateVerifier};
use signals_rs::common::NodeId;
use signals_rs::crypto::{Signature, SigningKey};
use signals_rs::hex::Position;
use std::time::{Duration, SystemTime};

#[test]
fn gsp_3300_update_verification() {
    let security = SecurityManager::new();
    let node_id = NodeId::generate();
    let key = SigningKey::generate();
    
    // Register node's public key
    security.register_node(node_id, key.public_key()).await?;
    
    // Create signed update with position
    let update = GossipUpdate {
        source_id: node_id,
        timestamp: SystemTime::now(),
        content: UpdateContent::Position(Position::new(1, -1, 0)),
    };
    let signature = key.sign(&update);
    
    // Verify valid update
    assert!(security.verify_update(&update, &signature).is_ok(),
        "Should accept valid signed update");
    
    // Try with wrong signature
    let wrong_key = SigningKey::generate();
    let wrong_sig = wrong_key.sign(&update);
    
    assert!(security.verify_update(&update, &wrong_sig).is_err(),
        "Should reject update with invalid signature");
}

#[test]
fn gsp_3301_replay_protection() {
    let security = SecurityManager::new();
    let verifier = security.update_verifier();
    let node_id = NodeId::generate();
    
    // Create original update with hex position
    let update = GossipUpdate {
        source_id: node_id,
        timestamp: SystemTime::now(),
        content: UpdateContent::Position(Position::new(0, 0, 0)),
    };
    
    // Process original
    verifier.process_update(&update).await?;
    
    // Try to replay same update
    let replay_result = verifier.process_update(&update).await;
    assert!(replay_result.is_err(),
        "Should reject replayed update");
    
    // Verify newer update still accepted
    let newer_update = GossipUpdate {
        source_id: node_id,
        timestamp: SystemTime::now() + Duration::from_secs(1),
        content: UpdateContent::Position(Position::new(1, -1, 0)),
    };
    
    assert!(verifier.process_update(&newer_update).await.is_ok(),
        "Should accept newer update from same source");
}

#[test]
fn gsp_3302_flood_protection() {
    let security = SecurityManager::new();
    let node_id = NodeId::generate();
    let key = SigningKey::generate();
    
    security.register_node(node_id, key.public_key()).await?;
    
    // Try to flood with position updates
    let mut flood_count = 0;
    let start = SystemTime::now();
    
    while SystemTime::now().duration_since(start)?.as_secs() < 1 {
        let update = GossipUpdate {
            source_id: node_id,
            timestamp: SystemTime::now(),
            content: UpdateContent::Position(Position::new(
                flood_count % 2,
                -(flood_count % 2),
                0
            )),
        };
        let signature = key.sign(&update);
        
        if security.verify_update(&update, &signature).is_ok() {
            flood_count += 1;
        }
    }
    
    assert!(flood_count < security.max_updates_per_second(),
        "Should limit update rate");
}

#[test]
fn gsp_3303_malicious_content() {
    let security = SecurityManager::new();
    let verifier = security.content_verifier();
    
    // Test various malicious position patterns
    let malicious_patterns = vec![
        // Invalid hex coordinates
        Position::new(1, 1, 1),
        // Extreme coordinates
        Position::new(i64::MAX, i64::MIN, 0),
        // Rapidly changing positions
        Position::new(0, 0, 0),
        Position::new(100, -50, -50),
        // Invalid transitions
        Position::new(1, -1, 0),
        Position::new(-5, 2, 3),
    ];
    
    for pos in malicious_patterns {
        let content = UpdateContent::Position(pos);
        let result = verifier.verify_content(&content);
        assert!(result.is_err(),
            "Should reject malicious position pattern");
    }
}

#[test]
fn gsp_3304_trust_revocation() {
    let security = SecurityManager::new();
    let node_id = NodeId::generate();
    let key = SigningKey::generate();
    
    // Register node
    security.register_node(node_id, key.public_key()).await?;
    
    // Create valid position update
    let update = GossipUpdate {
        source_id: node_id,
        timestamp: SystemTime::now(),
        content: UpdateContent::Position(Position::new(1, -1, 0)),
    };
    let signature = key.sign(&update);
    
    // Verify works initially
    assert!(security.verify_update(&update, &signature).is_ok());
    
    // Revoke trust
    security.revoke_trust(node_id).await?;
    
    // Verify fails after revocation
    assert!(security.verify_update(&update, &signature).is_err(),
        "Should reject updates from revoked node");
    
    // Verify revocation is permanent
    security.register_node(node_id, key.public_key()).await?;
    assert!(security.verify_update(&update, &signature).is_err(),
        "Revocation should be permanent");
} 