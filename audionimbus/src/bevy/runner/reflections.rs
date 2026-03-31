//! Reflections simulation runner.

use super::super::configuration::SimulationConfiguration;
use super::super::error::{SimulationErrorEvent, SimulationErrorSender};
use super::super::simulation::{Simulation, SimulationSharedInputs, SimulationThread};
use super::super::system_set::SpatialAudioSet;
use super::{Runner, Spawn, SyncFrame};
use crate::sealed::Sealed;
use crate::simulation::{DirectCompatible, PathingCompatible, Reflections};
use crate::wiring::{Allocate, ReflectionsFrame, ReflectionsOutput, SimulationStepError};
use bevy::prelude::{
    App, Entity, IntoScheduleConfigs, PostUpdate, Res, Resource, World, resource_exists,
};
use std::ops::{Deref, DerefMut};

/// Runner for reflections simulation.
pub struct RunnerReflections;

impl Sealed for RunnerReflections {}
impl Runner for RunnerReflections {
    type SimulationType = Reflections;
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
        let error_sender = world.resource::<SimulationErrorSender>().0.clone();

        let runner = world.resource_mut::<Simulation<C>>().spawn_reflections(
            move |error: SimulationStepError| {
                let _ = error_sender.try_send(SimulationErrorEvent {
                    thread: SimulationThread::Reflections,
                    error,
                });
            },
        );

        world.insert_resource(ReflectionsSimulation::<C>(runner));
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

/// Resource wrapping a [`wiring::ReflectionsSimulation`](crate::wiring::ReflectionsSimulation).
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
