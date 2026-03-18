use super::super::{
    Allocate, ReflectionsFrame, ReflectionsOutput, ReflectionsStep, SimulationRunner,
};
use super::{SharedSimulationOutput, Simulation};
use crate::effect::ReflectionEffectType;
use crate::ray_tracing::RayTracer;
use crate::simulation::{
    DirectCompatible, PathingCompatible, ReflectionEffectCompatible, Reflections,
    SimulationFlagsProvider,
};
use arc_swap::ArcSwap;
use object_pool::Pool;
use std::sync::Arc;

impl<T, D, P, RE> Simulation<T, D, Reflections, P, RE>
where
    T: 'static + RayTracer,
    D: 'static + Send + Sync + Clone + Default + DirectCompatible<D> + SimulationFlagsProvider,
    P: 'static + Send + Sync + Clone + Default + PathingCompatible<P> + SimulationFlagsProvider,
    RE: 'static
        + Send
        + Sync
        + Clone
        + Default
        + ReflectionEffectCompatible<Reflections, RE>
        + ReflectionEffectType,
    (): DirectCompatible<D> + PathingCompatible<P>,
    ReflectionsOutput<RE>: Allocate<ReflectionsFrame<D, Reflections, P, RE>>,
{
    pub fn spawn_reflections(&self) -> ReflectionsSimulation<D, P, RE> {
        let input = Arc::new(ArcSwap::new(Arc::new(ReflectionsFrame {
            sources: self.initial_sources.clone(),
            shared_inputs: Default::default(),
        })));

        let output = SharedSimulationOutput(Arc::new(ArcSwap::new(Arc::new(
            Arc::new(Pool::new(1, ReflectionsOutput::default))
                .pull_owned(ReflectionsOutput::default),
        ))));

        let mut simulator_for_commit = self.simulator.clone();
        let handle = SimulationRunner::new(
            input.clone(),
            output.0.clone(),
            self.commit_needed.clone(),
            move || simulator_for_commit.commit(),
        )
        .spawn(ReflectionsStep {
            simulator: self.simulator.clone(),
        });

        ReflectionsSimulation {
            handle,
            input,
            output,
        }
    }
}

/// Running reflections simulation thread.
pub struct ReflectionsSimulation<D, P, RE>
where
    RE: 'static + ReflectionEffectCompatible<Reflections, RE> + ReflectionEffectType,
{
    /// Thread handle.
    pub handle: std::thread::JoinHandle<()>,
    /// Shared input frame, updated each game frame.
    pub input: Arc<ArcSwap<ReflectionsFrame<D, Reflections, P, RE>>>,
    /// Shared output, read by the audio thread.
    pub output: SharedSimulationOutput<ReflectionsOutput<RE>>,
}
