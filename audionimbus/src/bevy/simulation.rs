use super::configuration::{DefaultSimulationConfiguration, SimulationConfiguration};
use bevy::prelude::{Entity, Resource};
use std::ops::{Deref, DerefMut};

#[derive(Resource)]
pub struct Simulation<C: SimulationConfiguration = DefaultSimulationConfiguration>(
    pub  crate::wiring::Simulation<
        Entity,
        C::RayTracer,
        C::Direct,
        C::Reflections,
        C::Pathing,
        C::ReflectionEffect,
    >,
);

impl<C: SimulationConfiguration> Deref for Simulation<C> {
    type Target = crate::wiring::Simulation<
        Entity,
        C::RayTracer,
        C::Direct,
        C::Reflections,
        C::Pathing,
        C::ReflectionEffect,
    >;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<C: SimulationConfiguration> DerefMut for Simulation<C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Resource, Debug)]
pub struct SimulationSharedInputs<C: SimulationConfiguration = DefaultSimulationConfiguration>(
    pub crate::simulation::SimulationSharedInputs<C::Direct, C::Reflections, C::Pathing>,
);

impl<C: SimulationConfiguration> Default for SimulationSharedInputs<C> {
    fn default() -> Self {
        Self(crate::simulation::SimulationSharedInputs::default())
    }
}
