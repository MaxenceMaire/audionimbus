//! Components for spatial audio sources.

use super::configuration::{DefaultSimulationConfiguration, SimulationConfiguration};
use crate::simulation::SimulationParameters;
use bevy::prelude::Component;

#[cfg(doc)]
use bevy::prelude::Transform;

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
