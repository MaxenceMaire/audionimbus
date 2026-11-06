#![cfg(feature = "wiring")]

#[cfg(feature = "dynamic")]
pub mod common;

#[cfg(any(feature = "static", not(feature = "dynamic")))]
mod r#static {
    #[cfg(not(feature = "static"))]
    use audionimbus as static_api;
    #[cfg(feature = "static")]
    use audionimbus::r#static as static_api;
    use audionimbus::wiring::*;
    use static_api::*;
    use std::time::Duration;

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

        simulation.request_simulator_commit();

        simulation.update_sources(|sources| {
            sources.push((
                (),
                SourceWithInputs {
                    source: source.clone(),
                    simulation_inputs: SimulationInputs {
                        source: CoordinateSystem::default(),
                        parameters: SimulationParameters::new()
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
                },
            ));
        });

        let on_error = |error| {
            eprintln!("{error}");
        };
        let mut direct_simulation = simulation.spawn_direct(on_error);
        let mut reverb_simulation = simulation.spawn_reflections_reverb(on_error);
        let mut pathing_simulation = simulation.spawn_pathing(on_error);

        std::thread::sleep(Duration::from_millis(200));

        assert_eq!(direct_simulation.output().load().len(), 1);

        let reverb_output = reverb_simulation.output().load();
        assert_eq!(reverb_output.sources.len(), 1);
        assert!(
            reverb_output.listener.is_none(),
            "listener should be None initially"
        );

        assert_eq!(pathing_simulation.output().load().len(), 1);

        let listener = SourceWithInputs {
            source: listener_source.clone(),
            simulation_inputs: SimulationInputs {
                source: CoordinateSystem::default(),
                parameters: SimulationParameters::new().with_reflections(ConvolutionParameters {
                    baked_data_identifier: None,
                }),
            },
        };
        reverb_simulation.set_input(ReflectionsReverbFrame {
            sources: simulation.sources.clone(),
            listener: Some(listener),
            shared_inputs: Default::default(),
        });

        std::thread::sleep(Duration::from_millis(200));

        assert!(
            reverb_simulation.output().load().listener.is_some(),
            "listener should be Some after it is provided"
        );

        simulation.shutdown();
        direct_simulation
            .join()
            .expect("direct simulation thread panicked");
        reverb_simulation
            .join()
            .expect("reverb simulation thread panicked");
        pathing_simulation
            .join()
            .expect("pathing simulation thread panicked");
    }

    #[cfg(feature = "wiring")]
    #[test]
    fn preserves_static_worker_extension_points() {
        use arc_swap::ArcSwap;
        use audionimbus::ray_tracing::DefaultRayTracer;
        use object_pool::Pool;
        use static_api::Scene;
        use static_api::*;
        use static_api::{
            SimulationInputs, SimulationParameters, SimulationSettings, SimulationSharedInputs,
            Simulator, Source,
        };
        use std::collections::HashMap;
        use std::sync::{Arc, Condvar, Mutex, atomic::AtomicBool};
        use std::time::{Duration, Instant};

        /// An application-owned input frame.
        struct Inputs {
            /// The sources published by the application.
            sources: Vec<(u32, SourceWithInputs<Direct, (), (), ()>)>,
            /// The listener parameters used by direct simulation.
            shared_inputs: SimulationSharedInputs<Direct>,
        }

        impl Resolve for Inputs {
            type Resolved<'a> = &'a Self;

            fn resolve(&self) -> Self::Resolved<'_> {
                self
            }
        }

        impl AsDirectInput<u32, Direct, (), (), ()> for &Inputs {
            fn as_direct_input(&self) -> DirectInput<'_, u32, Direct, (), (), ()> {
                DirectInput {
                    sources: &self.sources,
                    shared_inputs: &self.shared_inputs,
                }
            }
        }

        impl Allocate<&Inputs> for HashMap<u32, DirectEffectParams> {
            fn allocate(input: &&Inputs) -> Self {
                Self::with_capacity(input.sources.len())
            }
        }

        /// An application-owned step using the public direct input contract.
        struct Step(DirectStep<u32, DefaultRayTracer, (), (), ()>);

        impl SimulationStep<&Inputs> for Step {
            type Output = HashMap<u32, DirectEffectParams>;
            type Error = audionimbus::wiring::step::SimulationStepError;

            fn run(
                &mut self,
                input: &&Inputs,
                output: &mut Self::Output,
            ) -> Result<(), Self::Error> {
                self.0.run(input, output)
            }
        }

        let context = Context::default();
        let settings = SimulationSettings::new(&AudioSettings::default())
            .with_direct(DirectSimulationSettings::default());
        let simulator = Simulator::try_new(&context, &settings).unwrap();
        let source = Source::<Direct>::try_new(&simulator).unwrap();
        simulator.add_source(&source);
        simulator.commit();
        let mut simulation = Simulation::new::<u32>(simulator);
        simulation.update_sources(|sources| {
            sources.push((
                1,
                SourceWithInputs {
                    source,
                    simulation_inputs: SimulationInputs {
                        parameters: SimulationParameters::new().with_direct(
                            DirectSimulationParameters::new().with_distance_attenuation(
                                DistanceAttenuationModel::Callback {
                                    callback:
                                        audionimbus::callback::DistanceAttenuationCallback::new(
                                            |_| 0.5,
                                        ),
                                    dirty: true,
                                },
                            ),
                        ),
                        ..Default::default()
                    },
                },
            ))
        });
        let frame = DirectFrame::<u32, Direct, (), (), ()> {
            sources: simulation.sources.clone(),
            shared_inputs: Default::default(),
        };
        let mut direct_step = DirectStep::new::<u32>(simulation.simulator().clone());
        let mut direct_output = HashMap::new();
        direct_step
            .run(&frame.resolve(), &mut direct_output)
            .unwrap();
        assert_eq!(direct_output[&1].distance_attenuation, Some(0.5));
        let input = Arc::new(ArcSwap::new(Arc::new(Inputs {
            sources: simulation.sources.load().iter().cloned().collect(),
            shared_inputs: Default::default(),
        })));
        let pool = Arc::new(Pool::new(1, HashMap::<u32, DirectEffectParams>::new));
        let output = Arc::new(ArcSwap::new(Arc::new(pool.pull_owned(HashMap::new))));
        let shutdown = Arc::new(AtomicBool::new(false));
        let paused = Arc::new((Mutex::new(false), Condvar::new()));
        simulation.shutdowns.push(shutdown.clone());
        simulation.paused.push(paused.clone());
        let simulator = simulation.simulator().clone();
        let scene = Scene::<DefaultRayTracer>::try_new(&context).unwrap();
        simulation.request_scene_commits(&[scene]);
        simulation.request_simulator_commit();
        let worker =
            SimulationRunner::<Inputs, HashMap<u32, DirectEffectParams>, DefaultRayTracer>::new(
                input,
                output.clone(),
                simulation.simulator_commit_needed.clone(),
                move || simulator.commit(),
                simulation.pending_scene_commits.clone(),
                shutdown,
                paused,
            )
            .spawn(
                Step(DirectStep::new::<u32>(simulation.simulator().clone())),
                |error| panic!("{error}"),
            );
        let deadline = Instant::now() + Duration::from_secs(5);
        while output
            .load()
            .get(&1)
            .and_then(|parameters| parameters.distance_attenuation)
            != Some(0.5)
        {
            assert!(
                Instant::now() < deadline,
                "custom static worker did not publish output"
            );
            std::thread::yield_now();
        }
        assert!(simulation.pending_scene_commits.load().is_empty());
        simulation.pause();
        simulation.shutdown();
        worker.join().unwrap();
    }
}

