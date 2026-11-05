//! Direct simulation runner.

use super::super::binding::{ConfigurationBinding, DefaultConfiguration};
use super::super::configuration::SimulationConfiguration;
use super::super::error::{SimulationErrorEvent, SimulationErrorSender};
use super::super::simulation::{Simulation, SimulationSharedInputs, SimulationThread};
use super::super::system_set::SpatialAudioSet;
use super::{Runner, Spawn, SyncFrame, ToRunner};
use crate::sealed::Sealed;
use crate::simulation::{Direct, PathingCompatible, ReflectionsCompatible};
use crate::wiring::DirectFrame;
use bevy::prelude::{App, IntoScheduleConfigs, PostUpdate, Res, Resource, World, resource_exists};
use std::ops::{Deref, DerefMut};

/// Runner for direct (occlusion, attenuation) simulation.
pub struct RunnerDirect;

impl Sealed for RunnerDirect {}
impl Runner for RunnerDirect {
    type SimulationType = Direct;
}

impl ToRunner for Direct {
    type Runner = RunnerDirect;
}

impl<C: DirectSpawn> Spawn<C> for RunnerDirect {
    fn spawn(world: &mut World) {
        let error_sender = world
            .resource::<SimulationErrorSender<C::Error>>()
            .0
            .clone();
        let runner = C::spawn(&mut world.resource_mut::<Simulation<C>>().0, move |error| {
            let _ = error_sender.try_send(SimulationErrorEvent {
                thread: SimulationThread::Direct,
                error,
            });
        });
        world.insert_resource(DirectSimulation::<C>(runner));
    }
}

impl<C> SyncFrame<C> for RunnerDirect
where
    C: DirectSync,
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

/// Publishes a new frame to the direct simulation thread.
fn sync_direct_frame<C>(
    simulation: Res<Simulation<C>>,
    direct: Res<DirectSimulation<C>>,
    shared_inputs: Res<SimulationSharedInputs<C>>,
) where
    C: DirectSync,
{
    C::sync(&direct.0, &simulation.0, &shared_inputs.0);
}

/// Resource wrapping a [`wiring::DirectSimulation`](crate::wiring::simulation::DirectSimulation).
#[derive(Resource)]
pub struct DirectSimulation<C: ConfigurationBinding = DefaultConfiguration>(
    pub C::DirectSimulation,
);

impl<C: ConfigurationBinding> Deref for DirectSimulation<C> {
    type Target = C::DirectSimulation;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<C: ConfigurationBinding> DerefMut for DirectSimulation<C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Frame publication for configurations that enable direct.
pub trait DirectSync: ConfigurationBinding {
    /// Publishes inputs for the next worker iteration.
    fn sync(
        worker: &Self::DirectSimulation,
        simulation: &Self::Simulation,
        inputs: &Self::SharedInputs,
    );
}

impl<C> DirectSync for C
where
    C: SimulationConfiguration<Direct = Direct>,
{
    fn sync(
        worker: &Self::DirectSimulation,
        simulation: &Self::Simulation,
        inputs: &Self::SharedInputs,
    ) {
        worker.set_input(DirectFrame {
            sources: simulation.sources.clone(),
            shared_inputs: inputs.clone(),
        });
    }
}

#[cfg(feature = "dynamic")]
pub type DynamicDirectSimulation = DirectSimulation<super::super::binding::RuntimeConfiguration>;

#[cfg(feature = "dynamic")]
impl DirectSync for super::super::binding::RuntimeConfiguration {
    fn sync(worker: &Self::DirectSimulation, _: &Self::Simulation, inputs: &Self::SharedInputs) {
        worker.set_input(inputs.clone());
    }
}

/// Worker startup for configurations that enable direct.
pub trait DirectSpawn: ConfigurationBinding {
    /// Starts the selected worker.
    ///
    /// # Panics
    ///
    /// Panics if the selected runtime mode is disabled.
    fn spawn(
        simulation: &mut Self::Simulation,
        on_error: impl Fn(Self::Error) + Send + 'static,
    ) -> Self::DirectSimulation;
}

impl<C> DirectSpawn for C
where
    C: SimulationConfiguration<Direct = Direct>,
    (): ReflectionsCompatible<<C as SimulationConfiguration>::Reflections>
        + PathingCompatible<<C as SimulationConfiguration>::Pathing>,
{
    fn spawn(
        simulation: &mut Self::Simulation,
        on_error: impl Fn(Self::Error) + Send + 'static,
    ) -> Self::DirectSimulation {
        simulation.spawn_direct(on_error)
    }
}

#[cfg(feature = "dynamic")]
impl DirectSpawn for super::super::binding::RuntimeConfiguration {
    fn spawn(
        simulation: &mut Self::Simulation,
        on_error: impl Fn(Self::Error) + Send + 'static,
    ) -> Self::DirectSimulation {
        simulation
            .spawn_direct(on_error)
            .expect("worker mode is disabled")
    }
}
