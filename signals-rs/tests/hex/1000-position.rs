use signals_rs::hex::Position;

#[test]
fn hex_1000_position_constraints() {
    let valid_positions = vec![
        (0, 0, 0),
        (1, -1, 0),
        (2, -1, -1),
        (-2, 1, 1)
    ];
    
    for (x, y, z) in valid_positions {
        let pos = Position::new(x, y, z);
        assert!(pos.is_valid(), "Position ({x},{y},{z}) should be valid");
    }
}

#[test]
fn hex_1001_reject_invalid_coordinates() {
    let invalid_positions = vec![
        (1, 1, 1),    // Sum > 0
        (-1, -1, -1), // Sum < 0
        (0, 1, 0),    // Non-zero sum
    ];

    for (x, y, z) in invalid_positions {
        let pos = Position::new(x, y, z);
        assert!(!pos.is_valid(), "Position ({x},{y},{z}) should be invalid");
    }
}

#[test]
fn hex_1002_maintain_constraints_on_modification() {
    let mut pos = Position::new(0, 0, 0);
    assert!(pos.is_valid());

    // Test various modifications
    let modifications = vec![
        (1, -1, 0),   // Valid shift
        (-2, 1, 1),   // Valid shift
    ];

    for (dx, dy, dz) in modifications {
        pos.shift(dx, dy, dz);
        assert!(pos.is_valid(), "Position should remain valid after shift");
    }
}

#[test]
fn hex_1003_serialization_preserves_validity() {
    // Test cases covering edge cases and boundaries
    let test_positions = vec![
        Position::new(0, 0, 0),           // Origin
        Position::new(1, -1, 0),          // Basic valid position
        Position::new(i64::MAX/3, i64::MIN/3, -i64::MAX/3), // Large values
        Position::new(-42, 21, 21),       // Negative coordinates
    ];
    
    for original in test_positions {
        assert!(original.is_valid(), "Initial position must be valid");
        
        // Test JSON serialization
        let json = serde_json::to_string(&original).expect("JSON serialization failed");
        let from_json: Position = serde_json::from_str(&json).expect("JSON deserialization failed");
        assert!(from_json.is_valid(), "JSON deserialized position should be valid");
        assert_eq!(original, from_json, "JSON round-trip should preserve position exactly");
        
        // Test binary serialization
        let binary = bincode::serialize(&original).expect("Binary serialization failed");
        let from_binary: Position = bincode::deserialize(&binary).expect("Binary deserialization failed");
        assert!(from_binary.is_valid(), "Binary deserialized position should be valid");
        assert_eq!(original, from_binary, "Binary round-trip should preserve position exactly");
        
        // Test custom string format
        let string = original.to_string();
        let from_string = Position::from_str(&string).expect("String parsing failed");
        assert!(from_string.is_valid(), "String parsed position should be valid");
        assert_eq!(original, from_string, "String round-trip should preserve position exactly");
    }
}