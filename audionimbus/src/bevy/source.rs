//! Components for spatial audio sources.

use super::configuration::{DefaultSimulationConfiguration, SimulationConfiguration};
use super::simulation::Simulation;
use crate::simulation::{
    DirectCompatible, PathingCompatible, ReflectionEffectCompatible, ReflectionsCompatible,
    SimulationFlagsProvider, SimulationInputs, SimulationParameters,
};
use crate::wiring::SourceWithInputs;
use bevy::prelude::{
    Add, Component, Entity, GlobalTransform, On, Query, Remove, Res, ResMut, Without,
};

/// Spatial audio source component.
///
/// Attach this to any entity that should emit sound. The entity's [`GlobalTransform`] is used as the
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

/// Adds a `Source` to the simulator and requests a commit when it is spawned.
pub fn on_source_added<C: SimulationConfiguration>(
    event: On<Add, Source<C>>,
    query: Query<&Source<C>>,
    mut simulation: ResMut<Simulation<C>>,
) where
    C::Direct: DirectCompatible<C::Direct> + SimulationFlagsProvider,
    C::Reflections: ReflectionsCompatible<C::Reflections> + SimulationFlagsProvider,
    C::Pathing: PathingCompatible<C::Pathing> + SimulationFlagsProvider,
    C::ReflectionEffect: ReflectionEffectCompatible<C::Reflections, C::ReflectionEffect>,
{
    let source = query.get(event.entity).unwrap();
    simulation.0.simulator.add_source(&source.0);
    simulation.0.request_simulator_commit();
}

/// Removes a `Source` from the simulator and requests a commit when it is removed.
pub fn on_source_removed<C: SimulationConfiguration>(
    event: On<Remove, Source<C>>,
    query: Query<&Source<C>>,
    mut simulation: ResMut<Simulation<C>>,
) where
    C::Direct: DirectCompatible<C::Direct> + SimulationFlagsProvider,
    C::Reflections: ReflectionsCompatible<C::Reflections> + SimulationFlagsProvider,
    C::Pathing: PathingCompatible<C::Pathing> + SimulationFlagsProvider,
    C::ReflectionEffect: ReflectionEffectCompatible<C::Reflections, C::ReflectionEffect>,
{
    let source = query.get(event.entity).unwrap();
    simulation.0.simulator.remove_source(&source.0);
    simulation.0.request_simulator_commit();
}

/// Marks an entity as the listener for reverb simulation.
///
/// At most one entity should carry this component.
#[derive(Component, Debug)]
#[component(storage = "SparseSet")]
pub struct Listener;

/// Publishes a new snapshot of sources.
pub(crate) fn sync_sources<C: SimulationConfiguration>(
    mut query: Query<
        (
            Entity,
            &GlobalTransform,
            &Source<C>,
            Option<&SourceParameters<C>>,
        ),
        Without<Listener>,
    >,
    simulation: Res<Simulation<C>>,
) {
    simulation.0.update_sources(|snapshot| {
        for (entity, global_transform, source, simulation_parameters) in query.iter_mut() {
            let simulation_inputs = SimulationInputs {
                source: (*global_transform).into(),
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
