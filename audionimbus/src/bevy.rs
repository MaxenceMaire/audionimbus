use crate::context::Context;
use crate::effect::direct::DirectEffectParams;
use crate::effect::pathing::PathEffectParams;
use crate::effect::reflections::{Convolution, ReflectionEffectType};
use crate::geometry::CoordinateSystem;
use crate::ray_tracing::{DefaultRayTracer, RayTracer};
use crate::sealed::Sealed;
use crate::simulation::{
    Direct, DirectCompatible, Pathing, PathingCompatible, ReflectionEffectCompatible, Reflections,
    ReflectionsCompatible, SimulationFlagsProvider, SimulationInputs, SimulationParameters,
    SimulationSettings, Simulator,
};
use crate::wiring::{
    Allocate, DirectFrame, PathingFrame, ReflectionsFrame, ReflectionsOutput,
    ReflectionsReverbFrame, ReflectionsReverbOutput, SourceWithInputs,
};
use bevy::prelude::{
    App, Component, Entity, IntoScheduleConfigs, PostUpdate, Query, Res, Resource, SystemSet,
    Transform, With, Without, World, resource_exists,
};
use std::ops::{Deref, DerefMut};

/// Bundles simulation type parameters.
pub trait SimulationConfiguration: 'static + Send + Sync {
    type RayTracer: 'static + RayTracer + Send + Sync;
    type Direct: 'static
        + DirectCompatible<Self::Direct>
        + SimulationFlagsProvider
        + Send
        + Sync
        + Clone
        + Default;
    type Reflections: 'static
        + ReflectionsCompatible<Self::Reflections>
        + SimulationFlagsProvider
        + Send
        + Sync
        + Clone
        + Default;
    type Pathing: 'static
        + PathingCompatible<Self::Pathing>
        + SimulationFlagsProvider
        + Send
        + Sync
        + Clone
        + Default;
    type ReflectionEffect: 'static
        + ReflectionEffectCompatible<Self::Reflections, Self::ReflectionEffect>
        + ReflectionEffectType
        + Send
        + Sync
        + Clone
        + Default;
}

pub struct DefaultSimulationConfiguration;

impl SimulationConfiguration for DefaultSimulationConfiguration {
    type RayTracer = DefaultRayTracer;
    type Direct = Direct;
    type Reflections = Reflections;
    type Pathing = ();
    type ReflectionEffect = Convolution;
}

pub struct RunnerDirect;
pub struct RunnerReflections;
pub struct RunnerReflectionsReverb;
pub struct RunnerPathing;

pub trait Runner: Sealed {
    type SimulationType;
}

impl Runner for () {
    type SimulationType = ();
}

impl Sealed for RunnerDirect {}
impl Runner for RunnerDirect {
    type SimulationType = Direct;
}

impl Sealed for RunnerReflections {}
impl Runner for RunnerReflections {
    type SimulationType = Reflections;
}

impl Sealed for RunnerReflectionsReverb {}
impl Runner for RunnerReflectionsReverb {
    type SimulationType = Reflections;
}

impl Sealed for RunnerPathing {}
impl Runner for RunnerPathing {
    type SimulationType = Pathing;
}

pub trait ToRunner {
    type Runner: Runner;
}

impl ToRunner for () {
    type Runner = ();
}

impl ToRunner for Direct {
    type Runner = RunnerDirect;
}

impl ToRunner for Reflections {
    type Runner = RunnerReflectionsReverb;
}

impl ToRunner for Pathing {
    type Runner = RunnerPathing;
}

pub trait Spawn<C: SimulationConfiguration> {
    fn spawn(world: &mut World);
}

impl<C: SimulationConfiguration> Spawn<C> for () {
    fn spawn(_world: &mut World) {}
}

