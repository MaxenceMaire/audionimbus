const AUDIO_BUFFER_SINE_WAVE_440HZ_1S: &[u8] = include_bytes!("sine_wave_440Hz_1s.raw");

#[test]
fn test_initialization() {
    let context_settings = audionimbus::ContextSettings::default();

    let context_result = audionimbus::Context::try_new(&context_settings);
    assert!(context_result.is_ok());
}

#[test]
fn test_load_hrtf_default() {
    let context_settings = audionimbus::ContextSettings::default();
    let context = audionimbus::Context::try_new(&context_settings).unwrap();

    let audio_settings = audionimbus::AudioSettings::default();

    let hrtf_settings = audionimbus::HrtfSettings::default();

    let hrtf_result = audionimbus::Hrtf::try_new(&context, &audio_settings, &hrtf_settings);
    assert!(hrtf_result.is_ok());
}

// TODO: implement test.
#[test]
fn test_load_hrtf_sofa_filename() {}

// TODO: implement test.
#[test]
fn test_load_hrtf_sofa_buffer() {}

#[test]
fn test_binaural_effect() {
    let mut input_buffer =
        audionimbus::AudioBuffer::from(raw_to_deinterleaved(AUDIO_BUFFER_SINE_WAVE_440HZ_1S, 1));

    let mut output_buffer =
        audionimbus::AudioBuffer::with_num_channels_and_num_samples(2, input_buffer.num_samples);

    let context_settings = audionimbus::ContextSettings::default();
    let context = audionimbus::Context::try_new(&context_settings).unwrap();

    let audio_settings = audionimbus::AudioSettings {
        frame_size: input_buffer.data.len(),
        ..Default::default()
    };

    let hrtf_settings = audionimbus::HrtfSettings::default();

    let hrtf = audionimbus::Hrtf::try_new(&context, &audio_settings, &hrtf_settings).unwrap();

    let binaural_effect_settings = audionimbus::effect::BinauralEffectSettings { hrtf: &hrtf };

    let binaural_effect = audionimbus::effect::BinauralEffect::try_new(
        &context,
        &audio_settings,
        &binaural_effect_settings,
    )
    .unwrap();

    let binaural_effect_params = audionimbus::effect::BinauralEffectParams {
        direction: audionimbus::geometry::Direction::new(1.0, 1.0, 1.0),
        interpolation: audionimbus::HrtfInterpolation::Nearest,
        spatial_blend: 1.0,
        hrtf: &hrtf,
        peak_delays: None,
    };

    let _ = binaural_effect.apply(
        &binaural_effect_params,
        &mut input_buffer,
        &mut output_buffer,
    );

    let _ = output_buffer.interleave(&context);
}

fn raw_to_deinterleaved(
    raw_data: &[u8],
    num_channels: usize,
) -> audionimbus::DeinterleavedChannelSamples {
    let num_samples = raw_data.len() / 4; // Each sample is 4 bytes (32-bit float).

    let mut channels: audionimbus::DeinterleavedChannelSamples =
        vec![Vec::with_capacity(num_samples / num_channels); num_channels];

    for i in 0..num_samples {
        let sample_bytes = &raw_data[i * 4..(i + 1) * 4];

        let sample = f32::from_le_bytes(sample_bytes.try_into().unwrap());

        // Determine which channel the sample belongs to and add it.
        let channel_index = i % num_channels;
        channels[channel_index].push(sample);
    }

    channels
}

#[test]
fn test_ambisonics_encode_effect() {
    let mut input_buffer =
        audionimbus::AudioBuffer::from(raw_to_deinterleaved(AUDIO_BUFFER_SINE_WAVE_440HZ_1S, 1));

    let mut output_buffer =
        audionimbus::AudioBuffer::with_num_channels_and_num_samples(2, input_buffer.num_samples);

    let context_settings = audionimbus::ContextSettings::default();
    let context = audionimbus::Context::try_new(&context_settings).unwrap();

    let audio_settings = audionimbus::AudioSettings {
        frame_size: input_buffer.data.len(),
        ..Default::default()
    };

    let ambisonics_encode_effect_settings =
        audionimbus::effect::AmbisonicsEncodeEffectSettings { max_order: 0 };

    let ambisonics_encode_effect = audionimbus::effect::AmbisonicsEncodeEffect::try_new(
        &context,
        &audio_settings,
        &ambisonics_encode_effect_settings,
    )
    .unwrap();

    let ambisonics_encode_effect_params = audionimbus::effect::AmbisonicsEncodeEffectParams {
        direction: audionimbus::geometry::Direction::new(1.0, 1.0, 1.0),
        order: 0,
    };

    let _ = ambisonics_encode_effect.apply(
        &ambisonics_encode_effect_params,
        &mut input_buffer,
        &mut output_buffer,
    );

    let _ = output_buffer.interleave(&context);
}

