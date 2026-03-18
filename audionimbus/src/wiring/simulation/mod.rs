use crate::ray_tracing::RayTracer;
use crate::simulation::{SimulationInputs, Simulator, Source};
use arc_swap::ArcSwap;
use object_pool::{Pool, ReusableOwned};
use std::sync::{atomic::AtomicBool, Arc};

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
    initial_sources: Arc<ReusableOwned<Vec<SourceWithInputs<D, R, P, RE>>>>,
    commit_needed: Arc<AtomicBool>,
}

impl<T, D, R, P, RE> Simulation<T, D, R, P, RE>
where
    T: RayTracer,
{
    pub fn new(simulator: Simulator<T, D, R, P, RE>) -> Self {
        let sources_pool = Arc::new(Pool::new(4, Default::default));
        let initial_sources = Arc::new(sources_pool.pull_owned(Default::default));
        let commit_needed = Arc::new(AtomicBool::new(false));

        Self {
            simulator,
            sources_pool,
            initial_sources,
            commit_needed,
        }
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
