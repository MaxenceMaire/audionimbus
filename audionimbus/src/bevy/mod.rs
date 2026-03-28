use crate::geometry::CoordinateSystem;
use crate::simulation::{SimulationInputs, SimulationParameters};
use crate::wiring::SourceWithInputs;
use bevy::prelude::{Component, Entity, Query, Res, Resource, SystemSet, Transform, Without};

pub mod configuration;
pub mod plugin;
pub mod runner;
pub mod simulation;

pub use configuration::*;
pub use plugin::*;
pub use runner::*;
pub use simulation::*;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum SpatialAudioSet {
    SyncSources,
    SyncFrames,
}

#[derive(Component, Clone, Debug)]
pub struct Source<C: SimulationConfiguration = DefaultSimulationConfiguration>(
    pub crate::simulation::Source<C::Direct, C::Reflections, C::Pathing, C::ReflectionEffect>,
);

#[derive(Component, Default, Clone, Debug)]
pub struct SourceParameters<C: SimulationConfiguration = DefaultSimulationConfiguration>(
    pub SimulationParameters<C::Direct, C::Reflections, C::Pathing>,
);

#[derive(Resource, Debug)]
pub struct SimulationSharedInputs<C: SimulationConfiguration = DefaultSimulationConfiguration>(
    pub crate::simulation::SimulationSharedInputs<C::Direct, C::Reflections, C::Pathing>,
);

impl<C: SimulationConfiguration> Default for SimulationSharedInputs<C> {
    fn default() -> Self {
        Self(crate::simulation::SimulationSharedInputs::default())
    }
}

fn sync_sources<C: SimulationConfiguration>(
    mut query: Query<
        (Entity, &Transform, &Source<C>, Option<&SourceParameters<C>>),
        Without<Listener>,
    >,
    simulation: Res<Simulation<C>>,
) {
    simulation.0.update_sources(|snapshot| {
        for (entity, transform, source, simulation_parameters) in query.iter_mut() {
            let simulation_inputs = SimulationInputs {
                source: coordinate_system_from_transform(*transform),
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

fn coordinate_system_from_transform(transform: Transform) -> CoordinateSystem {
    CoordinateSystem {
        right: transform.right().to_array().into(),
        up: transform.up().to_array().into(),
        ahead: transform.forward().to_array().into(),
        origin: transform.translation.to_array().into(),
    }
}
