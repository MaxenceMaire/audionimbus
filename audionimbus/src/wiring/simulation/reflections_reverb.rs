use super::super::runner::{ReflectionsReverbFrame, SimulationRunner};
use super::super::step::{ReflectionsReverbOutput, ReflectionsReverbStep};
use super::{SharedSimulationOutput, Simulation, SourceWithInputs};
use crate::effect::ReflectionEffectType;
use crate::ray_tracing::RayTracer;
use crate::simulation::{
    DirectCompatible, PathingCompatible, ReflectionEffectCompatible, Reflections,
    SimulationFlagsProvider,
};
use arc_swap::ArcSwap;
use object_pool::Pool;
use std::sync::Arc;

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
{
    /// Spawns a reflections and reverb simulation thread.
    ///
    /// `listener` is the source placed at the listener's position, used for reverb simulation.
    pub fn spawn_reflections_reverb(
        &self,
        listener: SourceWithInputs<(), Reflections, (), RE>,
    ) -> ReflectionsReverbSimulation<D, P, RE> {
        let input = Arc::new(ArcSwap::new(Arc::new(ReflectionsReverbFrame {
            sources: self.sources.clone(),
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
            self.shutdown.clone(),
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

/// A running reflections and reverb simulation thread.
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

        let simulator_clone = simulator.clone();
        let mut simulation = Simulation::new(simulator);

        let listener_source =
            Source::<(), Reflections, (), Convolution>::try_new(&simulator_clone).unwrap();
        simulator_clone.add_source(&listener_source);
        simulation.request_commit();

        let listener = SourceWithInputs {
            source: listener_source,
            simulation_inputs: SimulationInputs::new(CoordinateSystem::default()).with_reflections(
                ConvolutionParameters {
                    baked_data_identifier: None,
                },
            ),
        };

        let reverb_simulation = simulation.spawn_reflections_reverb(listener);
        simulation.shutdown();
        reverb_simulation
            .handle
            .join()
            .expect("simulation thread panicked");
    }

    #[test]
    fn test_initial_listener_output_is_none() {
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

        let simulator_clone = simulator.clone();
        let mut simulation = Simulation::new(simulator);

        let listener_source =
            Source::<(), Reflections, (), Convolution>::try_new(&simulator_clone).unwrap();
        simulator_clone.add_source(&listener_source);
        simulation.request_commit();

        let listener = SourceWithInputs {
            source: listener_source,
            simulation_inputs: SimulationInputs::new(CoordinateSystem::default()).with_reflections(
                ConvolutionParameters {
                    baked_data_identifier: None,
                },
            ),
        };

        let reverb_simulation = simulation.spawn_reflections_reverb(listener);

        assert!(reverb_simulation.output.load().listener.is_none());

        simulation.shutdown();
        reverb_simulation
            .handle
            .join()
            .expect("simulation thread panicked");
    }
}
