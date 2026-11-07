//! Shared simulation pipelines and publication buffers.

use crate::audio_settings::AudioSettings;
#[cfg(feature = "dynamic")]
use crate::binding::Dynamic;
use crate::binding::Static;
use crate::geometry::scene::Scene as BoundScene;
use crate::ray_tracing::RayTracer;
use crate::simulation::inputs::SimulationInputs as BoundInputs;
#[cfg(feature = "dynamic")]
use crate::simulation::simulator::SimulationChoices;
use crate::simulation::simulator::Simulator as BoundSimulator;
use crate::simulation::simulator::StaticSimulator as Simulator;
use crate::simulation::source::Source as BoundSource;
use crate::wiring::binding::{InputBinding, PipelineBinding};
use arc_swap::ArcSwap;
use object_pool::{Pool, ReusableOwned};
use std::collections::HashSet;
use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicBool, Ordering},
};

#[cfg(doc)]
use crate::wiring::runner::SimulationRunner;

/// Simulation pipeline.
///
/// Spawns simulation threads via:
/// - [`Self::spawn_direct`](crate::wiring::simulation::Simulation)
/// - [`Self::spawn_reflections`](crate::wiring::simulation::Simulation)
/// - [`Self::spawn_reflections_reverb`](crate::wiring::simulation::Simulation)
/// - [`Self::spawn_pathing`](crate::wiring::simulation::Simulation)
///
/// Spawned simulation threads share the same sources, updated atomically by the game thread via
/// [`Self::update_sources`](crate::wiring::simulation::Simulation).
///
/// Uses memory pooling to avoid per-frame allocation.
///
/// # Type parameters
///
/// | Parameter | Role |
/// |---|---|
/// | `SourceId` | Identifier type for audio sources (e.g. `u32`) |
/// | `T` | Ray tracer backend |
/// | `D` | Direct simulation mode ([`Direct`](crate::simulation::Direct) or [`()`](primitive@unit)) |
/// | `R` | Reflections mode ([`Reflections`](crate::simulation::Reflections) or [`()`](primitive@unit)) |
/// | `P` | Pathing mode ([`Pathing`](crate::simulation::Pathing) or [`()`](primitive@unit)) |
/// | `RE` | Reflection effect type (see [`ReflectionEffectType`](crate::effect::ReflectionEffectType)) |
///
/// Using [`()`](primitive@unit) for a mode disables it.
///
/// # Example
///
/// ```
/// # #[cfg(feature = "static")]
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # use audionimbus::r#static::*;
/// # use audionimbus::*;
/// # use audionimbus::r#static::{SimulationSettings, Simulator};
/// # let context = Context::default();
/// # let audio_settings = AudioSettings::default();
/// let simulation_settings = SimulationSettings::new(&audio_settings)
///     .with_direct(DirectSimulationSettings {
///         max_num_occlusion_samples: 32,
///     })
///     .with_reflections(ConvolutionSettings {
///         max_num_rays: 4096,
///         num_diffuse_samples: 32,
///         max_duration: 2.0,
///         max_num_sources: 8,
///         num_threads: 2,
///         max_order: 2,
///     });
///
/// let mut simulator = Simulator::try_new(&context, &simulation_settings)?;
///
/// // `u32` is the source ID type. Any `Clone + Send + Sync` type works.
/// let mut simulation = Simulation::new::<u32>(simulator);
/// let on_error = |error| {
///     eprintln!("{error}");
/// };
/// let mut direct_simulation = simulation.spawn_direct(on_error);
/// let mut reflections_simulation = simulation.spawn_reflections(on_error);
///
/// simulation.shutdown();
/// direct_simulation.join().expect("direct thread panicked");
/// reflections_simulation.join().expect("reflections thread panicked");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// # }
/// # #[cfg(not(feature = "static"))]
/// # fn main() {}
/// ```
pub type StaticSimulation<SourceId, T, D, R, P, RE> =
    Simulation<SourceId, Static<(T, D, R, P, RE)>>;

