use super::{
    Allocate, DirectFrame, DirectStep, PathingFrame, PathingStep, ReflectionsFrame,
    ReflectionsOutput, ReflectionsReverbFrame, ReflectionsReverbOutput, ReflectionsReverbStep,
    ReflectionsStep, SimulationRunner,
};
use crate::effect::{DirectEffectParams, PathEffectParams, ReflectionEffectType};
use crate::ray_tracing::RayTracer;
use crate::simulation::{
    Direct, DirectCompatible, Pathing, PathingCompatible, ReflectionEffectCompatible, Reflections,
    ReflectionsCompatible, SimulationFlagsProvider, SimulationInputs, Simulator, Source,
};
use arc_swap::ArcSwap;
use object_pool::{Pool, ReusableOwned};
use std::sync::{atomic::AtomicBool, Arc};

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
    Vec<DirectEffectParams>: Allocate<DirectFrame<Direct, R, P, RE>>,
{
    pub fn spawn_direct(&self) -> DirectSimulation<R, P, RE> {
        let input = Arc::new(ArcSwap::new(Arc::new(DirectFrame {
            sources: self.initial_sources.clone(),
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

    /// Spawns a reflections and reverb simulation thread.
    ///
    /// `listener` is the source placed at the listener's position, used for listener-centic reverb
    /// simulation.
    pub fn spawn_reflections_reverb(
        &self,
        listener: SourceWithInputs<D, Reflections, P, RE>,
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

/// Running direct simulation thread.
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
