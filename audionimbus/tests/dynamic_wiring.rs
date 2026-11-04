#![cfg(all(feature = "dynamic", feature = "wiring"))]

use audionimbus::dynamic as runtime;
use audionimbus::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};

#[test]
fn rejects_unavailable_workers_and_sources() {
    let context = Context::default();
    let settings = runtime::SimulationSettings::new(&AudioSettings::default())
        .with_direct(DirectSimulationSettings::default());
    let simulator = runtime::Simulator::try_new(&context, &settings).unwrap();
    let other_simulator = runtime::Simulator::try_new(&context, &settings).unwrap();
    let source = runtime::Source::try_new(&simulator, SimulationFlags::DIRECT).unwrap();
    let foreign_source =
        runtime::Source::try_new(&other_simulator, SimulationFlags::DIRECT).unwrap();
    let mut simulation = runtime::Simulation::new::<u32>(simulator);
    simulation
        .update_sources(|sources| {
            sources.push((
                1,
                runtime::SourceWithInputs {
                    source: source.clone(),
                    simulation_inputs: Default::default(),
                },
            ))
        })
        .unwrap();
    let published = simulation.sources.load_full();
    assert!(matches!(
        simulation.update_sources(|sources| sources.push((
            2,
            runtime::SourceWithInputs {
                source: foreign_source,
                simulation_inputs: Default::default(),
            }
        ))),
        Err(runtime::SimulationError::ForeignSource)
    ));
    assert!(Arc::ptr_eq(&published, &simulation.sources.load_full()));
    assert!(matches!(
        simulation.update_sources(|sources| sources.push((
            1,
            runtime::SourceWithInputs {
                source: source.clone(),
                simulation_inputs: runtime::SimulationInputs {
                    parameters: runtime::SimulationParameters {
                        reflections: Some(ParametricParameters::default().into()),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            }
        ))),
        Err(runtime::SimulationError::DisabledModes(_))
    ));
    assert!(Arc::ptr_eq(&published, &simulation.sources.load_full()));
    assert!(matches!(
        simulation.spawn_reflections(|_| {}),
        Err(runtime::SimulationError::DisabledModes(_))
    ));
    assert!(matches!(
        simulation.spawn_reflections_reverb(|_| {}),
        Err(runtime::SimulationError::DisabledModes(_))
    ));
    assert!(matches!(
        simulation.spawn_pathing(|_| {}),
        Err(runtime::SimulationError::DisabledModes(_))
    ));
    assert!(matches!(
        simulation.set_listener(Some(runtime::SourceWithInputs {
            source,
            simulation_inputs: Default::default(),
        })),
        Err(runtime::SimulationError::DisabledModes(_))
    ));
    assert!(simulation.shutdowns.is_empty());
}

#[test]
fn partitions_sources_and_disables_modes() {
    let context = Context::default();
    let settings = runtime::SimulationSettings::new(&AudioSettings::default())
        .with_direct(DirectSimulationSettings::default())
        .with_reflections(runtime::ReflectionsSettings::Parametric(
            ParametricSettings {
                max_num_rays: 64,
                num_diffuse_samples: 8,
                max_duration: 0.25,
                max_num_sources: 4,
                num_threads: 1,
                max_order: 1,
            },
        ));
    let mut simulator = runtime::Simulator::try_new(&context, &settings).unwrap();
    let scene = runtime::Scene::try_new(&context, runtime::RayTracerSettings::Default).unwrap();
    scene.commit();
    simulator.try_set_scene(&scene).unwrap();
    let direct_source = runtime::Source::try_new(&simulator, SimulationFlags::DIRECT).unwrap();
    let reflection_source =
        runtime::Source::try_new(&simulator, SimulationFlags::REFLECTIONS).unwrap();
    let shared_source = runtime::Source::try_new(
        &simulator,
        SimulationFlags::DIRECT | SimulationFlags::REFLECTIONS,
    )
    .unwrap();
    for source in [&direct_source, &reflection_source, &shared_source] {
        simulator.try_add_source(source).unwrap();
    }
    simulator.commit();
    let direct_calls = Arc::new(AtomicUsize::new(0));
    let callback_calls = direct_calls.clone();
    let direct_parameters = DirectSimulationParameters::new().with_distance_attenuation(
        DistanceAttenuationModel::Callback {
            callback: audionimbus::callback::DistanceAttenuationCallback::new(move |_| {
                callback_calls.fetch_add(1, Ordering::Relaxed);
                0.25
            }),
            dirty: true,
        },
    );
    let mut simulation = runtime::Simulation::new::<u32>(simulator);
    assert!(matches!(
        simulation.update_sources(|sources| sources.push((
            2,
            runtime::SourceWithInputs {
                source: reflection_source.clone(),
                simulation_inputs: runtime::SimulationInputs {
                    parameters: runtime::SimulationParameters {
                        reflections: Some(ConvolutionParameters::default().into()),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            }
        ))),
        Err(runtime::SimulationError::AlgorithmMismatch { .. })
    ));
    assert!(simulation.sources.load().sources.is_empty());
    simulation
        .update_sources(|sources| {
            sources.push((
                1,
                runtime::SourceWithInputs {
                    source: direct_source.clone(),
                    simulation_inputs: runtime::SimulationInputs {
                        parameters: runtime::SimulationParameters {
                            direct: Some(direct_parameters),
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                },
            ));
            sources.push((
                2,
                runtime::SourceWithInputs {
                    source: reflection_source.clone(),
                    simulation_inputs: runtime::SimulationInputs {
                        parameters: runtime::SimulationParameters {
                            reflections: Some(ParametricParameters::default().into()),
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                },
            ));
            sources.push((
                3,
                runtime::SourceWithInputs {
                    source: shared_source,
                    simulation_inputs: runtime::SimulationInputs {
                        parameters: runtime::SimulationParameters {
                            direct: Some(DirectSimulationParameters::new()),
                            reflections: Some(ParametricParameters::default().into()),
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                },
            ));
        })
        .unwrap();
    let mut direct = simulation.spawn_direct(|error| panic!("{error}")).unwrap();
    let mut reflections = simulation
        .spawn_reflections(|error| {
            if !matches!(error, runtime::SimulationError::MissingInputs(_)) {
                panic!("{error}");
            }
        })
        .unwrap();
    reflections.set_input(runtime::SimulationSharedInputs::default().with_reflections(
        ReflectionsSharedInputs {
            num_rays: 64,
            num_bounces: 2,
            duration: 0.25,
            order: 1,
            irradiance_min_distance: 1.0,
        },
    ));
    let direct_output = direct.output();
    let reflection_output = reflections.output();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let direct_snapshot = direct_output.load();
        let reflection_snapshot = reflection_output.load();
        if direct_snapshot
            .get(&1)
            .and_then(|parameters| parameters.distance_attenuation)
            == Some(0.25)
            && reflection_snapshot.sources.contains_key(&2)
        {
            assert_eq!(direct_snapshot.len(), 2);
            assert!(direct_snapshot.contains_key(&3));
            assert_eq!(reflection_snapshot.sources.len(), 2);
            assert!(reflection_snapshot.sources.contains_key(&3));
            assert_eq!(
                reflection_snapshot.sources[&2].algorithm(),
                runtime::ReflectionAlgorithm::Parametric
            );
            break;
        }
        assert!(
            Instant::now() < deadline,
            "partitioned workers did not publish their outputs"
        );
        std::thread::yield_now();
    }
    simulation
        .update_sources(|sources| {
            sources.push((
                1,
                runtime::SourceWithInputs {
                    source: direct_source.clone(),
                    simulation_inputs: Default::default(),
                },
            ))
        })
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !reflection_output.load().sources.is_empty() {
        assert!(
            Instant::now() < deadline,
            "removed reflection sources remain published"
        );
        std::thread::yield_now();
    }
    let acknowledgment_source =
        runtime::Source::try_new(simulation.simulator(), SimulationFlags::DIRECT).unwrap();
    simulation
        .simulator()
        .try_add_source(&acknowledgment_source)
        .unwrap();
    simulation.request_simulator_commit();
    let acknowledgments = Arc::new(AtomicUsize::new(0));
    let callback_acknowledgments = acknowledgments.clone();
    simulation
        .update_sources(|sources| {
            sources.push((
                1,
                runtime::SourceWithInputs {
                    source: direct_source,
                    simulation_inputs: Default::default(),
                },
            ));
            sources.push((
                4,
                runtime::SourceWithInputs {
                    source: acknowledgment_source,
                    simulation_inputs: runtime::SimulationInputs {
                        parameters: runtime::SimulationParameters {
                            direct: Some(
                                DirectSimulationParameters::new().with_distance_attenuation(
                                    DistanceAttenuationModel::Callback {
                                        callback:
                                            audionimbus::callback::DistanceAttenuationCallback::new(
                                                move |_| {
                                                    callback_acknowledgments
                                                        .fetch_add(1, Ordering::Relaxed);
                                                    0.75
                                                },
                                            ),
                                        dirty: true,
                                    },
                                ),
                            ),
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                },
            ));
        })
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while direct_output
        .load()
        .get(&4)
        .and_then(|parameters| parameters.distance_attenuation)
        != Some(0.75)
    {
        assert!(
            Instant::now() < deadline,
            "direct worker missed the disabling update"
        );
        std::thread::yield_now();
    }
    let disabled_calls = direct_calls.load(Ordering::Relaxed);
    let target = acknowledgments.load(Ordering::Relaxed) + 20;
    while acknowledgments.load(Ordering::Relaxed) < target {
        assert!(
            Instant::now() < deadline,
            "direct worker stopped after disabling a mode"
        );
        std::thread::yield_now();
    }
    assert_eq!(direct_calls.load(Ordering::Relaxed), disabled_calls);
    simulation.pause();
    simulation.shutdown();
    direct.join().unwrap();
    reflections.join().unwrap();
}

#[test]
fn retains_reflection_outputs_and_updates_listener() {
    for algorithm in [
        runtime::ReflectionAlgorithm::Convolution,
        runtime::ReflectionAlgorithm::Parametric,
        runtime::ReflectionAlgorithm::Hybrid,
    ] {
        let context = Context::default();
        let audio_settings = AudioSettings::default();
        let reflections_settings = match algorithm {
            runtime::ReflectionAlgorithm::Convolution => {
                runtime::ReflectionsSettings::Convolution(ConvolutionSettings {
                    max_num_rays: 64,
                    num_diffuse_samples: 8,
                    max_duration: 0.25,
                    max_num_sources: 4,
                    num_threads: 1,
                    max_order: 1,
                })
            }
            runtime::ReflectionAlgorithm::Parametric => {
                runtime::ReflectionsSettings::Parametric(ParametricSettings {
                    max_num_rays: 64,
                    num_diffuse_samples: 8,
                    max_duration: 0.25,
                    max_num_sources: 4,
                    num_threads: 1,
                    max_order: 1,
                })
            }
            runtime::ReflectionAlgorithm::Hybrid => {
                runtime::ReflectionsSettings::Hybrid(HybridSettings {
                    max_num_rays: 64,
                    num_diffuse_samples: 8,
                    max_duration: 0.25,
                    max_num_sources: 4,
                    num_threads: 1,
                    max_order: 1,
                })
            }
            _ => unreachable!(),
        };
        let settings = runtime::SimulationSettings::new(&audio_settings)
            .with_reflections(reflections_settings);
        let mut simulator = runtime::Simulator::try_new(&context, &settings).unwrap();
        let scene = runtime::Scene::try_new(&context, runtime::RayTracerSettings::Default).unwrap();
        scene.commit();
        simulator.try_set_scene(&scene).unwrap();
        let source = runtime::Source::try_new(&simulator, SimulationFlags::REFLECTIONS).unwrap();
        let listener = runtime::Source::try_new(&simulator, SimulationFlags::REFLECTIONS).unwrap();
        simulator.try_add_source(&source).unwrap();
        simulator.try_add_source(&listener).unwrap();
        simulator.commit();
        let parameters: runtime::ReflectionsParameters = match algorithm {
            runtime::ReflectionAlgorithm::Convolution => ConvolutionParameters::default().into(),
            runtime::ReflectionAlgorithm::Parametric => ParametricParameters::default().into(),
            runtime::ReflectionAlgorithm::Hybrid => HybridParameters {
                hybrid_reverb_transition_time: 0.1,
                ..Default::default()
            }
            .into(),
            _ => unreachable!(),
        };
        let mut simulation = runtime::Simulation::new::<u32>(simulator);
        simulation
            .update_sources(|sources| {
                sources.push((
                    1,
                    runtime::SourceWithInputs {
                        source,
                        simulation_inputs: runtime::SimulationInputs {
                            parameters: runtime::SimulationParameters {
                                reflections: Some(parameters.clone()),
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                    },
                ))
            })
            .unwrap();
        simulation
            .set_listener(Some(runtime::SourceWithInputs {
                source: listener,
                simulation_inputs: runtime::SimulationInputs {
                    parameters: runtime::SimulationParameters {
                        reflections: Some(parameters),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            }))
            .unwrap();
        let mut worker = simulation
            .spawn_reflections_reverb(|error| {
                if !matches!(error, runtime::SimulationError::MissingInputs(_)) {
                    panic!("{error}");
                }
            })
            .unwrap();
        worker.set_input(runtime::SimulationSharedInputs::default().with_reflections(
            ReflectionsSharedInputs {
                num_rays: 64,
                num_bounces: 2,
                duration: 0.25,
                order: 1,
                irradiance_min_distance: 1.0,
            },
        ));
        let output = worker.output();
        let deadline = Instant::now() + Duration::from_secs(5);
        let snapshot = loop {
            let snapshot = output.load();
            if snapshot.sources.contains_key(&1) && snapshot.listener.is_some() {
                break snapshot;
            }
            assert!(
                Instant::now() < deadline,
                "reflection worker did not publish listener output"
            );
            std::thread::yield_now();
        };
        assert_eq!(snapshot.sources[&1].algorithm(), algorithm);
        assert_eq!(snapshot.listener.as_ref().unwrap().algorithm(), algorithm);
        simulation.set_listener(None).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while output.load().listener.is_some() {
            assert!(
                Instant::now() < deadline,
                "removed listener remains published"
            );
            std::thread::yield_now();
        }
        worker.pause();
        worker.shutdown();
        worker.join().unwrap();
        drop(worker);
        drop(simulation);
        drop(scene);
        let mut effect = runtime::ReflectionEffect::try_new(
            &context,
            &audio_settings,
            &ReflectionEffectSettings {
                impulse_response_size: audio_settings.sampling_rate / 4,
                num_channels: 4,
            },
            algorithm,
        )
        .unwrap();
        let input_samples = vec![0.0; audio_settings.frame_size as usize];
        let input = AudioBufferRef::try_from(input_samples.as_slice()).unwrap();
        let mut output_samples = vec![0.0; 4 * audio_settings.frame_size as usize];
        let mut buffer = AudioBufferMut::try_new(&mut output_samples, 4).unwrap();
        effect
            .apply(&snapshot.sources[&1], &input, &mut buffer)
            .unwrap();
        effect
            .apply(snapshot.listener.as_ref().unwrap(), &input, &mut buffer)
            .unwrap();
    }
}

#[test]
fn publishes_pathing_outputs_and_joins_paused_worker() {
    let context = Context::default();
    let audio_settings = AudioSettings::default();
    let simulation_settings = runtime::SimulationSettings::new(&audio_settings)
        .with_direct(DirectSimulationSettings::default())
        .with_pathing(PathingSimulationSettings::default());
    let mut simulator = runtime::Simulator::try_new(&context, &simulation_settings).unwrap();
    let mut probe_scene = Scene::try_new(&context).unwrap();
    let mut scene = runtime::Scene::try_new(&context, runtime::RayTracerSettings::Default).unwrap();
    let vertices = [
        Point::new(-4.0, 0.0, -4.0),
        Point::new(4.0, 0.0, -4.0),
        Point::new(4.0, 0.0, 4.0),
        Point::new(-4.0, 0.0, 4.0),
    ];
    let triangles = [Triangle::new(0, 1, 2), Triangle::new(0, 2, 3)];
    let materials = [Material::CONCRETE];
    let material_indices = [0, 0];
    let mesh_settings = StaticMeshSettings {
        vertices: &vertices,
        triangles: &triangles,
        materials: &materials,
        material_indices: &material_indices,
    };
    let mesh = StaticMesh::try_new(&probe_scene, &mesh_settings).unwrap();
    probe_scene.add_static_mesh(mesh);
    probe_scene.commit();
    let mesh = runtime::StaticMesh::try_new(&scene, &mesh_settings).unwrap();
    scene.try_add_static_mesh(mesh).unwrap();
    scene.commit();
    simulator.try_set_scene(&scene).unwrap();
    let mut probe_array = ProbeArray::try_new(&context).unwrap();
    probe_array.generate_probes(
        &probe_scene,
        &ProbeGenerationParams::UniformFloor {
            spacing: 2.0,
            height: 1.0,
            transform: Matrix::new([
                [4.0, 0.0, 0.0, 0.0],
                [0.0, 4.0, 0.0, 0.0],
                [0.0, 0.0, 4.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ]),
        },
    );
    assert!(probe_array.num_probes() > 0);
    let mut probe_batch = ProbeBatch::try_new(&context).unwrap();
    probe_batch.add_probe_array(&probe_array);
    probe_batch.commit();
    PathBaker::new()
        .bake(
            &context,
            &mut probe_batch,
            &probe_scene,
            PathBakeParams {
                identifier: BakedDataIdentifier::Pathing {
                    variation: BakedDataVariation::Dynamic,
                },
                num_samples: 1,
                visibility_range: 10.0,
                path_range: 10.0,
                num_threads: 1,
                radius: 0.1,
                threshold: 0.5,
            },
        )
        .unwrap();
    simulator.add_probe_batch(&probe_batch);
    let source = runtime::Source::try_new(
        &simulator,
        SimulationFlags::DIRECT | SimulationFlags::PATHING,
    )
    .unwrap();
    simulator.try_add_source(&source).unwrap();
    simulator.commit();

    let mut simulation = runtime::Simulation::new::<u32>(simulator);
    simulation
        .update_sources(|sources| {
            sources.push((
                1,
                runtime::SourceWithInputs {
                    source,
                    simulation_inputs: runtime::SimulationInputs {
                        source: CoordinateSystem {
                            origin: Point::new(1.0, 1.0, 0.0),
                            ..Default::default()
                        },
                        parameters: runtime::SimulationParameters {
                            pathing: Some(PathingSimulationParameters {
                                pathing_probes: probe_batch,
                                visibility_radius: 0.1,
                                visibility_threshold: 0.5,
                                visibility_range: 10.0,
                                pathing_order: 1,
                                enable_validation: true,
                                find_alternate_paths: true,
                                deviation: DeviationModel::default(),
                            }),
                            ..Default::default()
                        },
                    },
                },
            ))
        })
        .unwrap();
    let mut worker = simulation.spawn_pathing(|error| panic!("{error}")).unwrap();
    worker.set_input(runtime::SimulationSharedInputs::new(CoordinateSystem {
        origin: Point::new(-1.0, 1.0, 0.0),
        ..Default::default()
    }));
    let output = worker.output();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !output.load().contains_key(&1) {
        assert!(
            Instant::now() < deadline,
            "pathing worker did not publish source output"
        );
        std::thread::yield_now();
    }
    assert_eq!(output.load().len(), 1);
    worker.pause();
    simulation.shutdown();
    worker.join().unwrap();
    let snapshot = output.load();
    let path_effect_params = &snapshot[&1];
    let mut effect = PathEffect::try_new(
        &context,
        &audio_settings,
        &PathEffectSettings {
            max_order: 1,
            spatialization: None,
        },
    )
    .unwrap();
    let input_samples = vec![1.0; audio_settings.frame_size as usize];
    let input = AudioBufferRef::try_from(input_samples.as_slice()).unwrap();
    let mut samples = vec![0.0; 4 * audio_settings.frame_size as usize];
    let mut buffer = AudioBufferMut::try_new(&mut samples, 4).unwrap();
    effect
        .apply(path_effect_params, &input, &mut buffer)
        .unwrap();
    assert!(samples.iter().all(|sample| sample.is_finite()));
    assert!(samples.iter().any(|sample| sample.abs() > 0.0));
}
