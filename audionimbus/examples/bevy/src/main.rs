use audionimbus::bevy::{
    DebugPlugin, DefaultSimulationConfiguration, DirectSimulation, Listener, MainScene,
    Plugin as AudioNimbusPlugin, ReflectionsReverbSimulation, Scene, Simulation,
    SimulationSharedInputs, Source, StaticMesh,
};
use audionimbus::wiring::{ReflectionsReverbOutput, SharedSimulationOutput};
use audionimbus::{
    AmbisonicsDecodeEffect, AmbisonicsDecodeEffectParams, AmbisonicsDecodeEffectSettings,
    AudioBuffer, AudioBufferSettings, AudioSettings, BinauralEffect, BinauralEffectParams,
    BinauralEffectSettings, Context, Convolution, ConvolutionSettings, CoordinateSystem,
    DirectEffect, DirectEffectParams, DirectEffectSettings, DirectSimulationSettings, Direction,
    Hrtf, HrtfInterpolation, HrtfSettings, ReflectionEffect, ReflectionEffectParams,
    ReflectionEffectSettings, ReflectionsSharedInputs, Rendering, SimulationSettings,
    SpeakerLayout, num_ambisonics_channels,
};
use bevy::camera::visibility::NoFrustumCulling;
use bevy::camera_controller::free_camera::{FreeCamera, FreeCameraPlugin};
use bevy::light::GlobalAmbientLight;
use bevy::prelude::*;
use bevy::transform::TransformSystems;
use bevy_seedling::firewheel::{
    StreamInfo,
    channel_config::{ChannelConfig, ChannelCount},
    cpal::{CpalBackend, CpalConfig, CpalOutputConfig},
    event::ProcEvents,
    node::{
        AudioNode, AudioNodeInfo, AudioNodeProcessor, ConstructProcessorContext, EmptyConfig,
        ProcBuffers, ProcExtra, ProcInfo, ProcessStatus,
    },
};
use bevy_seedling::prelude::{Connect, MainBus, RegisterNode, SeedlingPlugin};
use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

const SAMPLE_RATE: u32 = 48_000;
const FRAME_SIZE: u32 = 1024;
const ROOM_SIZE: f32 = 20.0;
const LISTENER_HEIGHT: f32 = 1.8;
const IMPULSE_RESPONSE_DURATION: f32 = 2.0;
const AMBISONICS_ORDER: u32 = 3;
const AMBISONICS_CHANNELS: u32 = num_ambisonics_channels(AMBISONICS_ORDER);
const DIRECT_GAIN: f32 = 1.0;
const REFLECTIONS_GAIN: f32 = 0.3;
const REVERB_GAIN: f32 = 0.1;
const TONE_FREQUENCY: f32 = 440.0;
const TONE_AMPLITUDE: f32 = 1.0;
const TONE_ON_DURATION: f32 = 0.3;
const TONE_OFF_DURATION: f32 = 0.9;

