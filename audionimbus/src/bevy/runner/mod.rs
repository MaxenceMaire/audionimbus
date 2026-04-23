//! Runner typestate: marker types that map simulation modes to dedicated threads.

use super::configuration::SimulationConfiguration;
use crate::sealed::Sealed;
use bevy::prelude::{App, World};

mod direct;
mod pathing;
mod reflections;
mod reflections_reverb;

pub use direct::*;
pub use pathing::*;
pub use reflections::*;
pub use reflections_reverb::*;

/// Marker trait for a simulation runner.
///
/// The associated [`SimulationType`](Runner::SimulationType) links a runner to the simulation mode
/// it drives.
pub trait Runner: Sealed {
    type SimulationType;
}

impl Runner for () {
    type SimulationType = ();
}

/// Maps a simulation mode type to its [`Runner`].
pub trait ToRunner {
    type Runner: Runner;
}

impl ToRunner for () {
    type Runner = ();
}

/// Spawns a simulation thread and inserts its resource into the world.
pub trait Spawn<C: SimulationConfiguration> {
    /// Spawns the simulation thread for this runner and inserts it as a resource.
    fn spawn(world: &mut World);
}

impl<C: SimulationConfiguration> Spawn<C> for () {
    fn spawn(_world: &mut World) {}
}

/// Registers the frame-sync system for a simulation thread.
pub trait SyncFrame<C: SimulationConfiguration>: Sealed {
    /// Adds the frame-sync system for this runner.
    fn add_systems(app: &mut App);
}

impl<C: SimulationConfiguration> SyncFrame<C> for () {
    fn add_systems(_app: &mut App) {}
}
