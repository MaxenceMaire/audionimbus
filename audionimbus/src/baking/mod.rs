//! Baking of compute-intensive processes (reflections, pathing).
//!
//! Simulating reflections or pathing in real-time is very compute-intensive.
//! Bakers allow you to precompute these simulations beforehand.
//!
//! Baked data is stored in a [`ProbeBatch`], which can then be added to and used by a [`Simulator`].
//!
//! ## Available bakers
//!
//! - [`ReflectionsBaker`]: Precomputes how sound propagates from sources to listeners via reflections.
//!   Real-time reflection simulation (see [`ReflectionEffect`]) is very compute-intensive,
//!   so this baker lets you precompute reflections throughout a scene offline.
//!
//! - [`PathBaker`]: Precomputes pathing data, an alternative simulation method that finds
//!   the shortest unoccluded paths from sources to listeners by traveling between probes.
//!   Pathing requires probe generation (see [`PathEffect`]) and is typically baked offline.

#[cfg(doc)]
use crate::effect::path::PathEffect;
#[cfg(doc)]
use crate::effect::reflection::ReflectionEffect;
#[cfg(doc)]
use crate::probe::ProbeBatch;
#[cfg(doc)]
use crate::simulation::Simulator;

pub mod reflections;
pub use reflections::*;

pub mod baked_data;
pub use baked_data::*;
