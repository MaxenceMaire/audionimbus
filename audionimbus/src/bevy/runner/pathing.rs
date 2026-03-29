//! Pathing simulation runner.

use super::super::configuration::SimulationConfiguration;
use super::super::error::{SimulationErrorEvent, SimulationErrorSender};
use super::super::simulation::{Simulation, SimulationSharedInputs, SimulationThread};
use super::super::system_set::SpatialAudioSet;
use super::{Runner, Spawn, SyncFrame, ToRunner};
use crate::effect::pathing::PathEffectParams;
use crate::sealed::Sealed;
use crate::simulation::{DirectCompatible, Pathing, ReflectionsCompatible};
use crate::wiring::{Allocate, PathingFrame, SimulationStepError};
use bevy::prelude::{
    App, Entity, IntoScheduleConfigs, PostUpdate, Res, Resource, World, resource_exists,
};
use std::ops::{Deref, DerefMut};

/// Runner for pathing simulation.
pub struct RunnerPathing;

impl Sealed for RunnerPathing {}
impl Runner for RunnerPathing {
    type SimulationType = Pathing;
}

impl ToRunner for Pathing {
    type Runner = RunnerPathing;
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
        let error_sender = world.resource::<SimulationErrorSender>().0.clone();

        let runner = world.resource_mut::<Simulation<C>>().spawn_pathing(
            move |error: SimulationStepError| {
                let _ = error_sender.try_send(SimulationErrorEvent {
                    thread: SimulationThread::Direct,
                    error,
                });
            },
        );

        world.insert_resource(PathingSimulation::<C>(runner));
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

/// Resource wrapping a [`wiring::PathingSimulation`](crate::wiring::PathingSimulation).
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