#[cfg(feature = "dynamic")]
mod dynamic {
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
            let scene =
                runtime::Scene::try_new(&context, runtime::RayTracerSettings::Default).unwrap();
            scene.commit();
            simulator.try_set_scene(&scene).unwrap();
            let source =
                runtime::Source::try_new(&simulator, SimulationFlags::REFLECTIONS).unwrap();
            let listener =
                runtime::Source::try_new(&simulator, SimulationFlags::REFLECTIONS).unwrap();
            simulator.try_add_source(&source).unwrap();
            simulator.try_add_source(&listener).unwrap();
            simulator.commit();
            let parameters: runtime::ReflectionsParameters = match algorithm {
                runtime::ReflectionAlgorithm::Convolution => {
                    ConvolutionParameters::default().into()
                }
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
            .with_pathing(PathingSimulationSettings::default(), 1);
        let mut simulator = runtime::Simulator::try_new(&context, &simulation_settings).unwrap();
        let (_probe_scene, _scene, _probe_array, probe_batch, source) =
            super::common::baked_source(&context, &mut simulator);

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
}

#[cfg(any(feature = "static", not(feature = "dynamic")))]
mod direct {
    #[cfg(not(feature = "static"))]
    use audionimbus as static_api;
    #[cfg(feature = "static")]
    use audionimbus::r#static as static_api;
    use static_api::*;
    #[test]
    fn test_spawn_and_shutdown() {
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
                max_num_sources: 4,
                num_threads: 1,
                max_order: 1,
            });
        let simulator = Simulator::try_new(&context, &simulation_settings).unwrap();
        let mut simulation = Simulation::new::<()>(simulator);
        let mut direct_simulation = simulation.spawn_direct(|error| {
            eprintln!("{error}");
        });
        simulation.shutdown();
        direct_simulation
            .join()
            .expect("simulation thread panicked");
    }

    #[test]
    fn test_initial_output_is_empty() {
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
                max_num_sources: 4,
                num_threads: 1,
                max_order: 1,
            });
        let simulator = Simulator::try_new(&context, &simulation_settings).unwrap();
        let mut simulation = Simulation::new::<()>(simulator);
        let mut direct_simulation = simulation.spawn_direct(|error| {
            eprintln!("{error}");
        });

        assert!(direct_simulation.output().load().is_empty());

        simulation.shutdown();
        direct_simulation
            .join()
            .expect("simulation thread panicked");
    }
}

