use super::super::{Allocate, PathingFrame, PathingStep, SimulationRunner};
use super::{SharedSimulationOutput, Simulation};
use crate::effect::PathEffectParams;
use crate::ray_tracing::RayTracer;
use crate::simulation::{
    DirectCompatible, Pathing, ReflectionEffectCompatible, ReflectionsCompatible,
    SimulationFlagsProvider,
};
use arc_swap::ArcSwap;
use object_pool::Pool;
use std::sync::Arc;

impl<T, D, R, RE> Simulation<T, D, R, Pathing, RE>
where
    T: 'static + RayTracer,
    D: 'static + Send + Sync + Clone + Default + DirectCompatible<D> + SimulationFlagsProvider,
    R: 'static + Send + Sync + Clone + Default + ReflectionsCompatible<R> + SimulationFlagsProvider,
    RE: 'static + Send + Sync + Clone + Default + ReflectionEffectCompatible<R, RE>,
    (): DirectCompatible<D> + ReflectionsCompatible<R>,
    Vec<PathEffectParams>: Allocate<PathingFrame<D, R, Pathing, RE>>,
{
    /// Spawns a pathing simulation thread.
    pub fn spawn_pathing(&self) -> PathingSimulation<D, R, RE> {
        let input = Arc::new(ArcSwap::new(Arc::new(PathingFrame {
            sources: self.initial_sources.clone(),
            shared_inputs: Default::default(),
        })));

        let output = SharedSimulationOutput(Arc::new(ArcSwap::new(Arc::new(
            Arc::new(Pool::new(1, Vec::default)).pull_owned(Vec::default),
        ))));

        let mut simulator_for_commit = self.simulator.clone();
        let handle = SimulationRunner::new(
            input.clone(),
            output.0.clone(),
            self.commit_needed.clone(),
            move || simulator_for_commit.commit(),
            self.shutdown.clone(),
        )
        .spawn(PathingStep {
            simulator: self.simulator.clone(),
        });

        PathingSimulation {
            handle,
            input,
            output,
        }
    }
}

/// Running pathing simulation thread.
pub struct PathingSimulation<D, R, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    /// Thread handle.
    pub handle: std::thread::JoinHandle<()>,
    /// Shared input frame, updated each game frame.
    pub input: Arc<ArcSwap<PathingFrame<D, R, Pathing, RE>>>,
    /// Shared output, read by the audio thread.
    pub output: SharedSimulationOutput<Vec<PathEffectParams>>,
}
