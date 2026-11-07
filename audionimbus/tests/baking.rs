/// The lock for independent native bake tests.
static BAKE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(static_api)]
mod r#static {
    #[cfg(not(feature = "static"))]
    use audionimbus as static_api;
    #[cfg(feature = "static")]
    use audionimbus::r#static as static_api;
    use static_api::*;

    #[test]
    pub fn test_baking() {
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

        let scene = Scene::try_new(&context).unwrap();
        simulator.set_scene(&scene);

        // This specifies a 100x100x100 axis-aligned box.
        let box_transform = Matrix::new([
            [100.0, 0.0, 0.0, 0.0],
            [0.0, 100.0, 0.0, 0.0],
            [0.0, 0.0, 100.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]);

        let mut probe_array = ProbeArray::try_new(&context).unwrap();

        let probe_params = ProbeGenerationParams::UniformFloor {
            spacing: 2.0,
            height: 1.5,
            transform: box_transform,
        };
        probe_array.generate_probes(&scene, &probe_params);

        let mut probe_batch = ProbeBatch::try_new(&context).unwrap();
        probe_batch.add_probe_array(&probe_array);

        probe_batch.commit();

        let identifier = BakedDataIdentifier::Reflections {
            variation: BakedDataVariation::StaticSource {
                endpoint_influence: Sphere {
                    center: Point::default(), // World-space position of the souce.
                    radius: 100.0, // Only bake reflections for probes within 100m of the source.
                },
            },
        };

        let reflections_bake_params = ReflectionsBakeParams {
            identifier,
            bake_flags: ReflectionsBakeFlags::BAKE_CONVOLUTION,
            num_rays: 32768,
            num_diffuse_samples: 1024,
            num_bounces: 64,
            simulated_duration: 2.0,
            saved_duration: 2.0,
            order: 2,
            num_threads: 8,
            irradiance_min_distance: 1.0,
            bake_batch_size: 0,
        };
        ReflectionsBaker::<DefaultRayTracer>::new()
            .bake(&context, &mut probe_batch, &scene, reflections_bake_params)
            .unwrap();

        simulator.add_probe_batch(&probe_batch);
        simulator.commit();
    }

    /// Preserves static probe calls through a generic scene reference.
    fn generates_static_probes<T: RayTracer>(
        probe_array: &mut audionimbus::ProbeArray,
        scene: &Scene<T>,
        params: &audionimbus::ProbeGenerationParams,
    ) {
        probe_array.generate_probes(scene, params);
    }

    #[test]
    fn preserves_static_baking_api() {
        use audionimbus::{
            PathBakeParams, ProbeArray, ProbeGenerationParams, ReflectionsBakeParams,
        };
        use static_api::{BakeError, PathBaker, ReflectionsBaker};
        let context = Context::default();
        let scene = Scene::try_new(&context).unwrap();
        let mut probe_array = ProbeArray::try_new(&context).unwrap();
        generates_static_probes(
            &mut probe_array,
            &scene,
            &ProbeGenerationParams::Centroid {
                transform: Matrix4::IDENTITY,
            },
        );
        fn bake(
            context: &Context,
            probe_batch: &mut ProbeBatch,
            scene: &Scene,
            reflections_params: ReflectionsBakeParams,
            path_params: PathBakeParams,
        ) -> Result<(), BakeError> {
            let reflections_baker = ReflectionsBaker::<'static, DefaultRayTracer>::new();
            reflections_baker.bake(context, probe_batch, scene, reflections_params)?;

            PathBaker::<DefaultRayTracer>::new().bake(context, probe_batch, scene, path_params)
        }
        let _ = bake;

        let _: ReflectionsBaker<'_, audionimbus::ray_tracing::Embree> =
            ReflectionsBaker::<audionimbus::ray_tracing::Embree>::new();
        let _: ReflectionsBaker<'_, audionimbus::ray_tracing::CustomRayTracer> =
            ReflectionsBaker::<audionimbus::ray_tracing::CustomRayTracer>::new(16);
    }

    /// Creates the scene used by the bake cases.
    fn test_scene(context: &Context) -> Scene<DefaultRayTracer> {
        let mut scene = Scene::try_new(context).unwrap();

        // Create a simple room mesh.
        let vertices = vec![
            // Floor
            Vector3::new(-5.0, 0.0, -5.0),
            Vector3::new(5.0, 0.0, -5.0),
            Vector3::new(5.0, 0.0, 5.0),
            Vector3::new(-5.0, 0.0, 5.0),
            // Ceiling
            Vector3::new(-5.0, 3.0, -5.0),
            Vector3::new(5.0, 3.0, -5.0),
            Vector3::new(5.0, 3.0, 5.0),
            Vector3::new(-5.0, 3.0, 5.0),
        ];

        let triangles = [
            // Floor
            [0, 1, 2],
            [0, 2, 3],
            // Ceiling
            [4, 6, 5],
            [4, 7, 6],
            // Walls
            [0, 4, 5],
            [0, 5, 1],
            [1, 5, 6],
            [1, 6, 2],
            [2, 6, 7],
            [2, 7, 3],
            [3, 7, 4],
            [3, 4, 0],
        ]
        .iter()
        .map(|indices| Triangle::new(indices[0], indices[1], indices[2]))
        .collect::<Vec<_>>();

        let material_indices = vec![0; triangles.len()];
        let materials = vec![Material::default()];

        let settings = StaticMeshSettings {
            vertices: &vertices,
            triangles: &triangles,
            material_indices: &material_indices,
            materials: &materials,
        };
        let static_mesh = StaticMesh::try_new(&scene, &settings).unwrap();
        scene.add_static_mesh(static_mesh);
        scene.commit();

        scene
    }

    /// Creates the probe batch used by the bake cases.
    fn test_probe_batch(context: &Context, scene: &Scene) -> ProbeBatch {
        let mut probe_batch = ProbeBatch::try_new(context).unwrap();

        let params = ProbeGenerationParams::Centroid {
            transform: Matrix4::new([
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ]),
        };
        let mut probe_array = ProbeArray::try_new(context).unwrap();
        probe_array.generate_probes(scene, &params);

        probe_batch.add_probe_array(&probe_array);
        probe_batch.commit();

        probe_batch
    }

    mod pathing {
        use super::*;

        /// Exercises native baking with and without progress callbacks.
        #[test]
        fn test_bake() {
            let _bake_lock = super::super::BAKE_LOCK.lock().unwrap();

            // Simple bake
            {
                let context = Context::default();
                let scene = test_scene(&context);
                let mut probe_batch = test_probe_batch(&context, &scene);

                let baker = PathBaker::<DefaultRayTracer>::new();

                let params = PathBakeParams {
                    identifier: BakedDataIdentifier::Pathing {
                        variation: BakedDataVariation::Dynamic,
                    },
                    num_samples: 4,
                    radius: 0.5,
                    threshold: 0.3,
                    visibility_range: 5.0,
                    path_range: 10.0,
                    num_threads: 1,
                };

                assert!(
                    baker
                        .bake(&context, &mut probe_batch, &scene, params)
                        .is_ok()
                );
            }

            // With progress callback
            {
                let context = Context::default();
                let scene = test_scene(&context);
                let mut probe_batch = test_probe_batch(&context, &scene);

                let baker = PathBaker::<DefaultRayTracer>::new();

                let params = PathBakeParams {
                    identifier: BakedDataIdentifier::Pathing {
                        variation: BakedDataVariation::Dynamic,
                    },
                    num_samples: 4,
                    radius: 0.5,
                    threshold: 0.3,
                    visibility_range: 5.0,
                    path_range: 10.0,
                    num_threads: 1,
                };

                assert!(
                    baker
                        .bake_with_progress_callback(
                            &context,
                            &mut probe_batch,
                            &scene,
                            params,
                            ProgressCallback::new(|progress| {
                                println!("pathing bake progress: {:.1}%", progress * 100.0);
                            }),
                        )
                        .is_ok()
                );
            }
        }
    }

    mod reflections {
        use super::*;

        /// Exercises native baking with and without progress callbacks.
        #[test]
        fn test_bake() {
            let _bake_lock = super::super::BAKE_LOCK.lock().unwrap();

            // Convolution
            {
                let context = Context::default();
                let scene = test_scene(&context);
                let mut probe_batch = test_probe_batch(&context, &scene);

                let baker = ReflectionsBaker::<DefaultRayTracer>::new();

                let params = ReflectionsBakeParams {
                    identifier: BakedDataIdentifier::Reflections {
                        variation: BakedDataVariation::Reverb,
                    },
                    bake_flags: ReflectionsBakeFlags::BAKE_CONVOLUTION,
                    num_rays: 1024,
                    num_diffuse_samples: 32,
                    num_bounces: 8,
                    simulated_duration: 2.0,
                    saved_duration: 2.0,
                    order: 1,
                    num_threads: 2,
                    irradiance_min_distance: 1.0,
                    bake_batch_size: 8,
                };

                assert!(
                    baker
                        .bake(&context, &mut probe_batch, &scene, params)
                        .is_ok()
                );
            }

            // Parametric
            {
                let context = Context::default();
                let scene = test_scene(&context);
                let mut probe_batch = test_probe_batch(&context, &scene);

                let baker = ReflectionsBaker::<DefaultRayTracer>::new();

                let params = ReflectionsBakeParams {
                    identifier: BakedDataIdentifier::Reflections {
                        variation: BakedDataVariation::Reverb,
                    },
                    bake_flags: ReflectionsBakeFlags::BAKE_PARAMETRIC,
                    num_rays: 512,
                    num_diffuse_samples: 16,
                    num_bounces: 4,
                    simulated_duration: 1.0,
                    saved_duration: 1.0,
                    order: 1,
                    num_threads: 1,
                    irradiance_min_distance: 0.5,
                    bake_batch_size: 4,
                };

                assert!(
                    baker
                        .bake(&context, &mut probe_batch, &scene, params)
                        .is_ok()
                );
            }

            // Both flags
            {
                let context = Context::default();
                let scene = test_scene(&context);
                let mut probe_batch = test_probe_batch(&context, &scene);

                let baker = ReflectionsBaker::<DefaultRayTracer>::new();

                let params = ReflectionsBakeParams {
                    identifier: BakedDataIdentifier::Reflections {
                        variation: BakedDataVariation::Reverb,
                    },
                    bake_flags: ReflectionsBakeFlags::BAKE_CONVOLUTION
                        | ReflectionsBakeFlags::BAKE_PARAMETRIC,
                    num_rays: 512,
                    num_diffuse_samples: 16,
                    num_bounces: 4,
                    simulated_duration: 1.0,
                    saved_duration: 1.0,
                    order: 1,
                    num_threads: 1,
                    irradiance_min_distance: 0.5,
                    bake_batch_size: 4,
                };

                assert!(
                    baker
                        .bake(&context, &mut probe_batch, &scene, params)
                        .is_ok()
                );
            }

            // Static source
            {
                let context = Context::default();
                let scene = test_scene(&context);
                let mut probe_batch = test_probe_batch(&context, &scene);

                let baker = ReflectionsBaker::<DefaultRayTracer>::new();

                let params = ReflectionsBakeParams {
                    identifier: BakedDataIdentifier::Reflections {
                        variation: BakedDataVariation::StaticSource {
                            endpoint_influence: Sphere {
                                center: Vector3::new(0.0, 1.5, 0.0),
                                radius: 1.0,
                            },
                        },
                    },
                    bake_flags: ReflectionsBakeFlags::BAKE_CONVOLUTION,
                    num_rays: 512,
                    num_diffuse_samples: 16,
                    num_bounces: 4,
                    simulated_duration: 1.0,
                    saved_duration: 1.0,
                    order: 1,
                    num_threads: 1,
                    irradiance_min_distance: 0.5,
                    bake_batch_size: 4,
                };

                assert!(
                    baker
                        .bake(&context, &mut probe_batch, &scene, params)
                        .is_ok()
                );
            }

            // With progress callback
            {
                let context = Context::default();
                let scene = test_scene(&context);
                let mut probe_batch = test_probe_batch(&context, &scene);

                let baker = ReflectionsBaker::<DefaultRayTracer>::new();

                let params = ReflectionsBakeParams {
                    identifier: BakedDataIdentifier::Reflections {
                        variation: BakedDataVariation::Reverb,
                    },
                    bake_flags: ReflectionsBakeFlags::BAKE_CONVOLUTION,
                    num_rays: 512,
                    num_diffuse_samples: 16,
                    num_bounces: 4,
                    simulated_duration: 1.0,
                    saved_duration: 1.0,
                    order: 1,
                    num_threads: 1,
                    irradiance_min_distance: 0.5,
                    bake_batch_size: 4,
                };

                assert!(
                    baker
                        .bake_with_progress_callback(
                            &context,
                            &mut probe_batch,
                            &scene,
                            params,
                            ProgressCallback::new(|progress| {
                                println!("baking progress: {:.1}%", progress * 100.0);
                            }),
                        )
                        .is_ok()
                );
            }
        }
    }
}

