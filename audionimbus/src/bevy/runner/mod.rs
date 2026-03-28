use super::configuration::SimulationConfiguration;
use super::simulation::Simulation;
use super::source::{Source, SourceParameters};
use crate::sealed::Sealed;
use crate::simulation::{Pathing, Reflections};
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

pub trait Runner: Sealed {
    type SimulationType;
}

impl Runner for () {
    type SimulationType = ();
}

pub trait ToRunner {
    type Runner: Runner;
}

impl ToRunner for () {
    type Runner = ();
}

impl ToRunner for Reflections {
    type Runner = RunnerReflectionsReverb;
}

impl ToRunner for Pathing {
    type Runner = RunnerPathing;
}

pub trait Spawn<C: SimulationConfiguration> {
    fn spawn(world: &mut World);
}

impl<C: SimulationConfiguration> Spawn<C> for () {
    fn spawn(_world: &mut World) {}
}

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