impl<C> Spawn<C> for RunnerDirect
where
    C: SimulationConfiguration<Direct = Direct>,
    Vec<DirectEffectParams>:
        Allocate<DirectFrame<Entity, C::Direct, C::Reflections, C::Pathing, C::ReflectionEffect>>,
    (): ReflectionsCompatible<<C as SimulationConfiguration>::Reflections>
        + PathingCompatible<<C as SimulationConfiguration>::Pathing>,
{
    fn spawn(world: &mut World) {
        let runner = world.resource_mut::<Simulation<C>>().spawn_direct();
        world.insert_resource(DirectSimulation::<C>(runner));
    }
}

impl<C> Spawn<C> for RunnerReflections
where
    C: SimulationConfiguration<Reflections = Reflections>,
    ReflectionsOutput<Entity, C::ReflectionEffect>: Allocate<
        ReflectionsFrame<Entity, C::Direct, C::Reflections, C::Pathing, C::ReflectionEffect>,
    >,
    (): DirectCompatible<<C as SimulationConfiguration>::Direct>
        + PathingCompatible<<C as SimulationConfiguration>::Pathing>,
{
    fn spawn(world: &mut World) {
        let runner = world.resource_mut::<Simulation<C>>().spawn_reflections();
        world.insert_resource(ReflectionsSimulation::<C>(runner));
    }
}

impl<C> Spawn<C> for RunnerReflectionsReverb
where
    C: SimulationConfiguration<Reflections = Reflections>,
    ReflectionsReverbOutput<Entity, C::ReflectionEffect>: Allocate<
        ReflectionsReverbFrame<Entity, C::Direct, C::Reflections, C::Pathing, C::ReflectionEffect>,
    >,
    (): DirectCompatible<<C as SimulationConfiguration>::Direct>
        + PathingCompatible<<C as SimulationConfiguration>::Pathing>,
{
    fn spawn(world: &mut World) {
        let runner = world
            .resource_mut::<Simulation<C>>()
            .spawn_reflections_reverb();
        world.insert_resource(ReflectionsReverbSimulation::<C>(runner));
    }
}

impl<C> Spawn<C> for RunnerPathing
where
    C: SimulationConfiguration<Pathing = Pathing>,
    Vec<PathEffectParams>:
        Allocate<PathingFrame<Entity, C::Direct, C::Reflections, C::Pathing, C::ReflectionEffect>>,
    (): ReflectionsCompatible<<C as SimulationConfiguration>::Reflections>
        + DirectCompatible<<C as SimulationConfiguration>::Direct>,
{
    fn spawn(world: &mut World) {
        let runner = world.resource_mut::<Simulation<C>>().spawn_pathing();
        world.insert_resource(PathingSimulation::<C>(runner));
    }
}

pub trait SyncFrame<C: SimulationConfiguration>: Sealed {
    fn add_systems(app: &mut App);
}

impl<C: SimulationConfiguration> SyncFrame<C> for () {
    fn add_systems(_app: &mut App) {}
}

impl<C> SyncFrame<C> for RunnerDirect
where
    C: SimulationConfiguration<Direct = Direct>,
{
    fn add_systems(app: &mut App) {
        app.add_systems(
            PostUpdate,
            sync_direct_frame::<C>
                .run_if(resource_exists::<DirectSimulation<C>>)
                .in_set(SpatialAudioSet::SyncFrames),
        );
    }
}

impl<C> SyncFrame<C> for RunnerReflections
where
    C: SimulationConfiguration<Reflections = Reflections>,
{
    fn add_systems(app: &mut App) {
        app.add_systems(
            PostUpdate,
            sync_reflections_frame::<C>
                .run_if(resource_exists::<ReflectionsSimulation<C>>)
                .in_set(SpatialAudioSet::SyncFrames),
        );
    }
}

impl<C> SyncFrame<C> for RunnerReflectionsReverb
where
    C: SimulationConfiguration<Reflections = Reflections>,
{
    fn add_systems(app: &mut App) {
        app.add_systems(
            PostUpdate,
            sync_reflections_reverb_frame::<C>
                .run_if(resource_exists::<ReflectionsReverbSimulation<C>>)
                .in_set(SpatialAudioSet::SyncFrames),
        );
    }
}

