//! High-level simulation pipeline built on top of the [runner layer](super::runner).
//!
//! The entry point is [`Simulation`], which manages a shared source buffer and spawns typed
//! simulation threads.
//!
//! For lower-level control, use [`SimulationRunner`] directly.

#[cfg(doc)]
use super::runner::SimulationRunner;

use crate::geometry::Scene;
use crate::ray_tracing::RayTracer;
use crate::simulation::{SimulationInputs, Simulator, Source};
use arc_swap::ArcSwap;
use object_pool::{Pool, ReusableOwned};
use std::collections::HashSet;
use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicBool, Ordering},
};

mod direct;
pub use direct::*;
mod reflections;
pub use reflections::*;
mod reflections_reverb;
pub use reflections_reverb::*;
mod pathing;
pub use pathing::*;

/// Simulation pipeline.
///
/// Spawns simulation threads via:
/// - [`Self::spawn_direct`]
/// - [`Self::spawn_reflections`]
/// - [`Self::spawn_reflections_reverb`]
/// - [`Self::spawn_pathing`]
///
/// Spawned simulation threads share the same sources, updated atomically by the game thread via
/// [`Self::update_sources`].
///
/// Uses memory pooling to avoid per-frame allocation.
///
/// # Type parameters
///
/// | Parameter | Role |
/// |---|---|
/// | `SourceId` | Identifier type for audio sources (e.g. `u32`) |
/// | `T` | Ray tracer backend |
/// | `D` | Direct simulation mode ([`Direct`](crate::simulation::Direct) or [`()`](primitive@unit)) |
/// | `R` | Reflections mode ([`Reflections`](crate::simulation::Reflections) or [`()`](primitive@unit)) |
/// | `P` | Pathing mode ([`Pathing`](crate::simulation::Pathing) or [`()`](primitive@unit)) |
/// | `RE` | Reflection effect type (see [`ReflectionEffectType`](crate::effect::ReflectionEffectType)) |
///
/// Using [`()`](primitive@unit) for a mode disables it.
///
/// # Example
///
/// ```
/// # use audionimbus::wiring::*;
/// # use audionimbus::*;
/// # let context = Context::default();
/// # let audio_settings = AudioSettings::default();
/// let simulation_settings = SimulationSettings::new(&audio_settings)
///     .with_direct(DirectSimulationSettings {
///         max_num_occlusion_samples: 32,
///     })
///     .with_reflections(ConvolutionSettings {
///         max_num_rays: 4096,
///         num_diffuse_samples: 32,
///         max_duration: 2.0,
///         max_num_sources: 8,
///         num_threads: 2,
///         max_order: 2,
///     });
///
/// let mut simulator = Simulator::try_new(&context, &simulation_settings)?;
///
/// // `u32` is the source ID type. Any `Clone + Send + Sync` type works.
/// let mut simulation = Simulation::new::<u32>(simulator);
/// let on_error = |error| {
///     eprintln!("{error}");
/// };
/// let mut direct_simulation = simulation.spawn_direct(on_error);
/// let mut reflections_simulation = simulation.spawn_reflections(on_error);
///
/// simulation.shutdown();
/// direct_simulation.join().expect("direct thread panicked");
/// reflections_simulation.join().expect("reflections thread panicked");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct Simulation<SourceId, T, D, R, P, RE>
where
    T: RayTracer,
{
    /// The underlying simulator shared across all spawned simulation threads.
    pub simulator: Simulator<T, D, R, P, RE>,

    /// Pool of source buffers, reused across frames to avoid allocation.
    pub sources_pool: SourcesPool<SourceId, D, R, P, RE>,

    /// The current source buffer, atomically swapped each frame via [`Self::update_sources`].
    pub sources: SharedSources<SourceId, D, R, P, RE>,

    /// Set to `true` via [`Self::request_simulator_commit`] to trigger a simulator commit on the next
    /// simulation run.
    pub simulator_commit_needed: Arc<AtomicBool>,

    /// Scenes pending a commit.
    pub pending_scene_commits: Arc<ArcSwap<HashSet<Scene<T>>>>,

    /// Shutdown flags for each spawned simulation thread, in spawn order.
    pub shutdowns: Vec<Arc<AtomicBool>>,

    /// Pause flags for each spawned simulation thread, in spawn order.
    ///
    /// Simulation threads can be paused via [`Self::pause`] and resumed via [`Self::resume`].
    pub paused: Vec<Arc<(Mutex<bool>, Condvar)>>,
}

impl<T, D, R, P, RE> Simulation<(), T, D, R, P, RE>
where
    T: RayTracer,
{
    /// Creates a new simulation pipeline.
    ///
    /// `SourceId` is the identifier type for audio sources.
    /// It enables mapping each output back to its originating source.
    pub fn new<SourceId>(
        simulator: Simulator<T, D, R, P, RE>,
    ) -> Simulation<SourceId, T, D, R, P, RE> {
        let sources_pool = Arc::new(Pool::new(4, Default::default));
        let sources = Arc::new(ArcSwap::new(Arc::new(
            sources_pool.pull_owned(Default::default),
        )));
        let simulator_commit_needed = Arc::new(AtomicBool::new(false));
        let pending_scene_commits = Arc::new(ArcSwap::new(Arc::new(HashSet::new())));
        let shutdowns = vec![];
        let paused = vec![];

        Simulation {
            simulator,
            sources_pool,
            sources,
            simulator_commit_needed,
            pending_scene_commits,
            shutdowns,
            paused,
        }
    }
}

