//! Bevy resource wrapper for the core simulation pipeline.

use super::binding::{ConfigurationBinding, DefaultConfiguration};
use bevy::prelude::{Entity, Resource};
use std::borrow::{Borrow, BorrowMut};
use std::ops::{Deref, DerefMut};

/// Resource wrapping a [`wiring::Simulation`](crate::wiring::simulation::Simulation).
#[derive(Resource)]
pub struct Simulation<C: ConfigurationBinding = DefaultConfiguration>(pub WiringSimulation<C>);

impl<C: ConfigurationBinding> Deref for Simulation<C> {
    type Target = crate::wiring::simulation::pipeline::Simulation<Entity, C::Pipeline>;

    fn deref(&self) -> &Self::Target {
        self.0.borrow()
    }
}

impl<C: ConfigurationBinding> DerefMut for Simulation<C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.borrow_mut()
    }
}

/// Wiring simulation.
type WiringSimulation<C> = <C as ConfigurationBinding>::Simulation;

/// Shared simulation inputs, updated each frame by the game thread.
#[derive(Resource, Debug)]
pub struct SimulationSharedInputs<C: ConfigurationBinding = DefaultConfiguration>(
    pub C::SharedInputs,
);

impl<C: ConfigurationBinding> Default for SimulationSharedInputs<C> {
    fn default() -> Self {
        Self(C::SharedInputs::default())
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

/// Runtime simulation pipeline resource.
#[cfg(feature = "dynamic")]
pub type DynamicSimulation = Simulation<super::binding::RuntimeConfiguration>;

/// Shared inputs resource for runtime simulation.
#[cfg(feature = "dynamic")]
pub type DynamicSimulationSharedInputs =
    SimulationSharedInputs<super::binding::RuntimeConfiguration>;
