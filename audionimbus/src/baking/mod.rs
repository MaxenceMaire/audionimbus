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

use std::sync::Mutex;

#[cfg(doc)]
use crate::effect::pathing::PathEffect;
#[cfg(doc)]
use crate::effect::reflections::ReflectionEffect;
#[cfg(doc)]
use crate::probe::ProbeBatch;
#[cfg(doc)]
use crate::simulation::Simulator;

static BAKE_LOCK: Mutex<()> = Mutex::new(());

mod baked_data;
pub use baked_data::*;

mod error;
pub use error::*;

pub mod pathing;
pub use pathing::*;

pub mod reflections;
pub use reflections::*;
