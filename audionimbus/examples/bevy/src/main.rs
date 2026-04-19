use audionimbus::bevy::geometry::Scene;
use audionimbus::bevy::{DebugPlugin, Listener, MainScene, Plugin, Simulation, Source, StaticMesh};
use bevy::camera_controller::free_camera::{FreeCamera, FreeCameraPlugin};
use bevy::light::GlobalAmbientLight;
use bevy::prelude::*;

const ROOM_SIZE: f32 = 20.0;

fn main() {
    let mut app = App::new();

    app.add_plugins((
        DefaultPlugins,
        FreeCameraPlugin,
        Plugin::default(),
        DebugPlugin::default(),
    ));

    app.add_systems(Startup, (spawn_listener, spawn_orb, spawn_environment));

    app.add_systems(Update, orbit);

    app.run();
}

fn spawn_listener(mut commands: Commands) {
    commands.spawn((
        Listener,
        Camera3d::default(),
        Transform::from_xyz(0.0, 1.8, 0.0),
        FreeCamera::default(),
    ));
}

fn spawn_environment(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    context: Res<audionimbus::Context>,
) {
    commands.insert_resource(GlobalAmbientLight {
        color: Color::WHITE,
        brightness: 1000.0,
        affects_lightmapped_meshes: true,
    });

    let floor_mesh = meshes.add(
        Mesh::from(Plane3d {
            normal: Dir3::Y,
            half_size: Vec2::splat(ROOM_SIZE / 2.0),
        })
        .with_generated_tangents()
        .unwrap(),
    );
    let material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.5, 0.5, 0.5),
        ..default()
    });

    let room_mesh = meshes.add(Mesh::from(Cuboid::from_size(Vec3::splat(ROOM_SIZE))));

    let scene: Scene = Scene::try_new(&context).expect("failed to create top-level scene");

    commands
        .spawn((
            Name::new("MainScene"),
            scene,
            MainScene,
            Transform::default(),
            InheritedVisibility::default(),
        ))
        .with_children(|parent| {
            parent.spawn((
                Name::new("Floor"),
                Mesh3d(floor_mesh),
                MeshMaterial3d(material),
            ));

            parent.spawn((
                Name::new("Room"),
                StaticMesh,
                Mesh3d(room_mesh),
                Transform::from_xyz(0.0, ROOM_SIZE * 0.5, 0.0),
            ));
        });
}

fn spawn_orb(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    simulation: Res<Simulation>,
) {
    commands.spawn((
        Source::try_new(&simulation).expect("failed to create source"),
        Mesh3d(meshes.add(Sphere::new(0.2).mesh().uv(32, 18))),
        MeshMaterial3d(materials.add(Color::srgb(0.0, 0.6, 1.0))),
        Transform::from_xyz(0.0, 1.0, 0.0),
        Orbital {
            angle: 0.0,
            speed: 2.0,
            radius: 5.0,
        },
    ));
}

#[derive(Component)]
struct Orbital {
    angle: f32,
    speed: f32, // rad/s
    radius: f32,
}

fn orbit(time: Res<Time>, mut orbs: Query<(&mut Transform, &mut Orbital)>) {
    for (mut transform, mut orbital) in &mut orbs {
        orbital.angle += orbital.speed * time.delta_secs();

        transform.translation = Vec3::new(
            orbital.angle.cos() * orbital.radius,
            transform.translation.y,
            orbital.angle.sin() * orbital.radius,
        );
    }
}
