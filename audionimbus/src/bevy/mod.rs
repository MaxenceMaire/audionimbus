//! Bevy integration for AudioNimbus.
//!
//! This module provides ECS-friendly wrappers around AudioNimbus types.
//!
//! Internally, it builds on top of the [`crate::wiring`] module.
//! Simulation work is pushed onto dedicated threads, while the Bevy world publishes scene,
//! listener, and source state each frame.
//!
//! The Bevy integration stops at simulation output.
//! Applying direct sound, reflections, reverb, or pathing to audio buffers is left to the
//! implementer, allowing flexibility in the choice of playback backend.
//!
//! The [`SpatialAudioPlugin`] inserts the core resources, spawns the simulation runners required
//! by the the selected configuration, and keeps them synchronized with ECS state.
//!
//! ```rust,no_run
//! use audionimbus::bevy::{Scene as AudioScene, *};
//! use bevy::prelude::*;
//!
//! fn main() {
//!     App::new()
//!         .add_plugins((
//!             DefaultPlugins,
//!             SpatialAudioPlugin::default(),
//!         ))
//!         .add_systems(Startup, setup)
//!         .run();
//! }
//!
//! fn setup(mut commands: Commands, context: Res<Context>, simulation: Res<Simulation>) {
//!     commands.spawn((
//!         AudioScene::try_new(&context).unwrap(),
//!         MainScene,
//!         Transform::default(),
//!         Visibility::default(),
//!     ));
//!
//!     commands.spawn((
//!         Listener,
//!         Source::try_new(&simulation).unwrap(),
//!         Transform::default(),
//!     ));
//!
//!     commands.spawn((
//!         Source::try_new(&simulation).unwrap(),
//!         Transform::from_xyz(2.0, 0.0, 0.0),
//!     ));
//! }
//! ```
//!
//! From there, read the latest outputs from resources such as [`DirectSimulation`],
//! [`ReflectionsSimulation`], [`ReflectionsReverbSimulation`], or [`PathingSimulation`] and feed
//! them into your own audio pipeline.

pub mod prelude;
pub use prelude::*;

pub mod asset;
pub mod configuration;
pub mod debug;
pub mod error;
pub mod geometry;
pub mod hrtf;
pub mod plugin;
pub mod probe;
pub mod runner;
pub mod simulation;
pub mod source;
pub mod system_set;
