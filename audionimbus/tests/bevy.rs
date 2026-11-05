#![cfg(feature = "bevy")]

#[cfg(any(feature = "static", not(feature = "dynamic")))]
mod r#static {
    #[cfg(not(feature = "static"))]
    use audionimbus as core;
    use audionimbus::bevy::{
        asset, configuration::SimulationConfiguration, debug, error, geometry, plugin, runner,
        simulation, source,
    };
    #[cfg(feature = "static")]
    use audionimbus::r#static as core;
    use audionimbus::wiring as workers;
    use bevy::prelude::*;

    /// A downstream configuration without Clone, Debug, or Default.
    #[derive(TypePath)]
    struct CustomConfiguration;

    impl SimulationConfiguration for CustomConfiguration {
        type RayTracer = core::DefaultRayTracer;
        type Direct = core::Direct;
        type Reflections = core::Reflections;
        type Pathing = core::Pathing;
        type ReflectionEffect = core::Convolution;
    }

    #[test]
    fn preserves_custom_configuration() {
        fn register_direct_frames<C: SimulationConfiguration<Direct = core::Direct>>() {
            let _: fn(&mut App) = <runner::RunnerDirect as runner::SyncFrame<C>>::add_systems;
        }
        fn register_reflection_frames<
            C: SimulationConfiguration<Reflections = core::Reflections>,
        >() {
            let _: fn(&mut App) = <runner::RunnerReflections as runner::SyncFrame<C>>::add_systems;
            let _: fn(&mut App) =
                <runner::RunnerReflectionsReverb as runner::SyncFrame<C>>::add_systems;
        }
        fn register_pathing_frames<C: SimulationConfiguration<Pathing = core::Pathing>>() {
            let _: fn(&mut App) = <runner::RunnerPathing as runner::SyncFrame<C>>::add_systems;
        }
        register_direct_frames::<CustomConfiguration>();
        register_reflection_frames::<CustomConfiguration>();
        register_pathing_frames::<CustomConfiguration>();

        let context = core::Context::default();
        let settings = core::SimulationSettings::new(&core::AudioSettings::default())
            .with_direct(core::DirectSimulationSettings::default())
            .with_reflections(core::ConvolutionSettings::default())
            .with_pathing(core::PathingSimulationSettings::default());
        let mut pipeline = workers::Simulation::new::<Entity>(
            core::Simulator::try_new(&context, &settings).unwrap(),
        );
        let direct = runner::DirectSimulation::<CustomConfiguration>(pipeline.spawn_direct(|_| {}));
        let reflections = runner::ReflectionsSimulation::<CustomConfiguration>(
            pipeline.spawn_reflections(|_| {}),
        );
        let reverb = runner::ReflectionsReverbSimulation::<CustomConfiguration>(
            pipeline.spawn_reflections_reverb(|_| {}),
        );
        let pathing =
            runner::PathingSimulation::<CustomConfiguration>(pipeline.spawn_pathing(|_| {}));
        let simulation = simulation::Simulation::<CustomConfiguration>(pipeline);
        let source = source::Source::try_new(&simulation).unwrap();
        let _tuple_source = source::Source::<CustomConfiguration>(source.0.clone());
        let _: &core::Source<core::Direct, core::Reflections, core::Pathing, core::Convolution> =
            &source.0;
        let _: &workers::DirectSimulation<
            Entity,
            core::Reflections,
            core::Pathing,
            core::Convolution,
        > = &direct.0;
        let _: &workers::ReflectionsSimulation<
            Entity,
            core::Direct,
            core::Pathing,
            core::Convolution,
        > = &reflections.0;
        let _: &workers::ReflectionsReverbSimulation<
            Entity,
            core::Direct,
            core::Pathing,
            core::Convolution,
        > = &reverb.0;
        let _: &workers::PathingSimulation<
            Entity,
            core::Direct,
            core::Reflections,
            core::Convolution,
        > = &pathing.0;
        let scene = geometry::Scene::<CustomConfiguration>(core::Scene::try_new(&context).unwrap());
        let _: &core::Scene = &scene.0;
        let _: source::Source<CustomConfiguration> = source.0.clone().into();
        let _: geometry::Scene<CustomConfiguration> = scene.0.clone().into();
        let _: source::SourceParameters<CustomConfiguration> = Default::default();
        let shared_inputs = simulation::SimulationSharedInputs::<CustomConfiguration>(
            core::SimulationSharedInputs::default(),
        );
        let _: &core::SimulationSharedInputs<core::Direct, core::Reflections, core::Pathing> =
            &shared_inputs.0;
        let audio_plugin = plugin::SpatialAudioPlugin::with_config::<CustomConfiguration>(settings);
        let _default_plugin = plugin::SpatialAudioPlugin::new(
            core::SimulationSettings::new(&core::AudioSettings::default())
                .with_direct(core::DirectSimulationSettings::default())
                .with_reflections(core::ConvolutionSettings::default()),
        );
        let handle = Handle::<asset::SceneAsset>::default();
        let _: asset::SceneAssetSource<CustomConfiguration> =
            asset::SceneAssetSource::new(handle.clone());
        let _: audionimbus::bevy::SceneAssetSource = asset::SceneAssetSource::new(handle);
        let _debug_plugin = debug::SpatialAudioDebugPlugin::with_config::<CustomConfiguration>();
        let _default_debug_plugin = debug::SpatialAudioDebugPlugin::new();
        let mut app = App::new();
        app.add_observer(|event: On<error::SimulationErrorEvent>| {
            let _: audionimbus::wiring::SimulationStepError = event.error;
        });
        app.add_plugins(TransformPlugin)
            .init_resource::<Assets<Mesh>>()
            .add_plugins(audio_plugin);
        let mut schedule = app
            .world_mut()
            .resource_mut::<bevy::ecs::schedule::Schedules>()
            .remove(PostUpdate)
            .unwrap();
        schedule.graph_mut().initialize(app.world_mut());
        let source_id = app
            .world_mut()
            .register_component::<source::Source<CustomConfiguration>>();
        let simulation_id = app
            .world()
            .component_id::<simulation::Simulation<CustomConfiguration>>()
            .unwrap();
        let systems: Vec<_> = schedule
            .graph()
            .systems
            .iter()
            .map(|(key, _, _)| key)
            .collect();
        let mut publishers = 0;
        for key in systems {
            let system = schedule.graph_mut().systems.get_mut(key).unwrap();
            let access = bevy::ecs::system::System::initialize(system, app.world_mut());
            if !bevy::ecs::system::System::is_exclusive(system)
                && access.combined_access().has_read(source_id)
            {
                assert!(!access.combined_access().has_write(simulation_id));
                assert!(!bevy::ecs::system::System::has_deferred(system));
                publishers += 1;
            }
        }
        assert_eq!(publishers, 2);
        app.world()
            .resource::<simulation::Simulation<CustomConfiguration>>()
            .shutdown();
        simulation.shutdown();
    }

    #[test]
    fn computes_debug_defaults_once() {
        use bevy::asset::AssetPlugin;
        use bevy::gizmos::GizmoPlugin;
        use std::sync::atomic::{AtomicUsize, Ordering};

        /// The number of implicit parameter computations.
        static PARAMETER_CALLS: AtomicUsize = AtomicUsize::new(0);

        /// A configuration with observable implicit parameter construction.
        #[derive(TypePath)]
        struct DebugConfiguration;

        impl SimulationConfiguration for DebugConfiguration {
            type RayTracer = core::DefaultRayTracer;
            type Direct = core::Direct;
            type Reflections = core::Reflections;
            type Pathing = core::Pathing;
            type ReflectionEffect = core::Convolution;

            fn implicit_source_parameters()
            -> core::SimulationParameters<core::Direct, core::Reflections, core::Pathing>
            {
                PARAMETER_CALLS.fetch_add(1, Ordering::Relaxed);

                Default::default()
            }
        }

        let context = core::Context::default();
        let settings = core::SimulationSettings::new(&core::AudioSettings::default())
            .with_direct(core::DirectSimulationSettings::default())
            .with_reflections(core::ConvolutionSettings::default())
            .with_pathing(core::PathingSimulationSettings::default());
        let simulator = core::Simulator::try_new(&context, &settings).unwrap();
        let source = core::Source::try_new(&simulator).unwrap();
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            TransformPlugin,
            GizmoPlugin,
        ))
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<bevy::mesh::skinning::SkinnedMeshInverseBindposes>>()
        .add_plugins(debug::SpatialAudioDebugPlugin::with_config::<
            DebugConfiguration,
        >());
        let first = app
            .world_mut()
            .spawn((
                source::Source::<DebugConfiguration>(source.clone()),
                Transform::default(),
            ))
            .id();
        let second = app
            .world_mut()
            .spawn((
                source::Source::<DebugConfiguration>(source),
                Transform::default(),
            ))
            .id();
        app.update();
        assert_eq!(PARAMETER_CALLS.load(Ordering::Relaxed), 1);
        app.world_mut().entity_mut(first).despawn();
        app.world_mut().entity_mut(second).despawn();
        app.update();
        assert_eq!(PARAMETER_CALLS.load(Ordering::Relaxed), 2);
    }
}

