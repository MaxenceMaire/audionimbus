use super::super::runner::{ReflectionsFrame, SimulationRunner};
use super::super::step::{ReflectionsOutput, ReflectionsStep};
use super::{SharedSimulationOutput, Simulation};
use crate::effect::ReflectionEffectType;
use crate::ray_tracing::RayTracer;
use crate::simulation::{
    DirectCompatible, PathingCompatible, ReflectionEffectCompatible, Reflections,
    SimulationFlagsProvider,
};
use arc_swap::ArcSwap;
use object_pool::Pool;
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
    pub fn spawn_reflections(&mut self) -> ReflectionsSimulation<SourceId, D, P, RE> {
        let input = Arc::new(ArcSwap::new(Arc::new(ReflectionsFrame {
            sources: self.sources.clone(),
            shared_inputs: Default::default(),
        })));

        let output = SharedSimulationOutput(Arc::new(ArcSwap::new(Arc::new(
            Arc::new(Pool::new(1, ReflectionsOutput::default))
                .pull_owned(ReflectionsOutput::default),
        ))));

        let paused = Arc::new((Mutex::new(false), Condvar::new()));
        self.paused.push(paused.clone());

        let mut simulator_for_commit = self.simulator.clone();
        let handle = SimulationRunner::new(
            input.clone(),
            output.0.clone(),
            self.commit_needed.clone(),
            move || simulator_for_commit.commit(),
            self.shutdown.clone(),
            paused.clone(),
        )
        .spawn(ReflectionsStep::new::<SourceId>(self.simulator.clone()));

        ReflectionsSimulation {
            handle,
            input,
            output,
            paused,
        }
    }
}

/// A running reflections simulation thread.
pub struct ReflectionsSimulation<SourceId, D, P, RE>
where
    SourceId: 'static + Send + Sync,
    RE: 'static + ReflectionEffectCompatible<Reflections, RE> + ReflectionEffectType,
{
    /// Thread handle.
    pub handle: std::thread::JoinHandle<()>,
    /// Shared input frame, updated each game frame.
    pub input: SharedReflectionsInput<SourceId, D, P, RE>,
    /// Shared output, read by the audio thread.
    pub output: SharedSimulationOutput<ReflectionsOutput<SourceId, RE>>,
    /// Pause flag.
    pub paused: Arc<(Mutex<bool>, Condvar)>,
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

    /// Pauses the simulation thread after its current iteration completes.
    pub fn pause(&self) {
        *self.paused.0.lock().unwrap() = true;
    }

    /// Resumes a paused simulation thread.
    pub fn resume(&self) {
        *self.paused.0.lock().unwrap() = false;
        self.paused.1.notify_one();
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
        let reflections_simulation = simulation.spawn_reflections();
        simulation.shutdown();
        reflections_simulation
            .handle
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
        let reflections_simulation = simulation.spawn_reflections();

        assert!(reflections_simulation.output.load().sources.is_empty());

        simulation.shutdown();
        reflections_simulation
            .handle
            .join()
            .expect("simulation thread panicked");
    }
}
