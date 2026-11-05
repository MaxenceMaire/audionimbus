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
#[cfg(all(feature = "dynamic", not(feature = "static")))]
pub use pipeline::{DynamicSimulation as Simulation, DynamicSourceWithInputs as SourceWithInputs};
pub use pipeline::{
    SharedSimulationOutput, StaticSharedSources as SharedSources, StaticSourcesPool as SourcesPool,
};
#[cfg(any(feature = "static", not(feature = "dynamic")))]
pub use pipeline::{StaticSimulation as Simulation, StaticSourceWithInputs as SourceWithInputs};

/// Bindings must stay inaccessible to prevent mixing static and runtime handles.
pub(crate) mod direct;
#[cfg(all(feature = "dynamic", not(feature = "static")))]
pub use direct::DynamicDirectSimulation as DirectSimulation;
#[cfg(any(feature = "static", not(feature = "dynamic")))]
pub use direct::StaticDirectSimulation as DirectSimulation;
/// Bindings must stay inaccessible to prevent mixing static and runtime handles.
pub(crate) mod reflections;
#[cfg(all(feature = "dynamic", not(feature = "static")))]
pub use reflections::DynamicReflectionsSimulation as ReflectionsSimulation;
#[cfg(any(feature = "static", not(feature = "dynamic")))]
pub use reflections::StaticReflectionsSimulation as ReflectionsSimulation;
/// Bindings must stay inaccessible to prevent mixing static and runtime handles.
pub(crate) mod reflections_reverb;
#[cfg(all(feature = "dynamic", not(feature = "static")))]
pub use reflections_reverb::DynamicReflectionsReverbSimulation as ReflectionsReverbSimulation;
#[cfg(any(feature = "static", not(feature = "dynamic")))]
pub use reflections_reverb::StaticReflectionsReverbSimulation as ReflectionsReverbSimulation;
/// Bindings must stay inaccessible to prevent mixing static and runtime handles.
pub(crate) mod pathing;
#[cfg(all(feature = "dynamic", not(feature = "static")))]
pub use pathing::DynamicPathingSimulation as PathingSimulation;
#[cfg(any(feature = "static", not(feature = "dynamic")))]
pub use pathing::StaticPathingSimulation as PathingSimulation;
