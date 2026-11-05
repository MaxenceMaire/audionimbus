#[cfg(all(feature = "static", feature = "dynamic"))]
/// The lock for independent native bake tests.
static BAKE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(any(feature = "static", not(feature = "dynamic")))]
mod common;

#[cfg(any(feature = "static", not(feature = "dynamic")))]
mod r#static {
    use super::common::sine_wave;
    #[cfg(not(feature = "static"))]
    use audionimbus as static_api;
    #[cfg(feature = "static")]
    use audionimbus::r#static as static_api;
    use static_api::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

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
        let input_buffer = AudioBufferRef::try_from(sine_wave.as_slice()).unwrap();

        // Must have 4 channels (1st order Ambisonics) for this example.
        let mut output_container = vec![0.0; 4 * input_buffer.num_samples()];
        let mut output_buffer =
            AudioBufferMut::try_new(output_container.as_mut_slice(), 4 as usize).unwrap();

        let reflection_effect_params = simulation_outputs.reflections();
        let _ =
            reflection_effect.apply(&reflection_effect_params, &input_buffer, &mut output_buffer);
    }

    #[test]
    fn test_pathing_without_probes() {
        #[cfg(all(feature = "static", feature = "dynamic"))]
        let _bake_lock = super::BAKE_LOCK.lock().unwrap();

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
        let simulator = Simulator::try_new(&context, &simulation_settings).unwrap();

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
    fn direct_simulation_retains_distance_callback() {
        let context = Context::default();
        let audio_settings = AudioSettings::default();
        let simulation_settings =
            SimulationSettings::new(&audio_settings).with_direct(DirectSimulationSettings {
                max_num_occlusion_samples: 1,
            });
        let simulator = Simulator::try_new(&context, &simulation_settings).unwrap();
        let source = Source::try_new(&simulator).unwrap();

        let calls = Arc::new(AtomicUsize::new(0));
        let retained_calls = Arc::downgrade(&calls);
        let callback_calls = calls.clone();
        let inputs = SimulationInputs {
            source: CoordinateSystem::default(),
            parameters: SimulationParameters::new().with_direct(
                DirectSimulationParameters::new().with_distance_attenuation(
                    DistanceAttenuationModel::Callback {
                        callback: DistanceAttenuationCallback::new(move |_| {
                            callback_calls.fetch_add(1, Ordering::Relaxed);
                            0.5
                        }),
                        dirty: false,
                    },
                ),
            ),
        };

        source.set_direct_inputs(&inputs).unwrap();
        drop(inputs);
        drop(calls);
        assert!(retained_calls.upgrade().is_some());

        simulator.add_source(&source);
        simulator
            .set_shared_direct_inputs(&SimulationSharedInputs::new(CoordinateSystem::default()))
            .unwrap();
        simulator.commit();
        simulator.run_direct();
        assert!(retained_calls.upgrade().unwrap().load(Ordering::Relaxed) > 0);

        let replacement = SimulationInputs {
            source: CoordinateSystem::default(),
            parameters: SimulationParameters::new().with_direct(
                DirectSimulationParameters::new()
                    .with_distance_attenuation(DistanceAttenuationModel::default()),
            ),
        };
        source.set_direct_inputs(&replacement).unwrap();
        assert!(retained_calls.upgrade().is_none());
        simulator.run_direct();
    }

    /// Accepts source declarations without simulation marker bounds.
    fn accepts_unconstrained_source<D, R, P, RE>(_: Option<static_api::Source<D, R, P, RE>>) {}

    /// Preserves the simulator's original marker bounds.
    fn accepts_static_simulator<T: RayTracer, D, R, P, RE>(
        _: Option<static_api::Simulator<T, D, R, P, RE>>,
    ) {
    }

    /// Preserves the direct input subset's explicit generic arguments.
    fn sets_static_source_inputs<D, R, P, RE>(
        source: &static_api::Source<D, R, P, RE>,
        inputs: &static_api::SimulationInputs<D, R, P>,
    ) -> Result<(), audionimbus::ParameterValidationError>
    where
        D: 'static + audionimbus::DirectCompatible<D> + audionimbus::SimulationFlagsProvider,
        R: 'static + audionimbus::ReflectionsCompatible<R> + audionimbus::SimulationFlagsProvider,
        P: 'static + audionimbus::PathingCompatible<P> + audionimbus::SimulationFlagsProvider,
        RE: 'static,
    {
        source.set_inputs_subset::<D, R, P, D, R, P>(inputs)
    }

    #[test]
    fn preserves_static_simulation_api() {
        use audionimbus::{
            AudioSettings, Direct, DirectSimulationParameters, DirectSimulationSettings,
        };
        use static_api::{
            SimulationInputs, SimulationParameters, SimulationSettings, SimulationSharedInputs,
            Simulator, Source,
        };
        accepts_unconstrained_source::<String, String, String, String>(None);
        accepts_static_simulator::<DefaultRayTracer, String, String, String, String>(None);
        let context = Context::default();
        let audio_settings = AudioSettings::default();
        let settings = SimulationSettings::new(&audio_settings)
            .with_direct(DirectSimulationSettings::default());
        let simulator = Simulator::try_new(&context, &settings).unwrap();
        let source = Source::<Direct>::try_new::<DefaultRayTracer>(&simulator).unwrap();
        let subset =
            Source::<Direct>::try_new_subset::<DefaultRayTracer, Direct, (), (), ()>(&simulator)
                .unwrap();
        simulator.add_source::<Direct, (), (), ()>(&source);
        simulator.remove_source::<Direct, (), (), ()>(&subset);
        let inputs = SimulationInputs {
            source: Default::default(),
            parameters: SimulationParameters::new().with_direct(DirectSimulationParameters::new()),
        };
        sets_static_source_inputs(&source, &inputs).unwrap();
        simulator
            .set_shared_inputs_subset::<Direct, (), (), (), (), ()>(&SimulationSharedInputs::new(
                Default::default(),
            ))
            .unwrap();
        simulator.commit();
        simulator.run_direct();
        let outputs = source.get_outputs_subset::<Direct, (), ()>().unwrap();
        let _: audionimbus::DirectEffectParams = outputs.direct();
        let _: fn(&Simulator<DefaultRayTracer, Direct>) = Simulator::run_direct;
        let _: fn(
            &Source<Direct>,
        ) -> Result<
            static_api::SimulationOutputs<Direct, (), ()>,
            audionimbus::SteamAudioError,
        > = Source::get_outputs;
    }
}

