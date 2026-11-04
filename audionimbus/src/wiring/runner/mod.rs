//! Primitives for driving simulation steps on dedicated threads.
//!
//! The entry point is [`SimulationRunner`], which owns the thread loop, commit signalling, and
//! lock-free input/output.
//!
//! Most projects should start with [`Simulation`](super::simulation::Simulation), which composes
//! [`SimulationRunner`] internally and handles all the wiring for you.
//! Reach for [`SimulationRunner`] directly when you need to drive a custom
//! [`SimulationRunner`](super::step::SimulationStep).

/// Bindings must stay inaccessible to prevent mixing static and runtime handles.
pub(crate) mod worker;
use worker::SourcesGuard;
pub use worker::{Allocate, Clear, Resolve, Shrink, TypedSimulationRunner as SimulationRunner};

/// Bindings must stay inaccessible to prevent mixing static and runtime handles.
pub(crate) mod direct;
pub use direct::{
    TypedDirectFrame as DirectFrame, TypedResolvedDirectFrame as ResolvedDirectFrame,
};
/// Bindings must stay inaccessible to prevent mixing static and runtime handles.
pub(crate) mod pathing;
pub use pathing::{
    TypedPathingFrame as PathingFrame, TypedResolvedPathingFrame as ResolvedPathingFrame,
};
/// Bindings must stay inaccessible to prevent mixing static and runtime handles.
pub(crate) mod reflections;
pub use reflections::{
    TypedReflectionsFrame as ReflectionsFrame,
    TypedResolvedReflectionsFrame as ResolvedReflectionsFrame,
};
/// Bindings must stay inaccessible to prevent mixing static and runtime handles.
pub(crate) mod reflections_reverb;
pub use reflections_reverb::{
    TypedReflectionsReverbFrame as ReflectionsReverbFrame,
    TypedResolvedReflectionsReverbFrame as ResolvedReflectionsReverbFrame,
};
