//! Runtime-selected configuration.

mod geometry;
pub use geometry::{
    InstancedMesh, InstancedMeshSettings, RayTracerKind, RayTracerSettings, Scene, SceneError,
    StaticMesh,
};

mod reflections;
pub use reflections::{
    ReflectionAlgorithm, ReflectionEffect, ReflectionEffectParams, ReflectionError,
    ReflectionMixer, ReflectionsParameters, ReflectionsSettings,
};

mod simulation;
pub use simulation::{
    SimulationError, SimulationInputs, SimulationOutputs, SimulationParameters, SimulationSettings,
    SimulationSharedInputs, Simulator, Source,
};

mod baking;
pub use baking::{BakeError, PathBaker, ReflectionsBaker};

#[cfg(feature = "wiring")]
mod wiring;
#[cfg(feature = "wiring")]
pub use wiring::{
    DirectSimulation, PathingSimulation, ReflectionsOutput, ReflectionsReverbOutput,
    ReflectionsReverbSimulation, ReflectionsSimulation, Simulation, SimulationRunner,
    SourceWithInputs,
};
