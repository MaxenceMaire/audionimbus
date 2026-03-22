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
    Arc,
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
pub struct Simulation<T, D, R, P, RE>
where
    T: RayTracer,
{
    simulator: Simulator<T, D, R, P, RE>,
    sources_pool: SourcesPool<D, R, P, RE>,
    sources: SharedSources<D, R, P, RE>,
    commit_needed: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
}

impl<T, D, R, P, RE> Simulation<T, D, R, P, RE>
where
    T: RayTracer,
{
    /// Creates a new simulation pipeline.
    pub fn new(simulator: Simulator<T, D, R, P, RE>) -> Self {
        let sources_pool = Arc::new(Pool::new(4, Default::default));
        let sources = Arc::new(ArcSwap::new(Arc::new(
            sources_pool.pull_owned(Default::default),
        )));
        let commit_needed = Arc::new(AtomicBool::new(false));
        let shutdown = Arc::new(AtomicBool::new(false));

        Self {
            simulator,
            sources_pool,
            sources,
            commit_needed,
            shutdown,
        }
    }

    /// Updates the sources used by all simulation threads on their next run.
    ///
    /// `f` receives a pooled `Vec` to be populated.
    pub fn update<F>(&self, f: F)
    where
        F: FnOnce(&mut Vec<SourceWithInputs<D, R, P, RE>>),
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
pub(crate) type SourcesPool<D, R, P, RE> = Arc<Pool<Vec<SourceWithInputs<D, R, P, RE>>>>;

/// A shared, atomically-swappable source list.
pub(crate) type SharedSources<D, R, P, RE> =
    Arc<ArcSwap<ReusableOwned<Vec<SourceWithInputs<D, R, P, RE>>>>>;

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
