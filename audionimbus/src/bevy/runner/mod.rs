//! Runner typestate: marker types that map simulation modes to dedicated threads.

use super::configuration::SimulationConfiguration;
use super::simulation::Simulation;
use super::source::{Listener, Source, SourceParameters};
use crate::sealed::Sealed;
use crate::simulation::{SimulationInputs, SimulationParameters};
use crate::wiring::SourceWithInputs;
use bevy::prelude::{App, Entity, Query, Res, Transform, Without, World};

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
    fn spawn(world: &mut World);
}

impl<C: SimulationConfiguration> Spawn<C> for () {
    fn spawn(_world: &mut World) {}
}

/// Registers the frame-sync system for a simulation thread.
pub trait SyncFrame<C: SimulationConfiguration>: Sealed {
    fn add_systems(app: &mut App);
}

impl<C: SimulationConfiguration> SyncFrame<C> for () {
    fn add_systems(_app: &mut App) {}
}

pub(crate) fn sync_sources<C: SimulationConfiguration>(
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
