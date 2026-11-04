//! High-level simulation pipeline built on top of the [runner layer](super::runner).
//!
//! The entry point is [`Simulation`], which manages a shared source buffer and spawns typed
//! simulation threads.
//!
//! For lower-level control, use [`SimulationRunner`] directly.

#[cfg(doc)]
use super::runner::SimulationRunner;

/// Bindings must stay inaccessible to prevent mixing static and runtime handles.
pub(crate) mod pipeline;
use pipeline::SimulationControls;
pub use pipeline::{
    SharedSimulationOutput, TypedSharedSources as SharedSources, TypedSimulation as Simulation,
    TypedSourceWithInputs as SourceWithInputs, TypedSourcesPool as SourcesPool,
};

/// Bindings must stay inaccessible to prevent mixing static and runtime handles.
pub(crate) mod direct;
pub use direct::TypedDirectSimulation as DirectSimulation;
/// Bindings must stay inaccessible to prevent mixing static and runtime handles.
pub(crate) mod reflections;
pub use reflections::TypedReflectionsSimulation as ReflectionsSimulation;
/// Bindings must stay inaccessible to prevent mixing static and runtime handles.
pub(crate) mod reflections_reverb;
pub use reflections_reverb::TypedReflectionsReverbSimulation as ReflectionsReverbSimulation;
/// Bindings must stay inaccessible to prevent mixing static and runtime handles.
pub(crate) mod pathing;
pub use pathing::TypedPathingSimulation as PathingSimulation;
