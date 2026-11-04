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
    SharedSimulationOutput, StaticSharedSources as SharedSources, StaticSimulation as Simulation,
    StaticSourceWithInputs as SourceWithInputs, StaticSourcesPool as SourcesPool,
};

/// Bindings must stay inaccessible to prevent mixing static and runtime handles.
pub(crate) mod direct;
pub use direct::StaticDirectSimulation as DirectSimulation;
/// Bindings must stay inaccessible to prevent mixing static and runtime handles.
pub(crate) mod reflections;
pub use reflections::StaticReflectionsSimulation as ReflectionsSimulation;
/// Bindings must stay inaccessible to prevent mixing static and runtime handles.
pub(crate) mod reflections_reverb;
pub use reflections_reverb::StaticReflectionsReverbSimulation as ReflectionsReverbSimulation;
/// Bindings must stay inaccessible to prevent mixing static and runtime handles.
pub(crate) mod pathing;
pub use pathing::StaticPathingSimulation as PathingSimulation;