#[cfg(any(feature = "static", not(feature = "dynamic")))]
mod pathing {
    #[cfg(not(feature = "static"))]
    use audionimbus as static_api;
    #[cfg(feature = "static")]
    use audionimbus::r#static as static_api;
    use static_api::*;
    /// Creates a pathing pipeline with committed probe geometry.
    fn simulation() -> Simulation<(), DefaultRayTracer, (), (), Pathing, ()> {
        let context = Context::default();
        let audio_settings = AudioSettings::default();
        let simulation_settings =
            SimulationSettings::new(&audio_settings).with_pathing(PathingSimulationSettings {
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

        Simulation::new::<()>(simulator)
    }

    #[test]
    fn test_spawn_and_shutdown() {
        let mut simulation = simulation();
        let mut pathing_simulation = simulation.spawn_pathing(|error| {
            eprintln!("{error}");
        });
        simulation.shutdown();
        pathing_simulation
            .join()
            .expect("simulation thread panicked");
    }

    #[test]
    fn test_initial_output_is_empty() {
        let mut simulation = simulation();
        let mut pathing_simulation = simulation.spawn_pathing(|error| {
            eprintln!("{error}");
        });

        assert!(pathing_simulation.output().load().is_empty());

        simulation.shutdown();
        pathing_simulation
            .join()
            .expect("simulation thread panicked");
    }
}

#[cfg(any(feature = "static", not(feature = "dynamic")))]
mod pipeline {
    #[cfg(not(feature = "static"))]
    use audionimbus as static_api;
    #[cfg(feature = "static")]
    use audionimbus::r#static as static_api;
    use static_api::*;
    #[test]
    fn test_new() {
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
                max_num_sources: 4,
                num_threads: 1,
                max_order: 1,
            });
        let simulator = Simulator::try_new(&context, &simulation_settings).unwrap();
        let _simulation = Simulation::new::<()>(simulator);
    }

    #[test]
    fn test_update_clears_buffer_between_calls() {
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
                max_num_sources: 4,
                num_threads: 1,
                max_order: 1,
            });
        let simulator = Simulator::try_new(&context, &simulation_settings).unwrap();
        let simulator_clone = simulator.clone();
        let simulation = Simulation::new::<()>(simulator);

        let source =
            Source::<Direct, Reflections, (), Convolution>::try_new(&simulator_clone).unwrap();

        simulation.update_sources(|sources| {
            sources.push((
                (),
                SourceWithInputs {
                    source: source.clone(),
                    simulation_inputs: SimulationInputs {
                        source: CoordinateSystem::default(),
                        parameters: SimulationParameters::new()
                            .with_direct(DirectSimulationParameters::new())
                            .with_reflections(ConvolutionParameters {
                                baked_data_identifier: None,
                            }),
                    },
                },
            ));
            assert_eq!(sources.len(), 1);
        });

        simulation.update_sources(|sources| {
            assert!(
                sources.is_empty(),
                "update should receive an empty buffer each call"
            );
        });
    }

    #[test]
    fn test_shutdown() {
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
                max_num_sources: 4,
                num_threads: 1,
                max_order: 1,
            });
        let simulator = Simulator::try_new(&context, &simulation_settings).unwrap();
        let mut simulation = Simulation::new::<()>(simulator);
        let mut direct_simulation = simulation.spawn_direct(|error| {
            eprintln!("{error}");
        });
        simulation.shutdown();
        direct_simulation
            .join()
            .expect("simulation thread panicked");
    }
}

