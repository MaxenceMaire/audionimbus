use super::configuration::{DefaultSimulationConfiguration, SimulationConfiguration};
use super::runner::{Runner, Spawn, SyncFrame, ToRunner, sync_sources};
use super::simulation::{Simulation, SimulationSharedInputs};
use super::system_set::SpatialAudioSet;
use crate::context::Context;
use crate::simulation::{
    DirectCompatible, PathingCompatible, ReflectionsCompatible, SimulationFlagsProvider,
    SimulationSettings, Simulator,
};
use bevy::prelude::{App, Entity, IntoScheduleConfigs, PostUpdate};

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