#[test]
fn test_ambisonics_decode_effect() {
    let mut input_buffer =
        audionimbus::AudioBuffer::from(raw_to_deinterleaved(AUDIO_BUFFER_SINE_WAVE_440HZ_1S, 1));

    let mut output_buffer =
        audionimbus::AudioBuffer::with_num_channels_and_num_samples(2, input_buffer.num_samples);

    let context_settings = audionimbus::ContextSettings::default();
    let context = audionimbus::Context::try_new(&context_settings).unwrap();

    let audio_settings = audionimbus::AudioSettings {
        frame_size: input_buffer.data.len(),
        ..Default::default()
    };

    let hrtf_settings = audionimbus::HrtfSettings::default();

    let hrtf = audionimbus::Hrtf::try_new(&context, &audio_settings, &hrtf_settings).unwrap();

    let ambisonics_decode_effect_settings = audionimbus::effect::AmbisonicsDecodeEffectSettings {
        speaker_layout: audionimbus::SpeakerLayout::Mono,
        hrtf: &hrtf,
        max_order: 0,
    };

    let ambisonics_decode_effect = audionimbus::effect::AmbisonicsDecodeEffect::try_new(
        &context,
        &audio_settings,
        &ambisonics_decode_effect_settings,
    )
    .unwrap();

    let ambisonics_decode_effect_params = audionimbus::effect::AmbisonicsDecodeEffectParams {
        order: 0,
        hrtf: &hrtf,
        orientation: audionimbus::geometry::CoordinateSystem::default(),
        binaural: true,
    };

    let _ = ambisonics_decode_effect.apply(
        &ambisonics_decode_effect_params,
        &mut input_buffer,
        &mut output_buffer,
    );

    let _ = output_buffer.interleave(&context);
}

#[test]
fn test_direct_effect() {
    let mut input_buffer =
        audionimbus::AudioBuffer::from(raw_to_deinterleaved(AUDIO_BUFFER_SINE_WAVE_440HZ_1S, 1));

    let mut output_buffer =
        audionimbus::AudioBuffer::with_num_channels_and_num_samples(2, input_buffer.num_samples);

    let context_settings = audionimbus::ContextSettings::default();
    let context = audionimbus::Context::try_new(&context_settings).unwrap();

    let audio_settings = audionimbus::AudioSettings {
        frame_size: input_buffer.data.len(),
        ..Default::default()
    };

    let direct_effect_settings = audionimbus::effect::DirectEffectSettings { num_channels: 1 };

    let direct_effect = audionimbus::effect::DirectEffect::try_new(
        &context,
        &audio_settings,
        &direct_effect_settings,
    )
    .unwrap();

    let direct_effect_params = audionimbus::effect::DirectEffectParams {
        distance_attenuation: Some(0.6),
        air_absorption: Some(audionimbus::Equalizer([0.9, 0.7, 0.5])),
        directivity: Some(0.7),
        occlusion: Some(0.4),
        transmission: Some(audionimbus::Transmission::FrequencyIndependent(
            audionimbus::Equalizer([0.3, 0.2, 0.1]),
        )),
    };

    let _ = direct_effect.apply(&direct_effect_params, &mut input_buffer, &mut output_buffer);

    let _ = output_buffer.interleave(&context);
}

#[test]
fn test_distance_attenuation() {
    let context_settings = audionimbus::ContextSettings::default();
    let context = audionimbus::Context::try_new(&context_settings).unwrap();

    let source = audionimbus::Point::new(1.0, 1.0, 1.0);
    let listener = audionimbus::Point::new(0.0, 0.0, 0.0);

    let distance_attenuation_model = audionimbus::DistanceAttenuationModel::default();

    let distance_attenuation = audionimbus::distance_attenuation(
        &context,
        &source,
        &listener,
        &distance_attenuation_model,
    );

    assert_eq!(distance_attenuation, 0.57735026);
}

