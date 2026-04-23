//! Bevy resource wrapper for the core simulation pipeline.

use super::configuration::{DefaultSimulationConfiguration, SimulationConfiguration};
use bevy::prelude::{Entity, Resource};
use std::ops::{Deref, DerefMut};

/// Resource wrapping a [`wiring::Simulation`](crate::wiring::simulation::Simulation).
#[derive(Resource)]
pub struct Simulation<C: SimulationConfiguration = DefaultSimulationConfiguration>(
    pub WiringSimulation<C>,
);

impl<C: SimulationConfiguration> Deref for Simulation<C> {
    type Target = WiringSimulation<C>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<C: SimulationConfiguration> DerefMut for Simulation<C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Wiring simulation.
type WiringSimulation<C> = crate::wiring::simulation::Simulation<
    Entity,
    <C as SimulationConfiguration>::RayTracer,
    <C as SimulationConfiguration>::Direct,
    <C as SimulationConfiguration>::Reflections,
    <C as SimulationConfiguration>::Pathing,
    <C as SimulationConfiguration>::ReflectionEffect,
>;

/// Shared simulation inputs, updated each frame by the game thread.
#[derive(Resource, Debug)]
pub struct SimulationSharedInputs<C: SimulationConfiguration = DefaultSimulationConfiguration>(
    pub crate::simulation::SimulationSharedInputs<C::Direct, C::Reflections, C::Pathing>,
);

impl<C: SimulationConfiguration> Default for SimulationSharedInputs<C> {
    fn default() -> Self {
        Self(crate::simulation::SimulationSharedInputs::default())
    }
}

/// Identifies a simulation thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SimulationThread {
    /// Direct (occlusion, attenuation) simulation thread.
    Direct,
    /// Reflections simulation thread.
    Reflections,
    /// Combined reflections and listener-centric reverb simulation thread.
    ReflectionsReverb,
    /// Pathing simulation thread.
    Pathing,
}
