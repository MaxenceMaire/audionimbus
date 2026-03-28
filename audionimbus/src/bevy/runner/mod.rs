use super::configuration::SimulationConfiguration;
use crate::sealed::Sealed;
use crate::simulation::{Pathing, Reflections};
use bevy::prelude::{App, World};

mod direct;
mod pathing;
mod reflections;
mod reflections_reverb;

pub use direct::*;
pub use pathing::*;
pub use reflections::*;
pub use reflections_reverb::*;

pub trait Runner: Sealed {
    type SimulationType;
}

impl Runner for () {
    type SimulationType = ();
}

pub trait ToRunner {
    type Runner: Runner;
}

impl ToRunner for () {
    type Runner = ();
}

impl ToRunner for Reflections {
    type Runner = RunnerReflectionsReverb;
}

impl ToRunner for Pathing {
    type Runner = RunnerPathing;
}

pub trait Spawn<C: SimulationConfiguration> {
    fn spawn(world: &mut World);
}

impl<C: SimulationConfiguration> Spawn<C> for () {
    fn spawn(_world: &mut World) {}
}

pub trait SyncFrame<C: SimulationConfiguration>: Sealed {
    fn add_systems(app: &mut App);
}

impl<C: SimulationConfiguration> SyncFrame<C> for () {
    fn add_systems(_app: &mut App) {}
}
