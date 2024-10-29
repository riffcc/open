use bevy::prelude::*;
use bevy::math::Vec3;
use bevy::input::mouse::MouseMotion;
use bevy::input::mouse::MouseWheel;
use bevy::input::gamepad::GamepadButton;
use bevy::input::ButtonInput;
use rand::prelude::SliceRandom;

const DEFAULT_NODE_COUNT: usize = 500;

// Add performance optimization constants
const NODE_SCALE: f32 = 0.05; // Make spheres smaller
const LIGHT_INTENSITY: f32 = 1000.0; // Reduce light intensity
const EMISSIVE_STRENGTH: f32 = 0.3; // Reduce glow effect

// Add these constants at the top
const HEAT_DECAY_RATE: f32 = 2.0;   // Faster decay
const MAX_HEAT: f32 = 5.0;          // Even brighter!

// Adjust constants for better visual feedback
const NODES_PER_FRAME: usize = 3;  // How many nodes to activate each frame

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

fn calculate_ring_size(ring: usize) -> usize {
    if ring == 0 {
        1
    } else {
        6 * ring
    }
}

#[derive(Resource)]
struct NodeCount(usize);

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    node_count: Res<NodeCount>,
) {
    // Create shared mesh and materials
    let sphere_mesh = meshes.add(Mesh::from(Sphere::default()));
    let node_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.5, 0.0, 0.5),
        emissive: Color::srgb(
            0.5 * EMISSIVE_STRENGTH,
            0.0,
            0.5 * EMISSIVE_STRENGTH
        ).into(),
        ..default()
    });

    // Origin node with green material
    let origin_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.0, 1.0, 0.0),
        emissive: Color::srgb(
            0.0,
            1.0 * EMISSIVE_STRENGTH * 2.0, // Brighter for origin
            0.0
        ).into(),
        ..default()
    });

    // Camera
    commands.spawn((
        Camera3dBundle {
            transform: Transform::from_xyz(-2.0, 2.5, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
            ..default()
        },
        CameraController::default(),
    ));

    // Reduce light intensity
    commands.spawn(PointLightBundle {
        point_light: PointLight {
            intensity: LIGHT_INTENSITY,
            shadows_enabled: false, // Disable shadows for performance
            ..default()
        },
        transform: Transform::from_xyz(4.0, 8.0, 4.0),
        ..default()
    });

    // Origin node
    commands.spawn((
        PbrBundle {
            mesh: sphere_mesh.clone(),
            material: origin_material,
            transform: Transform::from_xyz(0.0, 0.0, 0.0)
                .with_scale(Vec3::splat(NODE_SCALE * 2.0)), // Bigger origin node
            ..default()
        },
        NetworkNode {
            depth: 0,
        },
        NodeHeat::default(),
    ));

    let mut nodes_created = 1;
    let mut ring = 1;
    
    // Keep adding rings until we reach 500 nodes
    while nodes_created < node_count.0 {
        let ring_size = calculate_ring_size(ring);
        let radius = ring as f32 * 0.5; // Adjust spacing between rings
        
        for i in 0..ring_size {
            let angle = (i as f32) * 2.0 * std::f32::consts::PI / ring_size as f32;
            let x = radius * angle.cos();
            let z = radius * angle.sin();
            // Add some vertical displacement based on ring number
            let y = (ring as f32 * 0.2 * angle.sin()).cos() * 0.3;
            
            commands.spawn((
                PbrBundle {
                    mesh: sphere_mesh.clone(),
                    material: node_material.clone(),
                    transform: Transform::from_xyz(x, y, z)
                        .with_scale(Vec3::splat(NODE_SCALE)), // Use NODE_SCALE
                    ..default()
                },
                NetworkNode {
                    depth: ring,
                },
                NodeHeat::default(),
            ));
        }
        
        nodes_created += ring_size;
        ring += 1;
    }
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

        // Zoom with mouse wheel - fix borrow issue
        for ev in mouse_wheel.read() {
            let forward = transform.forward();
            transform.translation += forward * ev.y * controller.zoom_speed;
        }
    }
} 