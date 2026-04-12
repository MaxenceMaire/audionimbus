use audionimbus::bevy::*;
use audionimbus::{
    AudioSettings, ConvolutionSettings, DirectSimulationSettings, SimulationSettings,
};
use bevy::camera_controller::free_camera::{FreeCamera, FreeCameraPlugin};
use bevy::light::GlobalAmbientLight;
use bevy::prelude::*;

fn main() {
    let mut app = App::new();

    app.add_plugins((DefaultPlugins, FreeCameraPlugin));

    app.add_plugins(audionimbus::bevy::Plugin::new(
        SimulationSettings::new(&AudioSettings::default())
            .with_direct(DirectSimulationSettings {
                max_num_occlusion_samples: 4,
            })
            .with_reflections(ConvolutionSettings {
                max_num_rays: 128,
                num_diffuse_samples: 8,
                max_duration: 0.5,
                max_num_sources: 8,
                num_threads: 1,
                max_order: 1,
            }),
    ));

    app.insert_resource(GlobalAmbientLight {
        color: Color::WHITE,
        brightness: 1000.0,
        affects_lightmapped_meshes: true,
    });

    app.add_systems(Startup, (spawn_listener, spawn_orb, spawn_environment));

    app.add_systems(Update, orbit);

    app.run();
}

fn spawn_listener(mut commands: Commands) {
    commands.spawn((
        Listener,
        Camera3d::default(),
        Transform::from_xyz(0.0, 2.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        FreeCamera::default(),
    ));
}

fn spawn_environment(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let floor_mesh = meshes.add(
        Mesh::from(Plane3d {
            normal: Dir3::Y,
            half_size: Vec2::splat(20.0),
        })
        .with_generated_tangents()
        .unwrap(),
    );
    let material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.5, 0.5, 0.5),
        ..default()
    });
    commands.spawn((Mesh3d(floor_mesh), MeshMaterial3d(material)));
}

fn spawn_orb(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
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
