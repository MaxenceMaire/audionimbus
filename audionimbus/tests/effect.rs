use audionimbus::*;

mod common;
use common::sine_wave;

#[test]
fn test_binaural_effect() {
    let frequency = 440.0;
    let amplitude = 0.5;
    let duration_secs = 0.1;
    let sample_rate = 48000;
    let sine_wave = sine_wave(frequency, amplitude, duration_secs, sample_rate);
    let input_buffer = AudioBuffer::try_with_data(&sine_wave).unwrap();
    let frame_size = sine_wave.len() as u32;

    let mut output_container = vec![0.0; 2 * input_buffer.num_samples() as usize];
    let output_buffer = AudioBuffer::try_with_data_and_settings(
        &mut output_container,
        AudioBufferSettings {
            num_channels: Some(2),
            ..Default::default()
        },
    )
    .unwrap();

    let context_settings = ContextSettings::default();
    let context = Context::try_new(&context_settings).unwrap();

    let audio_settings = AudioSettings {
        frame_size,
        ..Default::default()
    };

    let hrtf_settings = HrtfSettings::default();

    let hrtf = Hrtf::try_new(&context, &audio_settings, &hrtf_settings).unwrap();

    let binaural_effect_settings = effect::BinauralEffectSettings { hrtf: &hrtf };

    let mut binaural_effect =
        effect::BinauralEffect::try_new(&context, &audio_settings, &binaural_effect_settings)
            .unwrap();

    let binaural_effect_params = effect::BinauralEffectParams {
        direction: geometry::Direction::new(1.0, 1.0, 1.0),
        interpolation: HrtfInterpolation::Nearest,
        spatial_blend: 1.0,
        hrtf: &hrtf,
        peak_delays: None,
    };

    let _ = binaural_effect.apply(&binaural_effect_params, &input_buffer, &output_buffer);

    let mut interleaved =
        vec![0.0; (output_buffer.num_channels() * output_buffer.num_samples()) as usize];
    output_buffer.interleave(&context, &mut interleaved);
}

#[test]
fn test_ambisonics_encode_effect() {
    let frequency = 440.0;
    let amplitude = 0.5;
    let duration_secs = 0.1;
    let sample_rate = 48000;
    let sine_wave = sine_wave(frequency, amplitude, duration_secs, sample_rate);
    let input_buffer = AudioBuffer::try_with_data(&sine_wave).unwrap();
    let frame_size = sine_wave.len() as u32;

    let mut output_container = vec![0.0; input_buffer.num_samples() as usize];
    let output_buffer = AudioBuffer::try_with_data_and_settings(
        &mut output_container,
        AudioBufferSettings {
            num_channels: Some(1),
            ..Default::default()
        },
    )
    .unwrap();

    let context_settings = ContextSettings::default();
    let context = Context::try_new(&context_settings).unwrap();

    let audio_settings = AudioSettings {
        frame_size,
        ..Default::default()
    };

    let ambisonics_encode_effect_settings = effect::AmbisonicsEncodeEffectSettings { max_order: 0 };

    let mut ambisonics_encode_effect = effect::AmbisonicsEncodeEffect::try_new(
        &context,
        &audio_settings,
        &ambisonics_encode_effect_settings,
    )
    .unwrap();

    let ambisonics_encode_effect_params = effect::AmbisonicsEncodeEffectParams {
        direction: geometry::Direction::new(1.0, 1.0, 1.0),
        order: 0,
    };

    let _ = ambisonics_encode_effect.apply(
        &ambisonics_encode_effect_params,
        &input_buffer,
        &output_buffer,
    );

    let mut interleaved =
        vec![0.0; (output_buffer.num_channels() * output_buffer.num_samples()) as usize];
    output_buffer.interleave(&context, &mut interleaved);
}

#[test]
fn test_ambisonics_decode_effect() {
    let frequency = 440.0;
    let amplitude = 0.5;
    let duration_secs = 0.1;
    let sample_rate = 48000;
    let sine_wave = sine_wave(frequency, amplitude, duration_secs, sample_rate);
    let input_buffer = AudioBuffer::try_with_data(&sine_wave).unwrap();
    let frame_size = sine_wave.len() as u32;

    let mut output_container = vec![0.0; 2 * input_buffer.num_samples() as usize];
    let output_buffer = AudioBuffer::try_with_data_and_settings(
        &mut output_container,
        AudioBufferSettings {
            num_channels: Some(2),
            ..Default::default()
        },
    )
    .unwrap();

    let context_settings = ContextSettings::default();
    let context = Context::try_new(&context_settings).unwrap();

    let audio_settings = AudioSettings {
        frame_size,
        ..Default::default()
    };

    let hrtf_settings = HrtfSettings::default();

    let hrtf = Hrtf::try_new(&context, &audio_settings, &hrtf_settings).unwrap();

    let ambisonics_decode_effect_settings = effect::AmbisonicsDecodeEffectSettings {
        speaker_layout: SpeakerLayout::Mono,
        hrtf: &hrtf,
        max_order: 0,
    };

    let mut ambisonics_decode_effect = effect::AmbisonicsDecodeEffect::try_new(
        &context,
        &audio_settings,
        &ambisonics_decode_effect_settings,
    )
    .unwrap();

    let ambisonics_decode_effect_params = effect::AmbisonicsDecodeEffectParams {
        order: 0,
        hrtf: &hrtf,
        orientation: geometry::CoordinateSystem::default(),
        binaural: true,
    };

    let _ = ambisonics_decode_effect.apply(
        &ambisonics_decode_effect_params,
        &input_buffer,
        &output_buffer,
    );

    let mut interleaved =
        vec![0.0; (output_buffer.num_channels() * output_buffer.num_samples()) as usize];
    output_buffer.interleave(&context, &mut interleaved);
}

