use audionimbus::ray_tracing::{DefaultRayTracer, RayTracer};
use audionimbus::{
    Context, InstancedMesh, InstancedMeshSettings, Material, Matrix4, Point, Scene, StaticMesh,
    StaticMeshSettings, Triangle,
};

/// Accepts mesh declarations without ray tracer bounds.
fn accepts_unconstrained_static_mesh<T>(_: Option<StaticMesh<T>>) {}

/// Exercises insertion through the existing static generic bound.
fn add_static_mesh<T: RayTracer>(scene: &mut Scene<T>, mesh: StaticMesh<T>) {
    scene.add_static_mesh(mesh);
}

#[test]
fn preserves_static_geometry_api() {
    accepts_unconstrained_static_mesh::<String>(None);

    let context = Context::default();
    let mut scene = Scene::try_new(&context).unwrap();
    let explicit_scene = Scene::<DefaultRayTracer>::try_new(&context).unwrap();
    let _: Scene = explicit_scene;

    let mesh = StaticMesh::try_new(
        &scene,
        &StaticMeshSettings {
            vertices: &[
                Point::new(0.0, 0.0, 0.0),
                Point::new(1.0, 0.0, 0.0),
                Point::new(0.0, 1.0, 0.0),
            ],
            triangles: &[Triangle::new(0, 1, 2)],
            materials: &[Material::CONCRETE],
            material_indices: &[0],
        },
    )
    .unwrap();
    add_static_mesh(&mut scene, mesh);
    scene.commit();

    let sub_scene = Scene::try_new(&context).unwrap();
    let instance = InstancedMesh::try_new(
        &scene,
        &InstancedMeshSettings {
            sub_scene,
            transform: Matrix4::IDENTITY,
        },
    )
    .unwrap();
    scene.add_instanced_mesh(instance);
    scene.commit();
}

/// Preserves processing through the existing static capability bound.
fn apply_static_reflections<
    T: audionimbus::ReflectionEffectType + audionimbus::CanApplyDirectly,
>(
    reflection_effect: &mut audionimbus::ReflectionEffect<T>,
    reflection_effect_params: &audionimbus::ReflectionEffectParams<T>,
    input_buffer: &impl audionimbus::AudioBuffer,
    output_buffer: &mut audionimbus::AudioBufferMut<'_>,
) -> Result<audionimbus::AudioEffectState, audionimbus::EffectError> {
    reflection_effect.apply(reflection_effect_params, input_buffer, output_buffer)
}

#[test]
fn preserves_static_reflection_api() {
    use audionimbus::{
        AudioBufferMut, AudioBufferRef, AudioSettings, Convolution, Hybrid, Parametric,
        ReflectionEffect, ReflectionEffectParams, ReflectionEffectSettings, ReflectionMixer,
    };

    let context = Context::default();
    let audio_settings = AudioSettings::default();
    let reflection_effect_settings = ReflectionEffectSettings {
        impulse_response_size: audio_settings.sampling_rate,
        num_channels: 4,
    };
    let mut reflection_effect = ReflectionEffect::<Parametric>::try_new(
        &context,
        &audio_settings,
        &reflection_effect_settings,
    )
    .unwrap();
    let mut reflection_effect_params =
        ReflectionEffectParams::<Parametric>::new([1.0; 3], 4, audio_settings.sampling_rate);
    reflection_effect_params.set_num_channels(1).unwrap();
    reflection_effect_params
        .set_impulse_response_size(audio_settings.sampling_rate / 2)
        .unwrap();
    let input_samples = vec![0.0; audio_settings.frame_size as usize];
    let input_buffer = AudioBufferRef::try_from(input_samples.as_slice()).unwrap();
    let mut output_samples = vec![0.0; audio_settings.frame_size as usize];
    let mut output_buffer = AudioBufferMut::try_new(&mut output_samples, 1).unwrap();
    assert!(
        apply_static_reflections(
            &mut reflection_effect,
            &reflection_effect_params,
            &input_buffer,
            &mut output_buffer
        )
        .is_ok()
    );
    let _: &mut audionimbus_sys::IPLReflectionEffect = reflection_effect.raw_ptr_mut();
    let mut reflection_mixer = ReflectionMixer::<Convolution>::try_new(
        &context,
        &audio_settings,
        &reflection_effect_settings,
    )
    .unwrap();
    let _: &mut audionimbus_sys::IPLReflectionMixer = reflection_mixer.raw_ptr_mut();
    let _: fn(
        &mut ReflectionMixer<Convolution>,
        &mut ReflectionEffectParams<Convolution>,
        &mut AudioBufferMut<'_>,
    ) -> Result<audionimbus::AudioEffectState, audionimbus::EffectError> =
        ReflectionMixer::<Convolution>::apply;
    let _: ReflectionMixer<Hybrid> =
        ReflectionMixer::try_new(&context, &audio_settings, &reflection_effect_settings).unwrap();
    let _: ReflectionMixer<Parametric> =
        ReflectionMixer::try_new(&context, &audio_settings, &reflection_effect_settings).unwrap();
}