#[cfg(feature = "dynamic")]
mod dynamic {
    use audionimbus::baking::error::BakeError;
    use audionimbus::dynamic as runtime;
    use audionimbus::*;

    #[test]
    fn rejects_incompatible_bake_scenes() {
        use audionimbus::callback::{
            AnyHitCallback, BatchedAnyHitCallback, BatchedClosestHitCallback, ClosestHitCallback,
            CustomRayTracingCallbacks,
        };
        let context = Context::default();
        let scene = runtime::Scene::try_new(
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
        let ray_tracer_settings = runtime::RayTracerSettings::Default;
        let reflections_baker = runtime::ReflectionsBaker::new(&ray_tracer_settings);
        let mut probe_batch = ProbeBatch::try_new(&context).unwrap();
        let reflections_bake_params = ReflectionsBakeParams {
            identifier: BakedDataIdentifier::Reflections {
                variation: BakedDataVariation::Reverb,
            },
            bake_flags: ReflectionsBakeFlags::BAKE_PARAMETRIC,
            num_rays: 64,
            num_diffuse_samples: 16,
            num_bounces: 2,
            simulated_duration: 0.5,
            saved_duration: 0.5,
            order: 0,
            num_threads: 1,
            irradiance_min_distance: 1.0,
            bake_batch_size: 1,
        };
        assert!(matches!(
            reflections_baker.bake(&context, &mut probe_batch, &scene, reflections_bake_params),
            Err(runtime::BakeError::RayTracerMismatch {
                expected: runtime::RayTracerKind::Default,
                actual: runtime::RayTracerKind::Custom
            })
        ));
    }

    #[test]
    fn processes_runtime_baked_outputs() {
        let _bake_lock = super::BAKE_LOCK.lock().unwrap();

        use audionimbus::callback::ProgressCallback;
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        let context = Context::default();
        let ray_tracer_settings = runtime::RayTracerSettings::Default;
        let mut scene = runtime::Scene::try_new(&context, ray_tracer_settings.clone()).unwrap();
        let mesh = runtime::StaticMesh::try_new(
            &scene,
            &StaticMeshSettings {
                vertices: &[
                    Point::new(-4.0, 0.0, -4.0),
                    Point::new(4.0, 0.0, -4.0),
                    Point::new(4.0, 0.0, 4.0),
                    Point::new(-4.0, 0.0, 4.0),
                    Point::new(-4.0, 4.0, -4.0),
                    Point::new(4.0, 4.0, -4.0),
                    Point::new(4.0, 4.0, 4.0),
                    Point::new(-4.0, 4.0, 4.0),
                ],
                triangles: &[
                    Triangle::new(0, 1, 2),
                    Triangle::new(0, 2, 3),
                    Triangle::new(4, 6, 5),
                    Triangle::new(4, 7, 6),
                    Triangle::new(0, 4, 5),
                    Triangle::new(0, 5, 1),
                    Triangle::new(1, 5, 6),
                    Triangle::new(1, 6, 2),
                    Triangle::new(2, 6, 7),
                    Triangle::new(2, 7, 3),
                    Triangle::new(3, 7, 4),
                    Triangle::new(3, 4, 0),
                ],
                materials: &[Material::CONCRETE],
                material_indices: &[0; 12],
            },
        )
        .unwrap();
        scene.try_add_static_mesh(mesh).unwrap();
        scene.commit();
        let mut probe_array = ProbeArray::try_new(&context).unwrap();
        probe_array.generate_probes(
            &scene,
            &ProbeGenerationParams::UniformFloor {
                spacing: 2.0,
                height: 1.0,
                transform: Matrix4::new([
                    [4.0, 0.0, 0.0, 0.0],
                    [0.0, 4.0, 0.0, 0.0],
                    [0.0, 0.0, 4.0, 0.0],
                    [0.0, 0.0, 0.0, 1.0],
                ]),
            },
        );
        assert!(probe_array.num_probes() > 1);
        let mut probe_batch = ProbeBatch::try_new(&context).unwrap();
        probe_batch.add_probe_array(&probe_array);
        probe_batch.commit();
        let reflections_identifier = BakedDataIdentifier::Reflections {
            variation: BakedDataVariation::Reverb,
        };
        let nested_context = context.clone();
        #[cfg(feature = "static")]
        let nested_scene = audionimbus::r#static::Scene::try_new(&context).unwrap();
        let nested_runtime_scene = scene.clone();
        let nested_probe_batch = ProbeBatch::try_new(&context).unwrap();
        let busy_rejections = Arc::new(AtomicUsize::new(0));
        let progress_calls = Arc::new(AtomicUsize::new(0));
        let callback_lifetime = Arc::new(());
        let retained_callback = Arc::downgrade(&callback_lifetime);
        runtime::ReflectionsBaker::new(&ray_tracer_settings)
            .bake_with_progress_callback(
                &context,
                &mut probe_batch,
                &scene,
                ReflectionsBakeParams {
                    identifier: reflections_identifier,
                    bake_flags: ReflectionsBakeFlags::BAKE_CONVOLUTION
                        | ReflectionsBakeFlags::BAKE_PARAMETRIC,
                    num_rays: 256,
                    num_diffuse_samples: 32,
                    num_bounces: 4,
                    simulated_duration: 1.0,
                    saved_duration: 1.0,
                    order: 1,
                    num_threads: 1,
                    irradiance_min_distance: 1.0,
                    bake_batch_size: 1,
                },
                ProgressCallback::new({
                    let progress_calls = progress_calls.clone();
                    let busy_rejections = busy_rejections.clone();
                    move |_| {
                        let _ = &callback_lifetime;
                        progress_calls.fetch_add(1, Ordering::Relaxed);
                        let mut probe_batch = nested_probe_batch.clone();
                        #[cfg(feature = "static")]
                        {
                            let bake = audionimbus::r#static::PathBaker::new().bake(
                                &nested_context,
                                &mut probe_batch,
                                &nested_scene,
                                PathBakeParams {
                                    identifier: BakedDataIdentifier::Pathing {
                                        variation: BakedDataVariation::Dynamic,
                                    },
                                    num_samples: 1,
                                    radius: 0.1,
                                    threshold: 0.5,
                                    visibility_range: 10.0,
                                    path_range: 20.0,
                                    num_threads: 1,
                                },
                            );
                            if matches!(bake, Err(BakeError::BakeInProgress)) {
                                busy_rejections.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                        let bake = runtime::PathBaker::new().bake(
                            &nested_context,
                            &mut probe_batch,
                            &nested_runtime_scene,
                            PathBakeParams {
                                identifier: BakedDataIdentifier::Pathing {
                                    variation: BakedDataVariation::Dynamic,
                                },
                                num_samples: 1,
                                radius: 0.1,
                                threshold: 0.5,
                                visibility_range: 10.0,
                                path_range: 20.0,
                                num_threads: 1,
                            },
                        );
                        if matches!(bake, Err(BakeError::BakeInProgress)) {
                            busy_rejections.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }),
            )
            .unwrap();
        assert!(progress_calls.load(Ordering::Relaxed) > 0);
        let nested_bakes = if cfg!(feature = "static") { 2 } else { 1 };
        assert_eq!(
            busy_rejections.load(Ordering::Relaxed),
            nested_bakes * progress_calls.load(Ordering::Relaxed)
        );
        assert!(retained_callback.upgrade().is_none());
        assert!(probe_batch.data_size(reflections_identifier) > 0);
        let path_identifier = BakedDataIdentifier::Pathing {
            variation: BakedDataVariation::Dynamic,
        };
        let path_baker = runtime::PathBaker::default();
        let path_bake_params = PathBakeParams {
            identifier: path_identifier,
            num_samples: 1,
            radius: 0.1,
            threshold: 0.5,
            visibility_range: 10.0,
            path_range: 20.0,
            num_threads: 1,
        };
        path_baker
            .bake(&context, &mut probe_batch, &scene, path_bake_params)
            .unwrap();
        assert!(probe_batch.data_size(path_identifier) > 0);
        let path_progress_calls = Arc::new(AtomicUsize::new(0));
        path_baker
            .bake_with_progress_callback(
                &context,
                &mut probe_batch,
                &scene,
                path_bake_params,
                ProgressCallback::new({
                    let path_progress_calls = path_progress_calls.clone();
                    move |_| {
                        path_progress_calls.fetch_add(1, Ordering::Relaxed);
                    }
                }),
            )
            .unwrap();
        assert!(path_progress_calls.load(Ordering::Relaxed) > 0);
        let audio_settings = AudioSettings::default();
        let simulation_settings = runtime::SimulationSettings::new(&audio_settings)
            .with_reflections(ConvolutionSettings {
                max_num_rays: 256,
                num_diffuse_samples: 32,
                max_duration: 1.0,
                max_order: 1,
                max_num_sources: 1,
                num_threads: 1,
            })
            .with_pathing(PathingSimulationSettings::default(), 1);
        let mut simulator = runtime::Simulator::try_new(&context, &simulation_settings).unwrap();
        simulator.try_set_scene(&scene).unwrap();
        simulator.add_probe_batch(&probe_batch);
        let source = runtime::Source::try_new(
            &simulator,
            SimulationFlags::REFLECTIONS | SimulationFlags::PATHING,
        )
        .unwrap();
        simulator.try_add_source(&source).unwrap();
        simulator.commit();
        source
            .set_inputs(&runtime::SimulationInputs {
                source: CoordinateSystem {
                    origin: Point::new(1.0, 1.0, 1.0),
                    ..Default::default()
                },
                parameters: runtime::SimulationParameters {
                    reflections: Some(
                        ConvolutionParameters {
                            baked_data_identifier: Some(reflections_identifier),
                        }
                        .into(),
                    ),
                    pathing: Some(PathingSimulationParameters {
                        pathing_probes: probe_batch.clone(),
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
            })
            .unwrap();
        simulator
            .set_shared_inputs(
                SimulationFlags::REFLECTIONS | SimulationFlags::PATHING,
                &runtime::SimulationSharedInputs::new(CoordinateSystem {
                    origin: Point::new(2.0, 1.0, 1.0),
                    ..Default::default()
                })
                .with_reflections(ReflectionsSharedInputs {
                    num_rays: 256,
                    num_bounces: 4,
                    duration: 1.0,
                    order: 1,
                    irradiance_min_distance: 1.0,
                }),
            )
            .unwrap();
        simulator.run_reflections().unwrap();
        simulator.run_pathing().unwrap();
        let reflection_effect_params = source.get_reflections_outputs().unwrap();
        let mut reflection_effect = runtime::ReflectionEffect::try_new(
            &context,
            &audio_settings,
            &ReflectionEffectSettings {
                impulse_response_size: audio_settings.sampling_rate,
                num_channels: 4,
            },
            runtime::ReflectionAlgorithm::Convolution,
        )
        .unwrap();
        let mut input_samples = vec![0.0; audio_settings.frame_size as usize];
        input_samples[0] = 1.0;
        let mut output_samples = vec![0.0; 4 * audio_settings.frame_size as usize];
        let mut reflected_energy = 0.0_f64;
        for _ in 0..16 {
            let input_buffer = AudioBufferRef::try_from(input_samples.as_slice()).unwrap();
            let mut output_buffer = AudioBufferMut::try_new(&mut output_samples, 4).unwrap();
            reflection_effect
                .apply(&reflection_effect_params, &input_buffer, &mut output_buffer)
                .unwrap();
            assert!(output_samples.iter().all(|sample| sample.is_finite()));
            reflected_energy += output_samples
                .iter()
                .map(|sample| f64::from(*sample).powi(2))
                .sum::<f64>();
            input_samples.fill(0.0);
        }
        assert!(reflected_energy > 0.0);
        let path_effect_params = source.get_pathing_outputs().unwrap();
        assert_eq!(path_effect_params.sh_coeffs.len(), 4);
        let mut path_effect = PathEffect::try_new(
            &context,
            &audio_settings,
            &PathEffectSettings {
                max_order: 1,
                spatialization: None,
            },
        )
        .unwrap();
        input_samples.fill(1.0);
        let input_buffer = AudioBufferRef::try_from(input_samples.as_slice()).unwrap();
        let mut output_buffer = AudioBufferMut::try_new(&mut output_samples, 4).unwrap();
        path_effect
            .apply(&path_effect_params, &input_buffer, &mut output_buffer)
            .unwrap();
        assert!(output_samples.iter().all(|sample| sample.is_finite()));
        assert!(output_samples.iter().any(|sample| sample.abs() > 0.0));
    }

    #[test]
    #[ignore = "requires Radeon Rays and an OpenCL GPU"]
    fn rejects_incompatible_bake_devices() {
        use audionimbus::device::{
            OpenClDevice, OpenClDeviceList, OpenClDeviceSettings, OpenClDeviceType,
            RadeonRaysDevice,
        };
        let context = Context::default();
        let device_list = OpenClDeviceList::try_new(
            &context,
            &OpenClDeviceSettings {
                device_type: OpenClDeviceType::Gpu,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(device_list.num_devices() > 0);
        let open_cl_device = OpenClDevice::try_new(&context, &device_list, 0).unwrap();
        let radeon_rays_device = RadeonRaysDevice::try_new(&open_cl_device).unwrap();
        let scene = runtime::Scene::try_new(
            &context,
            runtime::RayTracerSettings::RadeonRays {
                open_cl_device: open_cl_device.clone(),
                radeon_rays_device: radeon_rays_device.clone(),
            },
        )
        .unwrap();
        let other_open_cl_device = OpenClDevice::try_new(&context, &device_list, 0).unwrap();
        let other_radeon_rays_device = RadeonRaysDevice::try_new(&open_cl_device).unwrap();
        let mut probe_batch = ProbeBatch::try_new(&context).unwrap();
        for ray_tracer_settings in [
            runtime::RayTracerSettings::RadeonRays {
                open_cl_device: other_open_cl_device,
                radeon_rays_device,
            },
            runtime::RayTracerSettings::RadeonRays {
                open_cl_device,
                radeon_rays_device: other_radeon_rays_device,
            },
        ] {
            assert_eq!(
                runtime::ReflectionsBaker::new(&ray_tracer_settings).bake(
                    &context,
                    &mut probe_batch,
                    &scene,
                    ReflectionsBakeParams {
                        identifier: BakedDataIdentifier::Reflections {
                            variation: BakedDataVariation::Reverb,
                        },
                        bake_flags: ReflectionsBakeFlags::BAKE_PARAMETRIC,
                        num_rays: 64,
                        num_diffuse_samples: 16,
                        num_bounces: 2,
                        simulated_duration: 0.5,
                        saved_duration: 0.5,
                        order: 0,
                        num_threads: 1,
                        irradiance_min_distance: 1.0,
                        bake_batch_size: 1,
                    },
                ),
                Err(runtime::BakeError::DeviceMismatch)
            );
        }
    }
}