#[test]
fn test_air_absorption() {
    let context_settings = audionimbus::ContextSettings::default();
    let context = audionimbus::Context::try_new(&context_settings).unwrap();

    let source = audionimbus::Point::new(1.0, 1.0, 1.0);
    let listener = audionimbus::Point::new(0.0, 0.0, 0.0);

    let air_absorption_model = audionimbus::AirAbsorptionModel::default();

    let air_absorption =
        audionimbus::air_absorption(&context, &source, &listener, &air_absorption_model);

    assert_eq!(air_absorption, [0.99965364, 0.9970598, 0.96896833]);
}

#[test]
fn test_directivity_attenuation() {
    let context_settings = audionimbus::ContextSettings::default();
    let context = audionimbus::Context::try_new(&context_settings).unwrap();

    let source = audionimbus::CoordinateSystem::default();
    let listener = audionimbus::Point::new(0.0, 0.0, 0.0);

    let directivity = audionimbus::Directivity::default();

    let directivity_attenuation =
        audionimbus::directivity_attenuation(&context, &source, &listener, &directivity);

    assert_eq!(directivity_attenuation, 0.70710677);
}

#[test]
fn test_scene() {
    let context_settings = audionimbus::ContextSettings::default();
    let context = audionimbus::Context::try_new(&context_settings).unwrap();

    let scene_settings = audionimbus::SceneSettings::default();

    let scene_result = audionimbus::Scene::try_new(&context, &scene_settings);

    assert!(scene_result.is_ok());
}

#[test]
fn test_static_mesh() {
    let context_settings = audionimbus::ContextSettings::default();
    let context = audionimbus::Context::try_new(&context_settings).unwrap();

    let scene_settings = audionimbus::SceneSettings::default();
    let scene = audionimbus::Scene::try_new(&context, &scene_settings).unwrap();

    // Four vertices of a unit square in the x-y plane.
    let vertices = vec![
        audionimbus::geometry::Point::new(0.0, 0.0, 0.0),
        audionimbus::geometry::Point::new(1.0, 0.0, 0.0),
        audionimbus::geometry::Point::new(1.0, 1.0, 0.0),
        audionimbus::geometry::Point::new(0.0, 1.0, 0.0),
    ];

    let triangles = vec![
        audionimbus::geometry::Triangle::new(0, 1, 2),
        audionimbus::geometry::Triangle::new(0, 2, 2),
    ];

    let materials = vec![audionimbus::geometry::Material {
        absorption: [0.1, 0.1, 0.1],
        scattering: 0.5,
        transmission: [0.2, 0.2, 0.2],
    }];

    // Both triangles use the same material.
    let material_indices = vec![0, 0];

    let static_mesh_settings = audionimbus::geometry::StaticMeshSettings {
        num_vertices: vertices.len(),
        num_triangles: triangles.len(),
        num_materials: materials.len(),
        vertices,
        triangles,
        material_indices,
        materials,
    };

    let static_mesh = audionimbus::StaticMesh::try_new(&scene, &static_mesh_settings).unwrap();

    scene.add_static_mesh(&static_mesh);

    scene.commit();
}

