use super::pipeline::Simulation as BoundSimulation;
use super::{SharedSimulationOutput, SimulationControls};
#[cfg(feature = "dynamic")]
use crate::binding::Dynamic;
use crate::binding::Static;
use crate::effect::ReflectionEffectType;
#[cfg(feature = "dynamic")]
use crate::effect::reflections::binding::ReflectionAlgorithm;
use crate::effect::reflections::binding::ReflectionBinding;
use crate::geometry::scene::Scene as BoundScene;
use crate::ray_tracing::RayTracer;
use crate::simulation::simulator::Simulator as BoundSimulator;
use crate::simulation::{
    DirectCompatible, PathingCompatible, ReflectionEffectCompatible, Reflections,
    SimulationFlagsProvider,
};
use crate::wiring::simulation::pipeline::{
    StaticSimulation as Simulation, StaticSourceWithInputs as SourceWithInputs,
};
use crate::wiring::{
    Allocate, ReflectionsReverbFrame, SimulationStep, SimulationStepError,
    binding::{FrameBinding, InputBinding},
    runner::{
        reflections_reverb::{
            ReflectionsReverbFrame as BoundReflectionsReverbFrame,
            ResolvedReflectionsReverbFrame as BoundResolvedReflectionsReverbFrame,
        },
        worker::SimulationRunner as BoundSimulationRunner,
    },
    step::reflections::ReflectionsBinding,
    step::reflections_reverb::{
        ReflectionsReverbOutput as BoundReflectionsReverbOutput,
        ReflectionsReverbStep as BoundReflectionsReverbStep,
    },
};
use arc_swap::ArcSwap;
use object_pool::Pool;
use std::hash::Hash;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Condvar, Mutex};

impl<SourceId, T, D, P, RE> Simulation<SourceId, T, D, Reflections, P, RE>
where
    SourceId: 'static + Send + Sync + Clone + Hash + Eq,
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
{
    /// Spawns a reflections and reverb simulation thread.
    pub fn spawn_reflections_reverb<LD, LP>(
        &mut self,
        on_error: impl Fn(SimulationStepError) + Send + 'static,
    ) -> StaticReflectionsReverbSimulation<SourceId, D, P, RE, LD, LP>
    where
        LD: 'static + Send + Sync + DirectCompatible<LD> + SimulationFlagsProvider,
        LP: 'static + Send + Sync + PathingCompatible<LP> + SimulationFlagsProvider,
        (): DirectCompatible<LD> + PathingCompatible<LP>,
    {
        let frame = ReflectionsReverbFrame {
            sources: self.sources.clone(),
            listener: None::<SourceWithInputs<LD, Reflections, LP, RE>>,
            shared_inputs: Default::default(),
        };

        self.spawn_reflections_reverb_shared(frame, on_error)
    }
}

impl<SourceId, B> BoundSimulation<SourceId, B>
where
    SourceId: 'static + Send + Sync + Clone + Hash + Eq,
    B: ReflectionsBinding + 'static,
    B::Sources: FrameBinding + 'static,
    BoundSimulator<B>: Send + Sync + Clone,
    BoundScene<B::Geometry>: Send + Sync,
    B::Error: Send + 'static,
{
    /// Starts this worker with the selected frame and simulation binding.
    fn spawn_reflections_reverb_shared<L: InputBinding + 'static>(
        &mut self,
        frame: BoundReflectionsReverbFrame<SourceId, B::Sources, L>,
        on_error: impl Fn(B::Error) + Send + 'static,
    ) -> ReflectionsReverbSimulation<SourceId, B::Sources, B::Reflection, L>
    where
        BoundReflectionsReverbFrame<SourceId, B::Sources, L>: Send + Sync,
        for<'a> BoundReflectionsReverbStep<SourceId, B>: SimulationStep<
                BoundResolvedReflectionsReverbFrame<'a, SourceId, B::Sources, L>,
                Output = BoundReflectionsReverbOutput<SourceId, B::Reflection>,
                Error = B::Error,
            >,
        for<'a> BoundReflectionsReverbOutput<SourceId, B::Reflection>:
            Allocate<BoundResolvedReflectionsReverbFrame<'a, SourceId, B::Sources, L>>,
    {
        let input = Arc::new(ArcSwap::new(Arc::new(frame)));

        let output = SharedSimulationOutput(Arc::new(ArcSwap::new(Arc::new(
            Arc::new(Pool::new(
                1,
                BoundReflectionsReverbOutput::<SourceId, B::Reflection>::default,
            ))
            .pull_owned(BoundReflectionsReverbOutput::<SourceId, B::Reflection>::default),
        ))));

        let paused = Arc::new((Mutex::new(false), Condvar::new()));
        let shutdown = Arc::new(AtomicBool::new(false));
        self.shutdowns.push(shutdown.clone());
        self.paused.push(paused.clone());

        let simulator_for_commit = self.simulator.clone();
        let handle = BoundSimulationRunner::new(
            input.clone(),
            output.0.clone(),
            self.simulator_commit_needed.clone(),
            move || simulator_for_commit.commit(),
            self.pending_scene_commits.clone(),
            shutdown.clone(),
            paused.clone(),
        )
        .spawn(
            BoundReflectionsReverbStep::<SourceId, B>::from(self.simulator.clone()),
            on_error,
        );

        ReflectionsReverbSimulation {
            input,
            output,
            controls: SimulationControls::new(handle, paused, shutdown),
        }
    }
}

