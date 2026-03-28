use super::configuration::{DefaultSimulationConfiguration, SimulationConfiguration};
use super::runner::Listener;
use super::runner::{Runner, Spawn, SyncFrame, ToRunner};
use super::simulation::{Simulation, SimulationSharedInputs};
use super::source::{Source, SourceParameters};
use super::system_set::SpatialAudioSet;
use crate::context::Context;
use crate::simulation::{
    DirectCompatible, PathingCompatible, ReflectionsCompatible, SimulationFlagsProvider,
    SimulationSettings, Simulator,
};
use crate::simulation::{SimulationInputs, SimulationParameters};
use crate::wiring::SourceWithInputs;
use bevy::prelude::{App, Entity, IntoScheduleConfigs, PostUpdate, Query, Res, Transform, Without};

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
                source: (*transform).into(),
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