#[test]
fn test_instanced_mesh() {
    let context_settings = audionimbus::ContextSettings::default();
    let context = audionimbus::Context::try_new(&context_settings).unwrap();

    let scene_settings = audionimbus::SceneSettings::default();
    let main_scene = audionimbus::Scene::try_new(&context, &scene_settings).unwrap();
    let sub_scene = audionimbus::Scene::try_new(&context, &scene_settings).unwrap();

    // Four vertices of a unit square in the x-y plane.
    let vertices = vec![
        audionimbus::geometry::Point::new(0.0, 0.0, 0.0),
        audionimbus::geometry::Point::new(1.0, 0.0, 0.0),
        audionimbus::geometry::Point::new(1.0, 1.0, 0.0),
        audionimbus::geometry::Point::new(0.0, 1.0, 0.0),
    ];

    let triangles = vec![
        audionimbus::geometry::Triangle::new(0, 1, 2),
        audionimbus::geometry::Triangle::new(0, 2, 2),
    ];

    let materials = vec![audionimbus::geometry::Material {
        absorption: [0.1, 0.1, 0.1],
        scattering: 0.5,
        transmission: [0.2, 0.2, 0.2],
    }];

    // Both triangles use the same material.
    let material_indices = vec![0, 0];

    let static_mesh_settings = audionimbus::geometry::StaticMeshSettings {
        num_vertices: vertices.len(),
        num_triangles: triangles.len(),
        num_materials: materials.len(),
        vertices,
        triangles,
        material_indices,
        materials,
    };

    let static_mesh = audionimbus::StaticMesh::try_new(&sub_scene, &static_mesh_settings).unwrap();
    sub_scene.add_static_mesh(&static_mesh);
    sub_scene.commit();

    let transform = audionimbus::Matrix::new([
        [1.0, 0.0, 0.0, 5.0], // Move 5 meters along the X axis.
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]);

    let instanced_mesh_settings = audionimbus::geometry::InstancedMeshSettings {
        sub_scene: &sub_scene,
        transform: &transform,
    };

    let mut instanced_mesh =
        audionimbus::InstancedMesh::try_new(&main_scene, &instanced_mesh_settings).unwrap();
    main_scene.add_instanced_mesh(&instanced_mesh);
    main_scene.commit();

    let new_transform = audionimbus::Matrix::new([
        [1.0, 0.0, 0.0, 10.0], // Move 10 meters along the X axis.
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]);

    instanced_mesh.update_transform(&main_scene, &new_transform);
    main_scene.commit();
}

#[test]
fn test_scene_serialization() {
    let context_settings = audionimbus::ContextSettings::default();
    let context = audionimbus::Context::try_new(&context_settings).unwrap();

    let scene_settings = audionimbus::SceneSettings::default();
    let scene = audionimbus::Scene::try_new(&context, &scene_settings).unwrap();

    // Four vertices of a unit square in the x-y plane.
    let vertices = vec![
        audionimbus::geometry::Point::new(0.0, 0.0, 0.0),
        audionimbus::geometry::Point::new(1.0, 0.0, 0.0),
        audionimbus::geometry::Point::new(1.0, 1.0, 0.0),
        audionimbus::geometry::Point::new(0.0, 1.0, 0.0),
    ];

    let triangles = vec![
        audionimbus::geometry::Triangle::new(0, 1, 2),
        audionimbus::geometry::Triangle::new(0, 2, 2),
    ];

    let materials = vec![audionimbus::geometry::Material {
        absorption: [0.1, 0.1, 0.1],
        scattering: 0.5,
        transmission: [0.2, 0.2, 0.2],
    }];

    // Both triangles use the same material.
    let material_indices = vec![0, 0];

    let static_mesh_settings = audionimbus::geometry::StaticMeshSettings {
        num_vertices: vertices.len(),
        num_triangles: triangles.len(),
        num_materials: materials.len(),
        vertices,
        triangles,
        material_indices,
        materials,
    };

    let static_mesh = audionimbus::StaticMesh::try_new(&scene, &static_mesh_settings).unwrap();

    let mut serialized_object = audionimbus::SerializedObject::try_new(&context).unwrap();

    static_mesh.save(&mut serialized_object);

    let loaded_static_mesh_result = audionimbus::StaticMesh::load(&scene, &mut serialized_object);
    assert!(loaded_static_mesh_result.is_ok());
}