impl<SourceId, T, D, R, P, RE> Simulation<SourceId, T, D, R, P, RE>
where
    T: RayTracer,
{
    /// Returns a reference to the underlying simulator.
    pub fn simulator(&self) -> &Simulator<T, D, R, P, RE> {
        &self.simulator
    }

    /// Returns a reference to the sources pool.
    pub fn sources_pool(&self) -> &SourcesPool<SourceId, D, R, P, RE> {
        &self.sources_pool
    }

    /// Updates the sources used by all simulation threads on their next run.
    ///
    /// `f` receives a pooled `Vec` to populate with the new frame's `(SourceId, SourceWithInputs)`
    /// pairs.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use audionimbus::wiring::*;
    /// # use audionimbus::*;
    /// # let context = Context::default();
    /// # let audio_settings = AudioSettings::default();
    /// # let simulation_settings = SimulationSettings::new(&audio_settings)
    /// #     .with_direct(DirectSimulationSettings { max_num_occlusion_samples: 4 });
    /// # let simulator = Simulator::try_new(&context, &simulation_settings)?;
    /// # let source = Source::<Direct, (), (), ()>::try_new(&simulator)?;
    /// # let simulation = Simulation::new::<u32>(simulator);
    /// # let transform = CoordinateSystem::default();
    /// simulation.update_sources(|sources| {
    ///     sources.push((
    ///         42,
    ///         SourceWithInputs {
    ///             source: source.clone(),
    ///             simulation_inputs: SimulationInputs {
    ///                 source: transform,
    ///                 parameters: SimulationParameters::new()
    ///                     .with_direct(DirectSimulationParameters::default()),
    ///             },
    ///         },
    ///     ));
    /// });
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn update_sources<F>(&self, f: F)
    where
        F: FnOnce(&mut Vec<(SourceId, SourceWithInputs<D, R, P, RE>)>),
    {
        let mut sources = self.sources_pool.pull_owned(Vec::default);
        sources.clear();
        f(&mut sources);
        self.sources.store(Arc::new(sources));
    }

    /// Signals all spawned simulation threads to commit the [`Simulator`] changes on their next run.
    pub fn request_simulator_commit(&mut self) {
        self.simulator_commit_needed.store(true, Ordering::Relaxed);
    }

    /// Requests simulation threads to commit the provided scenes on their next run.
    pub fn request_scene_commits(&self, scene_commits: &[Scene<T>]) {
        self.pending_scene_commits.rcu(|pending_scene_commits| {
            // SAFETY: Scene's Hash and Eq are based on pointer identity, not interior state.
            #[allow(clippy::mutable_key_type)]
            let mut new_pending_scene_commits =
                HashSet::with_capacity(pending_scene_commits.len() + scene_commits.len());
            new_pending_scene_commits.extend(pending_scene_commits.iter().cloned());
            new_pending_scene_commits.extend(scene_commits.iter().cloned());
            new_pending_scene_commits
        });
    }

    /// Signals all spawned simulation threads to stop.
    pub fn shutdown(&self) {
        for shutdown in &self.shutdowns {
            shutdown.store(true, Ordering::Relaxed);
        }

        // Wake paused threads so they can observe their shutdown flag.
        self.resume();
    }

    /// Pauses the simulation threads after their current iteration completes.
    pub fn pause(&self) {
        for thread in &self.paused {
            *thread.0.lock().unwrap() = true;
        }
    }

    /// Resumes paused simulation threads.
    pub fn resume(&self) {
        for thread in &self.paused {
            *thread.0.lock().unwrap() = false;
            thread.1.notify_one();
        }
    }
}

impl<SourceId, T, D, R, P, RE> Drop for Simulation<SourceId, T, D, R, P, RE>
where
    T: RayTracer,
{
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Lifecycle controls for a simulation thread.
pub(crate) struct SimulationControls {
    handle: Option<std::thread::JoinHandle<()>>,
    paused: Arc<(Mutex<bool>, Condvar)>,
    shutdown: Arc<AtomicBool>,
}

impl SimulationControls {
    /// Creates a new set of controls for a simulation thread.
    pub(crate) fn new(
        handle: std::thread::JoinHandle<()>,
        paused: Arc<(Mutex<bool>, Condvar)>,
        shutdown: Arc<AtomicBool>,
    ) -> Self {
        Self {
            handle: Some(handle),
            paused,
            shutdown,
        }
    }

    /// Pauses the simulation thread.
    pub(crate) fn pause(&self) {
        *self.paused.0.lock().unwrap() = true;
    }

    /// Resumes a paused simulation thread.
    pub(crate) fn resume(&self) {
        *self.paused.0.lock().unwrap() = false;
        self.paused.1.notify_one();
    }

    /// Requests shutdown of this simulation thread.
    pub(crate) fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
        *self.paused.0.lock().unwrap() = false;
        self.paused.1.notify_one();
    }

    /// Waits for the simulation thread to exit.
    pub(crate) fn join(&mut self) -> std::thread::Result<()> {
        self.handle
            .take()
            .map_or(Ok(()), std::thread::JoinHandle::join)
    }
}

