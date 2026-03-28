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