/// Accepts source declarations without simulation marker bounds.
fn accepts_unconstrained_source<D, R, P, RE>(_: Option<audionimbus::Source<D, R, P, RE>>) {}

/// Preserves the simulator's original marker bounds.
fn accepts_static_simulator<T: RayTracer, D, R, P, RE>(
    _: Option<audionimbus::Simulator<T, D, R, P, RE>>,
) {
}

/// Preserves the direct input subset's explicit generic arguments.
fn sets_static_source_inputs<D, R, P, RE>(
    source: &audionimbus::Source<D, R, P, RE>,
    inputs: &audionimbus::SimulationInputs<D, R, P>,
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
        SimulationInputs, SimulationParameters, SimulationSettings, SimulationSharedInputs,
        Simulator, Source,
    };
    accepts_unconstrained_source::<String, String, String, String>(None);
    accepts_static_simulator::<DefaultRayTracer, String, String, String, String>(None);
    let context = Context::default();
    let audio_settings = AudioSettings::default();
    let settings =
        SimulationSettings::new(&audio_settings).with_direct(DirectSimulationSettings::default());
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
        audionimbus::SimulationOutputs<Direct, (), ()>,
        audionimbus::SteamAudioError,
    > = Source::get_outputs;
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
        BakeError, PathBakeParams, PathBaker, ProbeArray, ProbeGenerationParams,
        ReflectionsBakeParams, ReflectionsBaker,
    };
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
    let _: audionimbus::baking::reflections::ReflectionsBaker<'_, DefaultRayTracer> =
        ReflectionsBaker::<DefaultRayTracer>::new();
    let _: audionimbus::baking::pathing::PathBaker<DefaultRayTracer> = PathBaker::new();
    let _: fn(
        &ReflectionsBaker<'static, DefaultRayTracer>,
        &Context,
        &mut audionimbus::ProbeBatch,
        &Scene,
        ReflectionsBakeParams,
    ) -> Result<(), BakeError> = ReflectionsBaker::<'static, DefaultRayTracer>::bake;
    let _: fn(
        &PathBaker<DefaultRayTracer>,
        &Context,
        &mut audionimbus::ProbeBatch,
        &Scene,
        PathBakeParams,
    ) -> Result<(), BakeError> = PathBaker::<DefaultRayTracer>::bake;

    let _: ReflectionsBaker<'_, audionimbus::ray_tracing::Embree> =
        ReflectionsBaker::<audionimbus::ray_tracing::Embree>::new();
    let _: ReflectionsBaker<'_, audionimbus::ray_tracing::CustomRayTracer> =
        ReflectionsBaker::<audionimbus::ray_tracing::CustomRayTracer>::new(16);
}

#[cfg(feature = "wiring")]
#[test]
fn preserves_static_worker_extension_points() {
    use arc_swap::ArcSwap;
    use audionimbus::ray_tracing::DefaultRayTracer;
    use audionimbus::wiring::*;
    use audionimbus::*;
    use object_pool::Pool;
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
        type Error = SimulationStepError;

        fn run(&mut self, input: &&Inputs, output: &mut Self::Output) -> Result<(), Self::Error> {
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
                                callback: audionimbus::callback::DistanceAttenuationCallback::new(
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
