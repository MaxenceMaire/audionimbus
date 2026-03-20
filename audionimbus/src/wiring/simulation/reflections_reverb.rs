use super::super::{
    Allocate, ReflectionsReverbFrame, ReflectionsReverbOutput, ReflectionsReverbStep,
    SimulationRunner,
};
use super::{SharedSimulationOutput, Simulation, SourceWithInputs};
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
    ReflectionsReverbOutput<RE>: Allocate<ReflectionsReverbFrame<D, Reflections, P, RE>>,
{
    /// Spawns a reflections and reverb simulation thread.
    ///
    /// `listener` is the source placed at the listener's position, used for listener-centic reverb
    /// simulation.
    pub fn spawn_reflections_reverb(
        &self,
        listener: SourceWithInputs<(), Reflections, (), RE>,
    ) -> ReflectionsReverbSimulation<D, P, RE> {
        let input = Arc::new(ArcSwap::new(Arc::new(ReflectionsReverbFrame {
            sources: self.initial_sources.clone(),
            listener,
            shared_inputs: Default::default(),
        })));

        let output = SharedSimulationOutput(Arc::new(ArcSwap::new(Arc::new(
            Arc::new(Pool::new(1, ReflectionsReverbOutput::default))
                .pull_owned(ReflectionsReverbOutput::default),
        ))));

        let mut simulator_for_commit = self.simulator.clone();
        let handle = SimulationRunner::new(
            input.clone(),
            output.0.clone(),
            self.commit_needed.clone(),
            move || simulator_for_commit.commit(),
        )
        .spawn(ReflectionsReverbStep {
            simulator: self.simulator.clone(),
        });

        ReflectionsReverbSimulation {
            handle,
            input,
            output,
        }
    }
}

/// Running reflections and reverb simulation thread.
pub struct ReflectionsReverbSimulation<D, P, RE>
where
    RE: 'static + ReflectionEffectCompatible<Reflections, RE> + ReflectionEffectType,
{
    /// Thread handle.
    pub handle: std::thread::JoinHandle<()>,
    /// Shared input frame, updated each game frame.
    pub input: Arc<ArcSwap<ReflectionsReverbFrame<D, Reflections, P, RE>>>,
    /// Shared output, read by the audio thread.
    pub output: SharedSimulationOutput<ReflectionsReverbOutput<RE>>,
}