impl Drop for SimulationControls {
    fn drop(&mut self) {
        self.shutdown();
        let _ = self.join();
    }
}

/// A pair of source and simulation inputs.
#[derive(Clone, Debug)]
pub struct SourceWithInputs<D, R, P, RE> {
    /// Spatial audio source.
    pub source: Source<D, R, P, RE>,

    /// Simulation inputs for the associated source.
    pub simulation_inputs: SimulationInputs<D, R, P>,
}

/// A pool of source list buffers, shared across simulation threads.
pub type SourcesPool<SourceId, D, R, P, RE> =
    Arc<Pool<Vec<(SourceId, SourceWithInputs<D, R, P, RE>)>>>;

/// A shared, atomically-swappable source list.
pub type SharedSources<SourceId, D, R, P, RE> =
    Arc<ArcSwap<ReusableOwned<Vec<(SourceId, SourceWithInputs<D, R, P, RE>)>>>>;

/// Simulation output shared between a simulation thread and the audio thread.
pub struct SharedSimulationOutput<T: 'static + Send + Sync>(
    pub(crate) Arc<ArcSwap<ReusableOwned<T>>>,
);

impl<T: 'static + Send + Sync> SharedSimulationOutput<T> {
    /// Returns a snapshot of the latest simulation output.
    ///
    /// Safe to call from the audio thread concurrently with simulation writes.
    pub fn load(&self) -> arc_swap::Guard<Arc<ReusableOwned<T>>> {
        self.0.load()
    }
}

impl<T: 'static + Send + Sync> Clone for SharedSimulationOutput<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;

    #[test]
    fn test_new() {
        let context = Context::default();
        let audio_settings = AudioSettings::default();
        let simulation_settings = SimulationSettings::new(&audio_settings)
            .with_direct(DirectSimulationSettings {
                max_num_occlusion_samples: 4,
            })
            .with_reflections(ConvolutionSettings {
                max_num_rays: 128,
                num_diffuse_samples: 8,
                max_duration: 0.5,
                max_num_sources: 4,
                num_threads: 1,
                max_order: 1,
            });
        let simulator = Simulator::try_new(&context, &simulation_settings).unwrap();
        let _simulation = Simulation::new::<()>(simulator);
    }

    #[test]
    fn test_update_clears_buffer_between_calls() {
        let context = Context::default();
        let audio_settings = AudioSettings::default();
        let simulation_settings = SimulationSettings::new(&audio_settings)
            .with_direct(DirectSimulationSettings {
                max_num_occlusion_samples: 4,
            })
            .with_reflections(ConvolutionSettings {
                max_num_rays: 128,
                num_diffuse_samples: 8,
                max_duration: 0.5,
                max_num_sources: 4,
                num_threads: 1,
                max_order: 1,
            });
        let simulator = Simulator::try_new(&context, &simulation_settings).unwrap();
        let simulator_clone = simulator.clone();
        let simulation = Simulation::new::<()>(simulator);

        let source =
            Source::<Direct, Reflections, (), Convolution>::try_new(&simulator_clone).unwrap();

        simulation.update_sources(|sources| {
            sources.push((
                (),
                SourceWithInputs {
                    source: source.clone(),
                    simulation_inputs: SimulationInputs {
                        source: CoordinateSystem::default(),
                        parameters: SimulationParameters::new()
                            .with_direct(DirectSimulationParameters::new())
                            .with_reflections(ConvolutionParameters {
                                baked_data_identifier: None,
                            }),
                    },
                },
            ));
            assert_eq!(sources.len(), 1);
        });

        simulation.update_sources(|sources| {
            assert!(
                sources.is_empty(),
                "update should receive an empty buffer each call"
            );
        });
    }

    #[test]
    fn test_shutdown() {
        let context = Context::default();
        let audio_settings = AudioSettings::default();
        let simulation_settings = SimulationSettings::new(&audio_settings)
            .with_direct(DirectSimulationSettings {
                max_num_occlusion_samples: 4,
            })
            .with_reflections(ConvolutionSettings {
                max_num_rays: 128,
                num_diffuse_samples: 8,
                max_duration: 0.5,
                max_num_sources: 4,
                num_threads: 1,
                max_order: 1,
            });
        let simulator = Simulator::try_new(&context, &simulation_settings).unwrap();
        let mut simulation = Simulation::new::<()>(simulator);
        let mut direct_simulation = simulation.spawn_direct(|error| {
            eprintln!("{error}");
        });
        simulation.shutdown();
        direct_simulation
            .join()
            .expect("simulation thread panicked");
    }
}
