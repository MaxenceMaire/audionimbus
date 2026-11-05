//! Pathing simulation runner.

use super::super::binding::{ConfigurationBinding, DefaultConfiguration};
use super::super::configuration::SimulationConfiguration;
use super::super::error::{SimulationErrorEvent, SimulationErrorSender};
use super::super::simulation::{Simulation, SimulationSharedInputs, SimulationThread};
use super::super::system_set::SpatialAudioSet;
use super::{Runner, Spawn, SyncFrame, ToRunner};
use crate::sealed::Sealed;
use crate::simulation::{DirectCompatible, Pathing, ReflectionsCompatible};
use crate::wiring::PathingFrame;
use bevy::prelude::{App, IntoScheduleConfigs, PostUpdate, Res, Resource, World, resource_exists};
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

impl<C: PathingSpawn> Spawn<C> for RunnerPathing {
    fn spawn(world: &mut World) {
        let error_sender = world
            .resource::<SimulationErrorSender<C::Error>>()
            .0
            .clone();
        let runner = C::spawn(&mut world.resource_mut::<Simulation<C>>().0, move |error| {
            let _ = error_sender.try_send(SimulationErrorEvent {
                thread: SimulationThread::Pathing,
                error,
            });
        });
        world.insert_resource(PathingSimulation::<C>(runner));
    }
}

impl<C> SyncFrame<C> for RunnerPathing
where
    C: PathingSync,
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

/// Publishes a new frame to the pathing simulation thread.
fn sync_pathing_frame<C>(
    simulation: Res<Simulation<C>>,
    pathing: Res<PathingSimulation<C>>,
    shared_inputs: Res<SimulationSharedInputs<C>>,
) where
    C: PathingSync,
{
    C::sync(&pathing.0, &simulation.0, &shared_inputs.0);
}

/// Resource wrapping a [`wiring::PathingSimulation`](crate::wiring::simulation::PathingSimulation).
#[derive(Resource)]
pub struct PathingSimulation<C: ConfigurationBinding = DefaultConfiguration>(
    pub C::PathingSimulation,
);

impl<C: ConfigurationBinding> Deref for PathingSimulation<C> {
    type Target = C::PathingSimulation;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<C: ConfigurationBinding> DerefMut for PathingSimulation<C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Frame publication for configurations that enable pathing.
pub trait PathingSync: ConfigurationBinding {
    /// Publishes inputs for the next worker iteration.
    fn sync(
        worker: &Self::PathingSimulation,
        simulation: &Self::Simulation,
        inputs: &Self::SharedInputs,
    );
}

impl<C> PathingSync for C
where
    C: SimulationConfiguration<Pathing = Pathing>,
{
    fn sync(
        worker: &Self::PathingSimulation,
        simulation: &Self::Simulation,
        inputs: &Self::SharedInputs,
    ) {
        worker.set_input(PathingFrame {
            sources: simulation.sources.clone(),
            shared_inputs: inputs.clone(),
        });
    }
}

#[cfg(feature = "dynamic")]
pub type DynamicPathingSimulation = PathingSimulation<super::super::binding::RuntimeConfiguration>;

#[cfg(feature = "dynamic")]
impl PathingSync for super::super::binding::RuntimeConfiguration {
    fn sync(worker: &Self::PathingSimulation, _: &Self::Simulation, inputs: &Self::SharedInputs) {
        worker.set_input(inputs.clone());
    }
}

/// Worker startup for configurations that enable pathing.
pub trait PathingSpawn: ConfigurationBinding {
    /// Starts the selected worker.
    ///
    /// # Panics
    ///
    /// Panics if the selected runtime mode is disabled.
    fn spawn(
        simulation: &mut Self::Simulation,
        on_error: impl Fn(Self::Error) + Send + 'static,
    ) -> Self::PathingSimulation;
}

impl<C> PathingSpawn for C
where
    C: SimulationConfiguration<Pathing = Pathing>,
    (): ReflectionsCompatible<<C as SimulationConfiguration>::Reflections>
        + DirectCompatible<<C as SimulationConfiguration>::Direct>,
{
    fn spawn(
        simulation: &mut Self::Simulation,
        on_error: impl Fn(Self::Error) + Send + 'static,
    ) -> Self::PathingSimulation {
        simulation.spawn_pathing(on_error)
    }
}

#[cfg(feature = "dynamic")]
impl PathingSpawn for super::super::binding::RuntimeConfiguration {
    fn spawn(
        simulation: &mut Self::Simulation,
        on_error: impl Fn(Self::Error) + Send + 'static,
    ) -> Self::PathingSimulation {
        simulation
            .spawn_pathing(on_error)
            .expect("worker mode is disabled")
    }
}