/// A simulation pipeline with runtime source participation.
///
/// ```
/// use audionimbus::{AudioSettings, Context, DirectSimulationSettings};
/// use audionimbus::dynamic::{Simulation, SimulationSettings, Simulator};
/// let context = Context::default();
/// let settings = SimulationSettings::new(&AudioSettings::default())
///     .with_direct(DirectSimulationSettings::default());
/// let simulator = Simulator::try_new(&context, &settings)?;
/// let mut simulation = Simulation::new::<u32>(simulator);
/// let mut worker = simulation.spawn_direct(|error| eprintln!("{error}"))?;
/// simulation.shutdown();
/// worker.join().unwrap();
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[cfg(feature = "dynamic")]
pub type DynamicSimulation<SourceId = ()> = Simulation<SourceId, Dynamic<SimulationChoices>>;

/// Storage shared by static and runtime simulation pipelines.
pub struct Simulation<SourceId, B: PipelineBinding> {
    /// The underlying simulator shared across all spawned simulation threads.
    pub simulator: BoundSimulator<B>,

    /// Pool of source buffers, reused across frames to avoid allocation.
    pub sources_pool: SourcesPool<SourceId, B::Sources>,

    /// The current source buffer, atomically swapped each frame via [`Self::update_sources`](crate::wiring::simulation::Simulation).
    pub sources: SharedSources<SourceId, B::Sources>,

    /// Set to `true` via [`Self::request_simulator_commit`] to trigger a simulator commit on the next
    /// simulation run.
    pub simulator_commit_needed: Arc<AtomicBool>,

    /// Scenes pending a commit.
    pub pending_scene_commits: Arc<ArcSwap<HashSet<BoundScene<B::Geometry>>>>,

    /// Shutdown flags for each spawned simulation thread, in spawn order.
    pub shutdowns: Vec<Arc<AtomicBool>>,

    /// Pause flags for each spawned simulation thread, in spawn order.
    ///
    /// Simulation threads can be paused via [`Self::pause`] and resumed via [`Self::resume`].
    pub paused: Vec<Arc<(Mutex<bool>, Condvar)>>,
}

impl<B: PipelineBinding> Simulation<(), B> {
    /// Creates a new simulation pipeline.
    ///
    /// `SourceId` is the identifier type for audio sources.
    /// It enables mapping each output back to its originating source.
    fn new_shared<SourceId>(simulator: BoundSimulator<B>) -> Simulation<SourceId, B> {
        let sources_pool = Arc::new(Pool::new(4, Default::default));
        let sources = Arc::new(ArcSwap::new(Arc::new(
            sources_pool.pull_owned(Default::default),
        )));
        let simulator_commit_needed = Arc::new(AtomicBool::new(false));
        let pending_scene_commits = Arc::new(ArcSwap::new(Arc::new(HashSet::new())));
        let shutdowns = vec![];
        let paused = vec![];
        Simulation {
            simulator,
            sources_pool,
            sources,
            simulator_commit_needed,
            pending_scene_commits,
            shutdowns,
            paused,
        }
    }
}

impl<SourceId, B: PipelineBinding> Simulation<SourceId, B> {
    /// Returns a reference to the underlying simulator.
    pub fn simulator(&self) -> &BoundSimulator<B> {
        &self.simulator
    }

    /// Returns the audio settings used to create the underlying simulator.
    pub const fn audio_settings(&self) -> AudioSettings {
        self.simulator.audio_settings()
    }

    /// Returns a reference to the sources pool.
    pub fn sources_pool(&self) -> &SourcesPool<SourceId, B::Sources> {
        &self.sources_pool
    }

    /// Signals all spawned simulation threads to commit the [`Simulator`] changes on their next run.
    pub fn request_simulator_commit(&mut self) {
        self.simulator_commit_needed.store(true, Ordering::Relaxed);
    }

    /// Requests simulation threads to commit the provided scenes on their next run.
    pub fn request_scene_commits(&self, scene_commits: &[BoundScene<B::Geometry>]) {
        self.pending_scene_commits.rcu(|pending_scene_commits| {
            // SAFETY: Scene's Hash and Eq are based on pointer identity, not interior state.
            #[allow(clippy::mutable_key_type)]
            let mut new_pending_scene_commits =
                HashSet::with_capacity(pending_scene_commits.len() + scene_commits.len());
            new_pending_scene_commits.extend(pending_scene_commits.iter().cloned());
            new_pending_scene_commits.extend(scene_commits.iter().cloned());
            new_pending_scene_commits
        });
    }

