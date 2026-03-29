use super::super::runner::{ReflectionsReverbFrame, SimulationRunner};
use super::super::step::{ReflectionsReverbOutput, ReflectionsReverbStep, SimulationStepError};
use super::{SharedSimulationOutput, Simulation, SourceWithInputs};
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
    /// Spawns a reflections and reverb simulation thread.
    pub fn spawn_reflections_reverb<LD, LP>(
        &mut self,
        on_error: impl Fn(SimulationStepError) + Send + 'static,
    ) -> ReflectionsReverbSimulation<SourceId, D, P, RE, LD, LP>
    where
        LD: 'static + Send + Sync + DirectCompatible<LD> + SimulationFlagsProvider,
        LP: 'static + Send + Sync + PathingCompatible<LP> + SimulationFlagsProvider,
        (): DirectCompatible<LD> + PathingCompatible<LP>,
    {
        let input = Arc::new(ArcSwap::new(Arc::new(ReflectionsReverbFrame {
            sources: self.sources.clone(),
            listener: None::<SourceWithInputs<LD, Reflections, LP, RE>>,
            shared_inputs: Default::default(),
        })));

        let output = SharedSimulationOutput(Arc::new(ArcSwap::new(Arc::new(
            Arc::new(Pool::new(1, ReflectionsReverbOutput::default))
                .pull_owned(ReflectionsReverbOutput::default),
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
        .spawn(
            ReflectionsReverbStep::new::<SourceId>(self.simulator.clone()),
            on_error,
        );

        ReflectionsReverbSimulation {
            handle,
            input,
            output,
            paused,
        }
    }
}

/// A running reflections and reverb simulation thread.
pub struct ReflectionsReverbSimulation<SourceId, D, P, RE, LD = D, LP = P>
where
    SourceId: 'static + Send + Sync,
    RE: 'static + ReflectionEffectCompatible<Reflections, RE> + ReflectionEffectType,
{
    /// Thread handle.
    pub handle: std::thread::JoinHandle<()>,
    /// Shared input frame, updated each game frame.
    pub input: SharedReflectionsReverbInput<SourceId, D, P, RE, LD, LP>,
    /// Shared output, read by the audio thread.
    pub output: SharedSimulationOutput<ReflectionsReverbOutput<SourceId, RE>>,
    /// Pause flag.
    pub paused: Arc<(Mutex<bool>, Condvar)>,
}

impl<SourceId, D, P, RE, LD, LP> ReflectionsReverbSimulation<SourceId, D, P, RE, LD, LP>
where
    SourceId: 'static + Send + Sync,
    RE: ReflectionEffectCompatible<Reflections, RE> + ReflectionEffectType,
    LD: Send + Sync,
    LP: Send + Sync,
{
    /// Updates the input frame used on the next run.
    pub fn set_input(
        &self,
        frame: ReflectionsReverbFrame<SourceId, D, Reflections, P, RE, LD, LP>,
    ) {
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

/// Shared, atomically-swappable input frame for a reflections and reverb simulation thread.
type SharedReflectionsReverbInput<SourceId, D, P, RE, LD = D, LP = P> =
    Arc<ArcSwap<ReflectionsReverbFrame<SourceId, D, Reflections, P, RE, LD, LP>>>;

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
        let mut simulation = Simulation::new::<()>(simulator);

        let listener_source =
            Source::<(), Reflections, (), Convolution>::try_new(&simulator_clone).unwrap();
        simulator_clone.add_source(&listener_source);
        simulation.request_commit();

        let reverb_simulation = simulation.spawn_reflections_reverb::<(), ()>(|error| {
            eprintln!("{error}");
        });
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
        let mut simulation = Simulation::new::<()>(simulator);

        let listener_source =
            Source::<(), Reflections, (), Convolution>::try_new(&simulator_clone).unwrap();
        simulator_clone.add_source(&listener_source);
        simulation.request_commit();

        let reverb_simulation = simulation.spawn_reflections_reverb::<(), ()>(|error| {
            eprintln!("{error}");
        });

        assert!(reverb_simulation.output.load().listener.is_none());

        simulation.shutdown();
        reverb_simulation
            .handle
            .join()
            .expect("simulation thread panicked");
    }
}
