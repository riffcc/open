use bevy::prelude::*;
use bevy::math::Vec3;
use bevy::input::mouse::MouseMotion;
use bevy::input::mouse::MouseWheel;
use bevy::input::gamepad::GamepadButton;
use bevy::input::ButtonInput;
use rand::prelude::SliceRandom;
use rand::Rng;

const GOLDEN_ANGLE: f32 = 2.39996; // ≈ 137.5 degrees in radians
const DEFAULT_NODE_COUNT: usize = 500;
const NODE_SCALE: f32 = 0.05;
const LIGHT_INTENSITY: f32 = 1000.0;
const EMISSIVE_STRENGTH: f32 = 0.3;
const HEAT_DECAY_RATE: f32 = 2.0;
const MAX_HEAT: f32 = 5.0;
const NODES_PER_FRAME: usize = 3;
const HEIGHT_STEP: f32 = 0.2;
const RADIUS_STEP: f32 = 0.5;
const BRANCH_PROBABILITY: f32 = 0.2;

const SCALE_FACTOR: f32 = 0.8;
const BASE_SPACING: f32 = 1.0;

// Network structure constants
const HEX_DIRECTIONS: [(f32, f32); 6] = [
    (1.0, 0.0),      // Right
    (0.5, 0.866),    // Up-Right
    (-0.5, 0.866),   // Up-Left
    (-1.0, 0.0),     // Left
    (-0.5, -0.866),  // Down-Left
    (0.5, -0.866),   // Down-Right
];


#[derive(Component)]
struct CameraController {
    pub enabled: bool,
    pub sensitivity: f32,
    pub key_speed: f32,
    pub zoom_speed: f32,
}

impl Default for CameraController {
    fn default() -> Self {
        Self {
            enabled: true,
            sensitivity: 0.5,
            key_speed: 2.0,
            zoom_speed: 1.0,
        }
    }
}

#[derive(Resource, Clone)]
struct NetworkConfig {
    hex_radius: f32,      // Distance between layers
    hex_step: f32,        // Vertical step between layers
    layer_scale: f32,     // How much each layer scales down
    max_layers: usize,    // Maximum number of layers to build
    node_scale: f32,      // Base size of nodes
    
    // Double helix parameters
    helix_radius: f32,    // Radius of the helix
    helix_step: f32,      // Vertical step of helix
    helix_nodes: usize,   // Nodes per helix revolution
    branch_levels: usize,
    branch_scale: f32,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            hex_radius: 1.0,
            hex_step: 0.3,
            layer_scale: 0.9,
            max_layers: 5,
            node_scale: 0.1,
            
            helix_radius: 0.5,
            helix_step: 0.2,
            helix_nodes: 6,
            branch_levels: 3,
            branch_scale: 0.7,
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let node_count = args.get(1)
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(DEFAULT_NODE_COUNT);

    App::new()
        .insert_resource(NodeCount(node_count))
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, (
            rotate_camera,
            update_node_heat,
            random_activation,
            camera_controller,
        ).chain())
        .run();
}

#[derive(Component)]
struct NetworkNode {
    depth: usize,
    layer: usize,
    connections: Vec<Entity>,
}

#[derive(Component)]
struct NodeHeat {
    heat: f32,
    last_activation: f32,
}

impl Default for NodeHeat {
    fn default() -> Self {
        Self {
            heat: 0.0,
            last_activation: 0.0,
        }
    }
}

#[derive(Resource)]
struct NodeCount(usize);

// Calculate how many nodes we can fit in each layer
fn calculate_max_depth(total_nodes: usize) -> usize {
    let mut nodes_in_tree = 1; // Center node
    let mut depth = 0;
    let nodes_per_layer = 6; // Hex pattern

    while nodes_in_tree < total_nodes {
        depth += 1;
        // Each new layer adds 6 * current_depth nodes (more subdivisions further out)
        nodes_in_tree += nodes_per_layer * depth;
    }

    depth
}

