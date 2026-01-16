use audionimbus::*;

mod common;
use common::sine_wave;

#[test]
fn test_simulation() {
    let context_settings = ContextSettings::default();
    let context = Context::try_new(&context_settings).unwrap();

    let audio_settings = AudioSettings::default();

    let sampling_rate = 48000;
    let frame_size = 1024;
    let max_order = 1;

    let mut simulator =
        Simulator::builder(SceneParams::Default, sampling_rate, frame_size, max_order)
            .with_direct(DirectSimulationSettings {
                max_num_occlusion_samples: 4,
            })
            .with_reflections(ReflectionsSimulationSettings::Convolution {
                max_num_rays: 4096,
                num_diffuse_samples: 32,
                max_duration: 2.0,
                max_num_sources: 8,
                num_threads: 2,
            })
            .with_pathing(PathingSimulationSettings {
                num_visibility_samples: 4,
            })
            .try_build(&context)
            .unwrap();

    let scene_settings = SceneSettings::default();
    let scene = Scene::try_new(&context, &scene_settings).unwrap();
    simulator.set_scene(&scene);

    let source_settings = SourceSettings {
        flags: SimulationFlags::DIRECT | SimulationFlags::REFLECTIONS,
    };
    let mut source = Source::try_new(&simulator, &source_settings).unwrap();

    let pathing_probes = ProbeBatch::try_new(&context).unwrap();
    let simulation_inputs = SimulationInputs {
        source: CoordinateSystem {
            right: Vector3::new(1.0, 0.0, 0.0),
            up: Vector3::new(0.0, 1.0, 0.0),
            ahead: Vector3::new(0.0, 0.0, 1.0),
            origin: Vector3::new(0.0, 0.0, 0.0),
        },
        direct_simulation: Some(DirectSimulationParameters {
            distance_attenuation: Some(DistanceAttenuationModel::default()),
            air_absorption: Some(AirAbsorptionModel::default()),
            directivity: Some(Directivity::default()),
            occlusion: Some(Occlusion {
                transmission: Some(TransmissionParameters {
                    num_transmission_rays: 1,
                }),
                algorithm: OcclusionAlgorithm::Raycast,
            }),
        }),
        reflections_simulation: Some(ReflectionsSimulationParameters::Convolution {
            baked_data_identifier: None,
        }),
        pathing_simulation: Some(PathingSimulationParameters {
            pathing_probes: &pathing_probes,
            visibility_radius: 1.0,
            visibility_threshold: 10.0,
            visibility_range: 10.0,
            pathing_order: 1,
            enable_validation: true,
            find_alternate_paths: true,
            deviation: DeviationModel::default(),
        }),
    };
    source.set_inputs(SimulationFlags::DIRECT, simulation_inputs);

    simulator.add_source(&source);

    let simulation_shared_inputs = SimulationSharedInputs {
        listener: CoordinateSystem::default(),
        num_rays: 4096,
        num_bounces: 16,
        duration: 2.0,
        order: 1,
        irradiance_min_distance: 1.0,
        pathing_visualization_callback: None,
    };
    simulator.set_shared_inputs(
        SimulationFlags::DIRECT | SimulationFlags::REFLECTIONS,
        &simulation_shared_inputs,
    );

    simulator.commit();

    simulator.run_direct();
    simulator.run_reflections();
    let simulation_outputs =
        source.get_outputs(SimulationFlags::DIRECT | SimulationFlags::REFLECTIONS);

    let reflection_effect_settings = ReflectionEffectSettings::Convolution {
        impulse_response_size: 2 * sampling_rate, // 2.0f (IR duration) * 44100 (sampling rate)
        num_channels: 4,                          // 1st order Ambisonics
    };
    let mut reflection_effect =
        ReflectionEffect::try_new(&context, &audio_settings, &reflection_effect_settings).unwrap();

    let frequency = 440.0;
    let amplitude = 0.5;
    let duration_secs = 0.1;
    let sine_wave = sine_wave(frequency, amplitude, duration_secs, sampling_rate);
    // Must be mono.
    let input_buffer = AudioBuffer::try_with_data(&sine_wave).unwrap();

    // Must have 4 channels (1st order Ambisonics) for this example.
    let mut output_container = vec![0.0; 4 * input_buffer.num_samples() as usize];
    let output_buffer = AudioBuffer::try_with_data_and_settings(
        &mut output_container,
        AudioBufferSettings {
            num_channels: Some(4),
            ..Default::default()
        },
    )
    .unwrap();

    let mut reflection_effect_params = simulation_outputs.reflections();
    reflection_effect_params.num_channels = 4; // use all channels of the IR.
    reflection_effect_params.impulse_response_size = 2 * sampling_rate; // use the full duration of the IR.
    let _ = reflection_effect.apply(&reflection_effect_params, &input_buffer, &output_buffer);
}
