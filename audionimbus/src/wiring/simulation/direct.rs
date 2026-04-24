use super::super::runner::{DirectFrame, SimulationRunner};
use super::super::step::{DirectStep, SimulationStepError};
use super::{SharedSimulationOutput, Simulation, SimulationControls};
use crate::effect::DirectEffectParams;
use crate::ray_tracing::RayTracer;
use crate::simulation::{
    Direct, PathingCompatible, ReflectionEffectCompatible, ReflectionsCompatible,
    SimulationFlagsProvider,
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
    ) -> DirectSimulation<SourceId, R, P, RE> {
        let input = Arc::new(ArcSwap::new(Arc::new(DirectFrame {
            sources: self.sources.clone(),
            shared_inputs: Default::default(),
        })));

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
            DirectStep::new::<SourceId>(self.simulator.clone()),
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
pub struct DirectSimulation<SourceId, R, P, RE>
where
    SourceId: 'static + Send + Sync,
    RE: ReflectionEffectCompatible<R, RE>,
{
    /// Shared input frame, updated each game frame.
    input: SharedDirectInput<SourceId, R, P, RE>,
    /// Shared output, read by the audio thread.
    output: SharedSimulationOutput<HashMap<SourceId, DirectEffectParams>>,
    /// Thread lifecycle controls.
    controls: SimulationControls,
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
