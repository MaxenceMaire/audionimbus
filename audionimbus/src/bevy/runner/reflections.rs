//! Reflections simulation runner.

use super::super::binding::{ConfigurationBinding, DefaultConfiguration};
use super::super::configuration::SimulationConfiguration;
use super::super::error::{SimulationErrorEvent, SimulationErrorSender};
use super::super::simulation::{Simulation, SimulationSharedInputs, SimulationThread};
use super::super::system_set::SpatialAudioSet;
use super::{Runner, Spawn, SyncFrame};
use crate::sealed::Sealed;
use crate::simulation::{DirectCompatible, PathingCompatible, Reflections};
use crate::wiring::ReflectionsFrame;
use bevy::prelude::{App, IntoScheduleConfigs, PostUpdate, Res, Resource, World, resource_exists};
use std::ops::{Deref, DerefMut};

/// Runner for reflections simulation.
pub struct RunnerReflections;

impl Sealed for RunnerReflections {}
impl Runner for RunnerReflections {
    type SimulationType = Reflections;
}

impl<C: ReflectionsSpawn> Spawn<C> for RunnerReflections {
    fn spawn(world: &mut World) {
        let error_sender = world
            .resource::<SimulationErrorSender<C::Error>>()
            .0
            .clone();
        let runner = C::spawn(&mut world.resource_mut::<Simulation<C>>().0, move |error| {
            let _ = error_sender.try_send(SimulationErrorEvent {
                thread: SimulationThread::Reflections,
                error,
            });
        });
        world.insert_resource(ReflectionsSimulation::<C>(runner));
    }
}

impl<C> SyncFrame<C> for RunnerReflections
where
    C: ReflectionsSync,
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

/// Publishes a new frame to the reflections simulation thread.
fn sync_reflections_frame<C>(
    simulation: Res<Simulation<C>>,
    reflections: Res<ReflectionsSimulation<C>>,
    shared_inputs: Res<SimulationSharedInputs<C>>,
) where
    C: ReflectionsSync,
{
    C::sync(&reflections.0, &simulation.0, &shared_inputs.0);
}

/// Resource wrapping a [`wiring::ReflectionsSimulation`](crate::wiring::simulation::ReflectionsSimulation).
#[derive(Resource)]
pub struct ReflectionsSimulation<C: ConfigurationBinding = DefaultConfiguration>(
    pub C::ReflectionsSimulation,
);

impl<C: ConfigurationBinding> Deref for ReflectionsSimulation<C> {
    type Target = C::ReflectionsSimulation;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<C: ConfigurationBinding> DerefMut for ReflectionsSimulation<C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Frame publication for configurations that enable reflections.
pub trait ReflectionsSync: ConfigurationBinding {
    /// Publishes inputs for the next worker iteration.
    fn sync(
        worker: &Self::ReflectionsSimulation,
        simulation: &Self::Simulation,
        inputs: &Self::SharedInputs,
    );
}

impl<C> ReflectionsSync for C
where
    C: SimulationConfiguration<Reflections = Reflections>,
{
    fn sync(
        worker: &Self::ReflectionsSimulation,
        simulation: &Self::Simulation,
        inputs: &Self::SharedInputs,
    ) {
        worker.set_input(ReflectionsFrame {
            sources: simulation.sources.clone(),
            shared_inputs: inputs.clone(),
        });
    }
}

#[cfg(feature = "dynamic")]
pub type DynamicReflectionsSimulation =
    ReflectionsSimulation<super::super::binding::RuntimeConfiguration>;

#[cfg(feature = "dynamic")]
impl ReflectionsSync for super::super::binding::RuntimeConfiguration {
    fn sync(
        worker: &Self::ReflectionsSimulation,
        _: &Self::Simulation,
        inputs: &Self::SharedInputs,
    ) {
        worker.set_input(inputs.clone());
    }
}

/// Worker startup for configurations that enable reflections.
pub trait ReflectionsSpawn: ConfigurationBinding {
    /// Starts the selected worker.
    ///
    /// # Panics
    ///
    /// Panics if the selected runtime mode is disabled.
    fn spawn(
        simulation: &mut Self::Simulation,
        on_error: impl Fn(Self::Error) + Send + 'static,
    ) -> Self::ReflectionsSimulation;
}

impl<C> ReflectionsSpawn for C
where
    C: SimulationConfiguration<Reflections = Reflections>,
    (): DirectCompatible<<C as SimulationConfiguration>::Direct>
        + PathingCompatible<<C as SimulationConfiguration>::Pathing>,
{
    fn spawn(
        simulation: &mut Self::Simulation,
        on_error: impl Fn(Self::Error) + Send + 'static,
    ) -> Self::ReflectionsSimulation {
        simulation.spawn_reflections(on_error)
    }
}

#[cfg(feature = "dynamic")]
impl ReflectionsSpawn for super::super::binding::RuntimeConfiguration {
    fn spawn(
        simulation: &mut Self::Simulation,
        on_error: impl Fn(Self::Error) + Send + 'static,
    ) -> Self::ReflectionsSimulation {
        simulation
            .spawn_reflections(on_error)
            .expect("worker mode is disabled")
    }
}