fn spawn_network_node(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    config: &NetworkConfig,
    position: Vec3,
    layer: usize,
    _angle: f32,
    nodes_left: &mut usize,
) -> Option<Entity> {
    if *nodes_left == 0 {
        return None;
    }

    let scale = config.node_scale * config.layer_scale.powi(layer as i32);
    
    let entity = commands.spawn((
        PbrBundle {
            mesh: meshes.add(Mesh::from(Sphere::default())),
            material: materials.add(StandardMaterial {
                base_color: Color::srgb(0.5, 0.0, 0.5),
                emissive: Color::srgb(0.5 * EMISSIVE_STRENGTH, 0.0, 0.5 * EMISSIVE_STRENGTH)
                    .with_alpha(0.3)
                    .into(),
                ..default()
            }),
            transform: Transform::from_translation(position)
                .with_scale(Vec3::splat(scale)),
            ..default()
        },
        NetworkNode {
            depth: 0,
            layer,
            connections: Vec::new(),
        },
        NodeHeat::default(),
    )).id();

    *nodes_left -= 1;
    Some(entity)
}

fn spawn_fractal_node(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    position: Vec3,
    depth: usize,
    max_depth: usize,
    scale: f32,
    nodes_left: &mut usize,
) {
    if *nodes_left == 0 {
        return;
    }

    // Spawn current node
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Mesh::from(Sphere::default())),
            material: materials.add(StandardMaterial {
                base_color: Color::srgb(0.5, 0.0, 0.5),
                emissive: Color::srgb(0.5 * EMISSIVE_STRENGTH, 0.0, 0.5 * EMISSIVE_STRENGTH)
                    .with_alpha(0.3)
                    .into(),
                ..default()
            }),
            transform: Transform::from_translation(position)
                .with_scale(Vec3::splat(scale)),
            ..default()
        },
        NetworkNode { depth, layer: depth, connections: Vec::new() },
        NodeHeat::default(),
    ));

    *nodes_left -= 1;

    // If we've reached max depth or no nodes left, stop recursing
    if depth >= max_depth || *nodes_left == 0 {
        return;
    }

    // Spawn child nodes in a hex pattern
    let new_scale = scale * SCALE_FACTOR;
    let spacing = BASE_SPACING * (SCALE_FACTOR.powf(depth as f32));

    for (dir_x, dir_z) in HEX_DIRECTIONS.iter() {
        let child_pos = position + Vec3::new(
            dir_x * spacing,
            (spacing * 0.3).sin() * 0.5,
            dir_z * spacing,
        );
        
        spawn_fractal_node(
            commands,
            meshes,
            materials,
            child_pos,
            depth + 1,
            max_depth,
            new_scale,
            nodes_left,
        );
    }
}

fn spawn_spiral_node(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    mut position: Vec3,
    depth: usize,
    angle: f32,
    radius: f32,
    scale: f32,
    nodes_left: &mut usize,
) {
    if *nodes_left == 0 {
        return;
    }

    // Spawn current node
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Mesh::from(Sphere::default())),
            material: materials.add(StandardMaterial {
                base_color: Color::srgb(0.5, 0.0, 0.5),
                emissive: Color::srgb(0.5 * EMISSIVE_STRENGTH, 0.0, 0.5 * EMISSIVE_STRENGTH)
                    .with_alpha(0.3)
                    .into(),
                ..default()
            }),
            transform: Transform::from_translation(position)
                .with_scale(Vec3::splat(scale)),
            ..default()
        },
        NetworkNode { depth, layer: depth, connections: Vec::new() },
        NodeHeat::default(),
    ));

    *nodes_left -= 1;

    if *nodes_left == 0 {
        return;
    }

    let mut rng = rand::thread_rng();

    // Continue the main spiral
    let new_angle = angle + GOLDEN_ANGLE;
    let new_radius = radius + RADIUS_STEP;
    let new_position = Vec3::new(
        new_radius * new_angle.cos(),
        position.y + HEIGHT_STEP,
        new_radius * new_angle.sin(),
    );

    // Spawn next node in main spiral
    spawn_spiral_node(
        commands,
        meshes,
        materials,
        new_position,
        depth + 1,
        new_angle,
        new_radius,
        scale * 0.95, // Slightly smaller
        nodes_left,
    );

    // Randomly create branches
    if rng.gen::<f32>() < BRANCH_PROBABILITY && *nodes_left > 0 {
        // Create a branch at an angle
        let branch_angle = new_angle + std::f32::consts::PI * 0.5;
        let branch_position = Vec3::new(
            position.x + (radius * 0.5) * branch_angle.cos(),
            position.y + HEIGHT_STEP * 0.5,
            position.z + (radius * 0.5) * branch_angle.sin(),
        );

        spawn_spiral_node(
            commands,
            meshes,
            materials,
            branch_position,
            depth + 1,
            branch_angle,
            radius * 0.5,
            scale * 0.8,
            nodes_left,
        );
    }
}

