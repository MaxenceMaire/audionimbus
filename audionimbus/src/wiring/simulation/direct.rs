use super::super::{DirectFrame, DirectStep, SimulationRunner};
use super::{SharedSimulationOutput, Simulation};
use crate::effect::{DirectEffectParams, ReflectionEffectType};
use crate::ray_tracing::RayTracer;
use crate::simulation::{
    Direct, PathingCompatible, ReflectionEffectCompatible, ReflectionsCompatible,
    SimulationFlagsProvider,
};
use arc_swap::ArcSwap;
use object_pool::Pool;
use std::sync::Arc;

impl<T, R, P, RE> Simulation<T, Direct, R, P, RE>
where
    T: 'static + RayTracer,
    R: 'static + Send + Sync + Clone + Default + ReflectionsCompatible<R> + SimulationFlagsProvider,
    P: 'static + Send + Sync + Clone + Default + PathingCompatible<P> + SimulationFlagsProvider,
    RE: 'static
        + Send
        + Sync
        + Clone
        + Default
        + ReflectionEffectCompatible<R, RE>
        + ReflectionEffectType,
    (): ReflectionsCompatible<R> + PathingCompatible<P>,
{
    /// Spawns a direct simulation thread.
    pub fn spawn_direct(&self) -> DirectSimulation<R, P, RE> {
        let input = Arc::new(ArcSwap::new(Arc::new(DirectFrame {
            sources: self.sources.clone(),
            shared_inputs: Default::default(),
        })));

        let output = SharedSimulationOutput::<Vec<DirectEffectParams>>(Arc::new(ArcSwap::new(
            Arc::new(Arc::new(Pool::new(1, Vec::default)).pull_owned(Vec::default)),
        )));

        let mut simulator_for_commit = self.simulator.clone();
        let handle = SimulationRunner::new(
            input.clone(),
            output.0.clone(),
            self.commit_needed.clone(),
            move || simulator_for_commit.commit(),
            self.shutdown.clone(),
        )
        .spawn(DirectStep {
            simulator: self.simulator.clone(),
        });

        DirectSimulation {
            handle,
            input,
            output,
        }
    }
}

/// A running direct simulation thread.
pub struct DirectSimulation<R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE> + ReflectionEffectType,
{
    /// Thread handle.
    pub handle: std::thread::JoinHandle<()>,
    /// Shared input frame, updated each game frame.
    pub input: Arc<ArcSwap<DirectFrame<Direct, R, P, RE>>>,
    /// Shared output, read by the audio thread.
    pub output: SharedSimulationOutput<Vec<DirectEffectParams>>,
}
