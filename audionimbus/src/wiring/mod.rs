//! High-level helpers for running simulations following best practices.
//!
//! A [multi-threaded simulation architecture](crate::simulation#multi-threading-architecture) is recommended to get the best out of Steam Audio.
//! However, getting the configuration right is error-prone, and mistakes can lead to poor
//! game performance.
//!
//! This module provides a toolbox to compose simulation pipelines that adhere to best practices,
//! using different levels of abstraction:
//! - [`Simulation`]: the recommended starting point.
//!   It can spawn concurrent simulation threads that all share the same lock-free [`Source`] buffers.
//!   The game thread publishes a new buffer of sources each frame, and the audio thread retrieves
//!   simulation outputs without ever blocking.
//! - [`SimulationRunner`] for advanced use cases where you need to drive a custom [`SimulationStep`] on a dedicated thread.
//!
//! For full control, you can bypass the `wiring` module entirely and use the core AudioNimbus API
//! directly (see the [`Simulator`]).
//!
//! This module is gated behind the `wiring` feature (enabled by default).

#[cfg(doc)]
use crate::simulation::{Simulator, Source};

mod runner;
pub use runner::*;
mod simulation;
pub use simulation::*;
mod step;
pub use step::*;