fn spawn_helix_node(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    config: &NetworkConfig,
    position: Vec3,
    radius: f32,
    angle: f32,
    height: f32,
    scale: f32,
    depth: usize,
    nodes_left: &mut usize,
) {
    if *nodes_left == 0 {
        return;
    }

    // Create node
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Mesh::from(Sphere::default())),
            material: materials.add(StandardMaterial {
                base_color: Color::srgb(0.5, 0.0, 0.5),
                emissive: Color::srgb(0.5 * EMISSIVE_STRENGTH, 0.0, 0.5 * EMISSIVE_STRENGTH)
                    .with_alpha(0.3)
                    .into(),
                ..default()
            }),
            transform: Transform::from_translation(position)
                .with_scale(Vec3::splat(scale)),
            ..default()
        },
        NetworkNode { depth, layer: depth, connections: Vec::new() },
        NodeHeat::default(),
    ));

    *nodes_left -= 1;

    // Branch based on depth
    if depth < config.branch_levels {
        for i in 0..6 {
            let branch_angle = angle + (i as f32) * std::f32::consts::PI / 3.0;
            let branch_pos = position + Vec3::new(
                radius * config.branch_scale * branch_angle.cos(),
                height + config.helix_step * 0.5,
                radius * config.branch_scale * branch_angle.sin(),
            );
            
            spawn_helix_node(
                commands,
                meshes,
                materials,
                config,
                branch_pos,
                radius * config.branch_scale,
                branch_angle,
                height + config.helix_step,
                scale * config.branch_scale,
                depth + 1,
                nodes_left,
            );
        }
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    node_count: Res<NodeCount>,
) {
    // Camera
    commands.spawn((
        Camera3dBundle {
            transform: Transform::from_xyz(-2.0, 2.5, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
            ..default()
        },
        CameraController::default(),
    ));

    // Light
    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            illuminance: LIGHT_INTENSITY,
            shadows_enabled: true,
            ..default()
        },
        transform: Transform::from_xyz(4.0, 8.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });

    let config = NetworkConfig::default();
    commands.insert_resource(config.clone());
    
    let mut nodes_left = node_count.0;
    let mut entities = Vec::new();
    
    // Spawn the singularity node
    let singularity = spawn_network_node(
        &mut commands,
        &mut meshes,
        &mut materials,
        &config,
        Vec3::ZERO,
        0,  // Core layer
        0.0,
        &mut nodes_left,
    ).unwrap();
    
    entities.push(singularity);
    
    // Build the double helix emerging from the singularity
    for i in 1..config.helix_nodes {
        let angle = (i as f32) * std::f32::consts::TAU / config.helix_nodes as f32;
        let height = i as f32 * config.helix_step;
        
        // Spawn two nodes for the double helix, emerging from the center
        for helix in 0..2 {
            let offset_angle = angle + helix as f32 * std::f32::consts::PI;
            let radius = config.helix_radius * (1.0 - (-(i as f32) * 0.5).exp()); // Gradually expand radius
            let pos = Vec3::new(
                radius * offset_angle.cos(),
                height,
                radius * offset_angle.sin(),
            );
            
            if let Some(entity) = spawn_network_node(
                &mut commands,
                &mut meshes,
                &mut materials,
                &config,
                pos,
                0,  // Core layer
                offset_angle,
                &mut nodes_left,
            ) {
                entities.push(entity);
            }
        }
    }
    
    // Rest of the hex layer code remains the same...
}

