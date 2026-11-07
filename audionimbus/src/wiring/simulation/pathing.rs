use super::pipeline::Simulation as BoundSimulation;
use super::{SharedSimulationOutput, SimulationControls};
#[cfg(feature = "dynamic")]
use crate::binding::Dynamic;
use crate::binding::Static;
use crate::effect::PathEffectParams;
use crate::geometry::scene::Scene as BoundScene;
use crate::ray_tracing::RayTracer;
use crate::simulation::simulator::Simulator as BoundSimulator;
use crate::simulation::{
    DirectCompatible, Pathing, ReflectionEffectCompatible, ReflectionsCompatible,
    SimulationFlagsProvider,
};
use crate::wiring::simulation::pipeline::StaticSimulation as Simulation;
use crate::wiring::{
    Allocate, PathingFrame, SimulationStep, SimulationStepError,
    binding::FrameBinding,
    runner::{
        pathing::{
            PathingFrame as BoundPathingFrame, ResolvedPathingFrame as BoundResolvedPathingFrame,
        },
        worker::SimulationRunner as BoundSimulationRunner,
    },
    step::pathing::{PathingBinding, PathingStep as BoundPathingStep},
};
use arc_swap::ArcSwap;
use object_pool::Pool;
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Condvar, Mutex};

impl<SourceId, T, D, R, RE> Simulation<SourceId, T, D, R, Pathing, RE>
where
    SourceId: 'static + Send + Sync + Clone + Hash + Eq,
    T: 'static + RayTracer,
    D: 'static + Send + Sync + Clone + Default + DirectCompatible<D> + SimulationFlagsProvider,
    R: 'static + Send + Sync + Clone + Default + ReflectionsCompatible<R> + SimulationFlagsProvider,
    RE: 'static + Send + Sync + Clone + Default + ReflectionEffectCompatible<R, RE>,
    (): DirectCompatible<D> + ReflectionsCompatible<R>,
{
    /// Spawns a pathing simulation thread.
    pub fn spawn_pathing(
        &mut self,
        on_error: impl Fn(SimulationStepError) + Send + 'static,
    ) -> StaticPathingSimulation<SourceId, D, R, RE> {
        let frame = PathingFrame {
            sources: self.sources.clone(),
            shared_inputs: Default::default(),
        };

        self.spawn_pathing_shared(frame, on_error)
    }
}

impl<SourceId, B> BoundSimulation<SourceId, B>
where
    SourceId: 'static + Send + Sync + Clone + Hash + Eq,
    B: PathingBinding + 'static,
    B::Sources: FrameBinding + 'static,
    BoundSimulator<B>: Send + Sync + Clone,
    BoundScene<B::Geometry>: Send + Sync,
    B::Error: Send + 'static,
{
    /// Starts this worker with the selected frame and simulation binding.
    fn spawn_pathing_shared(
        &mut self,
        frame: BoundPathingFrame<SourceId, B::Sources>,
        on_error: impl Fn(B::Error) + Send + 'static,
    ) -> PathingSimulation<SourceId, B::Sources>
    where
        BoundPathingFrame<SourceId, B::Sources>: Send + Sync,
        for<'a> BoundPathingStep<SourceId, B>: SimulationStep<
                BoundResolvedPathingFrame<'a, SourceId, B::Sources>,
                Output = HashMap<SourceId, crate::PathEffectParams>,
                Error = B::Error,
            >,
        for<'a> HashMap<SourceId, crate::PathEffectParams>:
            Allocate<BoundResolvedPathingFrame<'a, SourceId, B::Sources>>,
    {
        let input = Arc::new(ArcSwap::new(Arc::new(frame)));

        let output = SharedSimulationOutput(Arc::new(ArcSwap::new(Arc::new(
            Arc::new(Pool::new(1, HashMap::default)).pull_owned(HashMap::default),
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
            BoundPathingStep::<SourceId, B>::from(self.simulator.clone()),
            on_error,
        );

        PathingSimulation {
            input,
            output,
            controls: SimulationControls::new(handle, paused, shutdown),
        }
    }
}

/// A running pathing simulation thread.
///
/// Dropping this handle requests shutdown and joins the thread.
pub type StaticPathingSimulation<SourceId, D, R, RE> =
    PathingSimulation<SourceId, Static<(D, R, Pathing, RE)>>;

/// A running pathing simulation worker.
#[cfg(feature = "dynamic")]
pub type DynamicPathingSimulation<SourceId> = PathingSimulation<SourceId, Dynamic<()>>;

/// A running worker with binding-specific input and output storage.
pub struct PathingSimulation<SourceId, B: FrameBinding>
where
    SourceId: 'static + Send + Sync,
{
    /// Shared input frame, updated each game frame.
    input: Arc<ArcSwap<BoundPathingFrame<SourceId, B>>>,
    /// Shared output, read by the audio thread.
    output: SharedSimulationOutput<HashMap<SourceId, PathEffectParams>>,
    /// Thread lifecycle controls.
    controls: SimulationControls,
}

impl<SourceId, B: FrameBinding> PathingSimulation<SourceId, B>
where
    SourceId: 'static + Send + Sync,
{
    /// Returns the shared output produced by this simulation thread.
    pub fn output(&self) -> SharedSimulationOutput<HashMap<SourceId, PathEffectParams>> {
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

impl<SourceId, D, R, RE> StaticPathingSimulation<SourceId, D, R, RE>
where
    SourceId: 'static + Send + Sync,
    RE: ReflectionEffectCompatible<R, RE>,
{
    /// Updates the input frame used on the simulation thread's next run.
    pub fn set_input(&self, frame: PathingFrame<SourceId, D, R, Pathing, RE>) {
        self.input.store(Arc::new(frame));
    }
}

#[cfg(feature = "dynamic")]
impl<SourceId: 'static + Send + Sync + Clone + Hash + Eq> crate::dynamic::Simulation<SourceId> {
    /// Spawns a pathing simulation thread.
    ///
    /// # Errors
    ///
    /// - [`crate::dynamic::SimulationError::DisabledModes`] if the mode is unavailable.
    pub fn spawn_pathing(
        &mut self,
        on_error: impl Fn(crate::dynamic::SimulationError) + Send + 'static,
    ) -> Result<crate::dynamic::PathingSimulation<SourceId>, crate::dynamic::SimulationError> {
        let disabled = crate::SimulationFlags::PATHING & !self.simulator.flags();
        if !disabled.is_empty() {
            return Err(crate::dynamic::SimulationError::DisabledModes(disabled));
        }
        let frame = BoundPathingFrame::<SourceId, Dynamic<()>> {
            sources: self.sources.clone(),
            shared_inputs: Default::default(),
        };

        Ok(self.spawn_pathing_shared(frame, on_error))
    }
}

#[cfg(feature = "dynamic")]
impl<SourceId: 'static + Send + Sync> DynamicPathingSimulation<SourceId> {
    /// Updates shared inputs for the next worker iteration.
    pub fn set_input(&self, shared_inputs: crate::dynamic::SimulationSharedInputs) {
        let frame = BoundPathingFrame::<SourceId, Dynamic<()>> {
            sources: self.input.load().sources.clone(),
            shared_inputs,
        };
        self.input.store(Arc::new(frame));
    }
}
