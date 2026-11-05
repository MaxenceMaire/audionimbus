use super::pipeline::Simulation as BoundSimulation;
use super::{SharedSimulationOutput, SimulationControls};
#[cfg(feature = "dynamic")]
use crate::binding::Dynamic;
use crate::binding::Static;
use crate::effect::DirectEffectParams;
use crate::geometry::scene::Scene as BoundScene;
use crate::ray_tracing::RayTracer;
use crate::simulation::simulator::Simulator as BoundSimulator;
use crate::simulation::{
    Direct, PathingCompatible, ReflectionEffectCompatible, ReflectionsCompatible,
    SimulationFlagsProvider,
};
use crate::wiring::simulation::pipeline::StaticSimulation as Simulation;
use crate::wiring::{
    Allocate, DirectFrame, SimulationStep, SimulationStepError,
    binding::FrameBinding,
    runner::{
        direct::{
            DirectFrame as BoundDirectFrame, ResolvedDirectFrame as BoundResolvedDirectFrame,
        },
        worker::SimulationRunner as BoundSimulationRunner,
    },
    step::direct::{DirectBinding, DirectStep as BoundDirectStep},
};
use arc_swap::ArcSwap;
use object_pool::Pool;
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Condvar, Mutex};

impl<SourceId, T, R, P, RE> Simulation<SourceId, T, Direct, R, P, RE>
where
    SourceId: 'static + Send + Sync + Clone + Hash + Eq,
    T: 'static + RayTracer,
    R: 'static + Send + Sync + Clone + Default + ReflectionsCompatible<R> + SimulationFlagsProvider,
    P: 'static + Send + Sync + Clone + Default + PathingCompatible<P> + SimulationFlagsProvider,
    RE: 'static + Send + Sync + Clone + Default + ReflectionEffectCompatible<R, RE>,
    (): ReflectionsCompatible<R> + PathingCompatible<P>,
{
    /// Spawns a direct simulation thread.
    pub fn spawn_direct(
        &mut self,
        on_error: impl Fn(SimulationStepError) + Send + 'static,
    ) -> StaticDirectSimulation<SourceId, R, P, RE> {
        let frame = DirectFrame {
            sources: self.sources.clone(),
            shared_inputs: Default::default(),
        };

        self.spawn_direct_shared(frame, on_error)
    }
}

impl<SourceId, B> BoundSimulation<SourceId, B>
where
    SourceId: 'static + Send + Sync + Clone + Hash + Eq,
    B: DirectBinding + 'static,
    B::Sources: FrameBinding + 'static,
    BoundSimulator<B>: Send + Sync + Clone,
    BoundScene<B::Geometry>: Send + Sync,
    B::Error: Send + 'static,
{
    /// Starts this worker with the selected frame and simulation binding.
    fn spawn_direct_shared(
        &mut self,
        frame: BoundDirectFrame<SourceId, B::Sources>,
        on_error: impl Fn(B::Error) + Send + 'static,
    ) -> DirectSimulation<SourceId, B::Sources>
    where
        BoundDirectFrame<SourceId, B::Sources>: Send + Sync,
        for<'a> BoundDirectStep<SourceId, B>: SimulationStep<
                BoundResolvedDirectFrame<'a, SourceId, B::Sources>,
                Output = HashMap<SourceId, DirectEffectParams>,
                Error = B::Error,
            >,
        for<'a> HashMap<SourceId, DirectEffectParams>:
            Allocate<BoundResolvedDirectFrame<'a, SourceId, B::Sources>>,
    {
        let input = Arc::new(ArcSwap::new(Arc::new(frame)));

        let output = SharedSimulationOutput::<HashMap<SourceId, DirectEffectParams>>(Arc::new(
            ArcSwap::new(Arc::new(
                Arc::new(Pool::new(1, HashMap::default)).pull_owned(HashMap::default),
            )),
        ));

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
            BoundDirectStep::<SourceId, B>::from(self.simulator.clone()),
            on_error,
        );

        DirectSimulation {
            input,
            output,
            controls: SimulationControls::new(handle, paused, shutdown),
        }
    }
}

/// A running direct simulation thread.
///
/// Dropping this handle requests shutdown and joins the thread.
pub type StaticDirectSimulation<SourceId, R, P, RE> =
    DirectSimulation<SourceId, Static<(Direct, R, P, RE)>>;

/// A running direct simulation worker.
#[cfg(feature = "dynamic")]
pub type DynamicDirectSimulation<SourceId> = DirectSimulation<SourceId, Dynamic<()>>;

