use signals_rs::gsp::{NetworkVisualizer, VisConfig, HexGrid, VisualMetrics};
use signals_rs::hex::Position;
use signals_rs::common::NodeId;
use signals_rs::viz::{Color, Renderer, HexRenderer};
use std::time::{Duration, SystemTime};

#[test]
fn gsp_4100_hex_grid_rendering() {
    let visualizer = NetworkVisualizer::new(VisConfig {
        hex_size: 30.0,
        spacing: 5.0,
        color_scheme: ColorScheme::default(),
        ..Default::default()
    });
    
    // Create a test hex grid
    let mut grid = HexGrid::new();
    
    // Add three rings of hex cells
    for q in -2..=2 {
        for r in -2..=2 {
            let s = -q - r;
            if s.abs() <= 2 {
                let pos = Position::new(q, r, s);
                grid.add_node(NodeId::generate(), pos);
            }
        }
    }
    
    // Render grid
    let renderer = HexRenderer::new();
    let frame = visualizer.render_grid(&grid, &renderer)?;
    
    // Verify rendering properties
    assert_eq!(frame.node_count(), grid.node_count(),
        "All nodes should be rendered");
        
    // Verify hex layout
    for node in grid.nodes() {
        let pos = frame.get_node_position(node.id());
        assert!(pos.is_some(), "Each node should have a position");
        
        let hex_coords = visualizer.pixel_to_hex(pos.unwrap());
        assert_eq!(hex_coords, node.position(),
            "Rendered positions should match hex coordinates");
    }
}

#[test]
fn gsp_4101_network_state_visualization() {
    let visualizer = NetworkVisualizer::new(VisConfig::default());
    let metrics = VisualMetrics::new();
    
    // Create network with varying states
    let mut grid = HexGrid::new();
    let states = vec![
        (Position::new(0, 0, 0), NetworkState::Active),
        (Position::new(1, -1, 0), NetworkState::Degraded),
        (Position::new(1, 0, -1), NetworkState::Failed),
        (Position::new(0, 1, -1), NetworkState::Recovering),
    ];
    
    for (pos, state) in &states {
        let id = NodeId::generate();
        grid.add_node(id, *pos);
        grid.set_node_state(id, *state);
    }
    
    // Render with state coloring
    let frame = visualizer.render_network_state(&grid)?;
    
    // Verify state visualization
    for (pos, state) in states {
        let node_id = grid.get_node_at(&pos).unwrap().id();
        let color = frame.get_node_color(node_id).unwrap();
        
        match state {
            NetworkState::Active => {
                assert_eq!(color, Color::GREEN,
                    "Active nodes should be green");
            },
            NetworkState::Degraded => {
                assert_eq!(color, Color::YELLOW,
                    "Degraded nodes should be yellow");
            },
            NetworkState::Failed => {
                assert_eq!(color, Color::RED,
                    "Failed nodes should be red");
            },
            NetworkState::Recovering => {
                assert_eq!(color, Color::BLUE,
                    "Recovering nodes should be blue");
            },
        }
        
        metrics.record_state_visualization(pos, state, color);
    }
}

#[test]
fn gsp_4102_flow_visualization() {
    let visualizer = NetworkVisualizer::new(VisConfig::default());
    
    // Create hex grid with message flows
    let mut grid = HexGrid::new();
    let flows = vec![
        (Position::new(0, 0, 0), Position::new(1, -1, 0), 100),
        (Position::new(0, 0, 0), Position::new(0, 1, -1), 50),
        (Position::new(1, -1, 0), Position::new(2, -1, -1), 25),
    ];
    
    for (from, to, volume) in &flows {
        grid.add_node(NodeId::generate(), *from);
        grid.add_node(NodeId::generate(), *to);
        grid.add_flow(*from, *to, *volume);
    }
    
    // Render flows
    let frame = visualizer.render_flows(&grid)?;
    
    // Verify flow visualization
    for (from, to, volume) in flows {
        let edge = frame.get_edge(from, to).unwrap();
        
        assert!(edge.width > 0.0,
            "Flow edges should have visible width");
        assert!(edge.width.is_proportional_to(*volume),
            "Edge width should be proportional to flow volume");
        
        // Verify edge follows hex grid
        let path = edge.path();
        for segment in path.windows(2) {
            let hex_dist = Position::from_pixels(segment[0])
                .hex_distance(&Position::from_pixels(segment[1]));
            assert_eq!(hex_dist, 1,
                "Flow paths should follow hex grid");
        }
    }
}

#[test]
fn gsp_4103_temporal_animation() {
    let visualizer = NetworkVisualizer::new(VisConfig {
        animation_speed: 1.0,
        frame_rate: 60,
        ..Default::default()
    });
    
    // Create network changes over time
    let mut changes = vec![];
    let center = Position::new(0, 0, 0);
    
    // Node joining hex grid
    changes.push(NetworkChange {
        time: Duration::from_secs(0),
        position: center,
        change_type: ChangeType::Join,
    });
    
    // Node movement along hex paths
    for i in 1..=6 {
        let pos = center.hex_neighbor(i);
        changes.push(NetworkChange {
            time: Duration::from_secs(i),
            position: pos,
            change_type: ChangeType::Move,
        });
    }
    
    // Render animation
    let animation = visualizer.create_animation(changes)?;
    
    // Verify animation properties
    assert_eq!(animation.frame_count(),
        (6.0 * visualizer.config().frame_rate as f64) as usize,
        "Animation should have correct number of frames");
        
    // Verify smooth transitions
    for frame in animation.frames() {
        for node in frame.nodes() {
            let pos = node.position();
            assert!(pos.is_valid_hex_position(),
                "Animated positions should remain on hex grid");
        }
    }
}

#[test]
fn gsp_4104_metric_visualization() {
    let visualizer = NetworkVisualizer::new(VisConfig::default());
    let metrics = VisualMetrics::new();
    
    // Create hex grid with various metrics
    let mut grid = HexGrid::new();
    let node_metrics = vec![
        (Position::new(0, 0, 0), 1.0),    // High load
        (Position::new(1, -1, 0), 0.7),
        (Position::new(1, 0, -1), 0.4),
        (Position::new(0, 1, -1), 0.1),   // Low load
    ];
    
    for (pos, value) in &node_metrics {
        let id = NodeId::generate();
        grid.add_node(id, *pos);
        metrics.record_node_metric(id, *value);
    }
    
    // Render with metric visualization
    let frame = visualizer.render_metrics(&grid, &metrics)?;
    
    // Verify metric visualization
    for (pos, value) in node_metrics {
        let node_id = grid.get_node_at(&pos).unwrap().id();
        let visual = frame.get_node_visual(node_id).unwrap();
        
        assert!(visual.intensity.is_proportional_to(value),
            "Visual intensity should represent metric value");
        
        // Verify hex grid positioning maintained
        assert_eq!(visual.position.to_hex(), pos,
            "Metric visualization should maintain hex grid");
    }
} 