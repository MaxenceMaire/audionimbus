//! Components for spatial audio sources.

use super::configuration::{DefaultSimulationConfiguration, SimulationConfiguration};
use super::simulation::{Simulation, SimulationSharedInputs};
use crate::error::SteamAudioError;
use crate::geometry::CoordinateSystem;
use crate::simulation::{
    DirectCompatible, PathingCompatible, ReflectionEffectCompatible, ReflectionsCompatible,
    SimulationFlagsProvider, SimulationInputs, SimulationParameters,
};
use crate::wiring::SourceWithInputs;
use bevy::prelude::{
    Add, Commands, Component, Entity, GlobalTransform, On, Query, Remove, Res, ResMut, With,
    Without,
};
use std::ops::{Deref, DerefMut};

/// Spatial audio source component.
///
/// Attach this to any entity that should emit sound. The entity's [`GlobalTransform`] is used as the
/// source position each frame.
#[derive(Component, Clone, Debug)]
pub struct Source<C: SimulationConfiguration = DefaultSimulationConfiguration>(
    pub crate::simulation::Source<C::Direct, C::Reflections, C::Pathing, C::ReflectionEffect>,
);

impl<C: SimulationConfiguration> Source<C> {
    /// Creates a new source using the [`Simulation`] resource.
    ///
    /// Mirrors [`crate::simulation::Source::try_new`].
    pub fn try_new(simulation: &Simulation<C>) -> Result<Self, SteamAudioError> {
        crate::simulation::Source::<
            C::Direct,
            C::Reflections,
            C::Pathing,
            C::ReflectionEffect,
        >::try_new(simulation.simulator())
        .map(Self)
    }
}

impl<C: SimulationConfiguration> Deref for Source<C> {
    type Target =
        crate::simulation::Source<C::Direct, C::Reflections, C::Pathing, C::ReflectionEffect>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<C: SimulationConfiguration> DerefMut for Source<C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<C: SimulationConfiguration>
    From<crate::simulation::Source<C::Direct, C::Reflections, C::Pathing, C::ReflectionEffect>>
    for Source<C>
{
    fn from(
        source: crate::simulation::Source<
            C::Direct,
            C::Reflections,
            C::Pathing,
            C::ReflectionEffect,
        >,
    ) -> Self {
        Self(source)
    }
}

/// Per-source simulation parameters.
///
/// Optional companion to [`Source`]. When absent, [`SimulationParameters::default`] is used for
/// that source.
#[derive(Component, Default, Clone, Debug)]
pub struct SourceParameters<C: SimulationConfiguration = DefaultSimulationConfiguration>(
    pub SimulationParameters<C::Direct, C::Reflections, C::Pathing>,
);

impl<C: SimulationConfiguration> From<SimulationParameters<C::Direct, C::Reflections, C::Pathing>>
    for SourceParameters<C>
{
    fn from(params: SimulationParameters<C::Direct, C::Reflections, C::Pathing>) -> Self {
        Self(params)
    }
}

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

/// Marks an entity as the simulator listener.
///
/// The entity's [`GlobalTransform`] is copied into [`SimulationSharedInputs`] each frame.
///
/// If the entity also carries [`Source`], it is used as the listener-centric reverb source.
///
/// At most one entity should carry this component.
/// Adding it to a new entity removes it from any entity that previously held it.
#[derive(Component, Debug)]
#[component(storage = "SparseSet")]
pub struct Listener;

/// Enforces the [`Listener`] exclusivity invariant.
///
/// When `Listener` is added to an entity, this observer removes it from any other entity
/// currently holding it, making the new entity the active listener.
pub(crate) fn on_listener_added(
    event: On<Add, Listener>,
    listeners: Query<Entity, With<Listener>>,
    mut commands: Commands,
) {
    for entity in &listeners {
        if entity != event.entity {
            commands.entity(entity).remove::<Listener>();
        }
    }
}

/// Synchronizes the listener transform into shared simulation inputs.
pub(crate) fn sync_simulation_shared_inputs_listener<C: SimulationConfiguration>(
    listeners: Query<&GlobalTransform, With<Listener>>,
    mut shared_inputs: ResMut<SimulationSharedInputs<C>>,
) {
    shared_inputs.0.set_listener(
        listeners
            .iter()
            .next()
            .map_or_else(CoordinateSystem::default, |listener| (*listener).into()),
    );
}

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
