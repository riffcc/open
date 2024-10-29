use bevy::prelude::*;
use bevy::math::Vec3;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, (rotate_camera, pulse_nodes))
        .run();
}

#[derive(Component)]
struct NetworkNode {
    depth: usize,
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Camera
    commands.spawn(Camera3dBundle {
        transform: Transform::from_xyz(-2.0, 2.5, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });

    // Light
    commands.spawn(PointLightBundle {
        point_light: PointLight {
            intensity: 1500.0,
            shadows_enabled: true,
            ..default()
        },
        transform: Transform::from_xyz(4.0, 8.0, 4.0),
        ..default()
    });

    // Origin node (green sphere)
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Mesh::from(Sphere::default())),
            material: materials.add(StandardMaterial {
                base_color: Color::srgb(0.0, 1.0, 0.0),
                emissive: Color::srgb(0.0, 2.0, 0.0).into(),
                ..default()
            }),
            transform: Transform::from_xyz(0.0, 0.0, 0.0),
            ..default()
        },
        NetworkNode {
            depth: 0,
        },
    ));

    // Add a few test nodes in a hexagonal pattern
    let radius = 1.0;
    for i in 0..6 {
        let angle = i as f32 * std::f32::consts::PI / 3.0;
        let x = radius * angle.cos();
        let z = radius * angle.sin();
        
        commands.spawn((
            PbrBundle {
                mesh: meshes.add(Mesh::from(Sphere::default())),
                material: materials.add(StandardMaterial {
                    base_color: Color::srgb(0.5, 0.0, 0.5),
                    emissive: Color::srgb(0.5, 0.0, 0.5).into(),
                    ..default()
                }),
                transform: Transform::from_xyz(x, 0.0, z),
                ..default()
            },
            NetworkNode {
                depth: 1,
            },
        ));
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

fn pulse_nodes(
    time: Res<Time>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    query: Query<(&NetworkNode, &Handle<StandardMaterial>)>,
) {
    let pulse = (time.elapsed_seconds() * 2.0).sin() * 0.5 + 0.5;
    
    for (node, material_handle) in &query {
        if let Some(material) = materials.get_mut(material_handle) {
            let intensity = if node.depth == 0 { 2.0 } else { 0.5 };
            material.emissive = Color::srgba(
                0.5 * pulse * intensity,
                0.0,
                0.5 * pulse * intensity,
                1.0
            ).into();
        }
    }
} 