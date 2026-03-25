use super::super::runner::{DirectFrame, SimulationRunner};
use super::super::step::DirectStep;
use super::{SharedSimulationOutput, Simulation};
use crate::effect::{DirectEffectParams, ReflectionEffectType};
use crate::ray_tracing::RayTracer;
use crate::simulation::{
    Direct, PathingCompatible, ReflectionEffectCompatible, ReflectionsCompatible,
    SimulationFlagsProvider,
};
use arc_swap::ArcSwap;
use object_pool::Pool;
use std::sync::{Arc, Condvar, Mutex};

impl<SourceId, T, R, P, RE> Simulation<SourceId, T, Direct, R, P, RE>
where
    SourceId: 'static + Send + Sync + Clone,
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
    pub fn spawn_direct(&mut self) -> DirectSimulation<SourceId, R, P, RE> {
        let input = Arc::new(ArcSwap::new(Arc::new(DirectFrame {
            sources: self.sources.clone(),
            shared_inputs: Default::default(),
        })));

        let output =
            SharedSimulationOutput::<Vec<(SourceId, DirectEffectParams)>>(Arc::new(ArcSwap::new(
                Arc::new(Arc::new(Pool::new(1, Vec::default)).pull_owned(Vec::default)),
            )));

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
        .spawn(DirectStep::new::<SourceId>(self.simulator.clone()));

        DirectSimulation {
            handle,
            input,
            output,
            paused,
        }
    }
}

/// A running direct simulation thread.
pub struct DirectSimulation<SourceId, R, P, RE>
where
    SourceId: 'static + Send + Sync,
    RE: ReflectionEffectCompatible<R, RE>,
{
    /// Thread handle.
    pub handle: std::thread::JoinHandle<()>,
    /// Shared input frame, updated each game frame.
    pub input: SharedDirectInput<SourceId, R, P, RE>,
    /// Shared output, read by the audio thread.
    pub output: SharedSimulationOutput<Vec<(SourceId, DirectEffectParams)>>,
    /// Pause flag.
    pub paused: Arc<(Mutex<bool>, Condvar)>,
}

impl<SourceId, R, P, RE> DirectSimulation<SourceId, R, P, RE>
where
    SourceId: 'static + Send + Sync,
    RE: ReflectionEffectCompatible<R, RE>,
{
    /// Updates the input frame used on the next run.
    pub fn set_input(&self, frame: DirectFrame<SourceId, Direct, R, P, RE>) {
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

/// Shared, atomically-swappable input frame for a direct simulation thread.
type SharedDirectInput<SourceId, R, P, RE> = Arc<ArcSwap<DirectFrame<SourceId, Direct, R, P, RE>>>;

#[cfg(test)]
mod tests {
    use super::*;
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
        let direct_simulation = simulation.spawn_direct();
        simulation.shutdown();
        direct_simulation
            .handle
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
        let direct_simulation = simulation.spawn_direct();

        assert!(direct_simulation.output.load().is_empty());

        simulation.shutdown();
        direct_simulation
            .handle
            .join()
            .expect("simulation thread panicked");
    }
}
