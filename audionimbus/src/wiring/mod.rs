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
//!
//! ## Example
//!
//! The example below wires up direct (occlusion/attenuation) and reflections simulation
//! for a small scene with two sources.
//! It mirrors a typical game-loop structure [split into separate threads](crate::simulation#multi-threading-architecture):
//! - Game thread: Calls [`Simulation::update_sources`] each frame to publish the latest sources.
//! - Simulation threads: Spawned by [`Simulation::spawn_direct`] and similar methods; runs continuously,
//!   picking up the latest sources on each iteration.
//! - Audio thread: Reads the most recent simulation outputs available and applies the
//!   corresponding effects.
//!
//! ```
//! # use audionimbus::wiring::*;
//! # use audionimbus::*;
//! # let context = Context::default();
//! # let audio_settings = AudioSettings::default();
//! let simulation_settings = SimulationSettings::new(&audio_settings)
//!     .with_direct(DirectSimulationSettings {
//!         max_num_occlusion_samples: 32,
//!     })
//!     .with_reflections(ConvolutionSettings {
//!         max_num_rays: 4096,
//!         num_diffuse_samples: 32,
//!         max_duration: 2.0,
//!         max_num_sources: 8,
//!         num_threads: 2,
//!         max_order: 2,
//!     });
//!
//! let mut simulator = Simulator::try_new(&context, &simulation_settings)?;
//!
//! // Build and commit a scene so the simulator has geometry to trace rays against.
//! let mut scene = Scene::try_new(&context)?;
//! // ... add static meshes to the scene ...
//! scene.commit();
//! simulator.set_scene(&scene);
//! simulator.commit();
//!
//! // Create two sources.
//! let source_a = Source::<Direct, Reflections, (), Convolution>::try_new(&simulator)?;
//! let source_b = Source::<Direct, Reflections, (), Convolution>::try_new(&simulator)?;
//! simulator.add_source(&source_a);
//! simulator.add_source(&source_b);
//!
//! // Sources are identified by a u32 `SourceId` here, but any `Clone + Send + Sync` type works.
//! let mut simulation = Simulation::new::<u32>(simulator);
//!
//! // Commit is required after adding sources so the simulator picks them up.
//! simulation.request_commit();
//!
//! // Spawn one thread per simulation type.
//! // Each thread shares the same list of sources that the game thread writes every frame.
//! let direct_simulation = simulation.spawn_direct();
//! let reflections_simulation = simulation.spawn_reflections();
//!
//! // Game thread loop (runs every frame)
//! // Publish the world-state snapshot for this frame.
//! // Simulation threads will pick it up on their next iteration.
//! // In a real game this would live inside the frame/tick function.
//! # let listener_transform = CoordinateSystem::default();
//! # let source_a_transform = CoordinateSystem::default();
//! # let source_b_transform = CoordinateSystem::default();
//! simulation.update_sources(|sources| {
//!     for (id, source, transform) in [
//!         (0, &source_a, source_a_transform),
//!         (1, &source_b, source_b_transform),
//!     ] {
//!         sources.push((
//!             id,
//!             SourceWithInputs {
//!                 source: source.clone(),
//!                 simulation_inputs: SimulationInputs {
//!                     source: transform,
//!                     parameters: SimulationParameters::new()
//!                         .with_direct(DirectSimulationParameters::new())
//!                         .with_reflections(ConvolutionParameters {
//!                             baked_data_identifier: None,
//!                         }),
//!                 },
//!             },
//!         ));
//!     }
//! });
//!
//! // Audio thread
//! // `load` is lock-free and safe to call from the audio thread at any time.
//! let direct_outputs = direct_simulation.output.load();
//! let reflections_outputs = reflections_simulation.output.load();
//! for (id, params) in direct_outputs.iter() {
//!     // Apply direct effect for source `id`.
//! }
//! for (id, params) in reflections_outputs.sources.iter() {
//!     // Apply reflections effect for source `id`.
//! }
//!
//! // Teardown
//! simulation.shutdown();
//! direct_simulation.handle.join().expect("direct simulation thread panicked");
//! reflections_simulation.handle.join().expect("reflections simulation thread panicked");
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## Pausing and resuming simulation threads
//!
//! Simulation is compute-intensive.
//! If no sources are active (e.g. during a loading screen), you can pause all spawned threads
//! without tearing them down:
//!
//! ```
//! # use audionimbus::wiring::*;
//! # use audionimbus::*;
//! # let context = Context::default();
//! # let audio_settings = AudioSettings::default();
//! # let simulation_settings = SimulationSettings::new(&audio_settings)
//! #     .with_direct(DirectSimulationSettings {
//! #         max_num_occlusion_samples: 32,
//! #     });
//! # let simulator = Simulator::try_new(&context, &simulation_settings)?;
//! # let mut simulation = Simulation::new::<()>(simulator);
//! # let direct_simulator = simulation.spawn_direct();
//! // Pause all simulation threads.
//! simulation.pause();
//!
//! // ... do other work ...
//!
//! // Resume when gameplay is about to start again.
//! simulation.resume();
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! Individual simulations also expose their own [`pause`](DirectSimulation::pause) /
//! [`resume`](DirectSimulation::resume) methods for finer control.

#[cfg(doc)]
use crate::simulation::{Simulator, Source};
#[cfg(doc)]
use runner::SimulationRunner;
#[cfg(doc)]
use simulation::Simulation;
#[cfg(doc)]
use step::SimulationStep;

pub mod runner;
pub mod simulation;
pub mod step;

pub use runner::*;
pub use simulation::*;
pub use step::*;