#[cfg(feature = "dynamic")]
mod dynamic {
    use audionimbus::dynamic as runtime;
    use audionimbus::simulation::error::SimulationError;
    use audionimbus::*;

    #[test]
    fn rejects_disabled_simulation_modes() {
        let context = Context::default();
        let simulation_settings = runtime::SimulationSettings::new(&AudioSettings::default());
        let simulator = runtime::Simulator::try_new(&context, &simulation_settings).unwrap();
        assert!(matches!(
            simulator.run_direct(),
            Err(runtime::SimulationError::DisabledModes(_))
        ));
        assert!(matches!(
            simulator.run_reflections(),
            Err(runtime::SimulationError::DisabledModes(_))
        ));
        assert!(matches!(
            simulator.run_pathing(),
            Err(runtime::SimulationError::DisabledModes(_))
        ));
        assert!(matches!(
            runtime::Source::try_new(&simulator, SimulationFlags::DIRECT),
            Err(runtime::SimulationError::DisabledModes(_))
        ));
    }

    #[test]
    fn processes_runtime_reflection_outputs() {
        use std::sync::Arc;
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
                        max_num_rays: 256,
                        num_diffuse_samples: 32,
                        max_duration: 1.0,
                        max_num_sources: 2,
                        num_threads: 1,
                        max_order: 1,
                    })
                }
                runtime::ReflectionAlgorithm::Parametric => {
                    runtime::ReflectionsSettings::Parametric(ParametricSettings {
                        max_num_rays: 256,
                        num_diffuse_samples: 32,
                        max_duration: 1.0,
                        max_num_sources: 2,
                        num_threads: 1,
                        max_order: 1,
                    })
                }
                runtime::ReflectionAlgorithm::Hybrid => {
                    runtime::ReflectionsSettings::Hybrid(HybridSettings {
                        max_num_rays: 256,
                        num_diffuse_samples: 32,
                        max_duration: 1.0,
                        max_num_sources: 2,
                        num_threads: 1,
                        max_order: 1,
                    })
                }
                _ => unreachable!(),
            };
            let simulation_settings = runtime::SimulationSettings::new(&audio_settings)
                .with_direct(DirectSimulationSettings::default())
                .with_reflections(reflections_settings);
            let mut simulator =
                runtime::Simulator::try_new(&context, &simulation_settings).unwrap();
            let mut scene =
                runtime::Scene::try_new(&context, runtime::RayTracerSettings::Default).unwrap();
            let mesh = runtime::StaticMesh::try_new(
                &scene,
                &StaticMeshSettings {
                    vertices: &[
                        Point::new(-4.0, -4.0, -4.0),
                        Point::new(4.0, -4.0, -4.0),
                        Point::new(4.0, 4.0, -4.0),
                        Point::new(-4.0, 4.0, -4.0),
                        Point::new(-4.0, -4.0, 4.0),
                        Point::new(4.0, -4.0, 4.0),
                        Point::new(4.0, 4.0, 4.0),
                        Point::new(-4.0, 4.0, 4.0),
                    ],
                    triangles: &[
                        Triangle::new(0, 2, 1),
                        Triangle::new(0, 3, 2),
                        Triangle::new(4, 5, 6),
                        Triangle::new(4, 6, 7),
                        Triangle::new(0, 1, 5),
                        Triangle::new(0, 5, 4),
                        Triangle::new(3, 7, 6),
                        Triangle::new(3, 6, 2),
                        Triangle::new(0, 4, 7),
                        Triangle::new(0, 7, 3),
                        Triangle::new(1, 2, 6),
                        Triangle::new(1, 6, 5),
                    ],
                    materials: &[Material::CONCRETE],
                    material_indices: &[0; 12],
                },
            )
            .unwrap();
            scene.try_add_static_mesh(mesh).unwrap();
            scene.commit();
            simulator.try_set_scene(&scene).unwrap();
            let source = runtime::Source::try_new(
                &simulator,
                SimulationFlags::DIRECT | SimulationFlags::REFLECTIONS,
            )
            .unwrap();
            simulator.try_add_source(&source).unwrap();
            simulator.commit();
            let reflections = match algorithm {
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
            let callback_lifetime = Arc::new(());
            let retained_callback = Arc::downgrade(&callback_lifetime);
            source
                .set_inputs(&runtime::SimulationInputs {
                    source: CoordinateSystem {
                        origin: Point::new(1.0, 0.0, 0.0),
                        ..Default::default()
                    },
                    parameters: runtime::SimulationParameters {
                        direct: Some(DirectSimulationParameters::new().with_distance_attenuation(
                            DistanceAttenuationModel::Callback {
                                callback: DistanceAttenuationCallback::new(move |_| {
                                    let _ = &callback_lifetime;
                                    1.0
                                }),
                                dirty: false,
                            },
                        )),
                        reflections: Some(reflections),
                        ..Default::default()
                    },
                })
                .unwrap();
            source
                .set_inputs_subset(
                    SimulationFlags::DIRECT,
                    &runtime::SimulationInputs::default(),
                )
                .unwrap();
            assert!(retained_callback.upgrade().is_some());
            let simulation_shared_inputs = runtime::SimulationSharedInputs::new(
                CoordinateSystem::default(),
            )
            .with_reflections(ReflectionsSharedInputs {
                num_rays: 256,
                num_bounces: 4,
                duration: 1.0,
                order: 1,
                irradiance_min_distance: 1.0,
            });
            simulator
                .set_shared_inputs(SimulationFlags::REFLECTIONS, &simulation_shared_inputs)
                .unwrap();
            simulator.run_reflections().unwrap();
            let aggregate_outputs = source
                .get_outputs_subset(SimulationFlags::REFLECTIONS)
                .unwrap();
            assert!(aggregate_outputs.direct().is_none());
            assert!(aggregate_outputs.pathing().is_none());
            assert_eq!(
                aggregate_outputs.reflections().unwrap().algorithm(),
                algorithm
            );
            let reflection_effect_params = source.get_reflections_outputs().unwrap();
            assert_eq!(reflection_effect_params.algorithm(), algorithm);
            drop(aggregate_outputs);
            simulator.try_remove_source(&source).unwrap();
            simulator.commit();
            drop(source);
            drop(simulator);
            drop(scene);
            let reflection_effect_settings = ReflectionEffectSettings {
                num_channels: 4,
                impulse_response_size: audio_settings.sampling_rate,
            };
            let mut reflection_effect = runtime::ReflectionEffect::try_new(
                &context,
                &audio_settings,
                &reflection_effect_settings,
                algorithm,
            )
            .unwrap();
            let mut input_samples = vec![0.0; audio_settings.frame_size as usize];
            input_samples[0] = 1.0;
            let mut output_samples = vec![0.0; 4 * audio_settings.frame_size as usize];
            let mut energy = 0.0_f64;
            for frame in 0..16 {
                let input_buffer = AudioBufferRef::try_from(input_samples.as_slice()).unwrap();
                let mut output_buffer = AudioBufferMut::try_new(&mut output_samples, 4).unwrap();
                reflection_effect
                    .apply(&reflection_effect_params, &input_buffer, &mut output_buffer)
                    .unwrap();
                assert!(
                    output_samples.iter().all(|sample| sample.is_finite()),
                    "{algorithm:?}, frame {frame}"
                );
                energy += output_samples
                    .iter()
                    .map(|sample| f64::from(*sample).powi(2))
                    .sum::<f64>();
                input_samples.fill(0.0);
            }
            assert!(energy > 0.0, "{algorithm:?} produced no reflected audio");
            let mut output_buffer = AudioBufferMut::try_new(&mut output_samples, 4).unwrap();
            reflection_effect.tail(&mut output_buffer).unwrap();
            reflection_effect.reset();
            if algorithm == runtime::ReflectionAlgorithm::Convolution {
                let mut reflection_mixer = runtime::ReflectionMixer::try_new(
                    &context,
                    &audio_settings,
                    &reflection_effect_settings,
                    algorithm,
                )
                .unwrap();
                input_samples[0] = 1.0;
                let input_buffer = AudioBufferRef::try_from(input_samples.as_slice()).unwrap();
                reflection_effect
                    .apply_into_mixer(
                        &reflection_effect_params,
                        &input_buffer,
                        &mut output_buffer,
                        &reflection_mixer,
                    )
                    .unwrap();
                reflection_mixer
                    .apply(&reflection_effect_params, &mut output_buffer)
                    .unwrap();
                reflection_effect
                    .tail_into_mixer(&mut output_buffer, &reflection_mixer)
                    .unwrap();
            }
        }
    }

    #[test]
    fn processes_direct_source_subsets() {
        let context = Context::default();
        let audio_settings = AudioSettings::default();
        let simulation_settings = runtime::SimulationSettings::new(&audio_settings)
            .with_direct(DirectSimulationSettings {
                max_num_occlusion_samples: 4,
            })
            .with_reflections(ConvolutionSettings::default());
        let simulator = runtime::Simulator::try_new(&context, &simulation_settings).unwrap();
        let source = runtime::Source::try_new(&simulator, SimulationFlags::DIRECT).unwrap();
        simulator.try_add_source(&source).unwrap();
        simulator.commit();
        let simulation_inputs = runtime::SimulationInputs {
            source: CoordinateSystem {
                origin: Point::new(0.0, 0.0, 2.0),
                ..Default::default()
            },
            parameters: runtime::SimulationParameters {
                direct: Some(
                    DirectSimulationParameters::new()
                        .with_distance_attenuation(DistanceAttenuationModel::default()),
                ),
                ..Default::default()
            },
        };
        source.set_inputs(&simulation_inputs).unwrap();
        simulator
            .set_shared_inputs(
                SimulationFlags::DIRECT,
                &runtime::SimulationSharedInputs::new(CoordinateSystem::default()),
            )
            .unwrap();
        simulator.run_direct().unwrap();
        let simulation_outputs = source.get_outputs().unwrap();
        assert!(simulation_outputs.reflections().is_none());
        assert!(simulation_outputs.pathing().is_none());
        let direct_effect_params = source.get_direct_outputs().unwrap();
        assert_eq!(direct_effect_params.distance_attenuation, Some(0.5));
        assert_eq!(
            simulation_outputs.direct().unwrap().distance_attenuation,
            direct_effect_params.distance_attenuation
        );
        let mut direct_effect = DirectEffect::try_new(
            &context,
            &audio_settings,
            &DirectEffectSettings { num_channels: 1 },
        )
        .unwrap();
        let input_samples = vec![1.0; audio_settings.frame_size as usize];
        let input_buffer = AudioBufferRef::try_from(input_samples.as_slice()).unwrap();
        let mut output_samples = vec![0.0; audio_settings.frame_size as usize];
        for _ in 0..2 {
            let mut output_buffer = AudioBufferMut::try_new(&mut output_samples, 1).unwrap();
            direct_effect
                .apply(&direct_effect_params, &input_buffer, &mut output_buffer)
                .unwrap();
        }
        assert!((output_samples[output_samples.len() - 1] - 0.5).abs() < 0.001);
        assert!(matches!(
            source.get_reflections_outputs(),
            Err(runtime::SimulationError::DisabledModes(_))
        ));
        let unknown = SimulationFlags::from_bits_retain(1 << 12);
        assert!(matches!(
            source.get_outputs_subset(unknown),
            Err(runtime::SimulationError::UnknownFlags(_))
        ));
        assert!(matches!(
            source.set_inputs_subset(unknown, &simulation_inputs),
            Err(runtime::SimulationError::UnknownFlags(_))
        ));
        assert!(matches!(
            simulator.set_shared_inputs(unknown, &runtime::SimulationSharedInputs::default()),
            Err(runtime::SimulationError::UnknownFlags(_))
        ));
        assert!(matches!(
            runtime::Source::try_new(&simulator, unknown),
            Err(runtime::SimulationError::UnknownFlags(_))
        ));
    }

    #[test]
    fn rejects_incompatible_source_relationships() {
        let context = Context::default();
        let audio_settings = AudioSettings::default();
        let simulation_settings = runtime::SimulationSettings::new(&audio_settings)
            .with_reflections(ConvolutionSettings::default());
        let simulator = runtime::Simulator::try_new(&context, &simulation_settings).unwrap();
        let other_simulator = runtime::Simulator::try_new(&context, &simulation_settings).unwrap();
        let source = runtime::Source::try_new(&simulator, SimulationFlags::REFLECTIONS).unwrap();
        assert!(matches!(
            other_simulator.try_add_source(&source),
            Err(runtime::SimulationError::ForeignSource)
        ));
        assert!(matches!(
            other_simulator.try_remove_source(&source),
            Err(runtime::SimulationError::ForeignSource)
        ));
        let clone = simulator.clone();
        clone.try_add_source(&source).unwrap();
        clone.try_remove_source(&source).unwrap();
        let simulation_inputs = runtime::SimulationInputs {
            source: CoordinateSystem::default(),
            parameters: runtime::SimulationParameters {
                reflections: Some(ParametricParameters::default().into()),
                ..Default::default()
            },
        };
        assert!(matches!(
            source.set_inputs(&simulation_inputs),
            Err(runtime::SimulationError::AlgorithmMismatch { .. })
        ));
        assert!(matches!(
            simulator.run_reflections(),
            Err(runtime::SimulationError::Simulation(
                SimulationError::ReflectionsWithoutScene
            ))
        ));
        assert!(matches!(
            simulator.set_shared_inputs(
                SimulationFlags::REFLECTIONS,
                &runtime::SimulationSharedInputs::default()
            ),
            Err(runtime::SimulationError::MissingInputs(_))
        ));
        let simulation_inputs = runtime::SimulationSharedInputs::new(CoordinateSystem::default())
            .with_reflections(ReflectionsSharedInputs {
                num_rays: 257,
                num_bounces: 1,
                duration: 1.0,
                order: 1,
                irradiance_min_distance: 1.0,
            });
        assert!(matches!(
            simulator.set_shared_inputs(SimulationFlags::REFLECTIONS, &simulation_inputs),
            Err(runtime::SimulationError::ParameterValidation(
                ParameterValidationError::NumRaysExceedsMax { .. }
            ))
        ));
        drop(simulator);
        assert!(matches!(
            other_simulator.try_add_source(&source),
            Err(runtime::SimulationError::ForeignSource)
        ));
    }

    #[test]
    fn preserves_pathing_subset_resources() {
        #[cfg(all(feature = "static", feature = "dynamic"))]
        let _bake_lock = super::BAKE_LOCK.lock().unwrap();

        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        let context = Context::default();
        let audio_settings = AudioSettings::default();
        let simulation_settings = runtime::SimulationSettings::new(&audio_settings)
            .with_direct(DirectSimulationSettings::default())
            .with_pathing(PathingSimulationSettings::default());
        let mut simulator = runtime::Simulator::try_new(&context, &simulation_settings).unwrap();
        assert!(matches!(
            simulator.run_pathing(),
            Err(runtime::SimulationError::Simulation(
                SimulationError::PathingWithoutProbes
            ))
        ));
        let mut probe_scene =
            runtime::Scene::try_new(&context, runtime::RayTracerSettings::Default).unwrap();
        let mut scene =
            runtime::Scene::try_new(&context, runtime::RayTracerSettings::Default).unwrap();
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
        let mesh = runtime::StaticMesh::try_new(&probe_scene, &mesh_settings).unwrap();
        probe_scene.try_add_static_mesh(mesh).unwrap();
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
        runtime::PathBaker::new()
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
        let deviation_lifetime = Arc::new(());
        let retained_deviation = Arc::downgrade(&deviation_lifetime);
        let deviation = DeviationModel::Callback(DeviationCallback::new(move |_, _| {
            let _ = &deviation_lifetime;
            1.0
        }));
        let simulation_inputs = runtime::SimulationInputs {
            source: CoordinateSystem {
                origin: Point::new(1.0, 1.0, 0.0),
                ..Default::default()
            },
            parameters: runtime::SimulationParameters {
                direct: Some(DirectSimulationParameters::new()),
                pathing: Some(PathingSimulationParameters {
                    pathing_probes: probe_batch,
                    visibility_radius: 0.1,
                    visibility_threshold: 0.5,
                    visibility_range: 10.0,
                    pathing_order: 1,
                    enable_validation: true,
                    find_alternate_paths: true,
                    deviation,
                }),
                ..Default::default()
            },
        };
        source.set_inputs(&simulation_inputs).unwrap();
        drop(simulation_inputs);
        source
            .set_inputs_subset(
                SimulationFlags::DIRECT,
                &runtime::SimulationInputs::default(),
            )
            .unwrap();
        assert!(retained_deviation.upgrade().is_some());
        let callback_calls = Arc::new(AtomicUsize::new(0));
        let callback_lifetime = Arc::new(());
        let retained_callback = Arc::downgrade(&callback_lifetime);
        let simulation_shared_inputs = runtime::SimulationSharedInputs::new(CoordinateSystem {
            origin: Point::new(-1.0, 1.0, 0.0),
            ..Default::default()
        })
        .with_pathing_visualization_callback(PathingVisualizationCallback::new({
            let callback_calls = callback_calls.clone();
            move |_, _, _| {
                let _ = &callback_lifetime;
                callback_calls.fetch_add(1, Ordering::Relaxed);
            }
        }));
        simulator
            .set_shared_inputs(SimulationFlags::PATHING, &simulation_shared_inputs)
            .unwrap();
        drop(simulation_shared_inputs);
        simulator
            .set_shared_inputs(
                SimulationFlags::DIRECT,
                &runtime::SimulationSharedInputs::default(),
            )
            .unwrap();
        assert!(retained_callback.upgrade().is_some());
        simulator.run_pathing().unwrap();
        let path_effect_params = source.get_pathing_outputs().unwrap();
        assert_eq!(path_effect_params.order, 1);
        assert_eq!(path_effect_params.sh_coeffs.len(), 4);
        let simulation_outputs = source.get_outputs_subset(SimulationFlags::PATHING).unwrap();
        assert!(simulation_outputs.direct().is_none());
        assert!(simulation_outputs.reflections().is_none());
        assert_eq!(simulation_outputs.pathing().unwrap().sh_coeffs.len(), 4);
        let mut path_effect = PathEffect::try_new(
            &context,
            &audio_settings,
            &PathEffectSettings {
                max_order: 1,
                spatialization: None,
            },
        )
        .unwrap();
        let input_samples = vec![1.0; audio_settings.frame_size as usize];
        let input_buffer = AudioBufferRef::try_from(input_samples.as_slice()).unwrap();
        let mut output_samples = vec![0.0; 4 * audio_settings.frame_size as usize];
        let mut output_buffer = AudioBufferMut::try_new(&mut output_samples, 4).unwrap();
        path_effect
            .apply(&path_effect_params, &input_buffer, &mut output_buffer)
            .unwrap();
        assert!(output_samples.iter().all(|sample| sample.is_finite()));
        assert!(output_samples.iter().any(|sample| sample.abs() > 0.0));
        source
            .set_inputs_subset(
                SimulationFlags::PATHING,
                &runtime::SimulationInputs::default(),
            )
            .unwrap();
        assert!(retained_deviation.upgrade().is_none());
        simulator
            .set_shared_inputs(
                SimulationFlags::PATHING,
                &runtime::SimulationSharedInputs::default(),
            )
            .unwrap();
        assert!(retained_callback.upgrade().is_none());
    }

    #[test]
    fn disables_omitted_source_modes() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        let context = Context::default();
        let simulation_settings = runtime::SimulationSettings::new(&AudioSettings::default())
            .with_direct(DirectSimulationSettings::default());
        let simulator = runtime::Simulator::try_new(&context, &simulation_settings).unwrap();
        let source = runtime::Source::try_new(&simulator, SimulationFlags::DIRECT).unwrap();
        simulator.try_add_source(&source).unwrap();
        simulator.commit();
        let callback_calls = Arc::new(AtomicUsize::new(0));
        let simulation_inputs = runtime::SimulationInputs {
            source: CoordinateSystem::default(),
            parameters: runtime::SimulationParameters {
                direct: Some(DirectSimulationParameters::new().with_distance_attenuation(
                    DistanceAttenuationModel::Callback {
                        callback: DistanceAttenuationCallback::new({
                            let callback_calls = callback_calls.clone();
                            move |_| {
                                callback_calls.fetch_add(1, Ordering::Relaxed);
                                0.5
                            }
                        }),
                        dirty: false,
                    },
                )),
                ..Default::default()
            },
        };
        source.set_inputs(&simulation_inputs).unwrap();
        simulator
            .set_shared_inputs(
                SimulationFlags::DIRECT,
                &runtime::SimulationSharedInputs::default(),
            )
            .unwrap();
        simulator.run_direct().unwrap();
        let enabled_calls = callback_calls.load(Ordering::Relaxed);
        assert!(enabled_calls > 0);
        source
            .set_inputs(&runtime::SimulationInputs::default())
            .unwrap();
        simulator.run_direct().unwrap();
        assert_eq!(callback_calls.load(Ordering::Relaxed), enabled_calls);
    }

    #[test]
    fn commits_runtime_scene_replacements() {
        use audionimbus::callback::{
            AnyHitCallback, BatchedAnyHitCallback, BatchedClosestHitCallback, ClosestHitCallback,
            CustomRayTracingCallbacks,
        };
        use std::sync::Arc;
        let context = Context::default();
        let callback_lifetime = Arc::new(());
        let retained_callback = Arc::downgrade(&callback_lifetime);
        let ray_tracer_settings = runtime::RayTracerSettings::Custom {
            callbacks: CustomRayTracingCallbacks::new(
                ClosestHitCallback::new(move |_, _, _| {
                    let _ = &callback_lifetime;
                    None
                }),
                AnyHitCallback::new(|_, _, _| false),
                BatchedClosestHitCallback::new(|rays, _, _| vec![None; rays.len()]),
                BatchedAnyHitCallback::new(|rays, _, _| vec![false; rays.len()]),
            ),
            ray_batch_size: 16,
        };
        let simulation_settings = runtime::SimulationSettings::new(&AudioSettings::default())
            .with_ray_tracer(ray_tracer_settings.clone())
            .with_reflections(ConvolutionSettings::default());
        let mut simulator = runtime::Simulator::try_new(&context, &simulation_settings).unwrap();
        let scene = runtime::Scene::try_new(&context, ray_tracer_settings).unwrap();
        let incompatible_scene =
            runtime::Scene::try_new(&context, runtime::RayTracerSettings::Default).unwrap();
        assert!(matches!(
            simulator.try_set_scene(&incompatible_scene),
            Err(runtime::SimulationError::RayTracerMismatch { .. })
        ));
        simulator.try_set_scene(&scene).unwrap();
        assert!(matches!(
            simulator.run_reflections(),
            Err(runtime::SimulationError::Simulation(
                SimulationError::ReflectionsWithoutScene
            ))
        ));
        simulator.commit();
        simulator.try_set_scene(&scene).unwrap();
        simulator.commit();
        drop(scene);
        assert!(retained_callback.upgrade().is_some());
        let replacement = runtime::Scene::try_new(
            &context,
            runtime::RayTracerSettings::Custom {
                callbacks: CustomRayTracingCallbacks::new(
                    ClosestHitCallback::new(|_, _, _| None),
                    AnyHitCallback::new(|_, _, _| false),
                    BatchedClosestHitCallback::new(|rays, _, _| vec![None; rays.len()]),
                    BatchedAnyHitCallback::new(|rays, _, _| vec![false; rays.len()]),
                ),
                ray_batch_size: 16,
            },
        )
        .unwrap();
        simulator.try_set_scene(&replacement).unwrap();
        assert!(retained_callback.upgrade().is_some());
        simulator.commit();
        assert!(retained_callback.upgrade().is_none());
        replacement.commit();
    }
}
