use super::super::configuration::SimulationConfiguration;
use super::super::simulation::{Simulation, SimulationSharedInputs};
use super::super::source::{Listener, Source, SourceParameters};
use super::super::system_set::SpatialAudioSet;
use super::{Runner, Spawn, SyncFrame};
use crate::sealed::Sealed;
use crate::simulation::{
    DirectCompatible, PathingCompatible, Reflections, SimulationInputs, SimulationParameters,
};
use crate::wiring::{Allocate, ReflectionsReverbFrame, ReflectionsReverbOutput, SourceWithInputs};
use bevy::prelude::{
    App, Entity, IntoScheduleConfigs, PostUpdate, Query, Res, Resource, Transform, With, World,
    resource_exists,
};
use std::ops::{Deref, DerefMut};

pub struct RunnerReflectionsReverb;

impl Sealed for RunnerReflectionsReverb {}
impl Runner for RunnerReflectionsReverb {
    type SimulationType = Reflections;
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
                    source: (*transform).into(),
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
