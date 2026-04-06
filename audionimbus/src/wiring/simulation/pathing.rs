use super::super::runner::{PathingFrame, SimulationRunner};
use super::super::step::{PathingStep, SimulationStepError};
use super::{SharedSimulationOutput, Simulation};
use crate::effect::PathEffectParams;
use crate::ray_tracing::RayTracer;
use crate::simulation::{
    DirectCompatible, Pathing, ReflectionEffectCompatible, ReflectionsCompatible,
    SimulationFlagsProvider,
};
use arc_swap::ArcSwap;
use object_pool::Pool;
use std::sync::{Arc, Condvar, Mutex};

impl<SourceId, T, D, R, RE> Simulation<SourceId, T, D, R, Pathing, RE>
where
    SourceId: 'static + Send + Sync + Clone,
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
    ) -> PathingSimulation<SourceId, D, R, RE> {
        let input = Arc::new(ArcSwap::new(Arc::new(PathingFrame {
            sources: self.sources.clone(),
            shared_inputs: Default::default(),
        })));

        let output = SharedSimulationOutput(Arc::new(ArcSwap::new(Arc::new(
            Arc::new(Pool::new(1, Vec::default)).pull_owned(Vec::default),
        ))));

        let paused = Arc::new((Mutex::new(false), Condvar::new()));
        self.paused.push(paused.clone());

        let simulator_for_commit = self.simulator.clone();
        let handle = SimulationRunner::new(
            input.clone(),
            output.0.clone(),
            self.commit_needed.clone(),
            move || simulator_for_commit.commit(),
            self.pending_scene_commits.clone(),
            self.shutdown.clone(),
            paused.clone(),
        )
        .spawn(
            PathingStep::new::<SourceId>(self.simulator.clone()),
            on_error,
        );

        PathingSimulation {
            handle,
            input,
            output,
            paused,
        }
    }
}

/// A running pathing simulation thread.
pub struct PathingSimulation<SourceId, D, R, RE>
where
    SourceId: 'static + Send + Sync,
    RE: ReflectionEffectCompatible<R, RE>,
{
    /// Thread handle.
    pub handle: std::thread::JoinHandle<()>,
    /// Shared input frame, updated each game frame.
    pub input: SharedPathingInput<SourceId, D, R, RE>,
    /// Shared output, read by the audio thread.
    pub output: SharedSimulationOutput<Vec<(SourceId, PathEffectParams)>>,
    /// Pause flag.
    pub paused: Arc<(Mutex<bool>, Condvar)>,
}

impl<SourceId, D, R, RE> PathingSimulation<SourceId, D, R, RE>
where
    SourceId: 'static + Send + Sync,
    RE: ReflectionEffectCompatible<R, RE>,
{
    /// Updates the input frame used on the simulation thread's next run.
    pub fn set_input(&self, frame: PathingFrame<SourceId, D, R, Pathing, RE>) {
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

/// Shared, atomically-swappable input frame for a pathing simulation thread.
type SharedPathingInput<SourceId, D, R, RE> =
    Arc<ArcSwap<PathingFrame<SourceId, D, R, Pathing, RE>>>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;

    fn simulation() -> Simulation<(), DefaultRayTracer, (), (), Pathing, ()> {
        let context = Context::default();
        let audio_settings = AudioSettings::default();
        let simulation_settings =
            SimulationSettings::new(&audio_settings).with_pathing(PathingSimulationSettings {
                num_visibility_samples: 4,
            });
        let mut simulator = Simulator::try_new(&context, &simulation_settings).unwrap();

        let mut scene = Scene::try_new(&context).unwrap();
        let vertices = vec![
            Point::new(-10.0, 0.0, -10.0),
            Point::new(10.0, 0.0, -10.0),
            Point::new(10.0, 0.0, 10.0),
            Point::new(-10.0, 0.0, 10.0),
        ];
        let triangles = vec![Triangle::new(0, 1, 2), Triangle::new(0, 2, 3)];
        let materials = vec![Material::default()];
        let material_indices = vec![0usize, 0];
        let static_mesh = StaticMesh::try_new(
            &scene,
            &StaticMeshSettings {
                vertices: &vertices,
                triangles: &triangles,
                material_indices: &material_indices,
                materials: &materials,
            },
        )
        .unwrap();
        scene.add_static_mesh(static_mesh);
        scene.commit();
        simulator.set_scene(&scene);

        let mut probe_array = ProbeArray::try_new(&context).unwrap();
        probe_array.generate_probes(
            &scene,
            &ProbeGenerationParams::UniformFloor {
                spacing: 5.0,
                height: 1.5,
                transform: Matrix4::new([
                    [20.0, 0.0, 0.0, 0.0],
                    [0.0, 20.0, 0.0, 0.0],
                    [0.0, 0.0, 20.0, 0.0],
                    [0.0, 0.0, 0.0, 1.0],
                ]),
            },
        );
        let mut probe_batch = ProbeBatch::try_new(&context).unwrap();
        probe_batch.add_probe_array(&probe_array);
        probe_batch.commit();
        simulator.add_probe_batch(&probe_batch);
        simulator.commit();

        Simulation::new::<()>(simulator)
    }

    #[test]
    fn test_spawn_and_shutdown() {
        let mut simulation = simulation();
        let pathing_simulation = simulation.spawn_pathing(|error| {
            eprintln!("{error}");
        });
        simulation.shutdown();
        pathing_simulation
            .handle
            .join()
            .expect("simulation thread panicked");
    }

    #[test]
    fn test_initial_output_is_empty() {
        let mut simulation = simulation();
        let pathing_simulation = simulation.spawn_pathing(|error| {
            eprintln!("{error}");
        });

        assert!(pathing_simulation.output.load().is_empty());

        simulation.shutdown();
        pathing_simulation
            .handle
            .join()
            .expect("simulation thread panicked");
    }
}