/// A running reflections and reverb simulation thread.
///
/// Dropping this handle requests shutdown and joins the thread.
pub type StaticReflectionsReverbSimulation<SourceId, D, P, RE, LD = D, LP = P> =
    ReflectionsReverbSimulation<
        SourceId,
        Static<(D, Reflections, P, RE)>,
        Static<RE>,
        Static<(LD, Reflections, LP, RE)>,
    >;

/// A running reflection and listener reverb worker.
#[cfg(feature = "dynamic")]
pub type DynamicReflectionsReverbSimulation<SourceId> =
    ReflectionsReverbSimulation<SourceId, Dynamic<()>, Dynamic<ReflectionAlgorithm>>;

/// A running worker with binding-specific input and output storage.
pub struct ReflectionsReverbSimulation<
    SourceId,
    B: FrameBinding,
    E: ReflectionBinding + 'static,
    L: InputBinding = B,
> where
    SourceId: 'static + Send + Sync + Hash + Eq,
{
    /// Shared input frame, updated each game frame.
    input: Arc<ArcSwap<BoundReflectionsReverbFrame<SourceId, B, L>>>,
    /// Shared output, read by the audio thread.
    output: SharedSimulationOutput<BoundReflectionsReverbOutput<SourceId, E>>,
    /// Thread lifecycle controls.
    controls: SimulationControls,
}

impl<SourceId, B: FrameBinding, E: ReflectionBinding + 'static, L: InputBinding>
    ReflectionsReverbSimulation<SourceId, B, E, L>
where
    SourceId: 'static + Send + Sync + Hash + Eq,
{
    /// Returns the shared output produced by this simulation thread.
    pub fn output(&self) -> SharedSimulationOutput<BoundReflectionsReverbOutput<SourceId, E>> {
        self.output.clone()
    }

    /// Pauses the simulation thread after its current iteration completes.
    pub fn pause(&self) {
        self.controls.pause();
    }

    /// Resumes a paused simulation thread.
    pub fn resume(&self) {
        self.controls.resume();
    }

    /// Requests shutdown of this simulation thread.
    pub fn shutdown(&self) {
        self.controls.shutdown();
    }

    /// Waits for the simulation thread to exit.
    pub fn join(&mut self) -> std::thread::Result<()> {
        self.controls.join()
    }
}

impl<SourceId, D, P, RE, LD, LP> StaticReflectionsReverbSimulation<SourceId, D, P, RE, LD, LP>
where
    SourceId: 'static + Send + Sync + Hash + Eq,
    RE: ReflectionEffectCompatible<Reflections, RE> + ReflectionEffectType,
{
    /// Updates the input frame used on the next run.
    pub fn set_input(
        &self,
        frame: ReflectionsReverbFrame<SourceId, D, Reflections, P, RE, LD, LP>,
    ) {
        self.input.store(Arc::new(frame));
    }
}

#[cfg(feature = "dynamic")]
impl<SourceId: 'static + Send + Sync + Clone + Hash + Eq> crate::dynamic::Simulation<SourceId> {
    /// Spawns a reflections reverb simulation thread.
    /// Reports missing reflection inputs until
    /// [`crate::dynamic::ReflectionsReverbSimulation::set_input`] supplies them.
    ///
    /// # Errors
    ///
    /// - [`crate::dynamic::SimulationError::DisabledModes`] if the mode is unavailable.
    pub fn spawn_reflections_reverb(
        &mut self,
        on_error: impl Fn(crate::dynamic::SimulationError) + Send + 'static,
    ) -> Result<
        crate::dynamic::ReflectionsReverbSimulation<SourceId>,
        crate::dynamic::SimulationError,
    > {
        let disabled = crate::SimulationFlags::REFLECTIONS & !self.simulator.flags();
        if !disabled.is_empty() {
            return Err(crate::dynamic::SimulationError::DisabledModes(disabled));
        }
        let frame = BoundReflectionsReverbFrame::<SourceId, Dynamic<()>> {
            sources: self.sources.clone(),
            listener: None,
            shared_inputs: Default::default(),
        };

        Ok(self.spawn_reflections_reverb_shared(frame, on_error))
    }
}

#[cfg(feature = "dynamic")]
impl<SourceId: 'static + Send + Sync + Hash + Eq>
    crate::dynamic::ReflectionsReverbSimulation<SourceId>
{
    /// Updates shared inputs for the next worker iteration.
    pub fn set_input(&self, shared_inputs: crate::dynamic::SimulationSharedInputs) {
        let frame = BoundReflectionsReverbFrame::<SourceId, Dynamic<()>> {
            sources: self.input.load().sources.clone(),
            listener: None,
            shared_inputs,
        };
        self.input.store(Arc::new(frame));
    }
}
