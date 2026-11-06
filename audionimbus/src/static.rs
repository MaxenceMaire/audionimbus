//! Compile-time configuration.

pub use crate::baking::error::BakeError;
pub use crate::baking::pathing::baker::StaticPathBaker as PathBaker;
pub use crate::baking::reflections::baker::StaticReflectionsBaker as ReflectionsBaker;
pub use crate::effect::reflections::effect::{
    StaticReflectionEffect as ReflectionEffect,
    StaticReflectionEffectParams as ReflectionEffectParams,
};
pub use crate::effect::reflections::mixer::StaticReflectionMixer as ReflectionMixer;
pub use crate::geometry::instanced_mesh::{
    StaticInstancedMesh as InstancedMesh, StaticInstancedMeshSettings as InstancedMeshSettings,
};
pub use crate::geometry::scene::StaticScene as Scene;
pub use crate::geometry::static_mesh::StaticStaticMesh as StaticMesh;
pub use crate::prelude::*;
pub use crate::simulation::error::SimulationError;
pub use crate::simulation::inputs::{
    SimulationParameters, StaticSimulationInputs as SimulationInputs,
};
pub use crate::simulation::settings::StaticSimulationSettings as SimulationSettings;
pub use crate::simulation::simulator::{
    StaticSimulationSharedInputs as SimulationSharedInputs, StaticSimulator as Simulator,
};
pub use crate::simulation::source::{
    StaticSimulationOutputs as SimulationOutputs, StaticSource as Source,
};

#[cfg(feature = "wiring")]
pub use crate::wiring::runner::worker::StaticSimulationRunner as SimulationRunner;
#[cfg(feature = "wiring")]
pub use crate::wiring::simulation::direct::StaticDirectSimulation as DirectSimulation;
#[cfg(feature = "wiring")]
pub use crate::wiring::simulation::pathing::StaticPathingSimulation as PathingSimulation;
#[cfg(feature = "wiring")]
pub use crate::wiring::simulation::pipeline::StaticSimulation as Simulation;
#[cfg(feature = "wiring")]
pub use crate::wiring::simulation::pipeline::StaticSourceWithInputs as SourceWithInputs;
#[cfg(feature = "wiring")]
pub use crate::wiring::simulation::reflections::StaticReflectionsSimulation as ReflectionsSimulation;
#[cfg(feature = "wiring")]
pub use crate::wiring::simulation::reflections_reverb::StaticReflectionsReverbSimulation as ReflectionsReverbSimulation;
#[cfg(feature = "wiring")]
pub use crate::wiring::step::reflections::StaticReflectionsOutput as ReflectionsOutput;
#[cfg(feature = "wiring")]
pub use crate::wiring::step::reflections_reverb::StaticReflectionsReverbOutput as ReflectionsReverbOutput;
#[cfg(feature = "wiring")]
pub use crate::wiring::*;

/// Bevy integration with compile-time configurations.
#[cfg(feature = "bevy")]
pub mod bevy {
    pub use crate::bevy::*;
}

#[cfg(feature = "fmod")]
pub use crate::fmod_integration::static_api as fmod;
#[cfg(feature = "wwise")]
pub use crate::wwise_integration::static_api as wwise;