#[cfg(feature = "dynamic")]
mod runtime {
    use audionimbus::dynamic::{self as core, bevy as audio};
    use audionimbus::{
        AudioSettings, Context, CoordinateSystem, DirectSimulationParameters,
        DirectSimulationSettings, DistanceAttenuationModel, SimulationFlags,
    };
    use bevy::prelude::*;
    use std::time::{Duration, Instant};

    #[test]
    fn publishes_sources_and_removes_registration() {
        let settings = core::SimulationSettings::new(&AudioSettings::default())
            .with_direct(DirectSimulationSettings::default());
        #[cfg(feature = "static")]
        use audionimbus::r#static as static_core;
        #[cfg(feature = "static")]
        let static_context = Context::default();
        #[cfg(feature = "static")]
        let static_simulator = static_core::Simulator::try_new(
            &static_context,
            &static_core::SimulationSettings::new(&AudioSettings::default())
                .with_direct(DirectSimulationSettings::default())
                .with_reflections(audionimbus::ConvolutionSettings::default()),
        )
        .unwrap();
        let mut app = App::new();
        app.add_plugins(TransformPlugin)
            .init_resource::<Assets<Mesh>>()
            .add_plugins(audio::SpatialAudioPlugin::with_settings(settings));
        let context = app.world().resource::<Context>().clone();
        let scene =
            audio::Scene::try_with_ray_tracer(&context, core::RayTracerSettings::Default).unwrap();
        app.world_mut().spawn((scene, audio::MainScene));
        let source = audio::Source::try_with_flags(
            app.world().resource::<audio::Simulation>(),
            SimulationFlags::DIRECT,
        )
        .unwrap();
        let retained_source = source.0.clone();
        let entity = app
            .world_mut()
            .spawn((source, Transform::from_xyz(0.0, 0.0, 2.0)))
            .id();
        #[cfg(feature = "static")]
        {
            let static_source = static_core::Source::try_new(&static_simulator).unwrap();
            app.world_mut().entity_mut(entity).insert((
                static_core::bevy::Source::from(static_source),
                static_core::bevy::SourceParameters::default(),
            ));
        }
        app.update();
        let output = app.world().resource::<audio::DirectSimulation>().output();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if output
                .load()
                .get(&entity)
                .is_some_and(|params| params.distance_attenuation == Some(0.5))
            {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "direct worker did not publish distance attenuation"
            );
            std::thread::sleep(Duration::from_millis(2));
        }
        app.world().resource::<audio::Simulation>().shutdown();
        app.world_mut()
            .resource_mut::<audio::DirectSimulation>()
            .join()
            .unwrap();
        app.world_mut().entity_mut(entity).remove::<audio::Source>();
        #[cfg(feature = "static")]
        assert!(
            app.world()
                .entity(entity)
                .contains::<audionimbus::r#static::bevy::Source>()
        );
        app.update();
        let simulation = app.world().resource::<audio::Simulation>();
        assert!(simulation.sources.load().sources.is_empty());
        simulation.simulator().commit();
        retained_source
            .set_inputs(&core::SimulationInputs {
                source: CoordinateSystem {
                    origin: audionimbus::Point::new(0.0, 0.0, 4.0),
                    ..Default::default()
                },
                parameters: core::SimulationParameters {
                    direct: Some(
                        DirectSimulationParameters::new()
                            .with_distance_attenuation(DistanceAttenuationModel::default()),
                    ),
                    ..Default::default()
                },
            })
            .unwrap();
        simulation.simulator().run_direct().unwrap();
        assert_eq!(
            retained_source
                .get_direct_outputs()
                .unwrap()
                .distance_attenuation,
            Some(0.5)
        );
    }

    #[test]
    fn selects_reflection_parameters() {
        for algorithm in [
            core::ReflectionAlgorithm::Convolution,
            core::ReflectionAlgorithm::Parametric,
            core::ReflectionAlgorithm::Hybrid,
        ] {
            let reflections = match algorithm {
                core::ReflectionAlgorithm::Convolution => {
                    core::ReflectionsSettings::Convolution(audionimbus::ConvolutionSettings {
                        max_num_rays: 64,
                        num_diffuse_samples: 8,
                        max_duration: 2.0,
                        max_num_sources: 4,
                        num_threads: 1,
                        max_order: 1,
                    })
                }
                core::ReflectionAlgorithm::Parametric => {
                    core::ReflectionsSettings::Parametric(audionimbus::ParametricSettings {
                        max_num_rays: 64,
                        num_diffuse_samples: 8,
                        max_duration: 2.0,
                        max_num_sources: 4,
                        num_threads: 1,
                        max_order: 1,
                    })
                }
                core::ReflectionAlgorithm::Hybrid => {
                    core::ReflectionsSettings::Hybrid(audionimbus::HybridSettings {
                        max_num_rays: 64,
                        num_diffuse_samples: 8,
                        max_duration: 2.0,
                        max_num_sources: 4,
                        num_threads: 1,
                        max_order: 1,
                    })
                }
                _ => unreachable!(),
            };
            let settings = core::SimulationSettings::new(&AudioSettings::default())
                .with_direct(DirectSimulationSettings::default())
                .with_reflections(reflections);
            let mut app = App::new();
            app.add_plugins(TransformPlugin)
                .init_resource::<Assets<Mesh>>()
                .add_plugins(
                    audio::SpatialAudioPlugin::with_settings(settings).with_runners(
                        audio::RunnerSettings {
                            direct: true,
                            reflections: audio::ReflectionsRunner::Sources,
                            pathing: false,
                        },
                    ),
                );
            app.world_mut()
                .resource_mut::<audio::SimulationSharedInputs>()
                .0
                .set_reflections_shared_inputs(audionimbus::ReflectionsSharedInputs {
                    num_rays: 64,
                    num_bounces: 2,
                    duration: 2.0,
                    order: 1,
                    irradiance_min_distance: 1.0,
                });
            let scene = audio::Scene::try_with_ray_tracer(
                app.world().resource::<Context>(),
                core::RayTracerSettings::Default,
            )
            .unwrap();
            app.world_mut().spawn((scene, audio::MainScene));
            let direct = audio::Source::try_with_flags(
                app.world().resource::<audio::Simulation>(),
                SimulationFlags::DIRECT,
            )
            .unwrap();
            let reflections = audio::Source::try_with_flags(
                app.world().resource::<audio::Simulation>(),
                SimulationFlags::REFLECTIONS,
            )
            .unwrap();
            let direct_entity = app.world_mut().spawn((direct, Transform::default())).id();
            let reflected_entity = app
                .world_mut()
                .spawn((reflections, Transform::from_xyz(0.0, 0.0, 2.0)))
                .id();
            app.update();
            let simulation = app.world().resource::<audio::Simulation>();
            let snapshot = simulation.sources.load();
            assert_eq!(snapshot.direct.len(), 1);
            assert_eq!(snapshot.reflections.len(), 1);
            assert_eq!(snapshot.direct[0].0, direct_entity);
            assert_eq!(snapshot.reflections[0].0, reflected_entity);
            assert_eq!(
                snapshot.reflections[0]
                    .1
                    .simulation_inputs
                    .parameters
                    .reflections
                    .as_ref()
                    .unwrap()
                    .algorithm(),
                algorithm
            );
            assert!(
                snapshot.direct[0]
                    .1
                    .simulation_inputs
                    .parameters
                    .reflections
                    .is_none()
            );
            drop(snapshot);
            let output = app
                .world()
                .resource::<audio::ReflectionsSimulation>()
                .output();
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                if output.load().sources.contains_key(&reflected_entity) {
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "reflections worker did not publish source"
                );
                std::thread::sleep(Duration::from_millis(2));
            }
            let snapshot = output.load();
            assert!(!snapshot.sources.contains_key(&direct_entity));
            assert_eq!(snapshot.sources[&reflected_entity].algorithm(), algorithm);
            simulation.shutdown();
        }
    }

    #[test]
    fn publishes_listener_reverb() {
        let settings = core::SimulationSettings::new(&AudioSettings::default())
            .with_direct(DirectSimulationSettings::default())
            .with_reflections(audionimbus::ParametricSettings {
                max_num_rays: 64,
                num_diffuse_samples: 8,
                max_duration: 0.25,
                max_num_sources: 4,
                num_threads: 1,
                max_order: 1,
            });
        let mut app = App::new();
        app.add_plugins(TransformPlugin)
            .init_resource::<Assets<Mesh>>()
            .add_plugins(audio::SpatialAudioPlugin::with_settings(settings));
        app.world_mut()
            .resource_mut::<audio::SimulationSharedInputs>()
            .0
            .set_reflections_shared_inputs(audionimbus::ReflectionsSharedInputs {
                num_rays: 64,
                num_bounces: 2,
                duration: 0.25,
                order: 1,
                irradiance_min_distance: 1.0,
            });
        let scene = audio::Scene::try_with_ray_tracer(
            app.world().resource::<Context>(),
            core::RayTracerSettings::Default,
        )
        .unwrap();
        app.world_mut().spawn((scene, audio::MainScene));
        let listener = audio::Source::try_with_flags(
            app.world().resource::<audio::Simulation>(),
            SimulationFlags::REFLECTIONS,
        )
        .unwrap();
        let listener_entity = app
            .world_mut()
            .spawn((listener, audio::Listener, Transform::default()))
            .id();
        let source = audio::Source::try_with_flags(
            app.world().resource::<audio::Simulation>(),
            SimulationFlags::REFLECTIONS,
        )
        .unwrap();
        let source_entity = app
            .world_mut()
            .spawn((source, Transform::from_xyz(0.0, 0.0, 1.0)))
            .id();
        app.update();
        {
            let simulation = app.world().resource::<audio::Simulation>();
            let publication = simulation.sources.load();
            let listener = publication.listener.as_ref().unwrap();
            assert_eq!(
                listener
                    .simulation_inputs
                    .parameters
                    .reflections
                    .as_ref()
                    .unwrap()
                    .algorithm(),
                core::ReflectionAlgorithm::Parametric
            );
        }
        let output = app
            .world()
            .resource::<audio::ReflectionsReverbSimulation>()
            .output();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let snapshot = output.load();
            if snapshot.listener.is_some() && snapshot.sources.contains_key(&source_entity) {
                assert_eq!(
                    snapshot.listener.as_ref().unwrap().algorithm(),
                    core::ReflectionAlgorithm::Parametric
                );
                assert!(!snapshot.sources.contains_key(&listener_entity));
                break;
            }
            assert!(
                Instant::now() < deadline,
                "reverb worker did not publish listener"
            );
            std::thread::sleep(Duration::from_millis(2));
        }
        app.world_mut().despawn(listener_entity);
        let direct_listener = audio::Source::try_with_flags(
            app.world().resource::<audio::Simulation>(),
            SimulationFlags::DIRECT,
        )
        .unwrap();
        app.world_mut()
            .spawn((direct_listener, audio::Listener, Transform::default()));
        app.update();
        assert!(
            app.world()
                .resource::<audio::Simulation>()
                .sources
                .load()
                .listener
                .is_none()
        );
        app.world().resource::<audio::Simulation>().shutdown();
    }

    #[test]
    fn publishes_pathing_sources() {
        use audionimbus::{
            BakedDataIdentifier, BakedDataVariation, Matrix4, PathBakeParams,
            PathingSimulationParameters, Point, ProbeGenerationParams, StaticMeshSettings,
            Triangle,
        };
        let context = Context::default();
        let mut scene = core::Scene::try_new(&context, core::RayTracerSettings::Default).unwrap();
        let vertices = [
            Point::new(-2.0, 0.0, -2.0),
            Point::new(-2.0, 0.0, 2.0),
            Point::new(2.0, 0.0, -2.0),
            Point::new(2.0, 0.0, 2.0),
        ];
        let triangles = [Triangle::new(0, 1, 2), Triangle::new(2, 1, 3)];
        let mesh = core::StaticMesh::try_new(
            &scene,
            &StaticMeshSettings {
                vertices: &vertices,
                triangles: &triangles,
                materials: &[audionimbus::Material::default()],
                material_indices: &[0, 0],
            },
        )
        .unwrap();
        scene.try_add_static_mesh(mesh).unwrap();
        scene.commit();
        let mut probes = audionimbus::ProbeArray::try_new(&context).unwrap();
        probes.generate_probes(
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
        assert!(probes.num_probes() > 0);
        let mut probe_batch = audionimbus::ProbeBatch::try_new(&context).unwrap();
        probe_batch.add_probe_array(&probes);
        probe_batch.commit();
        core::PathBaker::new()
            .bake(
                &context,
                &mut probe_batch,
                &scene,
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
        let settings = core::SimulationSettings::new(&AudioSettings::default())
            .with_direct(DirectSimulationSettings::default())
            .with_pathing(audionimbus::PathingSimulationSettings {
                num_visibility_samples: 1,
            });
        let mut app = App::new();
        app.insert_resource(context)
            .add_plugins(TransformPlugin)
            .init_resource::<Assets<Mesh>>()
            .add_plugins(audio::SpatialAudioPlugin::with_settings(settings));
        app.world_mut()
            .spawn((audio::Scene::from(scene), audio::MainScene));
        app.world_mut()
            .spawn(audio::ProbeBatch::from(probe_batch.clone()));
        let source = audio::Source::try_with_flags(
            app.world().resource::<audio::Simulation>(),
            SimulationFlags::PATHING,
        )
        .unwrap();
        let entity = app
            .world_mut()
            .spawn((
                source,
                audio::SourceParameters::from(core::SimulationParameters {
                    pathing: Some(PathingSimulationParameters {
                        pathing_probes: probe_batch,
                        visibility_radius: 0.1,
                        visibility_threshold: 0.5,
                        visibility_range: 10.0,
                        pathing_order: 1,
                        enable_validation: true,
                        find_alternate_paths: true,
                        deviation: audionimbus::DeviationModel::default(),
                    }),
                    ..Default::default()
                }),
                Transform::from_xyz(1.0, 1.0, 0.0),
            ))
            .id();
        app.world_mut()
            .spawn((audio::Listener, Transform::from_xyz(-1.0, 1.0, 0.0)));
        app.update();
        let simulation = app.world().resource::<audio::Simulation>();
        assert_eq!(simulation.sources.load().pathing.len(), 1);
        assert!(simulation.sources.load().direct.is_empty());
        let output = app.world().resource::<audio::PathingSimulation>().output();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !output.load().contains_key(&entity) {
            assert!(
                Instant::now() < deadline,
                "pathing worker did not publish source"
            );
            std::thread::sleep(Duration::from_millis(2));
        }
        simulation.shutdown();
    }

    #[test]
    fn registers_and_removes_geometry() {
        let settings = core::SimulationSettings::new(&AudioSettings::default())
            .with_direct(DirectSimulationSettings::default());
        let mut app = App::new();
        app.add_plugins(TransformPlugin)
            .init_resource::<Assets<Mesh>>()
            .add_plugins(audio::SpatialAudioPlugin::with_settings(settings));
        let context = app.world().resource::<Context>();
        let scene =
            audio::Scene::try_with_ray_tracer(context, core::RayTracerSettings::Default).unwrap();
        let sub_scene =
            audio::Scene::try_with_ray_tracer(context, core::RayTracerSettings::Default).unwrap();
        let scene_entity = app
            .world_mut()
            .spawn((scene, audio::MainScene, Transform::default()))
            .id();
        let sub_scene_entity = app
            .world_mut()
            .spawn((sub_scene, Transform::default()))
            .id();
        let mesh = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(Mesh::from(Cuboid::default()));
        let mesh_entity = app
            .world_mut()
            .spawn((
                audio::StaticMesh,
                Mesh3d(mesh),
                Transform::default(),
                ChildOf(sub_scene_entity),
            ))
            .id();
        let instance_entity = app
            .world_mut()
            .spawn((
                audio::InstancedMesh(sub_scene_entity),
                Transform::from_xyz(0.0, 0.0, 1.0),
                ChildOf(scene_entity),
            ))
            .id();
        let source = audio::Source::try_with_flags(
            app.world().resource::<audio::Simulation>(),
            SimulationFlags::DIRECT,
        )
        .unwrap();
        let source_entity = app
            .world_mut()
            .spawn((source, Transform::from_xyz(0.0, 0.0, 2.0)))
            .id();
        let output = app.world().resource::<audio::DirectSimulation>().output();
        for (stage, occlusion) in [(0, 0.0), (1, 1.0), (2, 0.0), (3, 1.0)] {
            match stage {
                1 => {
                    app.world_mut()
                        .entity_mut(instance_entity)
                        .remove::<audio::InstancedMesh>();
                }
                2 => {
                    app.world_mut()
                        .entity_mut(instance_entity)
                        .insert(audio::InstancedMesh(sub_scene_entity));
                }
                3 => {
                    app.world_mut()
                        .entity_mut(mesh_entity)
                        .remove::<audio::StaticMesh>();
                }
                _ => {}
            }
            app.update();
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                if output
                    .load()
                    .get(&source_entity)
                    .is_some_and(|params| params.occlusion == Some(occlusion))
                {
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "geometry stage {stage} did not produce occlusion {occlusion}"
                );
                std::thread::sleep(Duration::from_millis(2));
            }
        }
        app.world().resource::<audio::Simulation>().shutdown();
    }

    #[derive(Resource, Default)]
    struct MissingScenes(Vec<audio::SimulationThread>);

    #[test]
    fn propagates_runtime_worker_errors() {
        let settings = core::SimulationSettings::new(&AudioSettings::default())
            .with_reflections(audionimbus::ConvolutionSettings::default());
        let mut app = App::new();
        app.add_plugins(TransformPlugin)
            .init_resource::<Assets<Mesh>>()
            .init_resource::<MissingScenes>()
            .add_plugins(audio::SpatialAudioPlugin::with_settings(settings));
        app.add_observer(
            |event: On<audio::SimulationErrorEvent>, mut errors: ResMut<MissingScenes>| {
                if matches!(
                    event.error,
                    core::SimulationError::Simulation(
                        audionimbus::simulation::error::SimulationError::ReflectionsWithoutScene
                    )
                ) {
                    errors.0.push(event.thread);
                }
            },
        );
        app.world_mut()
            .resource_mut::<audio::SimulationSharedInputs>()
            .0
            .set_reflections_shared_inputs(audionimbus::ReflectionsSharedInputs {
                num_rays: 64,
                num_bounces: 2,
                duration: 0.25,
                order: 1,
                irradiance_min_distance: 1.0,
            });
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            app.update();
            if app
                .world()
                .resource::<MissingScenes>()
                .0
                .contains(&audio::SimulationThread::ReflectionsReverb)
            {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "worker error did not reach observer"
            );
            std::thread::sleep(Duration::from_millis(2));
        }
        app.world().resource::<audio::Simulation>().shutdown();
    }

    /// Integration failures observed by the application.
    #[derive(Resource, Default)]
    struct IntegrationErrors(Vec<(Option<Entity>, audio::IntegrationOperation)>);

    #[test]
    fn reports_rejected_sources() {
        use std::sync::atomic::Ordering;
        let settings = core::SimulationSettings::new(&AudioSettings::default())
            .with_direct(DirectSimulationSettings::default());
        let mut app = App::new();
        app.add_plugins(TransformPlugin)
            .init_resource::<Assets<Mesh>>()
            .init_resource::<IntegrationErrors>()
            .add_plugins(
                audio::SpatialAudioPlugin::with_settings(settings).with_runners(
                    audio::RunnerSettings {
                        direct: false,
                        reflections: audio::ReflectionsRunner::Disabled,
                        pathing: false,
                    },
                ),
            );
        app.add_observer(
            |event: On<audio::IntegrationErrorEvent>, mut errors: ResMut<IntegrationErrors>| {
                assert!(matches!(event.error, core::SimulationError::ForeignSource));
                errors.0.push((event.entity, event.operation));
            },
        );
        let valid_source = audio::Source::try_with_flags(
            app.world().resource::<audio::Simulation>(),
            SimulationFlags::DIRECT,
        )
        .unwrap();
        app.world_mut().spawn((
            valid_source,
            Transform::default(),
            GlobalTransform::default(),
        ));
        app.update();
        assert!(app.world().resource::<IntegrationErrors>().0.is_empty());
        app.world()
            .resource::<audio::Simulation>()
            .simulator_commit_needed
            .store(false, Ordering::Relaxed);
        let foreign_simulator = core::Simulator::try_new(
            app.world().resource::<Context>(),
            &core::SimulationSettings::new(&AudioSettings::default())
                .with_direct(DirectSimulationSettings::default()),
        )
        .unwrap();
        let foreign_source =
            core::Source::try_new(&foreign_simulator, SimulationFlags::DIRECT).unwrap();
        let entity = app
            .world_mut()
            .spawn((
                audio::Source::from(foreign_source),
                Transform::default(),
                GlobalTransform::default(),
            ))
            .id();
        app.world_mut().flush();
        assert!(
            !app.world()
                .resource::<audio::Simulation>()
                .simulator_commit_needed
                .load(Ordering::Relaxed)
        );
        app.update();
        {
            let errors = &app.world().resource::<IntegrationErrors>().0;
            assert_eq!(errors.len(), 2);
            assert_eq!(errors[0].1, audio::IntegrationOperation::AddSource);
            assert_eq!(errors[0].0, Some(entity));
            assert_eq!(errors[1].1, audio::IntegrationOperation::PublishSources);
            assert_eq!(errors[1].0, None);
        }
        app.world_mut().entity_mut(entity).remove::<audio::Source>();
        app.world_mut().flush();
        assert!(
            !app.world()
                .resource::<audio::Simulation>()
                .simulator_commit_needed
                .load(Ordering::Relaxed)
        );
        app.update();
        let errors = &app.world().resource::<IntegrationErrors>().0;
        assert_eq!(errors.len(), 3);
        assert_eq!(errors[2].1, audio::IntegrationOperation::RemoveSource);
        assert_eq!(errors[2].0, Some(entity));
        app.world().resource::<audio::Simulation>().shutdown();
    }

    #[test]
    fn rejects_incompatible_main_scene() {
        use audionimbus::callback::{
            AnyHitCallback, BatchedAnyHitCallback, BatchedClosestHitCallback, ClosestHitCallback,
            CustomRayTracingCallbacks,
        };
        use std::sync::atomic::Ordering;
        let settings = core::SimulationSettings::new(&AudioSettings::default())
            .with_direct(DirectSimulationSettings::default());
        let mut app = App::new();
        app.add_plugins(TransformPlugin)
            .init_resource::<Assets<Mesh>>()
            .init_resource::<IntegrationErrors>()
            .add_plugins(
                audio::SpatialAudioPlugin::with_settings(settings).with_runners(
                    audio::RunnerSettings {
                        direct: false,
                        reflections: audio::ReflectionsRunner::Disabled,
                        pathing: false,
                    },
                ),
            );
        app.add_observer(
            |event: On<audio::IntegrationErrorEvent>, mut errors: ResMut<IntegrationErrors>| {
                assert!(matches!(
                    event.error,
                    core::SimulationError::RayTracerMismatch {
                        expected: core::RayTracerKind::Default,
                        actual: core::RayTracerKind::Custom,
                    }
                ));
                errors.0.push((event.entity, event.operation));
            },
        );
        let previous_scene = audio::Scene::try_with_ray_tracer(
            app.world().resource::<Context>(),
            core::RayTracerSettings::Default,
        )
        .unwrap();
        let previous_entity = app
            .world_mut()
            .spawn((previous_scene, audio::MainScene))
            .id();
        app.update();
        app.world()
            .resource::<audio::Simulation>()
            .simulator_commit_needed
            .store(false, Ordering::Relaxed);
        let rejected_scene = audio::Scene::try_with_ray_tracer(
            app.world().resource::<Context>(),
            core::RayTracerSettings::Custom {
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
        let rejected_entity = app
            .world_mut()
            .spawn((rejected_scene, audio::MainScene))
            .id();
        app.world_mut().flush();
        assert!(
            app.world()
                .get::<audio::MainScene>(previous_entity)
                .is_some()
        );
        assert!(
            app.world()
                .get::<audio::MainScene>(rejected_entity)
                .is_none()
        );
        assert!(
            !app.world()
                .resource::<audio::Simulation>()
                .simulator_commit_needed
                .load(Ordering::Relaxed)
        );
        let errors = &app.world().resource::<IntegrationErrors>().0;
        assert_eq!(
            errors.as_slice(),
            &[(Some(rejected_entity), audio::IntegrationOperation::SetScene)]
        );
        app.world().resource::<audio::Simulation>().shutdown();
    }

    #[test]
    fn reports_rejected_listener() {
        let settings = core::SimulationSettings::new(&AudioSettings::default()).with_reflections(
            audionimbus::ParametricSettings {
                max_num_rays: 64,
                num_diffuse_samples: 8,
                max_duration: 0.25,
                max_num_sources: 4,
                num_threads: 1,
                max_order: 1,
            },
        );
        let mut app = App::new();
        app.add_plugins(TransformPlugin)
            .init_resource::<Assets<Mesh>>()
            .init_resource::<IntegrationErrors>()
            .add_plugins(audio::SpatialAudioPlugin::with_settings(settings));
        app.add_observer(
            |event: On<audio::IntegrationErrorEvent>, mut errors: ResMut<IntegrationErrors>| {
                assert!(matches!(event.error, core::SimulationError::ForeignSource));
                errors.0.push((event.entity, event.operation));
            },
        );
        let scene = audio::Scene::try_with_ray_tracer(
            app.world().resource::<Context>(),
            core::RayTracerSettings::Default,
        )
        .unwrap();
        app.world_mut().spawn((scene, audio::MainScene));
        let simulator = core::Simulator::try_new(
            app.world().resource::<Context>(),
            &core::SimulationSettings::new(&AudioSettings::default()).with_reflections(
                audionimbus::ParametricSettings {
                    max_num_rays: 64,
                    num_diffuse_samples: 8,
                    max_duration: 0.25,
                    max_num_sources: 4,
                    num_threads: 1,
                    max_order: 1,
                },
            ),
        )
        .unwrap();
        let source = core::Source::try_new(&simulator, SimulationFlags::REFLECTIONS).unwrap();
        let entity = app
            .world_mut()
            .spawn((
                audio::Source::from(source),
                audio::Listener,
                Transform::default(),
                GlobalTransform::default(),
            ))
            .id();
        app.update();
        let errors = &app.world().resource::<IntegrationErrors>().0;
        assert!(errors.contains(&(Some(entity), audio::IntegrationOperation::AddSource)));
        assert!(errors.contains(&(Some(entity), audio::IntegrationOperation::PublishListener)));
        assert!(
            !errors
                .iter()
                .any(|(_, operation)| *operation == audio::IntegrationOperation::PublishSources)
        );
        app.world().resource::<audio::Simulation>().shutdown();
    }

    #[test]
    fn reloads_scene_asset_geometry() {
        use bevy::asset::{AssetEvent, AssetPlugin};
        let settings = core::SimulationSettings::new(&AudioSettings::default())
            .with_direct(DirectSimulationSettings::default());
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin))
            .init_resource::<Assets<Mesh>>()
            .add_plugins(
                audio::SpatialAudioPlugin::with_settings(settings).with_runners(
                    audio::RunnerSettings {
                        direct: false,
                        reflections: audio::ReflectionsRunner::Disabled,
                        pathing: false,
                    },
                ),
            );
        let context = app.world().resource::<Context>().clone();
        let scene = core::Scene::try_new(&context, core::RayTracerSettings::Default).unwrap();
        let bytes = scene.try_save(&context).unwrap().to_vec();
        let handle = app
            .world_mut()
            .resource_mut::<Assets<audio::SceneAsset>>()
            .add(audio::SceneAsset::new(bytes.clone()));
        let scene_entity = app
            .world_mut()
            .spawn((
                audio::SceneAssetSource::with_ray_tracer(
                    handle.clone(),
                    core::RayTracerSettings::Default,
                ),
                audio::MainScene,
                Transform::default(),
            ))
            .id();
        let sub_scene_entity = app
            .world_mut()
            .spawn((
                audio::SceneAssetSource::with_ray_tracer(
                    handle.clone(),
                    core::RayTracerSettings::Default,
                ),
                Transform::default(),
            ))
            .id();
        let mesh = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(Mesh::from(Cuboid::default()));
        let mesh_entity = app
            .world_mut()
            .spawn((
                audio::StaticMesh,
                Mesh3d(mesh),
                Transform::default(),
                ChildOf(sub_scene_entity),
            ))
            .id();
        let instance_entity = app
            .world_mut()
            .spawn((
                audio::InstancedMesh(sub_scene_entity),
                Transform::from_xyz(0.0, 0.0, 1.0),
                ChildOf(scene_entity),
            ))
            .id();
        let source = core::Source::try_new(
            app.world().resource::<audio::Simulation>().simulator(),
            SimulationFlags::DIRECT,
        )
        .unwrap();
        app.world_mut().spawn(audio::Source::from(source.clone()));
        for stage in 0..6 {
            match stage {
                1 => {
                    app.world_mut()
                        .resource_mut::<Assets<audio::SceneAsset>>()
                        .insert(handle.id(), audio::SceneAsset::new(bytes.clone()))
                        .unwrap();
                    app.world_mut()
                        .write_message(AssetEvent::<audio::SceneAsset>::Modified {
                            id: handle.id(),
                        });
                }
                2 | 3 => {
                    let replacement = app
                        .world_mut()
                        .resource_mut::<Assets<audio::SceneAsset>>()
                        .add(audio::SceneAsset::new(bytes.clone()));
                    app.world_mut()
                        .entity_mut(if stage == 2 {
                            sub_scene_entity
                        } else {
                            scene_entity
                        })
                        .insert(audio::SceneAssetSource::with_ray_tracer(
                            replacement,
                            core::RayTracerSettings::Default,
                        ));
                }
                4 => {
                    app.world_mut()
                        .entity_mut(mesh_entity)
                        .remove::<audio::StaticMesh>();
                }
                5 => {
                    app.world_mut()
                        .entity_mut(mesh_entity)
                        .insert(audio::StaticMesh);
                    app.world_mut()
                        .entity_mut(instance_entity)
                        .remove::<audio::InstancedMesh>();
                }
                _ => {}
            }
            app.update();
            assert!(
                app.world()
                    .entity(scene_entity)
                    .contains::<audio::MainScene>()
            );
            app.world()
                .get::<audio::Scene>(sub_scene_entity)
                .unwrap()
                .commit();
            app.world()
                .get::<audio::Scene>(scene_entity)
                .unwrap()
                .commit();
            let simulation = app.world().resource::<audio::Simulation>();
            simulation.simulator().commit();
            source
                .set_inputs(&core::SimulationInputs {
                    source: CoordinateSystem {
                        origin: audionimbus::Point::new(0.0, 0.0, 2.0),
                        ..Default::default()
                    },
                    parameters: core::SimulationParameters {
                        direct: Some(DirectSimulationParameters::new().with_occlusion(
                            audionimbus::Occlusion::new(audionimbus::OcclusionAlgorithm::Raycast),
                        )),
                        ..Default::default()
                    },
                })
                .unwrap();
            simulation.simulator().run_direct().unwrap();
            assert_eq!(
                source.get_direct_outputs().unwrap().occlusion,
                Some(if stage < 4 { 0.0 } else { 1.0 }),
                "asset stage {stage}"
            );
        }
        app.world_mut()
            .entity_mut(sub_scene_entity)
            .remove::<audio::Scene>();
        app.world_mut()
            .entity_mut(scene_entity)
            .remove::<audio::Scene>();
        assert!(
            !app.world()
                .entity(scene_entity)
                .contains::<audio::MainScene>()
        );
        app.world().resource::<audio::Simulation>().shutdown();
    }

    #[test]
    fn draws_runtime_sources_and_geometry() {
        use bevy::asset::AssetPlugin;
        use bevy::gizmos::{GizmoAsset, GizmoPlugin};
        let settings = core::SimulationSettings::new(&AudioSettings::default())
            .with_direct(DirectSimulationSettings::default())
            .with_reflections(audionimbus::ParametricSettings {
                max_num_rays: 64,
                num_diffuse_samples: 8,
                max_duration: 1.0,
                max_num_sources: 4,
                num_threads: 1,
                max_order: 1,
            });
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            TransformPlugin,
            GizmoPlugin,
        ))
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<bevy::mesh::skinning::SkinnedMeshInverseBindposes>>()
        .add_plugins((
            audio::SpatialAudioPlugin::with_settings(settings).with_runners(
                audio::RunnerSettings {
                    direct: false,
                    reflections: audio::ReflectionsRunner::Disabled,
                    pathing: false,
                },
            ),
            audio::SpatialAudioDebugPlugin::with_runtime(),
        ));
        let context = app.world().resource::<Context>();
        let scene =
            audio::Scene::try_with_ray_tracer(context, core::RayTracerSettings::Default).unwrap();
        let scene_entity = app
            .world_mut()
            .spawn((scene, audio::MainScene, Transform::default()))
            .id();
        let source = audio::Source::try_with_flags(
            app.world().resource::<audio::Simulation>(),
            SimulationFlags::DIRECT | SimulationFlags::REFLECTIONS,
        )
        .unwrap();
        let entity = app
            .world_mut()
            .spawn((
                source,
                Transform::from_xyz(3.0, 0.0, 0.0),
                ChildOf(scene_entity),
                audio::WireframeColor(Color::srgb(1.0, 0.0, 0.0)),
            ))
            .id();
        #[cfg(feature = "static")]
        app.world_mut()
            .entity_mut(entity)
            .insert(audionimbus::r#static::bevy::SourceParameters::default());
        app.update();
        let gizmos = app.world().resource::<Assets<GizmoAsset>>();
        let source_gizmo = gizmos.iter().next().unwrap().1.buffer();
        assert!(!source_gizmo.strip_positions.is_empty());
        assert!(source_gizmo.list_positions.is_empty());
        assert!(
            source_gizmo
                .strip_positions
                .iter()
                .filter(|position| position.is_finite())
                .all(|position| (position.x - 3.0).abs() <= 0.101)
        );
        assert!(
            source_gizmo
                .strip_colors
                .iter()
                .zip(&source_gizmo.strip_positions)
                .filter(|(_, position)| position.is_finite())
                .all(|(color, _)| *color == bevy::color::LinearRgba::RED)
        );
        app.world_mut()
            .entity_mut(entity)
            .insert(audio::SourceParameters::from(core::SimulationParameters {
                direct: Some(
                    DirectSimulationParameters::new()
                        .with_directivity(audionimbus::Directivity::WeightedDipole {
                            weight: 1.0,
                            power: 1.0,
                        })
                        .with_occlusion(audionimbus::Occlusion::new(
                            audionimbus::OcclusionAlgorithm::Volumetric {
                                radius: 0.5,
                                num_occlusion_samples: 8,
                            },
                        )),
                ),
                ..Default::default()
            }));
        app.update();
        let gizmos = app.world().resource::<Assets<GizmoAsset>>();
        let source_gizmo = gizmos.iter().next().unwrap().1.buffer();
        assert!(!source_gizmo.list_positions.is_empty());
        assert!(
            source_gizmo
                .strip_positions
                .iter()
                .any(|position| position.x > 3.4)
        );
        app.world_mut().entity_mut(entity).despawn();
        let mesh = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(Mesh::from(Cuboid::default()));
        app.world_mut().spawn((
            audio::StaticMesh,
            Mesh3d(mesh),
            ChildOf(scene_entity),
            Transform::default(),
        ));
        app.update();
        let gizmos = app.world().resource::<Assets<GizmoAsset>>();
        assert!(
            gizmos
                .iter()
                .any(|(_, gizmo)| !gizmo.buffer().list_positions.is_empty())
        );
        app.world().resource::<audio::Simulation>().shutdown();
    }
}

