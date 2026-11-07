#[path = "src/consts.rs"]
mod consts;
#[path = "src/dsp.rs"]
mod dsp;
#[path = "src/output.rs"]
mod output;
#[path = "src/room.rs"]
mod room;

use audionimbus::r#static as audio;
use audionimbus::wiring::{DirectFrame, ReflectionsReverbFrame};
use audionimbus::*;
use consts::*;
use output::{FRAME_SIZE, NUM_CHANNELS, SAMPLE_RATE};
use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};
use std::time::Duration;

fn main() {
    let reflections_settings = ConvolutionSettings {
        max_num_rays: 16_384,
        num_diffuse_samples: 128,
        max_duration: IMPULSE_RESPONSE_DURATION,
        max_num_sources: 2,
        num_threads: 2,
        max_order: AMBISONICS_ORDER,
    };

    let context = Context::default();
    let audio_settings = AudioSettings {
        sampling_rate: SAMPLE_RATE,
        frame_size: FRAME_SIZE,
    };
    let hrtf = Hrtf::try_new(&context, &audio_settings, &HrtfSettings::default()).unwrap();
    let mut scene = audio::Scene::try_new(&context).unwrap();
    let (vertices, triangles) = room::room(30.0, 10.0, 30.0);
    let mesh = audio::StaticMesh::try_new(
        &scene,
        &StaticMeshSettings {
            vertices: &vertices,
            triangles: &triangles,
            material_indices: &[0; 12],
            materials: &[Material {
                absorption: [0.08, 0.10, 0.12],
                scattering: 0.25,
                transmission: [0.0; 3],
            }],
        },
    )
    .unwrap();
    scene.add_static_mesh(mesh);
    scene.commit();

    let settings = audio::SimulationSettings::new(&audio_settings)
        .with_direct(DirectSimulationSettings {
            max_num_occlusion_samples: 32,
        })
        .with_reflections(reflections_settings);
    let mut simulator = audio::Simulator::try_new(&context, &settings).unwrap();
    simulator.set_scene(&scene);
    let source = audio::Source::try_new(&simulator).unwrap();
    let listener_source =
        audio::Source::<(), Reflections, (), Convolution>::try_new_subset(&simulator).unwrap();
    simulator.add_source(&source);
    simulator.add_source(&listener_source);
    simulator.commit();

    let mut simulation = audio::Simulation::new::<()>(simulator);
    simulation.request_simulator_commit();
    let on_error = |error| eprintln!("{error}");
    let direct_simulation = simulation.spawn_direct(on_error);
    let reflections_reverb_simulation = simulation.spawn_reflections_reverb::<(), ()>(on_error);
    let direct_output = direct_simulation.output();
    let reflections_reverb_output = reflections_reverb_simulation.output();
    let source_angle = Arc::new(AtomicU32::new(0_f32.to_bits()));
    let source_angle_for_thread = source_angle.clone();

    std::thread::spawn(move || {
        let listener = CoordinateSystem {
            origin: Point::new(0.0, LISTENER_HEIGHT, 0.0),
            ahead: Point::new(0.0, 0.0, -1.0),
            up: Point::new(0.0, 1.0, 0.0),
            right: Point::new(1.0, 0.0, 0.0),
        };
        let shared_inputs = audio::SimulationSharedInputs::new(listener)
            .with_direct()
            .with_reflections(ReflectionsSharedInputs {
                num_rays: 16_384,
                num_bounces: 32,
                duration: IMPULSE_RESPONSE_DURATION,
                order: AMBISONICS_ORDER,
                irradiance_min_distance: 1.0,
            });
        let listener = audio::SourceWithInputs {
            source: listener_source,
            simulation_inputs: audio::SimulationInputs {
                source: listener,
                parameters: audio::SimulationParameters::new()
                    .with_reflections(ConvolutionParameters::default()),
            },
        };

        let frame_duration = Duration::from_secs_f32(FRAME_SIZE as f32 / SAMPLE_RATE as f32);

        loop {
            let previous_angle = f32::from_bits(source_angle_for_thread.load(Ordering::Relaxed));
            let angle =
                (previous_angle + frame_duration.as_secs_f32()).rem_euclid(std::f32::consts::TAU);
            source_angle_for_thread.store(angle.to_bits(), Ordering::Relaxed);
            let direct_parameters = DirectSimulationParameters::new().with_occlusion(
                Occlusion::new(OcclusionAlgorithm::Raycast).with_transmission(
                    TransmissionParameters {
                        num_transmission_rays: 1,
                    },
                ),
            );
            simulation.update_sources(|sources| {
                sources.push((
                    (),
                    audio::SourceWithInputs {
                        source: source.clone(),
                        simulation_inputs: audio::SimulationInputs {
                            source: CoordinateSystem {
                                origin: Point::new(
                                    angle.cos() * 2.0,
                                    LISTENER_HEIGHT,
                                    angle.sin() * 2.0,
                                ),
                                ..Default::default()
                            },
                            parameters: audio::SimulationParameters::new()
                                .with_direct(direct_parameters)
                                .with_reflections(ConvolutionParameters::default()),
                        },
                    },
                ));
            });
            direct_simulation.set_input(DirectFrame {
                sources: simulation.sources.clone(),
                shared_inputs: shared_inputs.clone(),
            });
            reflections_reverb_simulation.set_input(ReflectionsReverbFrame {
                sources: simulation.sources.clone(),
                listener: Some(listener.clone()),
                shared_inputs: shared_inputs.clone(),
            });
            std::thread::sleep(frame_duration);
        }
    });

    let mut direct_effect = DirectEffect::try_new(
        &context,
        &audio_settings,
        &DirectEffectSettings { num_channels: 1 },
    )
    .unwrap();
    let mut binaural_effect = BinauralEffect::try_new(
        &context,
        &audio_settings,
        &BinauralEffectSettings { hrtf: hrtf.clone() },
    )
    .unwrap();
    let mut reflection_effects = Vec::new();
    let mut decode_effects = Vec::new();
    for _ in 0..2 {
        reflection_effects.push(
            audio::ReflectionEffect::<Convolution>::try_new(
                &context,
                &audio_settings,
                &ReflectionEffectSettings {
                    impulse_response_size: (IMPULSE_RESPONSE_DURATION * SAMPLE_RATE as f32) as u32,
                    num_channels: AMBISONICS_CHANNELS,
                },
            )
            .unwrap(),
        );
        decode_effects.push(
            AmbisonicsDecodeEffect::try_new(
                &context,
                &audio_settings,
                &AmbisonicsDecodeEffectSettings {
                    speaker_layout: SpeakerLayout::Stereo,
                    hrtf: hrtf.clone(),
                    max_order: AMBISONICS_ORDER,
                    rendering: Rendering::Binaural,
                },
            )
            .unwrap(),
        );
    }

    let mut tone_burst = dsp::ToneBurst::new(440.0, 0.3, 0.3, 0.9, SAMPLE_RATE);
    let mut dry_samples = vec![0.0; FRAME_SIZE as usize];
    let mut mono_samples = vec![0.0; FRAME_SIZE as usize];
    let mut direct_samples = vec![0.0; (FRAME_SIZE * NUM_CHANNELS) as usize];
    let mut ambisonics_samples = vec![0.0; (FRAME_SIZE * AMBISONICS_CHANNELS) as usize];
    let mut reflection_samples = [
        vec![0.0; (FRAME_SIZE * NUM_CHANNELS) as usize],
        vec![0.0; (FRAME_SIZE * NUM_CHANNELS) as usize],
    ];
    let (device, stream_config) = output::default_device();
    let _stream = output::start_output_stream(&device, &stream_config, move |output, _| {
        tone_burst.fill(&mut dry_samples);
        let dry_audio = AudioBufferRef::try_from(dry_samples.as_slice()).unwrap();
        let direct_snapshot = direct_output.load();
        let reflections_snapshot = reflections_reverb_output.load();
        let direct_params = direct_snapshot.get(&()).cloned().unwrap_or_default();
        let angle = f32::from_bits(source_angle.load(Ordering::Relaxed));

        let mut mono_audio = AudioBufferMut::try_from(mono_samples.as_mut_slice()).unwrap();
        direct_effect
            .apply(&direct_params, &dry_audio, &mut mono_audio)
            .unwrap();
        let mut stereo_audio =
            AudioBufferMut::try_new(&mut direct_samples, NUM_CHANNELS as usize).unwrap();
        binaural_effect
            .apply(
                &BinauralEffectParams {
                    direction: Direction::new(angle.cos(), 0.0, angle.sin()),
                    interpolation: HrtfInterpolation::Bilinear,
                    spatial_blend: 1.0,
                    hrtf: hrtf.clone(),
                    peak_delays: None,
                },
                &mono_audio,
                &mut stereo_audio,
            )
            .unwrap();

        let reflection_params = [
            reflections_snapshot.sources.get(&()),
            reflections_snapshot.listener.as_ref(),
        ];
        for (path, params) in reflection_params.into_iter().enumerate() {
            let Some(params) = params else {
                reflection_samples[path].fill(0.0);
                continue;
            };
            let mut ambisonics_audio =
                AudioBufferMut::try_new(&mut ambisonics_samples, AMBISONICS_CHANNELS as usize)
                    .unwrap();
            reflection_effects[path]
                .apply(params, &dry_audio, &mut ambisonics_audio)
                .unwrap();
            let mut stereo_audio =
                AudioBufferMut::try_new(&mut reflection_samples[path], NUM_CHANNELS as usize)
                    .unwrap();
            decode_effects[path]
                .apply(
                    &AmbisonicsDecodeEffectParams {
                        order: AMBISONICS_ORDER,
                        hrtf: hrtf.clone(),
                        orientation: CoordinateSystem::default(),
                    },
                    &ambisonics_audio,
                    &mut stereo_audio,
                )
                .unwrap();
        }

        for sample in &mut direct_samples {
            *sample *= DIRECT_GAIN;
        }

        dsp::mix_into(
            &mut direct_samples,
            &reflection_samples[0],
            REFLECTIONS_GAIN,
        );
        dsp::mix_into(&mut direct_samples, &reflection_samples[1], REVERB_GAIN);
        AudioBufferRef::try_new(&direct_samples, NUM_CHANNELS as usize)
            .unwrap()
            .interleave(&context, output)
            .unwrap();
    });

    std::thread::park();
}
