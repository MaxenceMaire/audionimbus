//! Components for spatial audio sources.

use super::configuration::{DefaultSimulationConfiguration, SimulationConfiguration};
use super::simulation::Simulation;
use crate::simulation::{SimulationInputs, SimulationParameters};
use crate::wiring::SourceWithInputs;
use bevy::prelude::{Component, Entity, Query, Res, Transform, Without};

/// Spatial audio source component.
///
/// Attach this to any entity that should emit sound. The entity's [`Transform`] is used as the
/// source position each frame.
#[derive(Component, Clone, Debug)]
pub struct Source<C: SimulationConfiguration = DefaultSimulationConfiguration>(
    pub crate::simulation::Source<C::Direct, C::Reflections, C::Pathing, C::ReflectionEffect>,
);

/// Per-source simulation parameters.
///
/// Optional companion to [`Source`]. When absent, [`SimulationParameters::default`] is used for
/// that source.
#[derive(Component, Default, Clone, Debug)]
pub struct SourceParameters<C: SimulationConfiguration = DefaultSimulationConfiguration>(
    pub SimulationParameters<C::Direct, C::Reflections, C::Pathing>,
);

/// Marks an entity as the listener for reverb simulation.
///
/// At most one entity should carry this component.
#[derive(Component, Debug)]
#[component(storage = "SparseSet")]
pub struct Listener;

/// Publishes a new snapshot of sources.
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