#[test]
fn test_direct_effect() {
    let frequency = 440.0;
    let amplitude = 0.5;
    let duration_secs = 0.1;
    let sample_rate = 48000;
    let sine_wave = sine_wave(frequency, amplitude, duration_secs, sample_rate);
    let input_buffer = AudioBuffer::try_with_data(&sine_wave).unwrap();
    let frame_size = sine_wave.len() as u32;

    let mut output_container = vec![0.0; 2 * input_buffer.num_samples() as usize];
    let output_buffer = AudioBuffer::try_with_data_and_settings(
        &mut output_container,
        AudioBufferSettings {
            num_channels: Some(2),
            ..Default::default()
        },
    )
    .unwrap();

    let context_settings = ContextSettings::default();
    let context = Context::try_new(&context_settings).unwrap();

    let audio_settings = AudioSettings {
        frame_size,
        ..Default::default()
    };

    let direct_effect_settings = effect::DirectEffectSettings { num_channels: 1 };

    let mut direct_effect =
        effect::DirectEffect::try_new(&context, &audio_settings, &direct_effect_settings).unwrap();

    let direct_effect_params = effect::DirectEffectParams {
        distance_attenuation: Some(0.6),
        air_absorption: Some(Equalizer([0.9, 0.7, 0.5])),
        directivity: Some(0.7),
        occlusion: Some(0.4),
        transmission: Some(Transmission::FrequencyIndependent(Equalizer([
            0.3, 0.2, 0.1,
        ]))),
    };

    let _ = direct_effect.apply(&direct_effect_params, &input_buffer, &output_buffer);

    let mut interleaved =
        vec![0.0; (output_buffer.num_channels() * output_buffer.num_samples()) as usize];
    output_buffer.interleave(&context, &mut interleaved);
}

#[test]
fn test_pathing() {
    let context = Context::default();

    let audio_settings = AudioSettings::default();

    let sampling_rate = 48000;
    let frame_size = 1024;
    let max_order = 1;

    let simulator = Simulator::builder(SceneParams::Default, sampling_rate, frame_size, max_order)
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

    let identifier = BakedDataIdentifier::Pathing {
        variation: BakedDataVariation::Dynamic,
    };

    let probe_array = ProbeArray::try_new(&context).unwrap();

    let mut probe_batch = ProbeBatch::try_new(&context).unwrap();
    probe_batch.add_probe_array(&probe_array);

    let path_bake_params = PathBakeParams {
        scene: &scene,
        probe_batch: &probe_batch,
        identifier: &identifier,
        num_samples: 1, // Trace a single ray to test if one probe can see another probe.
        visibility_range: 50.0, // Don't check visibility between probes that are > 50m apart.
        path_range: 100.0, // Don't store paths between probes that are > 100m apart.
        num_threads: 8,
        radius: f32::default(),
        threshold: f32::default(),
    };
    bake_path(&context, &path_bake_params, None);

    let source_settings = SourceSettings {
        flags: SimulationFlags::PATHING,
    };
    let mut source = Source::try_new(&simulator, &source_settings).unwrap();

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
            pathing_probes: &probe_batch,
            visibility_radius: 1.0,
            visibility_threshold: 10.0,
            visibility_range: 10.0,
            pathing_order: 1,
            enable_validation: true,
            find_alternate_paths: true,
            deviation: DeviationModel::default(),
        }),
    };
    source.set_inputs(SimulationFlags::PATHING, simulation_inputs);

    simulator.run_pathing();

    let path_effect_settings = PathEffectSettings {
        max_order: 1, // Render up to 1st order Ambisonic sound fields.
        spatialization: None,
    };
    let mut path_effect =
        PathEffect::try_new(&context, &audio_settings, &path_effect_settings).unwrap();

    let frequency = 440.0;
    let amplitude = 0.5;
    let duration_secs = 0.1;
    let sample_rate = 48000;
    let sine_wave = sine_wave(frequency, amplitude, duration_secs, sample_rate);
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

    let simulation_outputs = source.get_outputs(SimulationFlags::PATHING);
    let mut path_effect_params = simulation_outputs.pathing();
    path_effect_params.order = 1; // Render all 4 channels.

    let _ = path_effect.apply(&path_effect_params, &input_buffer, &output_buffer);

    let mut interleaved =
        vec![0.0; (output_buffer.num_channels() * output_buffer.num_samples()) as usize];
    output_buffer.interleave(&context, &mut interleaved);
}