fn rotate_camera(
    time: Res<Time>,
    mut query: Query<&mut Transform, With<Camera>>,
) {
    for mut transform in &mut query {
        transform.rotate_around(
            Vec3::ZERO,
            Quat::from_rotation_y(0.1 * time.delta_seconds()),
        );
    }
}

fn update_node_heat(
    time: Res<Time>,
    mut query: Query<(&mut NodeHeat, &Handle<StandardMaterial>, &NetworkNode)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let current_time = time.elapsed_seconds();
    
    for (mut heat, material_handle, node) in &mut query {
        if let Some(material) = materials.get_mut(material_handle.id()) {
            let age = current_time - heat.last_activation;
            let decay = (-age * HEAT_DECAY_RATE).exp();
            heat.heat = heat.heat * decay;

            // Brighter intensity
            let intensity = heat.heat * (1.0 - (node.depth as f32 * 0.1).min(0.7));
            material.emissive = Color::srgb(
                intensity,
                intensity * 0.5,
                intensity
            ).into();
        }
    }
}

fn random_activation(
    time: Res<Time>,
    mut query: Query<(Entity, &mut NodeHeat)>,
) {
    let mut rng = rand::thread_rng();
    
    // Collect just the entities into a Vec
    let entities: Vec<Entity> = query.iter().map(|(entity, _)| entity).collect();
    
    // Pick NODES_PER_FRAME random entities
    for _ in 0..NODES_PER_FRAME {
        if let Some(&entity) = entities.choose(&mut rng) {
            // Get this specific entity's heat
            if let Ok((_, mut heat)) = query.get_mut(entity) {
                heat.heat = MAX_HEAT;
                heat.last_activation = time.elapsed_seconds();
            }
        }
    }
}

fn camera_controller(
    time: Res<Time>,
    mut mouse_motion: EventReader<MouseMotion>,
    mut mouse_wheel: EventReader<MouseWheel>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    _gamepad: Res<ButtonInput<GamepadButton>>,
    mut query: Query<(&mut Transform, &CameraController), With<Camera>>,
) {
    let dt = time.delta_seconds();

    for (mut transform, controller) in &mut query {
        if !controller.enabled {
            continue;
        }

        // Mouse look (hold right mouse button)
        if mouse.pressed(MouseButton::Right) {
            for ev in mouse_motion.read() {
                let (mut yaw, mut pitch, _) = transform.rotation.to_euler(EulerRot::YXZ);
                yaw -= ev.delta.x * controller.sensitivity * dt;
                pitch -= ev.delta.y * controller.sensitivity * dt;
                transform.rotation = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0);
            }
        }

        // Keyboard movement
        let mut movement = Vec3::ZERO;
        if keyboard.pressed(KeyCode::KeyW) { movement.z -= 1.0; }
        if keyboard.pressed(KeyCode::KeyS) { movement.z += 1.0; }
        if keyboard.pressed(KeyCode::KeyA) { movement.x -= 1.0; }
        if keyboard.pressed(KeyCode::KeyD) { movement.x += 1.0; }
        if keyboard.pressed(KeyCode::Space) { movement.y += 1.0; }
        if keyboard.pressed(KeyCode::ShiftLeft) { movement.y -= 1.0; }

        // Apply movement
        if movement != Vec3::ZERO {
            let movement = transform.rotation * movement.normalize() * controller.key_speed * dt;
            transform.translation += movement;
        }

        // Zoom with mouse wheel
        for ev in mouse_wheel.read() {
            let forward = transform.forward();
            transform.translation += forward * ev.y * controller.zoom_speed;
        }
    }
}