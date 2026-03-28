use super::super::configuration::SimulationConfiguration;
use super::super::{Simulation, SimulationSharedInputs, SpatialAudioSet};
use super::{Runner, Spawn, SyncFrame, ToRunner};
use crate::effect::direct::DirectEffectParams;
use crate::sealed::Sealed;
use crate::simulation::{Direct, PathingCompatible, ReflectionsCompatible};
use crate::wiring::{Allocate, DirectFrame};
use bevy::prelude::{
    App, Entity, IntoScheduleConfigs, PostUpdate, Res, Resource, World, resource_exists,
};
use std::ops::{Deref, DerefMut};

pub struct RunnerDirect;

impl Sealed for RunnerDirect {}
impl Runner for RunnerDirect {
    type SimulationType = Direct;
}

impl ToRunner for Direct {
    type Runner = RunnerDirect;
}

impl<C> Spawn<C> for RunnerDirect
where
    C: SimulationConfiguration<Direct = Direct>,
    Vec<DirectEffectParams>:
        Allocate<DirectFrame<Entity, C::Direct, C::Reflections, C::Pathing, C::ReflectionEffect>>,
    (): ReflectionsCompatible<<C as SimulationConfiguration>::Reflections>
        + PathingCompatible<<C as SimulationConfiguration>::Pathing>,
{
    fn spawn(world: &mut World) {
        let runner = world.resource_mut::<Simulation<C>>().spawn_direct();
        world.insert_resource(DirectSimulation::<C>(runner));
    }
}

impl<C> SyncFrame<C> for RunnerDirect
where
    C: SimulationConfiguration<Direct = Direct>,
{
    fn add_systems(app: &mut App) {
        app.add_systems(
            PostUpdate,
            sync_direct_frame::<C>
                .run_if(resource_exists::<DirectSimulation<C>>)
                .in_set(SpatialAudioSet::SyncFrames),
        );
    }
}

fn sync_direct_frame<C>(
    simulation: Res<Simulation<C>>,
    direct: Res<DirectSimulation<C>>,
    shared_inputs: Res<SimulationSharedInputs<C>>,
) where
    C: SimulationConfiguration<Direct = Direct>,
{
    direct.set_input(DirectFrame {
        sources: simulation.sources.clone(),
        shared_inputs: shared_inputs.0.clone(),
    });
}

#[derive(Resource)]
pub struct DirectSimulation<C: SimulationConfiguration>(
    pub crate::wiring::DirectSimulation<Entity, C::Reflections, C::Pathing, C::ReflectionEffect>,
);

impl<C: SimulationConfiguration> Deref for DirectSimulation<C> {
    type Target =
        crate::wiring::DirectSimulation<Entity, C::Reflections, C::Pathing, C::ReflectionEffect>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<C: SimulationConfiguration> DerefMut for DirectSimulation<C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
