//! Simulation error reporting.

use super::simulation::SimulationThread;
use crate::wiring::step::SimulationStepError;
use bevy::prelude::{Commands, Event, NonSend};
use std::sync::mpsc;

/// Fired for every error that occurred in a background simulation thread.
#[derive(Event, Debug, Clone)]
pub struct SimulationErrorEvent {
    /// Which simulation thread produced the error.
    pub thread: SimulationThread,
    /// The original typed error from the failing simulation step.
    pub error: SimulationStepError,
}

/// Resource holding the sending half of the error channel.
#[derive(bevy::prelude::Resource, Clone)]
pub struct SimulationErrorSender(pub(crate) mpsc::SyncSender<SimulationErrorEvent>);

/// Resource holding the receiving half of the error channel.
pub struct SimulationErrorReceiver(pub(crate) mpsc::Receiver<SimulationErrorEvent>);

/// Number of errors the channel can buffer before `try_send` silently drops new ones.
pub const ERROR_CHANNEL_CAPACITY: usize = 64;

/// Creates a sender/receiver pair.
pub(crate) fn error_channel() -> (SimulationErrorSender, SimulationErrorReceiver) {
    let (sender, receiver) = mpsc::sync_channel(ERROR_CHANNEL_CAPACITY);
    (
        SimulationErrorSender(sender),
        SimulationErrorReceiver(receiver),
    )
}

/// System that drains the error channel and fire events.
pub(crate) fn propagate_simulation_errors(
    receiver: NonSend<SimulationErrorReceiver>,
    mut commands: Commands,
) {
    while let Ok(event) = receiver.0.try_recv() {
        commands.trigger(event);
    }
}
