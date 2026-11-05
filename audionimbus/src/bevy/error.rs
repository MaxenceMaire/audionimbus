//! Simulation error reporting.

use super::simulation::SimulationThread;
use crate::wiring::step::SimulationStepError;
use bevy::prelude::{Commands, Entity, Event, NonSend};
use std::sync::mpsc;

/// Fired for every error that occurred in a background simulation thread.
#[derive(Event, Debug, Clone)]
pub struct SimulationErrorEvent<E = DefaultSimulationError> {
    /// Which simulation thread produced the error.
    pub thread: SimulationThread,
    /// The original typed error from the failing simulation step.
    pub error: E,
}

/// Resource holding the sending half of the error channel.
#[derive(bevy::prelude::Resource, Clone)]
pub struct SimulationErrorSender<E = DefaultSimulationError>(
    pub(crate) mpsc::SyncSender<SimulationErrorEvent<E>>,
);

/// Resource holding the receiving half of the error channel.
pub struct SimulationErrorReceiver<E = DefaultSimulationError>(
    pub(crate) mpsc::Receiver<SimulationErrorEvent<E>>,
);

/// Number of errors the channel can buffer before `try_send` silently drops new ones.
pub const ERROR_CHANNEL_CAPACITY: usize = 64;

/// Creates a sender/receiver pair.
pub(crate) fn error_channel<E>() -> (SimulationErrorSender<E>, SimulationErrorReceiver<E>) {
    let (sender, receiver) = mpsc::sync_channel(ERROR_CHANNEL_CAPACITY);
    (
        SimulationErrorSender(sender),
        SimulationErrorReceiver(receiver),
    )
}

/// System that drains the error channel and fire events.
pub(crate) fn propagate_simulation_errors<E: Send + Sync + 'static>(
    receiver: NonSend<SimulationErrorReceiver<E>>,
    mut commands: Commands,
) {
    while let Ok(event) = receiver.0.try_recv() {
        commands.trigger(event);
    }
}

#[cfg(any(feature = "static", not(feature = "dynamic")))]
type DefaultSimulationError = SimulationStepError;

#[cfg(all(feature = "dynamic", not(feature = "static")))]
type DefaultSimulationError = crate::dynamic::SimulationError;

#[cfg(feature = "dynamic")]
pub type DynamicSimulationErrorEvent = SimulationErrorEvent<crate::dynamic::SimulationError>;

/// Reports a rejected operation from the Bevy integration.
///
/// Rejected source and listener updates retain their previous snapshots.
#[derive(Event, Debug)]
pub struct IntegrationErrorEvent<E = DefaultIntegrationError> {
    /// The affected entity, or no entity for a complete source snapshot.
    pub entity: Option<Entity>,
    /// The operation that failed.
    pub operation: IntegrationOperation,
    /// The original validation error.
    pub error: E,
}

/// An operation that can fail during ECS synchronization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegrationOperation {
    /// Source registration with the simulator.
    AddSource,
    /// Source removal from the simulator.
    RemoveSource,
    /// Main scene selection.
    SetScene,
    /// Publication of the complete source snapshot.
    PublishSources,
    /// Publication of the listener reverb source.
    PublishListener,
}

/// Triggers an event and logs a rejected runtime integration operation.
#[cfg(feature = "dynamic")]
pub fn report_integration_error(
    bevy::prelude::In(outcome): bevy::prelude::In<Result<(), DynamicIntegrationErrorEvent>>,
    mut commands: Commands,
) {
    if let Err(event) = outcome {
        bevy::log::error!(
            "failed spatial audio operation {:?} for {:?}: {}",
            event.operation,
            event.entity,
            event.error,
        );
        commands.trigger(event);
    }
}

#[cfg(any(feature = "static", not(feature = "dynamic")))]
type DefaultIntegrationError = std::convert::Infallible;

#[cfg(all(feature = "dynamic", not(feature = "static")))]
type DefaultIntegrationError = crate::dynamic::SimulationError;

#[cfg(feature = "dynamic")]
pub type DynamicIntegrationErrorEvent = IntegrationErrorEvent<crate::dynamic::SimulationError>;
