use crate::context::Context;
use crate::geometry::CoordinateSystem;
use crate::simulation::{
    DirectCompatible, PathingCompatible, ReflectionsCompatible, SimulationFlagsProvider,
    SimulationInputs, SimulationParameters, SimulationSettings, Simulator,
};
use crate::wiring::SourceWithInputs;
use bevy::prelude::{
    App, Component, Entity, IntoScheduleConfigs, PostUpdate, Query, Res, Resource, SystemSet,
    Transform, Without,
};
use std::ops::{Deref, DerefMut};

pub mod configuration;
pub use configuration::*;

pub mod runner;
pub use runner::*;

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
pub enum SpatialAudioSet {
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

fn coordinate_system_from_transform(transform: Transform) -> CoordinateSystem {
    CoordinateSystem {
        right: transform.right().to_array().into(),
        up: transform.up().to_array().into(),
        ahead: transform.forward().to_array().into(),
        origin: transform.translation.to_array().into(),
    }
}
