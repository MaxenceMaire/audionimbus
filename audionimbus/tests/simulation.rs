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

    let simulation_settings = SimulationSettings::new(sampling_rate, frame_size, max_order)
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
        });
    let mut simulator = Simulator::try_new(&context, &simulation_settings).unwrap();

    let scene = Scene::try_new(&context).unwrap();
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
    assert!(simulator.run_reflections().is_ok());
    let simulation_outputs = source
        .get_outputs(SimulationFlags::DIRECT | SimulationFlags::REFLECTIONS)
        .unwrap();

    let reflection_effect_settings = ReflectionEffectSettings {
        impulse_response_size: 2 * sampling_rate, // 2.0f (IR duration) * 48000 (sampling rate)
        num_channels: 4,                          // 1st order Ambisonics
    };
    let mut reflection_effect = ReflectionEffect::<Convolution>::try_new(
        &context,
        &audio_settings,
        &reflection_effect_settings,
    )
    .unwrap();

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
        AudioBufferSettings::with_num_channels(4),
    )
    .unwrap();

    let mut reflection_effect_params = simulation_outputs.reflections();
    reflection_effect_params.num_channels = 4; // use all channels of the IR.
    reflection_effect_params.impulse_response_size = 2 * sampling_rate; // use the full duration of the IR.
    let _ = reflection_effect.apply(&reflection_effect_params, &input_buffer, &output_buffer);
}

#[test]
fn test_pathing_without_probes() {
    let context = Context::default();

    const SAMPLING_RATE: u32 = 48_000;
    const FRAME_SIZE: u32 = 1024;
    const MAX_ORDER: u32 = 1;

    let simulation_settings = SimulationSettings::new(SAMPLING_RATE, FRAME_SIZE, MAX_ORDER)
        .with_pathing(PathingSimulationSettings {
            num_visibility_samples: 4,
        });
    let mut simulator = Simulator::try_new(&context, &simulation_settings).unwrap();

    let mut scene = Scene::try_new(&context).unwrap();
    let vertices = vec![
        Point::new(-50.0, 0.0, -50.0),
        Point::new(50.0, 0.0, -50.0),
        Point::new(50.0, 0.0, 50.0),
        Point::new(-50.0, 0.0, 50.0),
    ];
    let triangles = vec![Triangle::new(0, 1, 2), Triangle::new(0, 2, 3)];
    let materials = vec![Material::default()];
    let material_indices = vec![0, 0];
    let static_mesh = StaticMesh::try_new(
        &scene,
        &StaticMeshSettings {
            vertices: &vertices,
            triangles: &triangles,
            material_indices: &material_indices,
            materials: &materials,
        },
    )
    .unwrap();
    scene.add_static_mesh(static_mesh);
    scene.commit();
    simulator.set_scene(&scene);

    let identifier = BakedDataIdentifier::Pathing {
        variation: BakedDataVariation::Dynamic,
    };

    let mut probe_array = ProbeArray::try_new(&context).unwrap();
    let box_transform = Matrix::new([
        [100.0, 0.0, 0.0, 0.0],
        [0.0, 100.0, 0.0, 0.0],
        [0.0, 0.0, 100.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]);
    probe_array.generate_probes(
        &scene,
        &ProbeGenerationParams::UniformFloor {
            spacing: 2.0,
            height: 1.5,
            transform: box_transform,
        },
    );

    let mut probe_batch = ProbeBatch::try_new(&context).unwrap();
    // Add a probe array without commit.
    probe_batch.add_probe_array(&probe_array);
    // Add the probe batch to the simulator without any probes in it.
    simulator.add_probe_batch(&probe_batch);

    let path_bake_params = PathBakeParams {
        identifier,
        num_samples: 1, // Trace a single ray to test if one probe can see another probe.
        visibility_range: 50.0, // Don't check visibility between probes that are > 50m apart.
        path_range: 100.0, // Don't store paths between probes that are > 100m apart.
        num_threads: 8,
        radius: 1.0,
        threshold: 0.5,
    };
    PathBaker::new()
        .bake(&context, &mut probe_batch, &scene, path_bake_params)
        .unwrap();

    let source_settings = SourceSettings {
        flags: SimulationFlags::PATHING,
    };
    let mut source = Source::try_new(&simulator, &source_settings).unwrap();
    let simulation_inputs = SimulationInputs {
        source: CoordinateSystem::default(),
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
    simulator.add_source(&source);

    simulator.commit();
    assert_eq!(
        simulator.run_pathing(),
        Err(SimulationError::PathingWithoutProbes)
    );
}

#[test]
fn test_reflections_without_scene() {
    let context_settings = ContextSettings::default();
    let context = Context::try_new(&context_settings).unwrap();

    let sampling_rate = 48000;
    let frame_size = 1024;
    let max_order = 1;

    let simulation_settings = SimulationSettings::new(sampling_rate, frame_size, max_order)
        .with_reflections(ReflectionsSimulationSettings::Convolution {
            max_num_rays: 4096,
            num_diffuse_samples: 32,
            max_duration: 2.0,
            max_num_sources: 8,
            num_threads: 2,
        });
    let mut simulator = Simulator::try_new(&context, &simulation_settings).unwrap();

    let simulation_shared_inputs = SimulationSharedInputs {
        listener: CoordinateSystem::default(),
        num_rays: 4096,
        num_bounces: 16,
        duration: 2.0,
        order: 1,
        irradiance_min_distance: 1.0,
        pathing_visualization_callback: None,
    };
    simulator.set_shared_inputs(SimulationFlags::REFLECTIONS, &simulation_shared_inputs);

    simulator.commit();

    assert_eq!(
        simulator.run_reflections(),
        Err(SimulationError::ReflectionsWithoutScene)
    );
}