#[test]
fn exports_selected_bevy_paths() {
    let _: fn(&audionimbus::bevy::runner::DirectSimulation) =
        |_: &audionimbus::bevy::DirectSimulation| {};
    let _: fn(&audionimbus::bevy::runner::ReflectionsSimulation) =
        |_: &audionimbus::bevy::ReflectionsSimulation| {};
    let _: fn(&audionimbus::bevy::runner::ReflectionsReverbSimulation) =
        |_: &audionimbus::bevy::ReflectionsReverbSimulation| {};
    let _: fn(&audionimbus::bevy::runner::PathingSimulation) =
        |_: &audionimbus::bevy::PathingSimulation| {};
    #[cfg(feature = "dynamic")]
    let _: Option<audionimbus::bevy::RunnerSettings> = None;

    let _: fn(&audionimbus::bevy::asset::SceneAssetSource) =
        |_: &audionimbus::bevy::SceneAssetSource| {};
    let _: fn(&audionimbus::bevy::debug::SpatialAudioDebugPlugin) =
        |_: &audionimbus::bevy::SpatialAudioDebugPlugin| {};
    let handle = bevy::prelude::Handle::<audionimbus::bevy::SceneAsset>::default();
    #[cfg(any(feature = "static", not(feature = "dynamic")))]
    let asset: audionimbus::bevy::prelude::SceneAssetSource =
        audionimbus::bevy::SceneAssetSource::new(handle);
    #[cfg(any(feature = "static", not(feature = "dynamic")))]
    let debug: audionimbus::bevy::prelude::SpatialAudioDebugPlugin =
        audionimbus::bevy::SpatialAudioDebugPlugin::new();
    #[cfg(all(feature = "dynamic", not(feature = "static")))]
    let asset: audionimbus::bevy::prelude::SceneAssetSource =
        audionimbus::bevy::SceneAssetSource::with_ray_tracer(
            handle,
            audionimbus::dynamic::RayTracerSettings::Default,
        );
    #[cfg(all(feature = "dynamic", not(feature = "static")))]
    let debug: audionimbus::bevy::prelude::SpatialAudioDebugPlugin =
        audionimbus::bevy::SpatialAudioDebugPlugin::with_runtime();
    #[cfg(feature = "static")]
    let _: (
        audionimbus::r#static::bevy::SceneAssetSource,
        audionimbus::r#static::bevy::SpatialAudioDebugPlugin,
    ) = (asset, debug);
    #[cfg(all(feature = "dynamic", not(feature = "static")))]
    let _: (
        audionimbus::dynamic::bevy::SceneAssetSource,
        audionimbus::dynamic::bevy::SpatialAudioDebugPlugin,
    ) = (asset, debug);
    #[cfg(not(any(feature = "static", feature = "dynamic")))]
    drop((asset, debug));
    let context = audionimbus::Context::default();
    #[cfg(any(feature = "static", not(feature = "dynamic")))]
    let scene: audionimbus::bevy::geometry::Scene =
        audionimbus::bevy::geometry::Scene::try_new(&context).unwrap();
    #[cfg(all(feature = "dynamic", not(feature = "static")))]
    let scene = audionimbus::bevy::geometry::Scene::try_with_ray_tracer(
        &context,
        audionimbus::dynamic::RayTracerSettings::Default,
    )
    .unwrap();
    let scene: audionimbus::bevy::prelude::Scene = scene;
    let scene: audionimbus::bevy::Scene = scene;
    #[cfg(feature = "static")]
    let _: audionimbus::r#static::bevy::Scene = scene.clone();
    #[cfg(all(feature = "dynamic", not(feature = "static")))]
    let _: audionimbus::dynamic::bevy::Scene = scene.clone();
    #[cfg(feature = "dynamic")]
    {
        let runtime = audionimbus::dynamic::bevy::Scene::try_with_ray_tracer(
            &context,
            audionimbus::dynamic::RayTracerSettings::Default,
        )
        .unwrap();
        let _: audionimbus::bevy::geometry::Scene<_> = runtime;
    }
    drop(scene);
}
