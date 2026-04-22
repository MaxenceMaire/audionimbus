use crate::audio::{SharedDirection, SpatialNode};
use crate::consts::{AMBISONICS_ORDER, IMPULSE_RESPONSE_DURATION, LISTENER_HEIGHT, ROOM_SIZE};
use audionimbus::bevy::{
    DefaultSimulationConfiguration, DirectSimulation, Listener, MainScene,
    ReflectionsReverbSimulation, Scene, Simulation, SimulationSharedInputs, Source, StaticMesh,
};
use audionimbus::{Context, ReflectionsSharedInputs};
use bevy::camera::visibility::NoFrustumCulling;
use bevy::camera_controller::free_camera::FreeCamera;
use bevy::prelude::*;
use bevy_seedling::edge::Connect;
use bevy_seedling::prelude::MainBus;

pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Startup,
            (
                set_shared_simulation_inputs,
                spawn_listener,
                spawn_orb,
                spawn_environment,
            )
                .chain(),
        );

        app.add_systems(Update, orbit);

        app.add_systems(
            PostUpdate,
            sync_audio_directions.after(TransformSystems::Propagate),
        );
    }
}

fn set_shared_simulation_inputs(mut shared_inputs: ResMut<SimulationSharedInputs>) {
    shared_inputs
        .0
        .set_reflections_shared_inputs(ReflectionsSharedInputs {
            num_rays: 16_384,
            num_bounces: 32,
            duration: IMPULSE_RESPONSE_DURATION,
            order: AMBISONICS_ORDER,
            irradiance_min_distance: 1.0,
        });
}

fn spawn_listener(mut commands: Commands, simulation: Res<Simulation>) {
    commands.spawn((
        Name::new("Listener"),
        Listener,
        Source::try_new(&simulation).expect("failed to create listener source"),
        Camera3d::default(),
        Transform::from_xyz(0.0, LISTENER_HEIGHT, 0.0),
        FreeCamera::default(),
    ));
}

fn spawn_environment(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    context: Res<Context>,
) {
    commands.insert_resource(GlobalAmbientLight {
        brightness: 1000.0,
        ..Default::default()
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
        base_color: Color::srgb(0.25, 0.25, 0.25),
        perceptual_roughness: 0.95,
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
            Visibility::default(),
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
    context: Res<Context>,
    simulation: Res<Simulation>,
    direct_simulation: Res<DirectSimulation<DefaultSimulationConfiguration>>,
    reflections_reverb_simulation: Res<ReflectionsReverbSimulation<DefaultSimulationConfiguration>>,
) {
    let direction = SharedDirection::new(Vec3::X);

    let mut entity_commands = commands.spawn((
        Name::new("Orb"),
        Source::try_new(&simulation).expect("failed to create orb source"),
        Mesh3d(meshes.add(Sphere::new(0.12).mesh().uv(32, 18))),
        MeshMaterial3d(materials.add(StandardMaterial {
            emissive: LinearRgba::rgb(60.0, 75.0, 100.0),
            base_color: Color::srgb(0.55, 0.68, 0.9),
            ..default()
        })),
        Transform::from_xyz(6.0, LISTENER_HEIGHT, 0.0),
        Orbital {
            angle: 0.0,
            speed: 2.0,
            radius: 6.0,
        },
        PointLight::default(),
        NoFrustumCulling,
    ));

    let entity = entity_commands.id();
    entity_commands.insert(SpatialNode::new(
        entity,
        context.clone(),
        direct_simulation.output(),
        reflections_reverb_simulation.output(),
        direction,
    ));
    entity_commands.connect(MainBus);
}

#[derive(Component)]
struct Orbital {
    angle: f32,
    speed: f32,
    radius: f32,
}

fn orbit(time: Res<Time>, mut orbs: Query<(&mut Transform, &mut Orbital)>) {
    for (mut transform, mut orbital) in &mut orbs {
        orbital.angle += orbital.speed * time.delta_secs();
        transform.translation = Vec3::new(
            orbital.angle.cos() * orbital.radius,
            LISTENER_HEIGHT,
            orbital.angle.sin() * orbital.radius,
        );
    }
}

fn sync_audio_directions(
    listeners: Query<&GlobalTransform, With<Listener>>,
    emitters: Query<(&GlobalTransform, &SpatialNode)>,
) {
    let listener = listeners.single().expect("more than one listener");
    let listener_transform = listener.compute_transform();

    for (emitter_transform, node) in &emitters {
        let world_offset = emitter_transform.translation() - listener.translation();
        let local_offset = listener_transform.rotation.inverse() * world_offset;
        let direction = local_offset.normalize_or_zero();

        node.direction.store(if direction.length_squared() > 0.0 {
            direction
        } else {
            Vec3::NEG_Z
        });
    }
}