/// A running worker with binding-specific input and output storage.
pub struct DirectSimulation<SourceId, B: FrameBinding>
where
    SourceId: 'static + Send + Sync,
{
    /// Shared input frame, updated each game frame.
    input: Arc<ArcSwap<BoundDirectFrame<SourceId, B>>>,
    /// Shared output, read by the audio thread.
    output: SharedSimulationOutput<HashMap<SourceId, DirectEffectParams>>,
    /// Thread lifecycle controls.
    controls: SimulationControls,
}

impl<SourceId, B: FrameBinding> DirectSimulation<SourceId, B>
where
    SourceId: 'static + Send + Sync,
{
    /// Returns the shared output produced by this simulation thread.
    pub fn output(&self) -> SharedSimulationOutput<HashMap<SourceId, DirectEffectParams>> {
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

impl<SourceId, R, P, RE> StaticDirectSimulation<SourceId, R, P, RE>
where
    SourceId: 'static + Send + Sync,
    RE: ReflectionEffectCompatible<R, RE>,
{
    /// Updates the input frame used on the next run.
    pub fn set_input(&self, frame: DirectFrame<SourceId, Direct, R, P, RE>) {
        self.input.store(Arc::new(frame));
    }
}

#[cfg(feature = "dynamic")]
impl<SourceId: 'static + Send + Sync + Clone + Hash + Eq> crate::dynamic::Simulation<SourceId> {
    /// Spawns a direct simulation thread.
    ///
    /// # Errors
    ///
    /// - [`crate::dynamic::SimulationError::DisabledModes`] if the mode is unavailable.
    pub fn spawn_direct(
        &mut self,
        on_error: impl Fn(crate::dynamic::SimulationError) + Send + 'static,
    ) -> Result<crate::dynamic::DirectSimulation<SourceId>, crate::dynamic::SimulationError> {
        let disabled = crate::SimulationFlags::DIRECT & !self.simulator.flags();
        if !disabled.is_empty() {
            return Err(crate::dynamic::SimulationError::DisabledModes(disabled));
        }
        let frame = BoundDirectFrame::<SourceId, Dynamic<()>> {
            sources: self.sources.clone(),
            shared_inputs: Default::default(),
        };

        Ok(self.spawn_direct_shared(frame, on_error))
    }
}

#[cfg(feature = "dynamic")]
impl<SourceId: 'static + Send + Sync> DynamicDirectSimulation<SourceId> {
    /// Updates shared inputs for the next worker iteration.
    pub fn set_input(&self, shared_inputs: crate::dynamic::SimulationSharedInputs) {
        let frame = BoundDirectFrame::<SourceId, Dynamic<()>> {
            sources: self.input.load().sources.clone(),
            shared_inputs,
        };
        self.input.store(Arc::new(frame));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulation::settings::StaticSimulationSettings as SimulationSettings;
    use crate::simulation::simulator::StaticSimulator as Simulator;
    use crate::*;

    #[test]
    fn test_spawn_and_shutdown() {
        let context = Context::default();
        let audio_settings = AudioSettings::default();
        let simulation_settings = SimulationSettings::new(&audio_settings)
            .with_direct(DirectSimulationSettings {
                max_num_occlusion_samples: 4,
            })
            .with_reflections(ConvolutionSettings {
                max_num_rays: 128,
                num_diffuse_samples: 8,
                max_duration: 0.5,
                max_num_sources: 4,
                num_threads: 1,
                max_order: 1,
            });
        let simulator = Simulator::try_new(&context, &simulation_settings).unwrap();
        let mut simulation = Simulation::new::<()>(simulator);
        let mut direct_simulation = simulation.spawn_direct(|error| {
            eprintln!("{error}");
        });
        simulation.shutdown();
        direct_simulation
            .join()
            .expect("simulation thread panicked");
    }

    #[test]
    fn test_initial_output_is_empty() {
        let context = Context::default();
        let audio_settings = AudioSettings::default();
        let simulation_settings = SimulationSettings::new(&audio_settings)
            .with_direct(DirectSimulationSettings {
                max_num_occlusion_samples: 4,
            })
            .with_reflections(ConvolutionSettings {
                max_num_rays: 128,
                num_diffuse_samples: 8,
                max_duration: 0.5,
                max_num_sources: 4,
                num_threads: 1,
                max_order: 1,
            });
        let simulator = Simulator::try_new(&context, &simulation_settings).unwrap();
        let mut simulation = Simulation::new::<()>(simulator);
        let mut direct_simulation = simulation.spawn_direct(|error| {
            eprintln!("{error}");
        });

        assert!(direct_simulation.output().load().is_empty());

        simulation.shutdown();
        direct_simulation
            .join()
            .expect("simulation thread panicked");
    }
}