    /// Signals all spawned simulation threads to stop.
    pub fn shutdown(&self) {
        for shutdown in &self.shutdowns {
            shutdown.store(true, Ordering::Relaxed);
        }

        // Wake paused threads so they can observe their shutdown flag.
        self.resume();
    }

    /// Pauses the simulation threads after their current iteration completes.
    pub fn pause(&self) {
        for thread in &self.paused {
            *thread.0.lock().unwrap() = true;
        }
    }

    /// Resumes paused simulation threads.
    pub fn resume(&self) {
        for thread in &self.paused {
            *thread.0.lock().unwrap() = false;
            thread.1.notify_one();
        }
    }
}

impl<SourceId, B: PipelineBinding> Drop for Simulation<SourceId, B> {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl<T: RayTracer, D, R, P, RE> StaticSimulation<(), T, D, R, P, RE> {
    /// Creates a new simulation pipeline.
    ///
    /// `SourceId` is the identifier type for audio sources.
    /// It enables mapping each output back to its originating source.
    pub fn new<SourceId>(
        simulator: Simulator<T, D, R, P, RE>,
    ) -> StaticSimulation<SourceId, T, D, R, P, RE> {
        Self::new_shared(simulator)
    }
}

impl<SourceId, T: RayTracer, D, R, P, RE> StaticSimulation<SourceId, T, D, R, P, RE> {
    /// Updates the sources used by all simulation threads on their next run.
    ///
    /// `f` receives a pooled `Vec` to populate with the new frame's `(SourceId, SourceWithInputs)`
    /// pairs.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # #[cfg(feature = "static")]
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # use audionimbus::r#static::*;
    /// # use audionimbus::*;
    /// # use audionimbus::r#static::{SimulationInputs, SimulationParameters, SimulationSettings, Simulator, Source};
    /// # let context = Context::default();
    /// # let audio_settings = AudioSettings::default();
    /// # let simulation_settings = SimulationSettings::new(&audio_settings)
    /// #     .with_direct(DirectSimulationSettings { max_num_occlusion_samples: 4 });
    /// # let simulator = Simulator::try_new(&context, &simulation_settings)?;
    /// # let source = Source::<Direct, (), (), ()>::try_new(&simulator)?;
    /// # let simulation = Simulation::new::<u32>(simulator);
    /// # let transform = CoordinateSystem::default();
    /// simulation.update_sources(|sources| {
    ///     sources.push((
    ///         42,
    ///         SourceWithInputs {
    ///             source: source.clone(),
    ///             simulation_inputs: SimulationInputs {
    ///                 source: transform,
    ///                 parameters: SimulationParameters::new()
    ///                     .with_direct(DirectSimulationParameters::default()),
    ///             },
    ///         },
    ///     ));
    /// });
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// # }
    /// # #[cfg(not(feature = "static"))]
    /// # fn main() {}
    /// ```
    pub fn update_sources<F>(&self, f: F)
    where
        F: FnOnce(&mut Vec<(SourceId, SourceWithInputs<Static<(D, R, P, RE)>>)>),
    {
        let mut sources = self.sources_pool.pull_owned(Vec::default);
        sources.clear();
        f(&mut sources);
        self.sources.store(Arc::new(sources));
    }
}

/// Lifecycle controls for a simulation thread.
pub struct SimulationControls {
    /// The spawned thread awaiting a join.
    handle: Option<std::thread::JoinHandle<()>>,
    /// The pause flag and wake condition.
    paused: Arc<(Mutex<bool>, Condvar)>,
    /// The shutdown request.
    shutdown: Arc<AtomicBool>,
}

impl SimulationControls {
    /// Creates a new set of controls for a simulation thread.
    pub fn new(
        handle: std::thread::JoinHandle<()>,
        paused: Arc<(Mutex<bool>, Condvar)>,
        shutdown: Arc<AtomicBool>,
    ) -> Self {
        Self {
            handle: Some(handle),
            paused,
            shutdown,
        }
    }