#[test]
fn test_simulation() {
    let context_settings = audionimbus::ContextSettings::default();
    let context = audionimbus::Context::try_new(&context_settings).unwrap();

    let audio_settings = audionimbus::AudioSettings::default();

    let simulation_settings = audionimbus::SimulationSettings {
        flags: audionimbus::SimulationFlags::DIRECT | audionimbus::SimulationFlags::REFLECTIONS,
        scene_type: audionimbus::SceneType::Default,
        reflection_type: audionimbus::ReflectionEffectType::Convolution,
        max_num_occlusion_samples: 4,
        max_num_rays: 4096,
        num_diffuse_samples: 32,
        max_duration: 2.0,
        max_order: 1,
        max_num_sources: 8,
        num_threads: 2,
        ray_batch_size: usize::default(),
        num_visibility_samples: 4,
        sampling_rate: audio_settings.sampling_rate,
        frame_size: audio_settings.frame_size,
        open_cl_device: audionimbus::OpenClDevice::null(),
        radeon_rays_device: audionimbus::RadeonRaysDevice::null(),
        true_audio_next_device: audionimbus::TrueAudioNextDevice::null(),
    };
    let mut simulator = audionimbus::Simulator::try_new(&context, &simulation_settings).unwrap();

    let scene_settings = audionimbus::SceneSettings::default();
    let scene = audionimbus::Scene::try_new(&context, &scene_settings).unwrap();
    simulator.set_scene(&scene);

    let source_settings = audionimbus::SourceSettings {
        flags: audionimbus::SimulationFlags::DIRECT | audionimbus::SimulationFlags::REFLECTIONS,
    };
    let mut source = audionimbus::Source::try_new(&simulator, &source_settings).unwrap();

    let simulation_inputs = audionimbus::SimulationInputs {
        flags: audionimbus::SimulationFlags::DIRECT | audionimbus::SimulationFlags::REFLECTIONS,
        direct_flags: audionimbus::DirectSimulationFlags::OCCLUSION
            | audionimbus::DirectSimulationFlags::TRANSMISSION,
        source: audionimbus::CoordinateSystem {
            right: audionimbus::Vector3::new(1.0, 0.0, 0.0),
            up: audionimbus::Vector3::new(0.0, 1.0, 0.0),
            ahead: audionimbus::Vector3::new(0.0, 0.0, 1.0),
            origin: audionimbus::Vector3::new(1.0, 0.0, 0.0),
        },
        distance_attenuation_model: audionimbus::DistanceAttenuationModel::default(),
        air_absorption_model: audionimbus::AirAbsorptionModel::default(),
        directivity: audionimbus::Directivity::default(),
        occlusion: audionimbus::Occlusion::Raycast,
        reverb_scale: [1.0, 1.0, 1.0],
        hybrid_reverb_transition_time: f32::default(),
        hybrid_reverb_overlap_percent: f32::default(),
        baked_data_identifier: None,
        pathing_probes: audionimbus::ProbeBatch::try_new(&context).unwrap(),
        visibility_radius: 10.0,
        visibility_threshold: 0.0,
        visibility_range: 0.0,
        pathing_order: usize::default(),
        enable_validation: true,
        find_alternate_paths: true,
        num_transmission_rays: 1,
    };
    source.set_inputs(audionimbus::SimulationFlags::DIRECT, &simulation_inputs);

    simulator.add_source(&source);

    let simulation_shared_inputs = audionimbus::SimulationSharedInputs {
        listener: audionimbus::CoordinateSystem::default(),
        num_rays: 4096,
        num_bounces: 16,
        duration: 2.0,
        order: 1,
        irradiance_min_distance: 1.0,
        pathing_visualization_callback: None,
    };
    simulator.set_shared_inputs(
        audionimbus::SimulationFlags::DIRECT | audionimbus::SimulationFlags::REFLECTIONS,
        &simulation_shared_inputs,
    );

    simulator.commit();

    simulator.run_direct();
    simulator.run_reflections();
    let simulation_outputs = source.get_outputs(
        audionimbus::SimulationFlags::DIRECT | audionimbus::SimulationFlags::REFLECTIONS,
    );

    let reflection_effect_settings = audionimbus::ReflectionEffectSettings::Convolution {
        impulse_response_size: 88200, // 2.0f (IR duration) * 44100 (sampling rate)
        num_channels: 4,              // 1st order Ambisonics
    };
    let reflection_effect = audionimbus::ReflectionEffect::try_new(
        &context,
        &audio_settings,
        &reflection_effect_settings,
    )
    .unwrap();

    // Must be mono.
    let mut input_buffer =
        audionimbus::AudioBuffer::from(raw_to_deinterleaved(AUDIO_BUFFER_SINE_WAVE_440HZ_1S, 1));

    // Must have 4 channels (1st order Ambisonics) for this example.
    let mut output_buffer =
        audionimbus::AudioBuffer::with_num_channels_and_num_samples(4, input_buffer.num_samples);

    let mut reflection_effect_params = simulation_outputs.reflections();
    reflection_effect_params.num_channels = 4; // use all channels of the IR
    reflection_effect_params.impulse_response_size = 88200; // use the full duration of the IR
    let _ = reflection_effect.apply(
        &reflection_effect_params,
        &mut input_buffer,
        &mut output_buffer,
    );
}