fn main() {
    let audio_settings = AudioSettings {
        sampling_rate: SAMPLE_RATE,
        frame_size: FRAME_SIZE,
    };
    let simulation_settings = SimulationSettings::new(&audio_settings)
        .with_direct(DirectSimulationSettings {
            max_num_occlusion_samples: 32,
        })
        .with_reflections(ConvolutionSettings {
            max_num_rays: 16_384,
            num_diffuse_samples: 128,
            max_duration: IMPULSE_RESPONSE_DURATION,
            max_num_sources: 2,
            num_threads: 2,
            max_order: AMBISONICS_ORDER,
        });

    let seedling = SeedlingPlugin::<CpalBackend> {
        stream_config: CpalConfig {
            output: CpalOutputConfig {
                desired_sample_rate: Some(SAMPLE_RATE),
                desired_block_frames: Some(FRAME_SIZE),
                ..Default::default()
            },
            input: None,
        },
        ..Default::default()
    };

    let mut app = App::new();

    app.add_plugins((
        DefaultPlugins,
        FreeCameraPlugin,
        seedling,
        AudioNimbusPlugin::new(simulation_settings),
        DebugPlugin::default(),
    ));

    app.register_simple_node::<SpatialNode>();

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

    app.run();
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

#[derive(Clone)]
struct SharedDirection {
    x: Arc<AtomicU32>,
    y: Arc<AtomicU32>,
    z: Arc<AtomicU32>,
}

impl SharedDirection {
    fn new(direction: Vec3) -> Self {
        Self {
            x: Arc::new(AtomicU32::new(direction.x.to_bits())),
            y: Arc::new(AtomicU32::new(direction.y.to_bits())),
            z: Arc::new(AtomicU32::new(direction.z.to_bits())),
        }
    }

    fn load(&self) -> Direction {
        Direction::new(
            f32::from_bits(self.x.load(Ordering::Relaxed)),
            f32::from_bits(self.y.load(Ordering::Relaxed)),
            f32::from_bits(self.z.load(Ordering::Relaxed)),
        )
    }

    fn store(&self, direction: Vec3) {
        self.x.store(direction.x.to_bits(), Ordering::Relaxed);
        self.y.store(direction.y.to_bits(), Ordering::Relaxed);
        self.z.store(direction.z.to_bits(), Ordering::Relaxed);
    }
}

#[derive(Component, Clone)]
struct SpatialNode {
    entity: Entity,
    context: Context,
    direct_output: SharedSimulationOutput<Vec<(Entity, DirectEffectParams)>>,
    reflections_reverb_output: SharedSimulationOutput<ReflectionsReverbOutput<Entity, Convolution>>,
    direction: SharedDirection,
}

impl SpatialNode {
    fn new(
        entity: Entity,
        context: Context,
        direct_output: SharedSimulationOutput<Vec<(Entity, DirectEffectParams)>>,
        reflections_reverb_output: SharedSimulationOutput<
            ReflectionsReverbOutput<Entity, Convolution>,
        >,
        direction: SharedDirection,
    ) -> Self {
        Self {
            entity,
            context,
            direct_output,
            reflections_reverb_output,
            direction,
        }
    }
}

impl AudioNode for SpatialNode {
    type Configuration = EmptyConfig;

    fn info(&self, _config: &Self::Configuration) -> AudioNodeInfo {
        AudioNodeInfo::new()
            .debug_name("audionimbus_spatial_node")
            .channel_config(ChannelConfig {
                num_inputs: ChannelCount::ZERO,
                num_outputs: ChannelCount::STEREO,
            })
    }

    fn construct_processor(
        &self,
        _config: &Self::Configuration,
        cx: ConstructProcessorContext,
    ) -> impl AudioNodeProcessor {
        SpatialProcessor::new(
            self.entity,
            self.context.clone(),
            self.direct_output.clone(),
            self.reflections_reverb_output.clone(),
            self.direction.clone(),
            cx.stream_info,
        )
    }
}

struct SpatialProcessor {
    entity: Entity,
    context: Context,
    direct_output: SharedSimulationOutput<Vec<(Entity, DirectEffectParams)>>,
    reflections_reverb_output: SharedSimulationOutput<ReflectionsReverbOutput<Entity, Convolution>>,
    direction: SharedDirection,
    hrtf: Hrtf,
    direct_path: DirectPath,
    reflections_path: ConvolutionPath,
    reverb_path: ConvolutionPath,
    tone: ToneBurst,
    dry_buffer: Vec<f32>,
    mix_buffer: Vec<f32>,
    frame_size: usize,
}

impl SpatialProcessor {
    fn new(
        entity: Entity,
        context: Context,
        direct_output: SharedSimulationOutput<Vec<(Entity, DirectEffectParams)>>,
        reflections_reverb_output: SharedSimulationOutput<
            ReflectionsReverbOutput<Entity, Convolution>,
        >,
        direction: SharedDirection,
        stream_info: &StreamInfo,
    ) -> Self {
        let audio_settings = audio_settings_from_stream(stream_info);
        let hrtf = Hrtf::try_new(&context, &audio_settings, &HrtfSettings::default())
            .expect("failed to create HRTF");
        let frame_size = audio_settings.frame_size as usize;

        Self {
            entity,
            context: context.clone(),
            direct_output,
            reflections_reverb_output,
            direction,
            direct_path: DirectPath::new(&context, &audio_settings, hrtf.clone()),
            reflections_path: ConvolutionPath::new(&context, &audio_settings, hrtf.clone()),
            reverb_path: ConvolutionPath::new(&context, &audio_settings, hrtf.clone()),
            hrtf,
            tone: ToneBurst::new(
                TONE_FREQUENCY,
                TONE_AMPLITUDE,
                TONE_ON_DURATION,
                TONE_OFF_DURATION,
                stream_info.sample_rate.get(),
            ),
            dry_buffer: vec![0.0; frame_size],
            mix_buffer: vec![0.0; frame_size * 2],
            frame_size,
        }
    }

    fn rebuild_for_stream(&mut self, stream_info: &StreamInfo) {
        *self = Self::new(
            self.entity,
            self.context.clone(),
            self.direct_output.clone(),
            self.reflections_reverb_output.clone(),
            self.direction.clone(),
            stream_info,
        );
    }
}

impl AudioNodeProcessor for SpatialProcessor {
    fn process(
        &mut self,
        info: &ProcInfo,
        ProcBuffers { outputs, .. }: ProcBuffers,
        _events: &mut ProcEvents,
        _extra: &mut ProcExtra,
    ) -> ProcessStatus {
        let frames = info.frames;
        self.dry_buffer[..frames].fill(0.0);
        self.tone.fill(&mut self.dry_buffer[..frames]);

        if frames < self.frame_size {
            self.dry_buffer[frames..].fill(0.0);
        }

        let dry_audio = AudioBuffer::try_with_data(self.dry_buffer.as_slice()).unwrap();

        let direct_snapshot = self.direct_output.load();
        let direct_params = direct_snapshot
            .iter()
            .find_map(|(entity, params)| (*entity == self.entity).then_some(params.clone()))
            .unwrap_or_default();

        let reflections_snapshot = self.reflections_reverb_output.load();
        let source_reflections = reflections_snapshot
            .sources
            .iter()
            .find_map(|(entity, params)| (*entity == self.entity).then_some(params));
        let listener_reverb = reflections_snapshot.listener.as_ref();

        self.direct_path.process(
            &dry_audio,
            &direct_params,
            self.direction.load(),
            self.hrtf.clone(),
        );
        self.reflections_path
            .process(&dry_audio, source_reflections, self.hrtf.clone());
        self.reverb_path
            .process(&dry_audio, listener_reverb, self.hrtf.clone());

        self.mix_buffer
            .copy_from_slice(self.direct_path.stereo_buffer.as_slice());
        scale_buffer(&mut self.mix_buffer, DIRECT_GAIN);
        mix_into(
            &mut self.mix_buffer,
            self.reflections_path.stereo_buffer.as_slice(),
            REFLECTIONS_GAIN,
        );
        mix_into(
            &mut self.mix_buffer,
            self.reverb_path.stereo_buffer.as_slice(),
            REVERB_GAIN,
        );

        outputs[0].copy_from_slice(&self.mix_buffer[..frames]);
        outputs[1].copy_from_slice(&self.mix_buffer[self.frame_size..self.frame_size + frames]);

        ProcessStatus::OutputsModified
    }

    fn new_stream(
        &mut self,
        stream_info: &StreamInfo,
        _context: &mut bevy_seedling::firewheel::node::ProcStreamCtx,
    ) {
        self.rebuild_for_stream(stream_info);
    }
}

struct DirectPath {
    direct_effect: DirectEffect,
    binaural_effect: BinauralEffect,
    mono_buffer: Vec<f32>,
    stereo_buffer: Vec<f32>,
}

impl DirectPath {
    fn new(context: &Context, audio_settings: &AudioSettings, hrtf: Hrtf) -> Self {
        let frame_size = audio_settings.frame_size as usize;

        let direct_effect = DirectEffect::try_new(
            context,
            audio_settings,
            &DirectEffectSettings { num_channels: 1 },
        )
        .expect("failed to create direct effect");

        let binaural_effect =
            BinauralEffect::try_new(context, audio_settings, &BinauralEffectSettings { hrtf })
                .expect("failed to create binaural effect");

        Self {
            direct_effect,
            binaural_effect,
            mono_buffer: vec![0.0; frame_size],
            stereo_buffer: vec![0.0; frame_size * 2],
        }
    }

    fn process(
        &mut self,
        dry_audio: &AudioBuffer<&[f32]>,
        params: &DirectEffectParams,
        direction: Direction,
        hrtf: Hrtf,
    ) {
        let mono = AudioBuffer::try_with_data(self.mono_buffer.as_mut_slice())
            .expect("failed to build mono direct buffer");
        self.direct_effect
            .apply(params, dry_audio, &mono)
            .expect("failed to apply direct effect");

        let stereo = AudioBuffer::try_with_data_and_settings(
            self.stereo_buffer.as_mut_slice(),
            AudioBufferSettings::with_num_channels(2),
        )
        .expect("failed to build stereo direct buffer");

        self.binaural_effect
            .apply(
                &BinauralEffectParams {
                    direction,
                    interpolation: HrtfInterpolation::Bilinear,
                    spatial_blend: 1.0,
                    hrtf,
                    peak_delays: None,
                },
                &mono,
                &stereo,
            )
            .expect("failed to apply binaural effect");
    }
}

struct ConvolutionPath {
    reflection_effect: ReflectionEffect<Convolution>,
    decode_effect: AmbisonicsDecodeEffect,
    ambisonics_buffer: Vec<f32>,
    stereo_buffer: Vec<f32>,
}

impl ConvolutionPath {
    fn new(context: &Context, audio_settings: &AudioSettings, hrtf: Hrtf) -> Self {
        let frame_size = audio_settings.frame_size as usize;

        let reflection_effect = ReflectionEffect::<Convolution>::try_new(
            context,
            audio_settings,
            &ReflectionEffectSettings {
                impulse_response_size: (IMPULSE_RESPONSE_DURATION
                    * audio_settings.sampling_rate as f32)
                    as u32,
                num_channels: AMBISONICS_CHANNELS,
            },
        )
        .expect("failed to create reflection effect");

        let decode_effect = AmbisonicsDecodeEffect::try_new(
            context,
            audio_settings,
            &AmbisonicsDecodeEffectSettings {
                speaker_layout: SpeakerLayout::Stereo,
                hrtf,
                max_order: AMBISONICS_ORDER,
                rendering: Rendering::Binaural,
            },
        )
        .expect("failed to create ambisonics decode effect");

        Self {
            reflection_effect,
            decode_effect,
            ambisonics_buffer: vec![0.0; frame_size * AMBISONICS_CHANNELS as usize],
            stereo_buffer: vec![0.0; frame_size * 2],
        }
    }

    fn process(
        &mut self,
        dry_audio: &AudioBuffer<&[f32]>,
        params: Option<&ReflectionEffectParams<Convolution>>,
        hrtf: Hrtf,
    ) {
        let Some(params) = params else {
            self.stereo_buffer.fill(0.0);
            return;
        };

        let ambisonics = AudioBuffer::try_with_data_and_settings(
            self.ambisonics_buffer.as_mut_slice(),
            AudioBufferSettings::with_num_channels(AMBISONICS_CHANNELS),
        )
        .expect("failed to build ambisonics buffer");
        self.reflection_effect
            .apply(params, dry_audio, &ambisonics)
            .expect("failed to apply reflection effect");

        let stereo = AudioBuffer::try_with_data_and_settings(
            self.stereo_buffer.as_mut_slice(),
            AudioBufferSettings::with_num_channels(2),
        )
        .expect("failed to build stereo reflection buffer");
        self.decode_effect
            .apply(
                &AmbisonicsDecodeEffectParams {
                    order: AMBISONICS_ORDER,
                    hrtf,
                    orientation: CoordinateSystem::default(),
                },
                &ambisonics,
                &stereo,
            )
            .expect("failed to decode ambisonics reflections");
    }
}

struct ToneBurst {
    phase_increment: f32,
    amplitude: f32,
    phase: f32,
    cycle_samples: u32,
    on_samples: u32,
    cursor: u32,
    fade_samples: u32,
}

impl ToneBurst {
    fn new(
        frequency: f32,
        amplitude: f32,
        tone_duration: f32,
        pause_duration: f32,
        sample_rate: u32,
    ) -> Self {
        let phase_increment = std::f32::consts::TAU * frequency / sample_rate as f32;
        let on_samples = (tone_duration * sample_rate as f32) as u32;
        let off_samples = (pause_duration * sample_rate as f32) as u32;
        let fade_samples = (0.02 * sample_rate as f32) as u32;

        Self {
            phase_increment,
            amplitude,
            phase: 0.0,
            cycle_samples: on_samples + off_samples,
            on_samples,
            cursor: 0,
            fade_samples,
        }
    }

    fn fill(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            if self.cursor < self.on_samples {
                let remaining = self.on_samples - self.cursor;
                let fade_gain = if self.cursor < self.fade_samples {
                    self.cursor as f32 / self.fade_samples as f32
                } else if remaining <= self.fade_samples {
                    remaining as f32 / self.fade_samples as f32
                } else {
                    1.0
                };

                *sample = self.amplitude * self.phase.sin() * fade_gain;
                self.phase = (self.phase + self.phase_increment).rem_euclid(std::f32::consts::TAU);
            } else {
                *sample = 0.0;
            }
            self.cursor = (self.cursor + 1) % self.cycle_samples;
        }
    }
}

fn audio_settings_from_stream(stream_info: &StreamInfo) -> AudioSettings {
    AudioSettings {
        sampling_rate: stream_info.sample_rate.get(),
        frame_size: stream_info.max_block_frames.get(),
    }
}

fn mix_into(destination: &mut [f32], source: &[f32], gain: f32) {
    for (destination_sample, source_sample) in destination.iter_mut().zip(source.iter()) {
        *destination_sample += source_sample * gain;
    }
}

fn scale_buffer(buffer: &mut [f32], gain: f32) {
    for sample in buffer {
        *sample *= gain;
    }
}
