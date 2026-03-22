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
use std::sync::Arc;

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
{
    /// Spawns a direct simulation thread.
    pub fn spawn_direct(&self) -> DirectSimulation<R, P, RE> {
        let input = Arc::new(ArcSwap::new(Arc::new(DirectFrame {
            sources: self.sources.clone(),
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
            self.shutdown.clone(),
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

/// A running direct simulation thread.
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
        let mut simulation = Simulation::new(simulator);
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
        let mut simulation = Simulation::new(simulator);
        let direct_simulation = simulation.spawn_direct();

        assert!(direct_simulation.output.load().is_empty());

        simulation.shutdown();
        direct_simulation
            .handle
            .join()
            .expect("simulation thread panicked");
    }
}