#[test]
fn test_probe_generation() {
    let context_settings = audionimbus::ContextSettings::default();
    let context = audionimbus::Context::try_new(&context_settings).unwrap();

    let scene_settings = audionimbus::SceneSettings::default();
    let scene = audionimbus::Scene::try_new(&context, &scene_settings).unwrap();

    // This specifies a 100x100x100 axis-aligned box.
    let box_transform = audionimbus::Matrix::new([
        [100.0, 0.0, 0.0, 0.0],
        [0.0, 100.0, 0.0, 0.0],
        [0.0, 0.0, 100.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]);

    let mut probe_array = audionimbus::ProbeArray::try_new(&context).unwrap();

    let probe_params = audionimbus::ProbeGenerationParams::UniformFloor {
        spacing: 2.0,
        height: 1.5,
        transform: box_transform,
    };
    probe_array.generate_probes(&scene, &probe_params);

    let mut probe_batch = audionimbus::ProbeBatch::try_new(&context).unwrap();
    probe_batch.add_probe_array(&probe_array);

    probe_batch.commit();
}

#[test]
pub fn test_baking() {
    let context_settings = audionimbus::ContextSettings::default();
    let context = audionimbus::Context::try_new(&context_settings).unwrap();

    let audio_settings = audionimbus::AudioSettings::default();

    let simulation_settings = audionimbus::SimulationSettings {
        flags: audionimbus::SimulationFlags::DIRECT | audionimbus::SimulationFlags::REFLECTIONS,
        scene_type: audionimbus::SceneType::Default,
        reflection_type: audionimbus::ReflectionEffectType::Convolution,
        max_num_occlusion_samples: 4,
        max_num_rays: 4096,
        num_diffuse_samples: 32,
        max_duration: 2.0,
        max_order: 1,
        max_num_sources: 8,
        num_threads: 2,
        ray_batch_size: usize::default(),
        num_visibility_samples: 4,
        sampling_rate: audio_settings.sampling_rate,
        frame_size: audio_settings.frame_size,
        open_cl_device: audionimbus::OpenClDevice::null(),
        radeon_rays_device: audionimbus::RadeonRaysDevice::null(),
        true_audio_next_device: audionimbus::TrueAudioNextDevice::null(),
    };
    let mut simulator = audionimbus::Simulator::try_new(&context, &simulation_settings).unwrap();

    let scene_settings = audionimbus::SceneSettings::default();
    let scene = audionimbus::Scene::try_new(&context, &scene_settings).unwrap();
    simulator.set_scene(&scene);

    // This specifies a 100x100x100 axis-aligned box.
    let box_transform = audionimbus::Matrix::new([
        [100.0, 0.0, 0.0, 0.0],
        [0.0, 100.0, 0.0, 0.0],
        [0.0, 0.0, 100.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]);

    let mut probe_array = audionimbus::ProbeArray::try_new(&context).unwrap();

    let probe_params = audionimbus::ProbeGenerationParams::UniformFloor {
        spacing: 2.0,
        height: 1.5,
        transform: box_transform,
    };
    probe_array.generate_probes(&scene, &probe_params);

    let mut probe_batch = audionimbus::ProbeBatch::try_new(&context).unwrap();
    probe_batch.add_probe_array(&probe_array);

    probe_batch.commit();

    let identifier = audionimbus::BakedDataIdentifier::Reflections {
        variation: audionimbus::BakedDataVariation::StaticSource {
            endpoint_influence: audionimbus::Sphere {
                center: audionimbus::Point::default(), // World-space position of the souce.
                radius: 100.0, // Only bake reflections for probes within 100m of the source.
            },
        },
    };

    let reflections_bake_params = audionimbus::ReflectionsBakeParams {
        scene: &scene,
        probe_batch: &probe_batch,
        scene_type: audionimbus::SceneType::Default,
        identifier: &identifier,
        bake_flags: audionimbus::ReflectionsBakeFlags::BAKE_CONVOLUTION,
        num_rays: 32768,
        num_diffuse_samples: 1024,
        num_bounces: 64,
        simulated_duration: 2.0,
        saved_duration: 2.0,
        order: 2,
        num_threads: 8,
        ray_batch_size: usize::default(),
        irradiance_min_distance: 1.0,
        bake_batch_size: usize::default(),
        open_cl_device: &audionimbus::OpenClDevice::null(),
        radeon_rays_device: &audionimbus::RadeonRaysDevice::null(),
    };
    audionimbus::bake_reflections(&context, &reflections_bake_params, None);

    simulator.add_probe_batch(&probe_batch);
    simulator.commit();
}
