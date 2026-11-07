//! High-level simulation pipeline built on top of the [runner layer](super::runner).
//!
//! The entry point is [`Simulation`], which manages a shared source buffer and spawns typed
//! simulation threads.
//!
//! For lower-level control, use [`SimulationRunner`] directly.

#[cfg(doc)]
use crate::wiring::runner::SimulationRunner;

/// Bindings must stay inaccessible to prevent mixing static and runtime handles.
pub(crate) mod pipeline;
use pipeline::SimulationControls;
#[cfg(dynamic_api)]
pub use pipeline::{DynamicSimulation as Simulation, DynamicSourceWithInputs as SourceWithInputs};
pub use pipeline::{
    SharedSimulationOutput, StaticSharedSources as SharedSources, StaticSourcesPool as SourcesPool,
};
#[cfg(static_api)]
pub use pipeline::{StaticSimulation as Simulation, StaticSourceWithInputs as SourceWithInputs};

/// Bindings must stay inaccessible to prevent mixing static and runtime handles.
pub(crate) mod direct;
#[cfg(dynamic_api)]
pub use direct::DynamicDirectSimulation as DirectSimulation;
#[cfg(static_api)]
pub use direct::StaticDirectSimulation as DirectSimulation;
/// Bindings must stay inaccessible to prevent mixing static and runtime handles.
pub(crate) mod reflections;
#[cfg(dynamic_api)]
pub use reflections::DynamicReflectionsSimulation as ReflectionsSimulation;
#[cfg(static_api)]
pub use reflections::StaticReflectionsSimulation as ReflectionsSimulation;
/// Bindings must stay inaccessible to prevent mixing static and runtime handles.
pub(crate) mod reflections_reverb;
#[cfg(dynamic_api)]
pub use reflections_reverb::DynamicReflectionsReverbSimulation as ReflectionsReverbSimulation;
#[cfg(static_api)]
pub use reflections_reverb::StaticReflectionsReverbSimulation as ReflectionsReverbSimulation;
/// Bindings must stay inaccessible to prevent mixing static and runtime handles.
pub(crate) mod pathing;
#[cfg(dynamic_api)]
pub use pathing::DynamicPathingSimulation as PathingSimulation;
#[cfg(static_api)]
pub use pathing::StaticPathingSimulation as PathingSimulation;