impl<C> SyncFrame<C> for RunnerPathing
where
    C: SimulationConfiguration<Pathing = Pathing>,
{
    fn add_systems(app: &mut App) {
        app.add_systems(
            PostUpdate,
            sync_pathing_frame::<C>
                .run_if(resource_exists::<PathingSimulation<C>>)
                .in_set(SpatialAudioSet::SyncFrames),
        );
    }
}

pub struct Plugin<
    C: SimulationConfiguration = DefaultSimulationConfiguration,
    RD: Runner = (),
    RR: Runner = (),
    RP: Runner = (),
> {
    simulation_settings: SimulationSettings<
        C::RayTracer,
        C::Direct,
        C::Reflections,
        C::Pathing,
        C::ReflectionEffect,
    >,
    _phantom: std::marker::PhantomData<(RD, RR, RP)>,
}

impl<C, RD, RR, RP> Plugin<C, RD, RR, RP>
where
    C: SimulationConfiguration,
    C::Direct: ToRunner<Runner = RD>,
    C::Reflections: ToRunner<Runner = RR>,
    C::Pathing: ToRunner<Runner = RP>,
    RD: Runner,
    RR: Runner,
    RP: Runner,
{
    pub fn new(
        simulation_settings: SimulationSettings<
            C::RayTracer,
            C::Direct,
            C::Reflections,
            C::Pathing,
            C::ReflectionEffect,
        >,
    ) -> Self {
        Self {
            simulation_settings,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<C, RD, RR, RP> Plugin<C, RD, RR, RP>
where
    C: SimulationConfiguration,
    RD: Runner,
    RR: Runner,
    RP: Runner,
{
    pub fn with_runners<RD2, RR2, RP2>(self) -> Plugin<C, RD2, RR2, RP2>
    where
        RD2: Runner<SimulationType = RD::SimulationType>,
        RR2: Runner<SimulationType = RR::SimulationType>,
        RP2: Runner<SimulationType = RP::SimulationType>,
    {
        Plugin {
            simulation_settings: self.simulation_settings,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<C, RD, RR, RP> bevy::app::Plugin for Plugin<C, RD, RR, RP>
where
    C: SimulationConfiguration,
    RD: 'static + Runner + Send + Sync + Spawn<C> + SyncFrame<C>,
    RR: 'static + Runner + Send + Sync + Spawn<C> + SyncFrame<C>,
    RP: 'static + Runner + Send + Sync + Spawn<C> + SyncFrame<C>,
    RD::SimulationType: 'static
        + Send
        + Sync
        + Clone
        + Default
        + DirectCompatible<RD::SimulationType>
        + SimulationFlagsProvider,
    RR::SimulationType: 'static
        + Send
        + Sync
        + Clone
        + Default
        + ReflectionsCompatible<RR::SimulationType>
        + SimulationFlagsProvider,
    RP::SimulationType: 'static
        + Send
        + Sync
        + Clone
        + Default
        + PathingCompatible<RP::SimulationType>
        + SimulationFlagsProvider,
    (): DirectCompatible<RD::SimulationType>
        + ReflectionsCompatible<RR::SimulationType>
        + PathingCompatible<RP::SimulationType>,
{
    fn build(&self, app: &mut App) {
        let world = app.world_mut();

        let context = world.get_resource_or_insert_with(Context::default).clone();

        let simulator = Simulator::try_new(&context, &self.simulation_settings)
            .expect("failed to create simulator");
        let simulation = crate::wiring::Simulation::new::<Entity>(simulator);

        world.insert_resource(Simulation::<C>(simulation));
        world.insert_resource(SimulationSharedInputs::<C>::default());

        RD::spawn(world);
        RR::spawn(world);
        RP::spawn(world);

        app.configure_sets(
            PostUpdate,
            SpatialAudioSet::SyncSources.before(SpatialAudioSet::SyncFrames),
        );
        app.add_systems(
            PostUpdate,
            sync_sources::<C>.in_set(SpatialAudioSet::SyncSources),
        );

        RD::add_systems(app);
        RR::add_systems(app);
        RP::add_systems(app);
    }
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
enum SpatialAudioSet {
    SyncSources,
    SyncFrames,
}

#[derive(Resource)]
pub struct Simulation<C: SimulationConfiguration = DefaultSimulationConfiguration>(
    pub  crate::wiring::Simulation<
        Entity,
        C::RayTracer,
        C::Direct,
        C::Reflections,
        C::Pathing,
        C::ReflectionEffect,
    >,
);

impl<C: SimulationConfiguration> Deref for Simulation<C> {
    type Target = crate::wiring::Simulation<
        Entity,
        C::RayTracer,
        C::Direct,
        C::Reflections,
        C::Pathing,
        C::ReflectionEffect,
    >;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<C: SimulationConfiguration> DerefMut for Simulation<C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Resource)]
pub struct DirectSimulation<C: SimulationConfiguration>(
    pub crate::wiring::DirectSimulation<Entity, C::Reflections, C::Pathing, C::ReflectionEffect>,
);

impl<C: SimulationConfiguration> Deref for DirectSimulation<C> {
    type Target =
        crate::wiring::DirectSimulation<Entity, C::Reflections, C::Pathing, C::ReflectionEffect>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<C: SimulationConfiguration> DerefMut for DirectSimulation<C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Resource)]
pub struct ReflectionsSimulation<C: SimulationConfiguration>(
    pub crate::wiring::ReflectionsSimulation<Entity, C::Direct, C::Pathing, C::ReflectionEffect>,
);

impl<C: SimulationConfiguration> Deref for ReflectionsSimulation<C> {
    type Target =
        crate::wiring::ReflectionsSimulation<Entity, C::Direct, C::Pathing, C::ReflectionEffect>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<C: SimulationConfiguration> DerefMut for ReflectionsSimulation<C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Resource)]
pub struct ReflectionsReverbSimulation<C: SimulationConfiguration>(
    pub  crate::wiring::ReflectionsReverbSimulation<
        Entity,
        C::Direct,
        C::Pathing,
        C::ReflectionEffect,
    >,
);

impl<C: SimulationConfiguration> Deref for ReflectionsReverbSimulation<C> {
    type Target = crate::wiring::ReflectionsReverbSimulation<
        Entity,
        C::Direct,
        C::Pathing,
        C::ReflectionEffect,
    >;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<C: SimulationConfiguration> DerefMut for ReflectionsReverbSimulation<C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Resource)]
pub struct PathingSimulation<C: SimulationConfiguration>(
    pub crate::wiring::PathingSimulation<Entity, C::Direct, C::Reflections, C::ReflectionEffect>,
);

impl<C: SimulationConfiguration> Deref for PathingSimulation<C> {
    type Target =
        crate::wiring::PathingSimulation<Entity, C::Direct, C::Reflections, C::ReflectionEffect>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<C: SimulationConfiguration> DerefMut for PathingSimulation<C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Component, Clone, Debug)]
pub struct Source<C: SimulationConfiguration = DefaultSimulationConfiguration>(
    pub crate::simulation::Source<C::Direct, C::Reflections, C::Pathing, C::ReflectionEffect>,
);

#[derive(Component, Default, Clone, Debug)]
pub struct SourceParameters<C: SimulationConfiguration = DefaultSimulationConfiguration>(
    pub SimulationParameters<C::Direct, C::Reflections, C::Pathing>,
);

#[derive(Resource, Debug)]
pub struct SimulationSharedInputs<C: SimulationConfiguration = DefaultSimulationConfiguration>(
    pub crate::simulation::SimulationSharedInputs<C::Direct, C::Reflections, C::Pathing>,
);

impl<C: SimulationConfiguration> Default for SimulationSharedInputs<C> {
    fn default() -> Self {
        Self(crate::simulation::SimulationSharedInputs::default())
    }
}

/// The listener used for reverb simulation.
#[derive(Component, Debug)]
#[component(storage = "SparseSet")]
pub struct Listener;

fn sync_sources<C: SimulationConfiguration>(
    mut query: Query<
        (Entity, &Transform, &Source<C>, Option<&SourceParameters<C>>),
        Without<Listener>,
    >,
    simulation: Res<Simulation<C>>,
) {
    simulation.0.update_sources(|snapshot| {
        for (entity, transform, source, simulation_parameters) in query.iter_mut() {
            let simulation_inputs = SimulationInputs {
                source: coordinate_system_from_transform(*transform),
                parameters: simulation_parameters
                    .map_or_else(SimulationParameters::default, |params| params.0.clone()),
            };

            snapshot.push((
                entity,
                SourceWithInputs {
                    source: source.0.clone(),
                    simulation_inputs,
                },
            ));
        }
    });
}

fn sync_direct_frame<C>(
    simulation: Res<Simulation<C>>,
    direct: Res<DirectSimulation<C>>,
    shared_inputs: Res<SimulationSharedInputs<C>>,
) where
    C: SimulationConfiguration<Direct = Direct>,
{
    direct.set_input(DirectFrame {
        sources: simulation.sources.clone(),
        shared_inputs: shared_inputs.0.clone(),
    });
}

fn sync_reflections_frame<C>(
    simulation: Res<Simulation<C>>,
    reflections: Res<ReflectionsSimulation<C>>,
    shared_inputs: Res<SimulationSharedInputs<C>>,
) where
    C: SimulationConfiguration<Reflections = Reflections>,
{
    reflections.set_input(ReflectionsFrame {
        sources: simulation.sources.clone(),
        shared_inputs: shared_inputs.0.clone(),
    });
}

fn sync_reflections_reverb_frame<C>(
    query: Query<(&Transform, &Source<C>, Option<&SourceParameters<C>>), With<Listener>>,
    simulation: Res<Simulation<C>>,
    reflections_reverb: Res<ReflectionsReverbSimulation<C>>,
    shared_inputs: Res<SimulationSharedInputs<C>>,
) where
    C: SimulationConfiguration<Reflections = Reflections>,
{
    let mut query_iter = query.iter();

    #[cfg(debug_assertions)]
    if query_iter.len() > 1 {
        eprintln!("warning: found more than one listener; picking first item");
    }

    let listener =
        query_iter.next().map(
            |(transform, source, simulation_parameters)| SourceWithInputs {
                source: source.0.clone(),
                simulation_inputs: SimulationInputs {
                    source: coordinate_system_from_transform(*transform),
                    parameters: simulation_parameters
                        .map_or_else(SimulationParameters::default, |params| params.0.clone()),
                },
            },
        );

    reflections_reverb.set_input(ReflectionsReverbFrame {
        sources: simulation.sources.clone(),
        listener,
        shared_inputs: shared_inputs.0.clone(),
    });
}

fn sync_pathing_frame<C>(
    simulation: Res<Simulation<C>>,
    pathing: Res<PathingSimulation<C>>,
    shared_inputs: Res<SimulationSharedInputs<C>>,
) where
    C: SimulationConfiguration<Pathing = Pathing>,
{
    pathing.set_input(PathingFrame {
        sources: simulation.sources.clone(),
        shared_inputs: shared_inputs.0.clone(),
    });
}

fn coordinate_system_from_transform(transform: Transform) -> CoordinateSystem {
    CoordinateSystem {
        right: transform.right().to_array().into(),
        up: transform.up().to_array().into(),
        ahead: transform.forward().to_array().into(),
        origin: transform.translation.to_array().into(),
    }
}
