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

pub struct Simulation<T, D, R, P, RE>
where
    T: RayTracer,
{
    simulator: Simulator<T, D, R, P, RE>,
    sources_pool: Arc<Pool<Vec<SourceWithInputs<D, R, P, RE>>>>,
    sources: Arc<ArcSwap<ReusableOwned<Vec<SourceWithInputs<D, R, P, RE>>>>>,
    commit_needed: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
}

impl<T, D, R, P, RE> Simulation<T, D, R, P, RE>
where
    T: RayTracer,
{
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
    /// `f` receives a pooled `Vec` to populate.
    pub fn update<F>(&self, f: F)
    where
        F: FnOnce(&mut Vec<SourceWithInputs<D, R, P, RE>>),
    {
        let mut sources = self.sources_pool.pull_owned(Vec::default);
        sources.clear();
        f(&mut sources);
        self.sources.store(Arc::new(sources));
    }

    pub fn request_commit(&mut self) {
        self.commit_needed.store(true, Ordering::Relaxed);
    }

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

/// Simulation output shared between a simulation thread and the audio thread.
pub struct SharedSimulationOutput<T: 'static + Send + Sync>(pub Arc<ArcSwap<ReusableOwned<T>>>);