    /// Pauses the simulation thread.
    pub fn pause(&self) {
        *self.paused.0.lock().unwrap() = true;
    }

    /// Resumes a paused simulation thread.
    pub fn resume(&self) {
        *self.paused.0.lock().unwrap() = false;
        self.paused.1.notify_one();
    }

    /// Requests shutdown of this simulation thread.
    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
        *self.paused.0.lock().unwrap() = false;
        self.paused.1.notify_one();
    }

    /// Waits for the simulation thread to exit.
    pub fn join(&mut self) -> std::thread::Result<()> {
        self.handle
            .take()
            .map_or(Ok(()), std::thread::JoinHandle::join)
    }
}

impl Drop for SimulationControls {
    fn drop(&mut self) {
        self.shutdown();
        let _ = self.join();
    }
}

/// A pair of source and simulation inputs.
pub type StaticSourceWithInputs<D, R, P, RE> = SourceWithInputs<Static<(D, R, P, RE)>>;

/// A runtime source paired with its simulation inputs.
#[cfg(feature = "dynamic")]
pub type DynamicSourceWithInputs = SourceWithInputs<Dynamic<()>>;

/// Source handles and inputs selected by a pipeline binding.
pub struct SourceWithInputs<B: InputBinding> {
    /// Spatial audio source.
    pub source: BoundSource<B::Source>,
    /// Simulation inputs for the associated source.
    pub simulation_inputs: BoundInputs<B::Inputs>,
}

impl<B: InputBinding> Clone for SourceWithInputs<B>
where
    BoundInputs<B::Inputs>: Clone,
{
    fn clone(&self) -> Self {
        Self {
            source: self.source.clone(),
            simulation_inputs: self.simulation_inputs.clone(),
        }
    }
}

impl<B: InputBinding> std::fmt::Debug for SourceWithInputs<B>
where
    BoundSource<B::Source>: std::fmt::Debug,
    BoundInputs<B::Inputs>: std::fmt::Debug,
{
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SourceWithInputs")
            .field("source", &self.source)
            .field("simulation_inputs", &self.simulation_inputs)
            .finish()
    }
}

/// A pool of source list buffers, shared across simulation threads.
pub type StaticSourcesPool<SourceId, D, R, P, RE> = SourcesPool<SourceId, Static<(D, R, P, RE)>>;

/// A pool of published buffers selected by a pipeline binding.
pub type SourcesPool<SourceId, B> = Arc<Pool<<B as InputBinding>::Sources<SourceId>>>;

/// A shared, atomically-swappable source list.
pub type StaticSharedSources<SourceId, D, R, P, RE> =
    SharedSources<SourceId, Static<(D, R, P, RE)>>;

/// Published buffers shared by the game and simulation threads.
pub type SharedSources<SourceId, B> =
    Arc<ArcSwap<ReusableOwned<<B as InputBinding>::Sources<SourceId>>>>;

/// Simulation output shared between a simulation thread and the audio thread.
///
/// Publication storage stays internal so readers cannot recycle a buffer used by the audio thread.
pub struct SharedSimulationOutput<T: 'static + Send + Sync>(
    pub(crate) Arc<ArcSwap<ReusableOwned<T>>>,
);

impl<T: 'static + Send + Sync> SharedSimulationOutput<T> {
    /// Returns a snapshot of the latest simulation output.
    ///
    /// Safe to call from the audio thread concurrently with simulation writes.
    pub fn load(&self) -> arc_swap::Guard<Arc<ReusableOwned<T>>> {
        self.0.load()
    }
}

impl<T: 'static + Send + Sync> Clone for SharedSimulationOutput<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

#[cfg(all(doctest, feature = "dynamic", feature = "static"))]
/// Worker binding checks.
///
/// ```compile_fail
/// use audionimbus::r#static::Simulation;
/// fn pipeline(simulator: audionimbus::dynamic::Simulator) {
///     Simulation::new::<u32>(simulator);
/// }
/// ```
///
/// ```compile_fail
/// fn inputs(worker: &audionimbus::dynamic::DirectSimulation<u32>) {
///     worker.set_input(audionimbus::r#static::SimulationSharedInputs::default());
/// }
/// ```
struct WiringBindings;

