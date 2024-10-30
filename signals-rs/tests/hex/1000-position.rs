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
    let original = Position::new(1, -1, 0);
    assert!(original.is_valid());

    // Serialize
    let serialized = bincode::serialize(&original).expect("Serialization failed");
    
    // Deserialize
    let deserialized: Position = bincode::deserialize(&serialized).expect("Deserialization failed");
    
    assert!(deserialized.is_valid(), "Deserialized position should be valid");
    assert_eq!(original, deserialized, "Position should be preserved exactly");
} 