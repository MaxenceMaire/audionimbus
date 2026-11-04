use super::pipeline::Simulation as BoundSimulation;
use super::{SharedSimulationOutput, Simulation, SimulationControls};
#[cfg(feature = "dynamic")]
use crate::binding::Dynamic;
use crate::binding::Static;
use crate::effect::ReflectionEffectType;
use crate::effect::reflections::binding::ReflectionBinding;
use crate::geometry::scene::Scene as BoundScene;
use crate::ray_tracing::RayTracer;
use crate::simulation::simulator::Simulator as BoundSimulator;
use crate::simulation::{
    DirectCompatible, PathingCompatible, ReflectionEffectCompatible, Reflections,
    SimulationFlagsProvider,
};
use crate::wiring::{
    Allocate, ReflectionsFrame, SimulationStep, SimulationStepError,
    binding::FrameBinding,
    runner::{
        reflections::{
            ReflectionsFrame as BoundReflectionsFrame,
            ResolvedReflectionsFrame as BoundResolvedReflectionsFrame,
        },
        worker::SimulationRunner as BoundSimulationRunner,
    },
    step::reflections::{
        ReflectionsBinding, ReflectionsOutput as BoundReflectionsOutput,
        ReflectionsStep as BoundReflectionsStep,
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
    /// Spawns a reflections simulation thread.
    pub fn spawn_reflections(
        &mut self,
        on_error: impl Fn(SimulationStepError) + Send + 'static,
    ) -> TypedReflectionsSimulation<SourceId, D, P, RE> {
        let frame = ReflectionsFrame {
            sources: self.sources.clone(),
            shared_inputs: Default::default(),
        };

        self.spawn_reflections_shared(frame, on_error)
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
    fn spawn_reflections_shared(
        &mut self,
        frame: BoundReflectionsFrame<SourceId, B::Sources>,
        on_error: impl Fn(B::Error) + Send + 'static,
    ) -> ReflectionsSimulation<SourceId, B::Sources, B::Reflection>
    where
        BoundReflectionsFrame<SourceId, B::Sources>: Send + Sync,
        for<'a> BoundReflectionsStep<SourceId, B>: SimulationStep<
                BoundResolvedReflectionsFrame<'a, SourceId, B::Sources>,
                Output = BoundReflectionsOutput<SourceId, B::Reflection>,
                Error = B::Error,
            >,
        for<'a> BoundReflectionsOutput<SourceId, B::Reflection>:
            Allocate<BoundResolvedReflectionsFrame<'a, SourceId, B::Sources>>,
    {
        let input = Arc::new(ArcSwap::new(Arc::new(frame)));

        let output = SharedSimulationOutput(Arc::new(ArcSwap::new(Arc::new(
            Arc::new(Pool::new(
                1,
                BoundReflectionsOutput::<SourceId, B::Reflection>::default,
            ))
            .pull_owned(BoundReflectionsOutput::<SourceId, B::Reflection>::default),
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
            BoundReflectionsStep::<SourceId, B>::from(self.simulator.clone()),
            on_error,
        );

        ReflectionsSimulation {
            input,
            output,
            controls: SimulationControls::new(handle, paused, shutdown),
        }
    }
}

/// A running reflections simulation thread.
///
/// Dropping this handle requests shutdown and joins the thread.
pub type TypedReflectionsSimulation<SourceId, D, P, RE> =
    ReflectionsSimulation<SourceId, Static<(D, Reflections, P, RE)>, Static<RE>>;

/// A running worker with binding-specific input and output storage.
pub struct ReflectionsSimulation<SourceId, B: FrameBinding, E: ReflectionBinding + 'static>
where
    SourceId: 'static + Send + Sync,
{
    /// Shared input frame, updated each game frame.
    input: Arc<ArcSwap<BoundReflectionsFrame<SourceId, B>>>,
    /// Shared output, read by the audio thread.
    output: SharedSimulationOutput<BoundReflectionsOutput<SourceId, E>>,
    /// Thread lifecycle controls.
    controls: SimulationControls,
}

impl<SourceId, B: FrameBinding, E: ReflectionBinding + 'static>
    ReflectionsSimulation<SourceId, B, E>
where
    SourceId: 'static + Send + Sync,
{
    /// Returns the shared output produced by this simulation thread.
    pub fn output(&self) -> SharedSimulationOutput<BoundReflectionsOutput<SourceId, E>> {
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

impl<SourceId, D, P, RE> TypedReflectionsSimulation<SourceId, D, P, RE>
where
    SourceId: 'static + Send + Sync,
    RE: ReflectionEffectCompatible<Reflections, RE> + ReflectionEffectType,
{
    /// Updates the input frame used on the next run.
    pub fn set_input(&self, frame: ReflectionsFrame<SourceId, D, Reflections, P, RE>) {
        self.input.store(Arc::new(frame));
    }
}

#[cfg(feature = "dynamic")]
impl<SourceId: 'static + Send + Sync + Clone + Hash + Eq> crate::dynamic::Simulation<SourceId> {
    /// Spawns a reflections simulation thread.
    /// Reports missing reflection inputs until
    /// [`crate::dynamic::ReflectionsSimulation::set_input`] supplies them.
    ///
    /// # Errors
    ///
    /// - [`crate::dynamic::SimulationError::DisabledModes`] if the mode is unavailable.
    pub fn spawn_reflections(
        &mut self,
        on_error: impl Fn(crate::dynamic::SimulationError) + Send + 'static,
    ) -> Result<crate::dynamic::ReflectionsSimulation<SourceId>, crate::dynamic::SimulationError>
    {
        let disabled = crate::SimulationFlags::REFLECTIONS & !self.simulator.flags();
        if !disabled.is_empty() {
            return Err(crate::dynamic::SimulationError::DisabledModes(disabled));
        }
        let frame = BoundReflectionsFrame::<SourceId, Dynamic<()>> {
            sources: self.sources.clone(),
            shared_inputs: Default::default(),
        };

        Ok(self.spawn_reflections_shared(frame, on_error))
    }
}

#[cfg(feature = "dynamic")]
impl<SourceId: 'static + Send + Sync> crate::dynamic::ReflectionsSimulation<SourceId> {
    /// Updates shared inputs for the next worker iteration.
    pub fn set_input(&self, shared_inputs: crate::dynamic::SimulationSharedInputs) {
        let frame = BoundReflectionsFrame::<SourceId, Dynamic<()>> {
            sources: self.input.load().sources.clone(),
            shared_inputs,
        };
        self.input.store(Arc::new(frame));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;

    #[test]
    fn test_spawn_and_shutdown() {
        let context = Context::default();
        let audio_settings = AudioSettings::default();
        let simulation_settings =
            SimulationSettings::new(&audio_settings).with_reflections(ConvolutionSettings {
                max_num_rays: 128,
                num_diffuse_samples: 8,
                max_duration: 0.5,
                max_num_sources: 4,
                num_threads: 1,
                max_order: 1,
            });
        let mut simulator = Simulator::try_new(&context, &simulation_settings).unwrap();
        let scene = Scene::try_new(&context).unwrap();
        simulator.set_scene(&scene);
        simulator.commit();

        let mut simulation = Simulation::new::<()>(simulator);
        let mut reflections_simulation = simulation.spawn_reflections(|error| {
            eprintln!("{error}");
        });
        simulation.shutdown();
        reflections_simulation
            .join()
            .expect("simulation thread panicked");
    }

    #[test]
    fn test_initial_output_is_empty() {
        let context = Context::default();
        let audio_settings = AudioSettings::default();
        let simulation_settings =
            SimulationSettings::new(&audio_settings).with_reflections(ConvolutionSettings {
                max_num_rays: 128,
                num_diffuse_samples: 8,
                max_duration: 0.5,
                max_num_sources: 4,
                num_threads: 1,
                max_order: 1,
            });
        let mut simulator = Simulator::try_new(&context, &simulation_settings).unwrap();
        let scene = Scene::try_new(&context).unwrap();
        simulator.set_scene(&scene);
        simulator.commit();

        let mut simulation = Simulation::new::<()>(simulator);
        let mut reflections_simulation = simulation.spawn_reflections(|error| {
            eprintln!("{error}");
        });

        assert!(reflections_simulation.output().load().sources.is_empty());

        simulation.shutdown();
        reflections_simulation
            .join()
            .expect("simulation thread panicked");
    }
}
