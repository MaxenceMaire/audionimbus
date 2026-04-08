use super::super::runner::{ReflectionsFrame, SimulationRunner};
use super::super::step::{ReflectionsOutput, ReflectionsStep, SimulationStepError};
use super::{SharedSimulationOutput, Simulation, SimulationControls};
use crate::effect::ReflectionEffectType;
use crate::ray_tracing::RayTracer;
use crate::simulation::{
    DirectCompatible, PathingCompatible, ReflectionEffectCompatible, Reflections,
    SimulationFlagsProvider,
};
use arc_swap::ArcSwap;
use object_pool::Pool;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Condvar, Mutex};

impl<SourceId, T, D, P, RE> Simulation<SourceId, T, D, Reflections, P, RE>
where
    SourceId: 'static + Send + Sync + Clone,
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
    ) -> ReflectionsSimulation<SourceId, D, P, RE> {
        let input = Arc::new(ArcSwap::new(Arc::new(ReflectionsFrame {
            sources: self.sources.clone(),
            shared_inputs: Default::default(),
        })));

        let output = SharedSimulationOutput(Arc::new(ArcSwap::new(Arc::new(
            Arc::new(Pool::new(1, ReflectionsOutput::default))
                .pull_owned(ReflectionsOutput::default),
        ))));

        let paused = Arc::new((Mutex::new(false), Condvar::new()));
        let shutdown = Arc::new(AtomicBool::new(false));
        self.shutdowns.push(shutdown.clone());
        self.paused.push(paused.clone());

        let simulator_for_commit = self.simulator.clone();
        let handle = SimulationRunner::new(
            input.clone(),
            output.0.clone(),
            self.simulator_commit_needed.clone(),
            move || simulator_for_commit.commit(),
            self.pending_scene_commits.clone(),
            shutdown.clone(),
            paused.clone(),
        )
        .spawn(
            ReflectionsStep::new::<SourceId>(self.simulator.clone()),
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
pub struct ReflectionsSimulation<SourceId, D, P, RE>
where
    SourceId: 'static + Send + Sync,
    RE: 'static + ReflectionEffectCompatible<Reflections, RE> + ReflectionEffectType,
{
    /// Shared input frame, updated each game frame.
    input: SharedReflectionsInput<SourceId, D, P, RE>,
    /// Shared output, read by the audio thread.
    output: SharedSimulationOutput<ReflectionsOutput<SourceId, RE>>,
    /// Thread lifecycle controls.
    controls: SimulationControls,
}

impl<SourceId, D, P, RE> ReflectionsSimulation<SourceId, D, P, RE>
where
    SourceId: 'static + Send + Sync,
    RE: ReflectionEffectCompatible<Reflections, RE> + ReflectionEffectType,
{
    /// Updates the input frame used on the next run.
    pub fn set_input(&self, frame: ReflectionsFrame<SourceId, D, Reflections, P, RE>) {
        self.input.store(Arc::new(frame));
    }

    /// Returns the shared output produced by this simulation thread.
    pub fn output(&self) -> SharedSimulationOutput<ReflectionsOutput<SourceId, RE>> {
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

/// Shared, atomically-swappable input frame for a reflections simulation thread.
type SharedReflectionsInput<SourceId, D, P, RE> =
    Arc<ArcSwap<ReflectionsFrame<SourceId, D, Reflections, P, RE>>>;

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
