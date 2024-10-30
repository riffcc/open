use signals_rs::hex::Position;

#[test]
fn hex_1100_geometric_distance() {
    let test_cases = vec![
        // (pos1, pos2, expected_distance)
        ((0,0,0), (1,-1,0), 1),     // Adjacent hexes
        ((0,0,0), (2,-1,-1), 2),    // Two steps away
        ((1,-1,0), (-1,1,0), 2),    // Opposite sides
        ((2,-2,0), (-2,2,0), 4),    // Long straight line
        ((1,0,-1), (-1,1,0), 2),    // Diagonal movement
    ];

    for ((x1,y1,z1), (x2,y2,z2), expected) in test_cases {
        let pos1 = Position::new(x1,y1,z1);
        let pos2 = Position::new(x2,y2,z2);
        assert_eq!(pos1.hex_distance(&pos2), expected, 
            "Distance from ({},{},{}) to ({},{},{}) should be {}", 
            x1,y1,z1, x2,y2,z2, expected);
    }
}

#[test]
fn hex_1101_symmetrical_distance() {
    let positions = vec![
        (0,0,0),
        (1,-1,0),
        (2,-1,-1),
        (-2,1,1),
        (3,-2,-1)
    ];

    for (x1,y1,z1) in &positions {
        let pos1 = Position::new(*x1,*y1,*z1);
        for (x2,y2,z2) in &positions {
            let pos2 = Position::new(*x2,*y2,*z2);
            assert_eq!(
                pos1.hex_distance(&pos2),
                pos2.hex_distance(&pos1),
                "Distance should be symmetrical between ({},{},{}) and ({},{},{})",
                x1,y1,z1, x2,y2,z2
            );
        }
    }
}

#[test]
fn hex_1102_triangle_inequality() {
    let positions = vec![
        Position::new(0,0,0),
        Position::new(1,-1,0),
        Position::new(2,-1,-1),
        Position::new(-2,1,1),
        Position::new(3,-2,-1)
    ];

    for pos1 in &positions {
        for pos2 in &positions {
            for pos3 in &positions {
                let d12 = pos1.hex_distance(pos2);
                let d23 = pos2.hex_distance(pos3);
                let d13 = pos1.hex_distance(pos3);
                assert!(d13 <= d12 + d23, 
                    "Triangle inequality failed: d({:?},{:?}) > d({:?},{:?}) + d({:?},{:?})",
                    pos1, pos3, pos1, pos2, pos2, pos3);
            }
        }
    }
}

#[test]
fn hex_1103_self_distance() {
    let positions = vec![
        (0,0,0),
        (1,-1,0),
        (2,-1,-1),
        (-2,1,1),
        (3,-2,-1)
    ];

    for (x,y,z) in positions {
        let pos = Position::new(x,y,z);
        assert_eq!(pos.hex_distance(&pos), 0,
            "Distance to self should be 0 for position ({},{},{})", x,y,z);
    }
}

#[test]
fn hex_1104_adjacent_distance() {
    let center = Position::new(0,0,0);
    let adjacent_positions = vec![
        (1,-1,0),
        (1,0,-1),
        (0,1,-1),
        (-1,1,0),
        (-1,0,1),
        (0,-1,1)
    ];

    for (x,y,z) in adjacent_positions {
        let pos = Position::new(x,y,z);
        assert_eq!(center.hex_distance(&pos), 1,
            "Distance to adjacent hex ({},{},{}) should be 1", x,y,z);
    }
} 