#[cfg(feature = "dynamic")]
impl DynamicSimulation {
    /// Creates a simulation pipeline with runtime choices.
    pub fn new<SourceId>(
        simulator: crate::dynamic::Simulator,
    ) -> crate::dynamic::Simulation<SourceId> {
        Self::new_shared(simulator)
    }
}

#[cfg(feature = "dynamic")]
impl<SourceId: Clone> DynamicSimulation<SourceId> {
    /// Validates and publishes sources for their available worker modes.
    ///
    /// Missing parameters disable their modes on the next worker iteration.
    ///
    /// # Errors
    ///
    /// - [`crate::dynamic::SimulationError::ForeignSource`] if another simulator created a source.
    /// - [`crate::dynamic::SimulationError::DisabledModes`] if inputs request an unavailable mode.
    /// - [`crate::dynamic::SimulationError::AlgorithmMismatch`] if reflection parameters use another algorithm.
    pub fn update_sources(
        &mut self,
        fill: impl FnOnce(&mut Vec<(SourceId, crate::dynamic::SourceWithInputs)>),
    ) -> Result<(), crate::dynamic::SimulationError> {
        let mut sources = self.sources_pool.pull_owned(Default::default);
        sources.sources.clear();
        fill(&mut sources.sources);
        let creator = self.simulator.source_resources().simulator;
        for (_, source_with_inputs) in &sources.sources {
            if let Some(error) = source_with_inputs.validation_error(&creator) {
                return Err(error);
            }
        }
        sources.listener = self.sources.load().listener.clone();
        sources.partition();
        self.sources.store(Arc::new(sources));

        Ok(())
    }

    /// Publishes the optional listener used by reflection reverb workers.
    ///
    /// # Errors
    ///
    /// - [`crate::dynamic::SimulationError::ForeignSource`] if another simulator created the listener.
    /// - [`crate::dynamic::SimulationError::DisabledModes`] if the listener cannot run reflections or its inputs request an unavailable mode.
    /// - [`crate::dynamic::SimulationError::AlgorithmMismatch`] if reflection parameters use another algorithm.
    pub fn set_listener(
        &mut self,
        listener: Option<crate::dynamic::SourceWithInputs>,
    ) -> Result<(), crate::dynamic::SimulationError> {
        if let Some(listener) = &listener {
            let creator = self.simulator.source_resources().simulator;
            if let Some(error) = listener.validation_error(&creator) {
                return Err(error);
            }
            if !listener
                .source
                .flags()
                .contains(crate::SimulationFlags::REFLECTIONS)
            {
                return Err(crate::dynamic::SimulationError::DisabledModes(
                    crate::SimulationFlags::REFLECTIONS,
                ));
            }
        }
        let current_sources = self.sources.load();
        let mut sources = self.sources_pool.pull_owned(Default::default);
        sources.sources.clear();
        sources
            .sources
            .extend(current_sources.sources.iter().cloned());
        sources.listener = listener;
        sources.partition();
        self.sources.store(Arc::new(sources));

        Ok(())
    }
}

#[cfg(feature = "dynamic")]
impl DynamicSourceWithInputs {
    /// Returns the first creator, participation, or reflection algorithm mismatch.
    fn validation_error(
        &self,
        creator: &std::sync::Weak<
            Mutex<
                crate::simulation::simulator::SimulatorShared<
                    crate::binding::Dynamic<crate::simulation::simulator::SimulationChoices>,
                >,
            >,
        >,
    ) -> Option<crate::dynamic::SimulationError> {
        if !self.source.creator().ptr_eq(creator) {
            return Some(crate::dynamic::SimulationError::ForeignSource);
        }
        let disabled = self.simulation_inputs.parameters.flags() & !self.source.flags();
        if !disabled.is_empty() {
            return Some(crate::dynamic::SimulationError::DisabledModes(disabled));
        }
        if let Some(parameters) = &self.simulation_inputs.parameters.reflections
            && let Some(expected) = self.source.reflection_algorithm()
        {
            let actual = parameters.algorithm();
            if expected != actual {
                return Some(crate::dynamic::SimulationError::AlgorithmMismatch {
                    expected,
                    actual,
                });
            }
        }

        None
    }
}