#[cfg(any(feature = "static", not(feature = "dynamic")))]
mod reflections {
    #[cfg(not(feature = "static"))]
    use audionimbus as static_api;
    #[cfg(feature = "static")]
    use audionimbus::r#static as static_api;
    use static_api::*;
    #[test]
    fn test_spawn_and_shutdown() {
        let context = Context::default();
        let audio_settings = AudioSettings::default();
        let simulation_settings =
            SimulationSettings::new(&audio_settings).with_reflections(ConvolutionSettings {
                max_num_rays: 128,
                num_diffuse_samples: 8,
                max_duration: 0.5,
                max_num_sources: 4,
                num_threads: 1,
                max_order: 1,
            });
        let mut simulator = Simulator::try_new(&context, &simulation_settings).unwrap();
        let scene = Scene::try_new(&context).unwrap();
        simulator.set_scene(&scene);
        simulator.commit();

        let mut simulation = Simulation::new::<()>(simulator);
        let mut reflections_simulation = simulation.spawn_reflections(|error| {
            eprintln!("{error}");
        });
        simulation.shutdown();
        reflections_simulation
            .join()
            .expect("simulation thread panicked");
    }

    #[test]
    fn test_initial_output_is_empty() {
        let context = Context::default();
        let audio_settings = AudioSettings::default();
        let simulation_settings =
            SimulationSettings::new(&audio_settings).with_reflections(ConvolutionSettings {
                max_num_rays: 128,
                num_diffuse_samples: 8,
                max_duration: 0.5,
                max_num_sources: 4,
                num_threads: 1,
                max_order: 1,
            });
        let mut simulator = Simulator::try_new(&context, &simulation_settings).unwrap();
        let scene = Scene::try_new(&context).unwrap();
        simulator.set_scene(&scene);
        simulator.commit();

        let mut simulation = Simulation::new::<()>(simulator);
        let mut reflections_simulation = simulation.spawn_reflections(|error| {
            eprintln!("{error}");
        });

        assert!(reflections_simulation.output().load().sources.is_empty());

        simulation.shutdown();
        reflections_simulation
            .join()
            .expect("simulation thread panicked");
    }
}

#[cfg(any(feature = "static", not(feature = "dynamic")))]
mod reflections_reverb {
    #[cfg(not(feature = "static"))]
    use audionimbus as static_api;
    #[cfg(feature = "static")]
    use audionimbus::r#static as static_api;
    use static_api::*;
    #[test]
    fn test_spawn_and_shutdown() {
        let context = Context::default();
        let audio_settings = AudioSettings::default();
        let simulation_settings =
            SimulationSettings::new(&audio_settings).with_reflections(ConvolutionSettings {
                max_num_rays: 128,
                num_diffuse_samples: 8,
                max_duration: 0.5,
                max_num_sources: 4,
                num_threads: 1,
                max_order: 1,
            });
        let mut simulator = Simulator::try_new(&context, &simulation_settings).unwrap();
        let scene = Scene::try_new(&context).unwrap();
        simulator.set_scene(&scene);
        simulator.commit();

        let simulator_clone = simulator.clone();
        let mut simulation = Simulation::new::<()>(simulator);

        let listener_source =
            Source::<(), Reflections, (), Convolution>::try_new(&simulator_clone).unwrap();
        simulator_clone.add_source(&listener_source);
        simulation.request_simulator_commit();

        let mut reverb_simulation = simulation.spawn_reflections_reverb::<(), ()>(|error| {
            eprintln!("{error}");
        });
        simulation.shutdown();
        reverb_simulation
            .join()
            .expect("simulation thread panicked");
    }

    #[test]
    fn test_initial_listener_output_is_none() {
        let context = Context::default();
        let audio_settings = AudioSettings::default();
        let simulation_settings =
            SimulationSettings::new(&audio_settings).with_reflections(ConvolutionSettings {
                max_num_rays: 128,
                num_diffuse_samples: 8,
                max_duration: 0.5,
                max_num_sources: 4,
                num_threads: 1,
                max_order: 1,
            });
        let mut simulator = Simulator::try_new(&context, &simulation_settings).unwrap();
        let scene = Scene::try_new(&context).unwrap();
        simulator.set_scene(&scene);
        simulator.commit();

        let simulator_clone = simulator.clone();
        let mut simulation = Simulation::new::<()>(simulator);

        let listener_source =
            Source::<(), Reflections, (), Convolution>::try_new(&simulator_clone).unwrap();
        simulator_clone.add_source(&listener_source);
        simulation.request_simulator_commit();

        let mut reverb_simulation = simulation.spawn_reflections_reverb::<(), ()>(|error| {
            eprintln!("{error}");
        });

        assert!(reverb_simulation.output().load().listener.is_none());

        simulation.shutdown();
        reverb_simulation
            .join()
            .expect("simulation thread panicked");
    }
}
