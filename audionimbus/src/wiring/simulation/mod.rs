//! High-level simulation pipeline built on top of the [runner layer](super::runner).
//!
//! The entry point is [`Simulation`], which manages a shared source buffer and spawns typed
//! simulation threads.
//!
//! For lower-level control, use [`SimulationRunner`] directly.

#[cfg(doc)]
use super::runner::SimulationRunner;

use crate::ray_tracing::RayTracer;
use crate::simulation::{SimulationInputs, Simulator, Source};
use arc_swap::ArcSwap;
use object_pool::{Pool, ReusableOwned};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Condvar, Mutex,
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
/// [`Self::update`].
///
/// Uses memory pooling to avoid per-frame allocation.
pub struct Simulation<SourceId, T, D, R, P, RE>
where
    T: RayTracer,
{
    /// The underlying simulator shared across all spawned simulation threads.
    pub simulator: Simulator<T, D, R, P, RE>,
    /// Pool of source buffers, reused across frames to avoid allocation.
    pub sources_pool: SourcesPool<SourceId, D, R, P, RE>,
    /// The current source buffer, atomically swapped each frame via [`Self::update`].
    pub sources: SharedSources<SourceId, D, R, P, RE>,
    /// Set to `true` via [`Self::request_commit`] to trigger a simulator commit on the next
    /// simulation run.
    pub commit_needed: Arc<AtomicBool>,
    /// Set to `true` via [`Self::shutdown`] to stop all spawned simulation threads after their
    /// current iteration.
    pub shutdown: Arc<AtomicBool>,
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
    pub fn new<SourceId>(
        simulator: Simulator<T, D, R, P, RE>,
    ) -> Simulation<SourceId, T, D, R, P, RE> {
        let sources_pool = Arc::new(Pool::new(4, Default::default));
        let sources = Arc::new(ArcSwap::new(Arc::new(
            sources_pool.pull_owned(Default::default),
        )));
        let commit_needed = Arc::new(AtomicBool::new(false));
        let shutdown = Arc::new(AtomicBool::new(false));
        let paused = vec![];

        Simulation {
            simulator,
            sources_pool,
            sources,
            commit_needed,
            shutdown,
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
    /// `f` receives a pooled `Vec` to be populated.
    pub fn update<F>(&self, f: F)
    where
        F: FnOnce(&mut Vec<SourceWithInputs<SourceId, D, R, P, RE>>),
    {
        let mut sources = self.sources_pool.pull_owned(Vec::default);
        sources.clear();
        f(&mut sources);
        self.sources.store(Arc::new(sources));
    }

    /// Signals all spawned simulation threads to commit on their next run.
    pub fn request_commit(&mut self) {
        self.commit_needed.store(true, Ordering::Relaxed);
    }

    /// Signals all spawned simulation threads to stop.
    pub fn shutdown(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);

        // Wake paused threads so they can observe the shutdown flag.
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

/// A pair of source and simulation inputs.
#[derive(Clone, Debug)]
pub struct SourceWithInputs<SourceId, D, R, P, RE> {
    /// Source identifier.
    pub id: SourceId,
    /// Spatial audio source.
    pub source: Source<D, R, P, RE>,
    /// Simulation inputs for the associated source.
    pub simulation_inputs: SimulationInputs<D, R, P>,
}

/// A pool of source list buffers, shared across simulation threads.
pub type SourcesPool<SourceId, D, R, P, RE> =
    Arc<Pool<Vec<SourceWithInputs<SourceId, D, R, P, RE>>>>;

/// A shared, atomically-swappable source list.
pub type SharedSources<SourceId, D, R, P, RE> =
    Arc<ArcSwap<ReusableOwned<Vec<SourceWithInputs<SourceId, D, R, P, RE>>>>>;

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

        simulation.update(|sources| {
            sources.push(SourceWithInputs {
                id: (),
                source: source.clone(),
                simulation_inputs: SimulationInputs::new(CoordinateSystem::default())
                    .with_direct(DirectSimulationParameters::new())
                    .with_reflections(ConvolutionParameters {
                        baked_data_identifier: None,
                    }),
            });
            assert_eq!(sources.len(), 1);
        });

        simulation.update(|sources| {
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
        let direct_simulation = simulation.spawn_direct();
        simulation.shutdown();
        direct_simulation
            .handle
            .join()
            .expect("simulation thread panicked");
    }
}
