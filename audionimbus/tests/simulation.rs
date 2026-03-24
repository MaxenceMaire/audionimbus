use audionimbus::wiring::*;
use audionimbus::*;
use std::time::Duration;

mod common;
use common::sine_wave;

#[test]
fn test_simulation() {
    let context = Context::default();
    let audio_settings = AudioSettings::default();

    let simulation_settings = SimulationSettings::new(&audio_settings)
        .with_direct(DirectSimulationSettings {
            max_num_occlusion_samples: 4,
        })
        .with_reflections(ConvolutionSettings {
            max_num_rays: 4096,
            num_diffuse_samples: 32,
            max_duration: 2.0,
            max_num_sources: 8,
            num_threads: 2,
            max_order: 1,
        })
        .with_pathing(PathingSimulationSettings {
            num_visibility_samples: 4,
        });
    let mut simulator = Simulator::try_new(&context, &simulation_settings).unwrap();

    let scene = Scene::try_new(&context).unwrap();
    simulator.set_scene(&scene);

    let source = Source::try_new(&simulator).unwrap();

    let pathing_probes = ProbeBatch::try_new(&context).unwrap();
    let simulation_inputs = SimulationInputs {
        source: CoordinateSystem {
            right: Vector3::new(1.0, 0.0, 0.0),
            up: Vector3::new(0.0, 1.0, 0.0),
            ahead: Vector3::new(0.0, 0.0, 1.0),
            origin: Vector3::new(0.0, 0.0, 0.0),
        },
        parameters: SimulationParameters::new()
            .with_direct(
                DirectSimulationParameters::new()
                    .with_distance_attenuation(DistanceAttenuationModel::default())
                    .with_air_absorption(AirAbsorptionModel::default())
                    .with_directivity(Directivity::default())
                    .with_occlusion(
                        Occlusion::new(OcclusionAlgorithm::Raycast).with_transmission(
                            TransmissionParameters {
                                num_transmission_rays: 1,
                            },
                        ),
                    ),
            )
            .with_reflections(ConvolutionParameters {
                baked_data_identifier: None,
            })
            .with_pathing(PathingSimulationParameters {
                pathing_probes,
                visibility_radius: 1.0,
                visibility_threshold: 10.0,
                visibility_range: 10.0,
                pathing_order: 1,
                enable_validation: true,
                find_alternate_paths: true,
                deviation: DeviationModel::default(),
            }),
    };
    source.set_direct_inputs(&simulation_inputs).unwrap();

    simulator.add_source(&source);

    let simulation_shared_inputs = SimulationSharedInputs::new(CoordinateSystem::default())
        .with_reflections(ReflectionsSharedInputs {
            num_rays: 4096,
            num_bounces: 16,
            duration: 2.0,
            order: 1,
            irradiance_min_distance: 1.0,
        });
    simulator
        .set_shared_inputs(&simulation_shared_inputs)
        .unwrap();

    simulator.commit();

    simulator.run_direct();
    assert!(simulator.run_reflections().is_ok());
    let simulation_outputs = source
        .get_outputs_subset::<Direct, Reflections, ()>()
        .unwrap();

    let reflection_effect_settings = ReflectionEffectSettings {
        // 2.0f (IR duration) * 48000 (sampling rate)
        impulse_response_size: 2 * audio_settings.sampling_rate,
        num_channels: num_ambisonics_channels(1),
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
    let sine_wave = sine_wave(
        frequency,
        amplitude,
        duration_secs,
        audio_settings.sampling_rate,
    );
    // Must be mono.
    let input_buffer = AudioBuffer::try_with_data(&sine_wave).unwrap();

    // Must have 4 channels (1st order Ambisonics) for this example.
    let mut output_container = vec![0.0; 4 * input_buffer.num_samples() as usize];
    let output_buffer = AudioBuffer::try_with_data_and_settings(
        &mut output_container,
        AudioBufferSettings::with_num_channels(4),
    )
    .unwrap();

    let reflection_effect_params = simulation_outputs.reflections();
    let _ = reflection_effect.apply(&reflection_effect_params, &input_buffer, &output_buffer);
}

#[test]
fn test_pathing_without_probes() {
    let context = Context::default();
    let audio_settings = AudioSettings::default();

    let simulation_settings = SimulationSettings::new(&audio_settings)
        .with_direct(DirectSimulationSettings {
            max_num_occlusion_samples: 4,
        })
        .with_reflections(ConvolutionSettings {
            max_num_rays: 4096,
            num_diffuse_samples: 32,
            max_duration: 2.0,
            max_num_sources: 8,
            num_threads: 2,
            max_order: 1,
        })
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

    let mut pathing_probes = ProbeBatch::try_new(&context).unwrap();
    // Add a probe array without commit.
    pathing_probes.add_probe_array(&probe_array);
    // Add the probe batch to the simulator without any probes in it.
    simulator.add_probe_batch(&pathing_probes);

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
        .bake(&context, &mut pathing_probes, &scene, path_bake_params)
        .unwrap();

    let source = Source::try_new(&simulator).unwrap();
    let simulation_inputs = SimulationInputs {
        source: CoordinateSystem::default(),
        parameters: SimulationParameters::new()
            .with_direct(
                DirectSimulationParameters::new()
                    .with_distance_attenuation(DistanceAttenuationModel::default())
                    .with_air_absorption(AirAbsorptionModel::default())
                    .with_directivity(Directivity::default())
                    .with_occlusion(
                        Occlusion::new(OcclusionAlgorithm::Raycast).with_transmission(
                            TransmissionParameters {
                                num_transmission_rays: 1,
                            },
                        ),
                    ),
            )
            .with_reflections(ConvolutionParameters {
                baked_data_identifier: None,
            })
            .with_pathing(PathingSimulationParameters {
                pathing_probes,
                visibility_radius: 1.0,
                visibility_threshold: 10.0,
                visibility_range: 10.0,
                pathing_order: 1,
                enable_validation: true,
                find_alternate_paths: true,
                deviation: DeviationModel::default(),
            }),
    };
    source.set_pathing_inputs(&simulation_inputs).unwrap();
    simulator.add_source(&source);

    simulator.commit();
    assert_eq!(
        simulator.run_pathing(),
        Err(SimulationError::PathingWithoutProbes)
    );
}

#[test]
fn test_reflections_without_scene() {
    let context = Context::default();
    let audio_settings = AudioSettings::default();

    let simulation_settings =
        SimulationSettings::new(&audio_settings).with_reflections(ConvolutionSettings {
            max_num_rays: 4096,
            num_diffuse_samples: 32,
            max_duration: 2.0,
            max_num_sources: 8,
            num_threads: 2,
            max_order: 1,
        });
    let mut simulator = Simulator::try_new(&context, &simulation_settings).unwrap();

    let simulation_shared_inputs = SimulationSharedInputs::new(CoordinateSystem::default())
        .with_reflections(ReflectionsSharedInputs {
            num_rays: 4096,
            num_bounces: 16,
            duration: 2.0,
            order: 1,
            irradiance_min_distance: 1.0,
        });
    simulator
        .set_shared_reflections_inputs(&simulation_shared_inputs)
        .unwrap();

    simulator.commit();

    assert_eq!(
        simulator.run_reflections(),
        Err(SimulationError::ReflectionsWithoutScene)
    );
}

#[test]
fn test_wiring_simulation() {
    let context = Context::default();
    let audio_settings = AudioSettings::default();

    let simulation_settings = SimulationSettings::new(&audio_settings)
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
        })
        .with_pathing(PathingSimulationSettings {
            num_visibility_samples: 4,
        });
    let mut simulator = Simulator::try_new(&context, &simulation_settings).unwrap();

    let mut scene = Scene::try_new(&context).unwrap();
    let vertices = vec![
        Point::new(-10.0, 0.0, -10.0),
        Point::new(10.0, 0.0, -10.0),
        Point::new(10.0, 0.0, 10.0),
        Point::new(-10.0, 0.0, 10.0),
    ];
    let triangles = vec![Triangle::new(0, 1, 2), Triangle::new(0, 2, 3)];
    let materials = vec![Material::default()];
    let material_indices = vec![0usize, 0];
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

    let mut probe_array = ProbeArray::try_new(&context).unwrap();
    probe_array.generate_probes(
        &scene,
        &ProbeGenerationParams::UniformFloor {
            spacing: 5.0,
            height: 1.5,
            transform: Matrix4::new([
                [20.0, 0.0, 0.0, 0.0],
                [0.0, 20.0, 0.0, 0.0],
                [0.0, 0.0, 20.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ]),
        },
    );
    let mut probe_batch = ProbeBatch::try_new(&context).unwrap();
    probe_batch.add_probe_array(&probe_array);
    probe_batch.commit();
    simulator.add_probe_batch(&probe_batch);
    simulator.commit();

    let simulator_clone = simulator.clone();
    let mut simulation = Simulation::new::<()>(simulator);

    let source =
        Source::<Direct, Reflections, Pathing, Convolution>::try_new(&simulator_clone).unwrap();
    simulator_clone.add_source(&source);

    let listener_source =
        Source::<(), Reflections, (), Convolution>::try_new_subset(&simulator_clone).unwrap();
    simulator_clone.add_source(&listener_source);

    simulation.request_commit();

    simulation.update(|sources| {
        sources.push((
            (),
            SourceWithInputs {
                source: source.clone(),
                simulation_inputs: SimulationInputs::new(CoordinateSystem::default())
                    .with_direct(
                        DirectSimulationParameters::new()
                            .with_distance_attenuation(DistanceAttenuationModel::default())
                            .with_air_absorption(AirAbsorptionModel::default())
                            .with_directivity(Directivity::default()),
                    )
                    .with_reflections(ConvolutionParameters {
                        baked_data_identifier: None,
                    })
                    .with_pathing(PathingSimulationParameters {
                        pathing_probes: probe_batch.clone(),
                        visibility_radius: 1.0,
                        visibility_threshold: 0.1,
                        visibility_range: 50.0,
                        pathing_order: 1,
                        enable_validation: false,
                        find_alternate_paths: false,
                        deviation: DeviationModel::Default,
                    }),
            },
        ));
    });

    let listener = SourceWithInputs {
        source: listener_source,
        simulation_inputs: SimulationInputs::new(CoordinateSystem::default()).with_reflections(
            ConvolutionParameters {
                baked_data_identifier: None,
            },
        ),
    };

    let direct_simulation = simulation.spawn_direct();
    let reverb_simulation = simulation.spawn_reflections_reverb(listener);
    let pathing_simulation = simulation.spawn_pathing();

    std::thread::sleep(Duration::from_millis(200));

    assert_eq!(direct_simulation.output.load().len(), 1);

    let reverb_output = reverb_simulation.output.load();
    assert_eq!(reverb_output.sources.len(), 1);
    assert!(
        reverb_output.listener.is_some(),
        "listener reverb output should be populated after at least one simulation run"
    );

    assert_eq!(pathing_simulation.output.load().len(), 1);

    simulation.shutdown();
    direct_simulation
        .handle
        .join()
        .expect("direct simulation thread panicked");
    reverb_simulation
        .handle
        .join()
        .expect("reverb simulation thread panicked");
    pathing_simulation
        .handle
        .join()
        .expect("pathing simulation thread panicked");
}
