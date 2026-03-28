use super::configuration::{DefaultSimulationConfiguration, SimulationConfiguration};
use crate::simulation::SimulationParameters;
use bevy::prelude::Component;

#[derive(Component, Clone, Debug)]
pub struct Source<C: SimulationConfiguration = DefaultSimulationConfiguration>(
    pub crate::simulation::Source<C::Direct, C::Reflections, C::Pathing, C::ReflectionEffect>,
);

#[derive(Component, Default, Clone, Debug)]
pub struct SourceParameters<C: SimulationConfiguration = DefaultSimulationConfiguration>(
    pub SimulationParameters<C::Direct, C::Reflections, C::Pathing>,
);

/// The listener used for reverb simulation.
#[derive(Component, Debug)]
#[component(storage = "SparseSet")]
pub struct Listener;
