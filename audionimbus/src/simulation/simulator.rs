//! Simulation handles, scene registration, and shared listener inputs.

use super::settings::{SimulationSettings, StaticSimulationSettings};
#[cfg(feature = "dynamic")]
use super::source::DynamicSourceResources;
use super::source::{Source, SourceBinding, StaticSource};
use super::{
    Direct, DirectCompatible, ParameterValidationError, Pathing, PathingCompatible,
    ReflectionEffectCompatible, Reflections, ReflectionsCompatible, SimulationError,
    SimulationFlags, SimulationFlagsProvider,
};
#[cfg(feature = "dynamic")]
use crate::binding::Dynamic;
use crate::binding::Static;
pub use crate::callback::PathingVisualizationCallback;
use crate::device::{OpenClDevice, RadeonRaysDevice, TrueAudioNextDevice};
#[cfg(feature = "dynamic")]
use crate::dynamic::{self, RayTracerKind, ReflectionAlgorithm};
use crate::error::to_option_error;
use crate::geometry::scene::{GeometryBinding, Scene as BoundScene};
use crate::ray_tracing::RayTracer;
use crate::{AudioSettings, Context, CoordinateSystem, ProbeBatch, Scene, SteamAudioError};
use std::collections::HashMap;
#[cfg(feature = "dynamic")]
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, MutexGuard};

/// Geometry and callback storage selected by a simulator binding.
pub trait SimulatorBinding: Clone {
    /// The scene binding accepted by this simulator.
    type Geometry: GeometryBinding;
    /// The callbacks retained after a shared input update.
    type Callbacks: Default + std::fmt::Debug;

    /// Retains callbacks referenced by the selected native simulation modes.
    fn retain_callback(
        shared: &Mutex<SimulatorShared<Self>>,
        flags: SimulationFlags,
        callback: Option<&PathingVisualizationCallback>,
    );
}

impl<T: RayTracer, D, R, P, RE> SimulatorBinding for Static<(T, D, R, P, RE)> {
    type Geometry = Static<T>;
    type Callbacks = ();

    fn retain_callback(
        _: &Mutex<SimulatorShared<Self>>,
        _: SimulationFlags,
        _: Option<&PathingVisualizationCallback>,
    ) {
    }
}

/// Runtime simulator choices and ray-tracing device ownership.
#[cfg(feature = "dynamic")]
#[derive(Clone, Debug)]
pub struct SimulationChoices {
    /// The available simulation modes.
    pub flags: SimulationFlags,
    /// The selected ray tracer.
    pub ray_tracer: RayTracerKind,
    /// The selected reflection algorithm.
    pub reflection_algorithm: Option<ReflectionAlgorithm>,
    /// The OpenCL device selected for Radeon Rays.
    pub ray_open_cl_device: Option<OpenClDevice>,
}

#[cfg(feature = "dynamic")]
impl SimulatorBinding for Dynamic<SimulationChoices> {
    type Geometry = Dynamic<RayTracerKind>;
    type Callbacks = Option<PathingVisualizationCallback>;

    fn retain_callback(
        shared: &Mutex<SimulatorShared<Self>>,
        flags: SimulationFlags,
        callback: Option<&PathingVisualizationCallback>,
    ) {
        if flags.contains(SimulationFlags::PATHING) {
            shared.lock().unwrap()._callbacks = callback.cloned();
        }
    }
}

/// Manages direct and indirect sound propagation simulation for multiple sources.
///
/// Your application will typically create one simulator object and use it to run simulations with different source and listener parameters between consecutive simulation runs.
/// The simulator can also be reused across scene changes.
///
/// `Simulator` is a reference-counted handle to an underlying Steam Audio object.
/// Cloning it is cheap; it produces a new handle pointing to the same underlying object, while
/// incrementing a reference count.
/// The underlying object is destroyed when all handles are dropped.
///
/// # Examples
///
/// Basic simulation workflow (shown with direct sound; reflections and pathing follow a similar pattern):
///
/// ```
/// # use audionimbus::*;
/// # let context = Context::default();
/// let audio_settings = AudioSettings::default();
///
/// // Create a simulator.
/// let settings = SimulationSettings::new(&audio_settings)
///     .with_direct(DirectSimulationSettings {
///         max_num_occlusion_samples: 4,
///     });
/// let mut simulator = Simulator::try_new(&context, &settings)?;
///
/// // Set up a scene.
/// let scene = Scene::try_new(&context)?;
///
/// // ... add and commit geometry to the scene ...
///
/// simulator.set_scene(&scene);
/// simulator.commit();
///
/// let mut source = Source::try_new(&simulator)?;
/// simulator.add_source(&source);
///
/// // Configure simulation parameters.
/// let simulation_inputs = SimulationInputs {
///     source: CoordinateSystem::default(),
///     parameters: SimulationParameters::new()
///         .with_direct(DirectSimulationParameters::new()
///             .with_distance_attenuation(DistanceAttenuationModel::default()))
/// };
/// source.set_direct_inputs(&simulation_inputs);
///
/// // Set shared parameters.
/// let shared_inputs = SimulationSharedInputs::new(CoordinateSystem::default());
/// simulator.set_shared_direct_inputs(&shared_inputs);
///
/// // Run the simulation.
/// simulator.run_direct();
///
/// // Get results.
/// let outputs = source.get_outputs()?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub type StaticSimulator<T, D = (), R = (), P = (), RE = ()> = Simulator<Static<(T, D, R, P, RE)>>;

/// A simulator with runtime-selected modes, backend, and reflection algorithm.
///
/// Static and runtime simulation handles cannot be mixed.
///
/// ```
/// use audionimbus::{AudioSettings, Context, CoordinateSystem, DirectSimulationParameters,
///     DirectSimulationSettings, DistanceAttenuationModel, SimulationFlags};
/// use audionimbus::dynamic::{SimulationInputs, SimulationParameters, SimulationSettings,
///     SimulationSharedInputs, Simulator, Source};
/// let context = Context::default();
/// let settings = SimulationSettings::new(&AudioSettings::default())
///     .with_direct(DirectSimulationSettings::default());
/// let simulator = Simulator::try_new(&context, &settings)?;
/// let source = Source::try_new(&simulator, SimulationFlags::DIRECT)?;
/// simulator.try_add_source(&source)?;
/// simulator.commit();
/// source.set_inputs(&SimulationInputs {
///     source: CoordinateSystem::default(),
///     parameters: SimulationParameters {
///         direct: Some(DirectSimulationParameters::new()
///             .with_distance_attenuation(DistanceAttenuationModel::default())),
///         ..Default::default()
///     },
/// })?;
/// simulator.set_shared_inputs(SimulationFlags::DIRECT, &SimulationSharedInputs::default())?;
/// simulator.run_direct()?;
/// let parameters = source.get_direct_outputs()?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[cfg(feature = "dynamic")]
pub type DynamicSimulator = Simulator<Dynamic<SimulationChoices>>;

/// Storage shared by static and runtime simulators.
pub struct Simulator<B: SimulatorBinding> {
    /// The native handle or output storage.
    inner: audionimbus_sys::IPLSimulator,
    /// The resources shared by cloned handles.
    shared: Arc<Mutex<SimulatorShared<B>>>,

    /// Audio settings used to create this simulator.
    audio_settings: AudioSettings,

    /// The maximum number of occlusion samples specified during creation.
    max_num_occlusion_samples: Option<u32>,

    /// The maximum number of rays specified during creation.
    max_num_rays: Option<u32>,

    /// The maximum duration specified during creation.
    max_duration: Option<f32>,

    /// Synchronization lock for direct simulation operations.
    direct_lock: Option<SimulationLock>,

    /// Synchronization lock for reflections simulation operations.
    reflections_lock: Option<SimulationLock>,

    /// Synchronization lock for pathing simulation operations.
    pathing_lock: Option<SimulationLock>,

    /// The retained OpenCL device.
    _open_cl_device: Option<OpenClDevice>,
    /// The retained Radeon Rays device.
    _radeon_rays_device: Option<RadeonRaysDevice>,
    /// The retained TrueAudio Next device.
    _true_audio_next_device: Option<TrueAudioNextDevice>,
    /// The compile-time or runtime simulation binding.
    binding: B,
}

impl<B> std::fmt::Debug for Simulator<B>
where
    B: SimulatorBinding + std::fmt::Debug,
    B::Geometry: std::fmt::Debug,
{
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Simulator")
            .field("inner", &self.inner)
            .field("shared", &self.shared)
            .field("audio_settings", &self.audio_settings)
            .field("max_num_occlusion_samples", &self.max_num_occlusion_samples)
            .field("max_num_rays", &self.max_num_rays)
            .field("max_duration", &self.max_duration)
            .field("direct_lock", &self.direct_lock)
            .field("reflections_lock", &self.reflections_lock)
            .field("pathing_lock", &self.pathing_lock)
            .field("binding", &self.binding)
            .finish_non_exhaustive()
    }
}

/// A shared lock for one simulation mode.
pub type SimulationLock = Arc<Mutex<()>>;

/// Shared ownership of [`crate::Simulator`] data across clones.
#[derive(Debug)]
pub struct SimulatorShared<B: SimulatorBinding> {
    /// Number of probes after the last commit.
    /// Used to ensure the simulator has probes before running pathing.
    committed_num_probes: usize,

    /// Pending probe batches to be committed.
    pending_probe_batches: HashMap<audionimbus_sys::IPLProbeBatch, usize>,

    /// The scene currently visible to simulation after the last simulator commit.
    committed_scene: Option<BoundScene<B::Geometry>>,

    /// The next scene to become visible after the next simulator commit.
    pending_scene: Option<BoundScene<B::Geometry>>,
    /// Callbacks retained by the runtime binding.
    _callbacks: B::Callbacks,
}

impl<B: SimulatorBinding> Default for SimulatorShared<B> {
    fn default() -> Self {
        Self {
            _callbacks: B::Callbacks::default(),
            committed_num_probes: 0,
            pending_probe_batches: HashMap::new(),
            committed_scene: None,
            pending_scene: None,
        }
    }
}

impl<T, D, R, P, RE> StaticSimulator<T, D, R, P, RE>
where
    T: RayTracer,
    D: 'static,
    R: 'static,
    P: 'static,
    RE: 'static,
{
    /// Creates a new [`crate::Simulator`] and returns a handle to it.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if creation fails.
    ///
    /// # Examples
    ///
    /// ```
    /// # use audionimbus::*;
    /// # let context = Context::default();
    /// # let audio_settings = AudioSettings::default();
    /// let settings = SimulationSettings::new(&audio_settings)
    ///     .with_direct(DirectSimulationSettings {
    ///         max_num_occlusion_samples: 4,
    ///     })
    ///     .with_reflections(ConvolutionSettings {
    ///         max_num_rays: 4096,
    ///         num_diffuse_samples: 32,
    ///         max_duration: 2.0,
    ///         max_num_sources: 8,
    ///         num_threads: 2,
    ///         max_order: 1,
    ///     })
    ///     .with_pathing(PathingSimulationSettings {
    ///         num_visibility_samples: 4,
    ///     });
    /// let simulator = Simulator::try_new(&context, &settings)?;
    /// # Ok::<(), audionimbus::SteamAudioError>(())
    /// ```
    pub fn try_new(
        context: &Context,
        settings: &StaticSimulationSettings<T, D, R, P, RE>,
    ) -> Result<Self, SteamAudioError> {
        let mut flags = SimulationFlags::empty();
        if std::any::TypeId::of::<D>() == std::any::TypeId::of::<Direct>() {
            flags |= SimulationFlags::DIRECT;
        }
        if std::any::TypeId::of::<R>() == std::any::TypeId::of::<Reflections>() {
            flags |= SimulationFlags::REFLECTIONS;
        }
        if std::any::TypeId::of::<P>() == std::any::TypeId::of::<Pathing>() {
            flags |= SimulationFlags::PATHING;
        }

        Self::try_new_shared(context, settings, flags)
    }

    /// Specifies the scene within which all subsequent simulations should be run.
    ///
    /// Call [`Self::commit`] after calling this function for the changes to take effect.
    ///
    /// This function cannot be called while any simulation is running. Either will block until the
    /// other finishes.
    pub fn set_scene(&mut self, scene: &Scene<T>) {
        self.set_scene_shared(scene);
    }

    /// Adds a source to the set of sources processed by a simulator in subsequent simulations.
    ///
    /// Call [`Self::commit`] after calling this function for the changes to take effect.
    pub fn add_source<SrcD, SrcR, SrcP, SrcRE>(
        &self,
        source: &StaticSource<SrcD, SrcR, SrcP, SrcRE>,
    ) where
        SrcD: DirectCompatible<D> + 'static,
        SrcR: ReflectionsCompatible<R> + 'static,
        SrcP: PathingCompatible<P> + 'static,
        SrcRE: ReflectionEffectCompatible<SrcR, RE> + 'static,
    {
        self.add_source_shared(source);
    }

    /// Removes a source from the set of sources processed by a simulator in subsequent simulations.
    ///
    /// Call [`Self::commit`] after calling this function for the changes to take effect.
    pub fn remove_source<SrcD, SrcR, SrcP, SrcRE>(
        &self,
        source: &StaticSource<SrcD, SrcR, SrcP, SrcRE>,
    ) where
        SrcD: DirectCompatible<D> + 'static,
        SrcR: ReflectionsCompatible<R> + 'static,
        SrcP: PathingCompatible<P> + 'static,
        SrcRE: ReflectionEffectCompatible<SrcR, RE> + 'static,
    {
        self.remove_source_shared(source);
    }

    /// Specifies simulation parameters that are not associated with any particular source.
    ///
    /// Convenience method abstracting the more expressive [`Self::set_shared_inputs_subset`].
    ///
    /// Also see:
    /// - [`Self::set_shared_direct_inputs`]
    /// - [`Self::set_shared_reflections_inputs`]
    /// - [`Self::set_shared_pathing_inputs`]
    ///
    /// # Threading Considerations
    ///
    /// This method will block if a simulation is currently running for the
    /// specified type(s).
    /// Call this from the same thread that runs the corresponding simulation(s) to avoid blocking.
    ///
    /// It is safe to call this method concurrently with simulations of different types.
    /// For example, setting `DIRECT` shared inputs while a `REFLECTIONS` simulation is running
    /// will not block.
    ///
    /// See the [module-level documentation](crate::simulation) for threading guidelines.
    ///
    /// # Examples
    ///
    /// ```
    /// # use audionimbus::*;
    /// # let context = Context::default();
    /// # let audio_settings = AudioSettings::default();
    /// # let simulation_settings = SimulationSettings::new(&audio_settings)
    /// #     .with_direct(DirectSimulationSettings { max_num_occlusion_samples: 4 })
    /// #     .with_reflections(ConvolutionSettings {
    /// #         max_num_rays: 4096,
    /// #         num_diffuse_samples: 32,
    /// #         max_duration: 2.0,
    /// #         max_num_sources: 8,
    /// #         num_threads: 2,
    /// #         max_order: 1,
    /// #     });
    /// # let simulator = Simulator::try_new(&context, &simulation_settings)?;
    /// let shared_inputs = SimulationSharedInputs::new(CoordinateSystem::default())
    ///     .with_reflections(ReflectionsSharedInputs {
    ///         num_rays: 4096,
    ///         num_bounces: 16,
    ///         duration: 2.0,
    ///         order: 1,
    ///         irradiance_min_distance: 1.0,
    ///     });
    /// simulator.set_shared_inputs(&shared_inputs)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Arguments
    ///
    /// - `shared_inputs`: the shared input parameters to set.
    ///
    /// # Errors
    ///
    /// Returns [`ParameterValidationError`] if any parameters exceed the maximums set during
    /// simulator initialization.
    pub fn set_shared_inputs<InD, InR, InP>(
        &self,
        shared_inputs: &StaticSimulationSharedInputs<InD, InR, InP>,
    ) -> Result<(), ParameterValidationError>
    where
        D: DirectCompatible<D> + SimulationFlagsProvider,
        R: ReflectionsCompatible<R> + ReflectionsCompatible<InR> + SimulationFlagsProvider,
        P: PathingCompatible<P> + SimulationFlagsProvider,
        InD: DirectCompatible<D> + SimulationFlagsProvider,
        InR: ReflectionsCompatible<R> + SimulationFlagsProvider,
        InP: PathingCompatible<P> + SimulationFlagsProvider,
    {
        self.set_shared_inputs_subset::<D, R, P, InD, InR, InP>(shared_inputs)
    }

    /// Specifies simulation parameters that are not associated with any particular source.
    ///
    /// Only blocks for the requested simulation types, allowing concurrent setting of shared
    /// parameters across simulation threads.
    ///
    /// The last three generic parameters `InD`, `InR`, and `InP` can be inferred from the
    /// `shared_inputs` argument using `_`.
    ///
    /// Also see:
    /// - [`Self::set_shared_direct_inputs`]
    /// - [`Self::set_shared_reflections_inputs`]
    /// - [`Self::set_shared_pathing_inputs`]
    ///
    /// # Threading Considerations
    ///
    /// This method will block if a simulation is currently running for the
    /// specified type(s).
    /// Call this from the same thread that runs the corresponding simulation(s) to avoid blocking.
    ///
    /// It is safe to call this method concurrently with simulations of different types.
    /// For example, setting `DIRECT` shared inputs while a `REFLECTIONS` simulation is running
    /// will not block.
    ///
    /// See the [module-level documentation](crate::simulation) for threading guidelines.
    ///
    /// # Examples
    ///
    /// ```
    /// # use audionimbus::*;
    /// # let context = Context::default();
    /// # let audio_settings = AudioSettings::default();
    /// # let simulation_settings = SimulationSettings::new(&audio_settings)
    /// #     .with_direct(DirectSimulationSettings { max_num_occlusion_samples: 4 })
    /// #     .with_reflections(ConvolutionSettings {
    /// #         max_num_rays: 4096,
    /// #         num_diffuse_samples: 32,
    /// #         max_duration: 2.0,
    /// #         max_num_sources: 8,
    /// #         num_threads: 2,
    /// #         max_order: 1,
    /// #     });
    /// # let simulator = Simulator::try_new(&context, &simulation_settings)?;
    /// let shared_inputs = SimulationSharedInputs::new(CoordinateSystem::default())
    ///     .with_reflections(ReflectionsSharedInputs {
    ///         num_rays: 4096,
    ///         num_bounces: 16,
    ///         duration: 2.0,
    ///         order: 1,
    ///         irradiance_min_distance: 1.0,
    ///     });
    ///
    /// // Direct simulation thread...
    /// simulator.set_shared_inputs_subset::<Direct, (), (), _, _, _>(&shared_inputs)?;
    ///
    /// // Reflections simulation thread... Does not block the direct thread.
    /// simulator.set_shared_inputs_subset::<(), Reflections, (), _, _, _>(&shared_inputs)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Arguments
    ///
    /// - `shared_inputs`: the shared input parameters to set.
    ///
    /// # Errors
    ///
    /// Returns [`ParameterValidationError`] if any parameters exceed the maximums set during
    /// simulator initialization.
    pub fn set_shared_inputs_subset<SubD, SubR, SubP, InD, InR, InP>(
        &self,
        // `InD` and `InP` are intentionally loosely constrained here.
        // `StaticSimulationSharedInputs` does not contain any Direct- or Pathing-specific data.
        // Allowing arbitrary `InD` and `InP` avoids forcing callers to construct a value via
        // `StaticSimulationSharedInputs::with_direct` and `StaticSimulationSharedInputs::with_pathing`, which
        // would add unnecessary boilerplate with no semantic benefit.
        shared_inputs: &StaticSimulationSharedInputs<InD, InR, InP>,
    ) -> Result<(), ParameterValidationError>
    where
        SubD: DirectCompatible<D> + SimulationFlagsProvider,
        SubR: ReflectionsCompatible<R> + ReflectionsCompatible<InR> + SimulationFlagsProvider,
        SubP: PathingCompatible<P> + SimulationFlagsProvider,
        InD: DirectCompatible<D> + SimulationFlagsProvider,
        InR: ReflectionsCompatible<R> + SimulationFlagsProvider,
        InP: PathingCompatible<P> + SimulationFlagsProvider,
    {
        self.set_shared_inputs_shared(SubD::flags() | SubR::flags() | SubP::flags(), shared_inputs)
    }

    /// Returns a mutable reference to the raw FFI pointer.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub const fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLSimulator {
        &mut self.inner
    }
}

impl<B: SimulatorBinding> Simulator<B> {
    /// Registers the native source for the next simulator commit.
    fn add_source_shared<S: SourceBinding>(&self, source: &Source<S>) {
        unsafe {
            audionimbus_sys::iplSourceAdd(source.raw_ptr(), self.raw_ptr());
        }
    }

    /// Removes the native source at the next simulator commit.
    fn remove_source_shared<S: SourceBinding>(&self, source: &Source<S>) {
        unsafe {
            audionimbus_sys::iplSourceRemove(source.raw_ptr(), self.raw_ptr());
        }
    }

    /// Returns the audio settings used to create this simulator.
    pub const fn audio_settings(&self) -> AudioSettings {
        self.audio_settings
    }
}

impl<T, R, P, RE> StaticSimulator<T, Direct, R, P, RE>
where
    T: RayTracer,
    R: 'static,
    P: 'static,
    RE: 'static,
{
    /// Specifies direct simulation parameters that are not associated with any particular source.
    ///
    /// Convenience method abstracting the more expressive [`Self::set_shared_inputs_subset`].
    /// See the [module-level documentation](crate::simulation) for threading guidelines.
    ///
    /// # Examples
    ///
    /// ```
    /// # use audionimbus::*;
    /// # let context = Context::default();
    /// # let audio_settings = AudioSettings::default();
    /// # let simulation_settings = SimulationSettings::new(&audio_settings)
    /// #     .with_direct(DirectSimulationSettings {
    /// #         max_num_occlusion_samples: 4,
    /// #     });
    /// # let simulator = Simulator::try_new(&context, &simulation_settings)?;
    /// let shared_inputs = SimulationSharedInputs::new(CoordinateSystem::default());
    /// simulator.set_shared_direct_inputs(&shared_inputs)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Arguments
    ///
    /// - `shared_inputs`: the shared input parameters to set.
    ///
    /// # Errors
    ///
    /// Returns [`ParameterValidationError`] if any parameters exceed the maximums set during
    /// simulator initialization.
    pub fn set_shared_direct_inputs<InD, InR, InP>(
        &self,
        shared_inputs: &StaticSimulationSharedInputs<InD, InR, InP>,
    ) -> Result<(), ParameterValidationError>
    where
        InD: DirectCompatible<Direct> + SimulationFlagsProvider,
        InR: ReflectionsCompatible<R> + SimulationFlagsProvider,
        InP: PathingCompatible<P> + SimulationFlagsProvider,
        (): ReflectionsCompatible<R> + ReflectionsCompatible<InR>,
        (): PathingCompatible<P> + PathingCompatible<InP>,
    {
        self.set_shared_inputs_subset::<Direct, (), (), InD, InR, InP>(shared_inputs)
    }

    /// Runs a direct simulation for all sources added to the simulator.
    ///
    /// This may include distance attenuation, air absorption, directivity, occlusion, and transmission.
    ///
    /// # Performance Considerations
    ///
    /// This function should not be called from the audio processing thread if occlusion
    /// and/or transmission are enabled, as these calculations can be CPU-intensive.
    pub fn run_direct(&self) {
        self.run_direct_shared()
    }
}

impl<T, D, P, RE> StaticSimulator<T, D, Reflections, P, RE>
where
    T: RayTracer,
    D: 'static,
    P: 'static,
    RE: 'static,
{
    /// Specifies reflections simulation parameters that are not associated with any particular source.
    ///
    /// Convenience method abstracting the more expressive [`Self::set_shared_inputs_subset`].
    /// See the [module-level documentation](crate::simulation) for threading guidelines.
    ///
    /// # Examples
    ///
    /// ```
    /// # use audionimbus::*;
    /// # let context = Context::default();
    /// # let audio_settings = AudioSettings::default();
    /// # let simulation_settings = SimulationSettings::new(&audio_settings)
    /// #     .with_reflections(ConvolutionSettings {
    /// #         max_num_rays: 4096,
    /// #         num_diffuse_samples: 32,
    /// #         max_duration: 2.0,
    /// #         max_num_sources: 8,
    /// #         num_threads: 1,
    /// #         max_order: 1,
    /// #     });
    /// # let simulator = Simulator::try_new(&context, &simulation_settings)?;
    /// let shared_inputs = SimulationSharedInputs::new(CoordinateSystem::default())
    ///     .with_reflections(ReflectionsSharedInputs {
    ///         num_rays: 4096,
    ///         num_bounces: 16,
    ///         duration: 2.0,
    ///         order: 1,
    ///         irradiance_min_distance: 1.0,
    ///     });
    /// simulator.set_shared_reflections_inputs(&shared_inputs)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Arguments
    ///
    /// - `shared_inputs`: the shared input parameters to set.
    ///
    /// # Errors
    ///
    /// Returns [`ParameterValidationError`] if any parameters exceed the maximums set during
    /// simulator initialization.
    pub fn set_shared_reflections_inputs<InD, InP>(
        &self,
        shared_inputs: &StaticSimulationSharedInputs<InD, Reflections, InP>,
    ) -> Result<(), ParameterValidationError>
    where
        InD: DirectCompatible<D> + SimulationFlagsProvider,
        InP: PathingCompatible<P> + SimulationFlagsProvider,
        (): DirectCompatible<D> + DirectCompatible<InD>,
        (): PathingCompatible<P> + PathingCompatible<InP>,
    {
        self.set_shared_inputs_subset::<(), Reflections, (), InD, Reflections, InP>(shared_inputs)
    }

    /// Runs a reflections simulation for all sources added to the simulator.
    ///
    /// # Performance Considerations
    ///
    /// This function is CPU-intensive and should be called from a dedicated simulation thread
    /// to avoid blocking either the audio processing thread or the game's main update thread.
    ///
    /// # Errors
    ///
    /// Returns [`SimulationError::ReflectionsWithoutScene`] if no scene was set.
    ///
    /// Reflection simulation requires a [`Scene`] to be set on the simulator via
    /// `Simulator::set_scene` and committed via `Simulator::commit` before
    /// running simulations.
    pub fn run_reflections(&self) -> Result<(), SimulationError> {
        self.run_reflections_shared()
    }
}

impl<T, D, R, RE> StaticSimulator<T, D, R, Pathing, RE>
where
    T: RayTracer,
    D: 'static,
    R: 'static,
    RE: 'static,
{
    /// Specifies pathing simulation parameters that are not associated with any particular source.
    ///
    /// Convenience method abstracting the more expressive [`Self::set_shared_inputs_subset`].
    /// See the [module-level documentation](crate::simulation) for threading guidelines.
    ///
    /// # Examples
    ///
    /// ```
    /// # use audionimbus::*;
    /// # let context = Context::default();
    /// # let audio_settings = AudioSettings::default();
    /// # let simulation_settings = SimulationSettings::new(&audio_settings)
    /// #     .with_pathing(PathingSimulationSettings {
    /// #         num_visibility_samples: 4,
    /// #     });
    /// # let simulator = Simulator::try_new(&context, &simulation_settings)?;
    /// let shared_inputs = SimulationSharedInputs::new(CoordinateSystem::default());
    /// simulator.set_shared_pathing_inputs(&shared_inputs)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Arguments
    ///
    /// - `shared_inputs`: the shared input parameters to set.
    ///
    /// # Errors
    ///
    /// Returns [`ParameterValidationError`] if any parameters exceed the maximums set during
    /// simulator initialization.
    pub fn set_shared_pathing_inputs<InD, InR, InP>(
        &self,
        shared_inputs: &StaticSimulationSharedInputs<InD, InR, InP>,
    ) -> Result<(), ParameterValidationError>
    where
        InD: DirectCompatible<D> + SimulationFlagsProvider,
        InR: ReflectionsCompatible<R> + SimulationFlagsProvider,
        InP: PathingCompatible<Pathing> + SimulationFlagsProvider,
        (): DirectCompatible<D> + DirectCompatible<InD>,
        (): ReflectionsCompatible<R> + ReflectionsCompatible<InR>,
    {
        self.set_shared_inputs_subset::<(), (), Pathing, InD, InR, InP>(shared_inputs)
    }

    /// Runs a pathing simulation for all sources added to the simulator.
    ///
    /// # Performance Considerations
    ///
    /// This function is CPU-intensive and should be called from a dedicated simulation thread
    /// to avoid blocking either the audio processing thread or the game's main update thread.
    ///
    /// # Errors
    ///
    /// Returns [`SimulationError::PathingWithoutProbes`] if no probes were committed.
    ///
    /// Pathing requires at least one probe batch to be added to the simulator
    /// via `Simulator::add_probe_batch` and committed via `Simulator::commit` before running
    /// simulations.
    pub fn run_pathing(&self) -> Result<(), SimulationError> {
        self.run_pathing_shared()
    }
}

impl<B: SimulatorBinding> Drop for Simulator<B> {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplSimulatorRelease(&raw mut self.inner) }
    }
}

unsafe impl<T: RayTracer, D, R, P, RE> Send for StaticSimulator<T, D, R, P, RE> {}

#[cfg(feature = "dynamic")]
unsafe impl Send for dynamic::Simulator {}

unsafe impl<T: RayTracer, D, R, P, RE> Sync for StaticSimulator<T, D, R, P, RE> {}

#[cfg(feature = "dynamic")]
unsafe impl Sync for dynamic::Simulator {}

impl<T, D, R, P, RE> Clone for StaticSimulator<T, D, R, P, RE>
where
    T: RayTracer,
    D: 'static,
    R: 'static,
    P: 'static,
    RE: 'static,
{
    /// Retains an additional reference to the simulator.
    ///
    /// The returned [`crate::Simulator`] shares the same underlying Steam Audio object.
    fn clone(&self) -> Self {
        self.clone_shared()
    }
}

impl<T, D, R, P, RE> PartialEq for StaticSimulator<T, D, R, P, RE>
where
    T: RayTracer,
    D: 'static,
    R: 'static,
    P: 'static,
    RE: 'static,
{
    fn eq(&self, other: &Self) -> bool {
        self.raw_ptr() == other.raw_ptr()
    }
}

impl<T, D, R, P, RE> Eq for StaticSimulator<T, D, R, P, RE>
where
    T: RayTracer,
    D: 'static,
    R: 'static,
    P: 'static,
    RE: 'static,
{
}

impl<T, D, R, P, RE> Hash for StaticSimulator<T, D, R, P, RE>
where
    T: RayTracer,
    D: 'static,
    R: 'static,
    P: 'static,
    RE: 'static,
{
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::ptr::hash(self.raw_ptr(), state);
    }
}

impl<B: SimulatorBinding> Simulator<B> {
    /// Creates the native simulator and its mode locks.
    ///
    /// # Errors
    ///
    /// - [`SteamAudioError::Unspecified`] if the native operation fails.
    /// - [`SteamAudioError::OutOfMemory`] if allocation fails.
    /// - [`SteamAudioError::Initialization`] if a native dependency fails.
    fn try_new_shared(
        context: &Context,
        settings: &SimulationSettings<B>,
        flags: SimulationFlags,
    ) -> Result<Self, SteamAudioError> {
        let direct_lock = if flags.contains(SimulationFlags::DIRECT) {
            Some(Arc::new(Mutex::new(())))
        } else {
            None
        };

        let reflections_lock = if flags.contains(SimulationFlags::REFLECTIONS) {
            Some(Arc::new(Mutex::new(())))
        } else {
            None
        };

        let pathing_lock = if flags.contains(SimulationFlags::PATHING) {
            Some(Arc::new(Mutex::new(())))
        } else {
            None
        };

        let mut ffi_settings = settings.to_ffi();
        let (binding, open_cl_device, radeon_rays_device, true_audio_next_device) =
            settings.simulator_parts();

        let mut simulator = Self {
            inner: std::ptr::null_mut(),
            binding: binding.clone(),
            // Safety: thread safety is upheld by the `unsafe impl Send + Sync` on `StaticSimulator`.
            #[allow(clippy::arc_with_non_send_sync)]
            shared: Arc::new(Mutex::new(SimulatorShared::default())),
            audio_settings: settings.audio_settings(),
            max_num_occlusion_samples: (ffi_settings.flags.0
                & audionimbus_sys::IPLSimulationFlags::IPL_SIMULATIONFLAGS_DIRECT.0
                != 0)
                .then_some(ffi_settings.maxNumOcclusionSamples as u32),
            max_num_rays: (ffi_settings.flags.0
                & audionimbus_sys::IPLSimulationFlags::IPL_SIMULATIONFLAGS_REFLECTIONS.0
                != 0)
                .then_some(ffi_settings.maxNumRays as u32),
            max_duration: (ffi_settings.flags.0
                & audionimbus_sys::IPLSimulationFlags::IPL_SIMULATIONFLAGS_REFLECTIONS.0
                != 0)
                .then_some(ffi_settings.maxDuration),
            direct_lock,
            reflections_lock,
            pathing_lock,
            _open_cl_device: open_cl_device.clone(),
            _radeon_rays_device: radeon_rays_device.clone(),
            _true_audio_next_device: true_audio_next_device.clone(),
        };

        let status = unsafe {
            audionimbus_sys::iplSimulatorCreate(
                context.raw_ptr(),
                &mut ffi_settings,
                &raw mut simulator.inner,
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        Ok(simulator)
    }

    /// Specifies the scene within which all subsequent simulations should be run.
    ///
    /// Call [`Self::commit`] after calling this function for the changes to take effect.
    ///
    /// This function cannot be called while any simulation is running. Either will block until the
    /// other finishes.
    fn set_scene_shared(&mut self, scene: &BoundScene<B::Geometry>) {
        let previous_pending_scene = {
            let mut shared = self.shared.lock().unwrap();
            let is_noop = shared.pending_scene.as_ref() == Some(scene)
                || (shared.pending_scene.is_none()
                    && shared.committed_scene.as_ref() == Some(scene));

            if is_noop {
                return;
            }

            shared.pending_scene.replace(scene.clone())
        };

        let _guards = self.acquire_all_locks();
        let simulator = self.raw_ptr();
        let lock_handles = self.lock_handles();

        if let Some(previous_pending_scene) = previous_pending_scene {
            previous_pending_scene.unregister_simulator(simulator, 1);
        }

        scene.register_simulator(simulator, &lock_handles, 1);

        unsafe { audionimbus_sys::iplSimulatorSetScene(self.raw_ptr(), scene.raw_ptr_shared()) }
    }

    /// Adds a probe batch for use in subsequent simulations.
    /// Sources that require baked data can then use the data contained in the specified probe batch.
    ///
    /// Call [`Self::commit`] after calling this function for the changes to take effect.
    ///
    /// This function cannot be called while any simulation is running. Either will block until the
    /// other finishes.
    pub fn add_probe_batch(&mut self, probe_batch: &ProbeBatch) {
        let _guards = self.acquire_all_locks();
        let raw_ptr = probe_batch.raw_ptr();

        unsafe {
            audionimbus_sys::iplSimulatorAddProbeBatch(self.raw_ptr(), probe_batch.raw_ptr());
        }

        let mut shared = self.shared.lock().unwrap();
        shared
            .pending_probe_batches
            .insert(raw_ptr, probe_batch.committed_num_probes());
    }

    /// Removes a probe batch from use in subsequent simulations.
    /// Sources that require baked data will then stop using the data contained in the specified probe batch.
    ///
    /// Call [`Self::commit`] after calling this function for the changes to take effect.
    ///
    /// This function cannot be called while any simulation is running. Either will block until the
    /// other finishes.
    pub fn remove_probe_batch(&mut self, probe_batch: &ProbeBatch) {
        let _guards = self.acquire_all_locks();
        let raw_ptr = probe_batch.raw_ptr();

        unsafe {
            audionimbus_sys::iplSimulatorRemoveProbeBatch(self.raw_ptr(), probe_batch.raw_ptr());
        }

        let mut shared = self.shared.lock().unwrap();
        shared.pending_probe_batches.remove(&raw_ptr);
    }

    /// Commits changes to the scene or probe batches used for simulation.
    ///
    /// Call this function after calling [`Self::set_scene`], [`Self::add_probe_batch`], or [`Self::remove_probe_batch`] for the changes to take effect.
    ///
    /// This function cannot be called while any simulation is running. Either will block until the
    /// other finishes.
    pub fn commit(&self) {
        let _guards = self.acquire_all_locks();
        let simulator = self.raw_ptr();

        unsafe { audionimbus_sys::iplSimulatorCommit(self.raw_ptr()) }

        let previous_committed_scene = {
            let mut shared = self.shared.lock().unwrap();
            shared.committed_num_probes = shared.pending_probe_batches.values().sum();

            if let Some(pending_scene) = shared.pending_scene.take() {
                shared.committed_scene.replace(pending_scene)
            } else {
                None
            }
        };

        if let Some(previous_committed_scene) = previous_committed_scene {
            previous_committed_scene.unregister_simulator(simulator, 1);
        }
    }

    /// Acquires locks for the simulation types specified in the given flags.
    fn acquire_locks_for_flags(&self, flags: SimulationFlags) -> Vec<MutexGuard<'_, ()>> {
        let mut locks = Vec::new();

        if flags.contains(SimulationFlags::DIRECT)
            && let Some(lock) = &self.direct_lock
        {
            locks.push(lock);
        }

        if flags.contains(SimulationFlags::REFLECTIONS)
            && let Some(lock) = &self.reflections_lock
        {
            locks.push(lock);
        }

        if flags.contains(SimulationFlags::PATHING)
            && let Some(lock) = &self.pathing_lock
        {
            locks.push(lock);
        }

        // Match scene commit lock ordering to avoid deadlocks.
        locks.sort_unstable_by_key(|lock| Arc::as_ptr(lock) as usize);
        locks.into_iter().map(|lock| lock.lock().unwrap()).collect()
    }

    /// Acquires locks for all simulation types enabled on this simulator.
    fn acquire_all_locks(&self) -> Vec<MutexGuard<'_, ()>> {
        let mut locks: Vec<_> = [
            self.direct_lock.as_ref(),
            self.reflections_lock.as_ref(),
            self.pathing_lock.as_ref(),
        ]
        .into_iter()
        .flatten()
        .collect();
        // Match scene commit lock ordering to avoid deadlocks.
        locks.sort_unstable_by_key(|lock| Arc::as_ptr(lock) as usize);
        locks.into_iter().map(|lock| lock.lock().unwrap()).collect()
    }

    /// Returns this simulator's simulation locks in a stable order.
    fn lock_handles(&self) -> Vec<SimulationLock> {
        let mut locks: Vec<_> = [
            self.direct_lock.as_ref(),
            self.reflections_lock.as_ref(),
            self.pathing_lock.as_ref(),
        ]
        .into_iter()
        .flatten()
        .cloned()
        .collect();
        locks.sort_unstable_by_key(|lock| Arc::as_ptr(lock) as usize);
        locks
    }

    /// Validates shared simulation inputs against the limits set during initialization.
    fn validate_shared_inputs<I>(
        &self,
        shared_inputs: &SimulationSharedInputs<I>,
    ) -> Result<(), ParameterValidationError> {
        let Some(reflections_inputs) = &shared_inputs.reflections_shared_inputs else {
            return Ok(());
        };

        // Validate num_rays.
        if let Some(max) = self.max_num_rays
            && reflections_inputs.num_rays > max
        {
            return Err(ParameterValidationError::NumRaysExceedsMax {
                requested: reflections_inputs.num_rays,
                max,
            });
        }

        // Validate duration.
        if let Some(max) = self.max_duration
            && reflections_inputs.duration > max
        {
            return Err(ParameterValidationError::DurationExceedsMax {
                requested: reflections_inputs.duration,
                max,
            });
        }

        Ok(())
    }

    /// Returns the raw FFI pointer to the underlying simulator.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub const fn raw_ptr(&self) -> audionimbus_sys::IPLSimulator {
        self.inner
    }

    /// Validates and publishes shared inputs for the selected modes.
    ///
    /// # Errors
    ///
    /// - [`ParameterValidationError::NumRaysExceedsMax`] if ray counts exceed the creation limit.
    /// - [`ParameterValidationError::DurationExceedsMax`] if duration exceeds the creation limit.
    fn set_shared_inputs_shared<I>(
        &self,
        simulation_flags: SimulationFlags,
        shared_inputs: &SimulationSharedInputs<I>,
    ) -> Result<(), ParameterValidationError> {
        self.validate_shared_inputs(shared_inputs)?;

        let _guards = self.acquire_locks_for_flags(simulation_flags);
        B::retain_callback(
            &self.shared,
            simulation_flags,
            shared_inputs.pathing_visualization_callback.as_ref(),
        );

        unsafe {
            audionimbus_sys::iplSimulatorSetSharedInputs(
                self.raw_ptr(),
                simulation_flags.into(),
                &mut audionimbus_sys::IPLSimulationSharedInputs::from(shared_inputs),
            );
        }

        Ok(())
    }

    /// Runs the native direct simulation under its lock.
    fn run_direct_shared(&self) {
        let _guard = self
            .direct_lock
            .as_ref()
            .expect("direct_lock must exist when direct simulation is enabled")
            .lock()
            .unwrap();

        unsafe {
            audionimbus_sys::iplSimulatorRunDirect(self.raw_ptr());
        }
    }

    /// Runs the native reflections simulation under its lock.
    ///
    /// # Errors
    ///
    /// - [`SimulationError::ReflectionsWithoutScene`] if no scene was committed.
    fn run_reflections_shared(&self) -> Result<(), SimulationError> {
        let _guard = self
            .reflections_lock
            .as_ref()
            .expect("reflections_lock must exist when reflections simulation is enabled")
            .lock()
            .unwrap();

        let shared = self.shared.lock().unwrap();
        if shared.committed_scene.is_none() {
            return Err(SimulationError::ReflectionsWithoutScene);
        }

        unsafe {
            audionimbus_sys::iplSimulatorRunReflections(self.raw_ptr());
        }

        Ok(())
    }

    /// Runs the native pathing simulation under its lock.
    ///
    /// # Errors
    ///
    /// - [`SimulationError::PathingWithoutProbes`] if no probes were committed.
    fn run_pathing_shared(&self) -> Result<(), SimulationError> {
        let _guard = self
            .pathing_lock
            .as_ref()
            .expect("pathing_lock must exist when pathing simulation is enabled")
            .lock()
            .unwrap();

        let shared = self.shared.lock().unwrap();
        if shared.committed_num_probes == 0 {
            return Err(SimulationError::PathingWithoutProbes);
        }

        unsafe {
            audionimbus_sys::iplSimulatorRunPathing(self.raw_ptr());
        }

        Ok(())
    }
}

impl<B: SimulatorBinding> Simulator<B> {
    /// Retains the native simulator and its shared resources.
    fn clone_shared(&self) -> Self {
        // SAFETY: The simulator will not be destroyed until all references are released.
        Self {
            binding: self.binding.clone(),
            inner: unsafe { audionimbus_sys::iplSimulatorRetain(self.inner) },
            shared: Arc::clone(&self.shared),
            audio_settings: self.audio_settings,
            max_num_occlusion_samples: self.max_num_occlusion_samples,
            max_num_rays: self.max_num_rays,
            max_duration: self.max_duration,
            direct_lock: self.direct_lock.clone(),
            reflections_lock: self.reflections_lock.clone(),
            pathing_lock: self.pathing_lock.clone(),
            _open_cl_device: self._open_cl_device.clone(),
            _radeon_rays_device: self._radeon_rays_device.clone(),
            _true_audio_next_device: self._true_audio_next_device.clone(),
        }
    }
}

#[cfg(feature = "dynamic")]
impl DynamicSimulator {
    /// Creates a simulator with the selected modes, backend, and algorithm.
    ///
    /// # Errors
    ///
    /// - [`dynamic::SimulationError::DeviceMismatch`] for conflicting Radeon Rays and TAN devices.
    /// - [`dynamic::SimulationError::SteamAudio`] if native creation fails.
    pub fn try_new(
        context: &Context,
        settings: &dynamic::SimulationSettings,
    ) -> Result<Self, dynamic::SimulationError> {
        let (binding, open_cl_device, _, true_audio_next_device) = settings.simulator_parts();
        if let Some(ray_open_cl_device) = &binding.0.ray_open_cl_device
            && true_audio_next_device.is_some()
            && open_cl_device.as_ref() != Some(ray_open_cl_device)
        {
            return Err(dynamic::SimulationError::DeviceMismatch);
        }

        Self::try_new_shared(context, settings, settings.flags())
            .map_err(dynamic::SimulationError::SteamAudio)
    }

    /// Returns the available simulation modes.
    pub const fn flags(&self) -> SimulationFlags {
        self.binding.0.flags
    }

    /// Returns the selected reflection algorithm.
    pub const fn reflection_algorithm(&self) -> Option<ReflectionAlgorithm> {
        self.binding.0.reflection_algorithm
    }

    /// Selects a compatible scene for the next commit.
    ///
    /// # Errors
    ///
    /// - [`dynamic::SimulationError::RayTracerMismatch`] for a different scene backend.
    /// - [`dynamic::SimulationError::DeviceMismatch`] for different Radeon Rays devices.
    pub fn try_set_scene(
        &mut self,
        scene: &dynamic::Scene,
    ) -> Result<(), dynamic::SimulationError> {
        let expected = self.binding.0.ray_tracer;
        let actual = scene.ray_tracer();
        if actual != expected {
            return Err(dynamic::SimulationError::RayTracerMismatch { expected, actual });
        }
        if !scene.shared.lock().unwrap().uses_devices(
            self._open_cl_device.as_ref(),
            self._radeon_rays_device.as_ref(),
        ) {
            return Err(dynamic::SimulationError::DeviceMismatch);
        }
        self.set_scene_shared(scene);

        Ok(())
    }

    /// Adds a source from this simulator to the next commit.
    ///
    /// # Errors
    ///
    /// - [`dynamic::SimulationError::ForeignSource`] if another simulator created the source.
    pub fn try_add_source(&self, source: &dynamic::Source) -> Result<(), dynamic::SimulationError> {
        let creator = source.creator();
        if !creator.ptr_eq(&Arc::downgrade(&self.shared)) {
            return Err(dynamic::SimulationError::ForeignSource);
        }
        self.add_source_shared(source);

        Ok(())
    }

    /// Removes a source from this simulator at the next commit.
    ///
    /// # Errors
    ///
    /// - [`dynamic::SimulationError::ForeignSource`] if another simulator created the source.
    pub fn try_remove_source(
        &self,
        source: &dynamic::Source,
    ) -> Result<(), dynamic::SimulationError> {
        let creator = source.creator();
        if !creator.ptr_eq(&Arc::downgrade(&self.shared)) {
            return Err(dynamic::SimulationError::ForeignSource);
        }
        self.remove_source_shared(source);

        Ok(())
    }

    /// Publishes shared inputs for selected available modes.
    ///
    /// # Errors
    ///
    /// - [`dynamic::SimulationError::UnknownFlags`] for unknown mode bits.
    /// - [`dynamic::SimulationError::DisabledModes`] for unavailable modes.
    /// - [`dynamic::SimulationError::MissingInputs`] for missing selected reflection inputs.
    /// - [`dynamic::SimulationError::ParameterValidation`] if inputs exceed creation limits.
    pub fn set_shared_inputs(
        &self,
        flags: SimulationFlags,
        inputs: &dynamic::SimulationSharedInputs,
    ) -> Result<(), dynamic::SimulationError> {
        let unknown = flags.bits() & !SimulationFlags::all().bits();
        if unknown != 0 {
            return Err(dynamic::SimulationError::UnknownFlags(unknown));
        }
        let disabled = flags & !self.flags();
        if !disabled.is_empty() {
            return Err(dynamic::SimulationError::DisabledModes(disabled));
        }
        if flags.contains(SimulationFlags::REFLECTIONS)
            && inputs.reflections_shared_inputs.is_none()
        {
            return Err(dynamic::SimulationError::MissingInputs(
                SimulationFlags::REFLECTIONS,
            ));
        }

        self.set_shared_inputs_shared(flags, inputs)
            .map_err(dynamic::SimulationError::ParameterValidation)
    }

    /// Runs direct simulation for the registered sources.
    ///
    /// # Errors
    ///
    /// - [`dynamic::SimulationError::DisabledModes`] if direct simulation is unavailable.
    pub fn run_direct(&self) -> Result<(), dynamic::SimulationError> {
        if !self.flags().contains(SimulationFlags::DIRECT) {
            return Err(dynamic::SimulationError::DisabledModes(
                SimulationFlags::DIRECT,
            ));
        }
        self.run_direct_shared();

        Ok(())
    }

    /// Runs reflection simulation for the registered sources.
    ///
    /// # Errors
    ///
    /// - [`dynamic::SimulationError::DisabledModes`] if reflection simulation is unavailable.
    /// - [`dynamic::SimulationError::Simulation`] if no scene was committed.
    pub fn run_reflections(&self) -> Result<(), dynamic::SimulationError> {
        if !self.flags().contains(SimulationFlags::REFLECTIONS) {
            return Err(dynamic::SimulationError::DisabledModes(
                SimulationFlags::REFLECTIONS,
            ));
        }

        self.run_reflections_shared()
            .map_err(dynamic::SimulationError::Simulation)
    }

    /// Runs pathing simulation for the registered sources.
    ///
    /// # Errors
    ///
    /// - [`dynamic::SimulationError::DisabledModes`] if pathing simulation is unavailable.
    /// - [`dynamic::SimulationError::Simulation`] if no probes were committed.
    pub fn run_pathing(&self) -> Result<(), dynamic::SimulationError> {
        if !self.flags().contains(SimulationFlags::PATHING) {
            return Err(dynamic::SimulationError::DisabledModes(
                SimulationFlags::PATHING,
            ));
        }

        self.run_pathing_shared()
            .map_err(dynamic::SimulationError::Simulation)
    }
}

#[cfg(feature = "dynamic")]
impl Clone for DynamicSimulator {
    fn clone(&self) -> Self {
        self.clone_shared()
    }
}

#[cfg(feature = "dynamic")]
impl PartialEq for DynamicSimulator {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

#[cfg(feature = "dynamic")]
impl Eq for DynamicSimulator {}

#[cfg(feature = "dynamic")]
impl Hash for DynamicSimulator {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        std::ptr::hash(self.inner, hasher);
    }
}

/// Simulation parameters that are not specific to any source.
pub type StaticSimulationSharedInputs<D = (), R = (), P = ()> =
    SimulationSharedInputs<Static<(D, R, P)>>;

/// Listener coordinates and shared runtime simulation inputs.
#[cfg(feature = "dynamic")]
pub type DynamicSimulationSharedInputs = SimulationSharedInputs<Dynamic<()>>;

/// Storage shared by static and runtime shared inputs.
#[derive(Default, Debug)]
pub struct SimulationSharedInputs<B> {
    /// The position and orientation of the listener.
    listener: CoordinateSystem,

    /// Reflections shared inputs.
    reflections_shared_inputs: Option<ReflectionsSharedInputs>,

    /// Optional callback for visualizing valid path segments during call to `Simulator::run_pathing`.
    pathing_visualization_callback: Option<PathingVisualizationCallback>,

    /// The compile-time or runtime simulation binding.
    _binding: B,
}

impl StaticSimulationSharedInputs {
    /// Creates new [`crate::SimulationSharedInputs`].
    ///
    /// # Arguments
    ///
    /// - `listener`: The position and orientation of the listener.
    pub fn new(listener: CoordinateSystem) -> Self {
        Self {
            listener,
            _binding: Static::new(),
            reflections_shared_inputs: None,
            pathing_visualization_callback: None,
        }
    }
}

impl<D, R, P> StaticSimulationSharedInputs<D, R, P> {
    /// Returns shared simulation inputs with direct simulation.
    pub fn with_direct(self) -> StaticSimulationSharedInputs<Direct, R, P> {
        let Self {
            listener,
            reflections_shared_inputs,
            pathing_visualization_callback,
            ..
        } = self;

        StaticSimulationSharedInputs {
            _binding: Static::new(),
            listener,
            reflections_shared_inputs,
            pathing_visualization_callback,
        }
    }

    /// Sets the reflections simulation values of the shared inputs.
    pub fn with_reflections(
        self,
        inputs: ReflectionsSharedInputs,
    ) -> StaticSimulationSharedInputs<D, Reflections, P> {
        let Self {
            listener,
            pathing_visualization_callback,
            ..
        } = self;

        StaticSimulationSharedInputs {
            _binding: Static::new(),
            listener,
            reflections_shared_inputs: Some(inputs),
            pathing_visualization_callback,
        }
    }

    /// Sets the pathing simulation values of the shared inputs.
    pub fn with_pathing(self) -> StaticSimulationSharedInputs<D, R, Pathing> {
        let Self {
            listener,
            reflections_shared_inputs,
            pathing_visualization_callback,
            ..
        } = self;

        StaticSimulationSharedInputs {
            _binding: Static::new(),
            listener,
            reflections_shared_inputs,
            pathing_visualization_callback,
        }
    }

    /// Sets the pathing simulation values of the shared inputs, with a pathing visualization
    /// callback.
    ///
    /// # Arguments
    ///
    /// - `pathing_visualization_callback`: Callback for visualizing valid path segments during call to `Simulator::run_pathing`.
    pub fn with_pathing_visualization_callback(
        self,
        pathing_visualization_callback: PathingVisualizationCallback,
    ) -> StaticSimulationSharedInputs<D, R, Pathing> {
        let Self {
            listener,
            reflections_shared_inputs,
            ..
        } = self;

        StaticSimulationSharedInputs {
            _binding: Static::new(),
            listener,
            reflections_shared_inputs,
            pathing_visualization_callback: Some(pathing_visualization_callback),
        }
    }
}

impl<D, P> StaticSimulationSharedInputs<D, Reflections, P> {
    /// Sets the shared inputs to use for reflections simulation.
    pub fn set_reflections_shared_inputs(&mut self, shared_inputs: ReflectionsSharedInputs) {
        self.reflections_shared_inputs.replace(shared_inputs);
    }
}

impl<D, R> StaticSimulationSharedInputs<D, R, Pathing> {
    /// Sets the visualization callback to use for pathing simulation.
    pub fn set_pathing_visualization_callback(
        &mut self,
        visualization_callback: PathingVisualizationCallback,
    ) {
        self.pathing_visualization_callback
            .replace(visualization_callback);
    }
}

impl<B> SimulationSharedInputs<B> {
    /// Sets the position and orientation of the listener.
    pub const fn set_listener(&mut self, listener: CoordinateSystem) {
        self.listener = listener;
    }
}

impl<B> From<&SimulationSharedInputs<B>> for audionimbus_sys::IPLSimulationSharedInputs {
    fn from(simulation_shared_inputs: &SimulationSharedInputs<B>) -> Self {
        let (pathing_visualization_callback, pathing_user_data) = simulation_shared_inputs
            .pathing_visualization_callback
            .as_ref()
            .map_or((None, std::ptr::null_mut()), |callback| {
                let (callback_fn, user_data) = callback.as_raw_parts();
                (Some(callback_fn), user_data)
            });

        let (num_rays, num_bounces, duration, order, irradiance_min_distance) =
            simulation_shared_inputs.reflections_shared_inputs.map_or(
                (0, 0, 0.0, 0, 0.0),
                |inputs| {
                    (
                        inputs.num_rays as i32,
                        inputs.num_bounces as i32,
                        inputs.duration,
                        inputs.order as i32,
                        inputs.irradiance_min_distance,
                    )
                },
            );

        Self {
            listener: simulation_shared_inputs.listener.into(),
            numRays: num_rays,
            numBounces: num_bounces,
            duration,
            order,
            irradianceMinDistance: irradiance_min_distance,
            pathingVisCallback: pathing_visualization_callback,
            pathingUserData: pathing_user_data,
        }
    }
}

#[cfg(feature = "dynamic")]
impl DynamicSimulationSharedInputs {
    /// Creates shared inputs for the supplied listener.
    pub fn new(listener: CoordinateSystem) -> Self {
        Self {
            listener,
            reflections_shared_inputs: None,
            pathing_visualization_callback: None,
            _binding: Dynamic(()),
        }
    }

    /// Adds shared reflection inputs.
    pub fn with_reflections(mut self, inputs: ReflectionsSharedInputs) -> Self {
        self.reflections_shared_inputs = Some(inputs);

        self
    }

    /// Adds a path visualization callback.
    pub fn with_pathing_visualization_callback(
        mut self,
        callback: PathingVisualizationCallback,
    ) -> Self {
        self.pathing_visualization_callback = Some(callback);

        self
    }

    /// Sets shared reflection inputs.
    pub fn set_reflections_shared_inputs(&mut self, inputs: ReflectionsSharedInputs) {
        self.reflections_shared_inputs = Some(inputs);
    }

    /// Sets the path visualization callback.
    pub fn set_pathing_visualization_callback(&mut self, callback: PathingVisualizationCallback) {
        self.pathing_visualization_callback = Some(callback);
    }
}

impl<B: Clone> SimulationSharedInputs<B> {
    /// Retains the shared input callback and copies the listener inputs.
    fn clone_shared(&self) -> Self {
        Self {
            listener: self.listener,
            reflections_shared_inputs: self.reflections_shared_inputs,
            pathing_visualization_callback: self.pathing_visualization_callback.clone(),
            _binding: self._binding.clone(),
        }
    }
}

impl<D: Clone, R: Clone, P: Clone> Clone for StaticSimulationSharedInputs<D, R, P> {
    fn clone(&self) -> Self {
        self.clone_shared()
    }
}

#[cfg(feature = "dynamic")]
impl Clone for DynamicSimulationSharedInputs {
    fn clone(&self) -> Self {
        self.clone_shared()
    }
}

/// Reflections shared inputs.
///
/// Used as an argument to `SimulationSharedInputs::with_reflections`.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct ReflectionsSharedInputs {
    /// The number of rays to trace from the listener.
    /// Increasing this value results in more accurate reflections, at the cost of increased CPU usage.
    pub num_rays: u32,

    /// The number of times each ray traced from the listener is reflected when it encounters a solid object.
    /// Increasing this value results in longer, more accurate reverb tails, at the cost of increased CPU usage during simulation.
    pub num_bounces: u32,

    /// The duration (in seconds) of the impulse responses generated when simulating reflections.
    /// Increasing this value results in longer, more accurate reverb tails, at the cost of increased CPU usage during audio processing.
    pub duration: f32,

    /// The Ambisonic order of the impulse responses generated when simulating reflections.
    /// Increasing this value results in more accurate directional variation of reflected sound, at the cost of increased CPU usage during audio processing.
    pub order: u32,

    /// When calculating how much sound energy reaches a surface directly from a source, any source that is closer than [`Self::irradiance_min_distance`] to the surface is assumed to be at a distance of [`Self::irradiance_min_distance`], for the purposes of energy calculations.
    pub irradiance_min_distance: f32,
}

impl<B: SimulatorBinding> Simulator<B> {
    /// Returns the limits and locks retained by a new source.
    ///
    /// These locks must stay internal to preserve scene commit lock ordering.
    pub(crate) fn source_parts(&self) -> (Option<u32>, [Option<SimulationLock>; 3]) {
        (
            self.max_num_occlusion_samples,
            [
                self.direct_lock.clone(),
                self.reflections_lock.clone(),
                self.pathing_lock.clone(),
            ],
        )
    }
}

#[cfg(feature = "dynamic")]
impl DynamicSimulator {
    /// Returns the creator identity and devices retained by a new runtime source.
    ///
    /// Creator storage must stay internal to preserve simulator registration lock ordering.
    pub(crate) fn source_resources(&self) -> DynamicSourceResources {
        DynamicSourceResources {
            simulator: Arc::downgrade(&self.shared),
            true_audio_next_device: self._true_audio_next_device.clone(),
            ..Default::default()
        }
    }
}

/// Errors from runtime simulation construction, inputs, and output retrieval.
#[cfg(feature = "dynamic")]
#[derive(Debug)]
pub enum DynamicSimulationError {
    /// A native operation failed.
    SteamAudio(crate::SteamAudioError),
    /// Simulation parameters exceed creation limits.
    ParameterValidation(crate::ParameterValidationError),
    /// Simulation requires a committed scene or probes.
    Simulation(crate::SimulationError),
    /// Requested modes are unavailable.
    DisabledModes(SimulationFlags),
    /// Requested flags contain unknown bits.
    UnknownFlags(u32),
    /// Required shared inputs are absent for these modes.
    MissingInputs(SimulationFlags),
    /// The scene and simulator use different ray tracers.
    RayTracerMismatch {
        /// The simulator's ray tracer.
        expected: RayTracerKind,
        /// The scene's ray tracer.
        actual: RayTracerKind,
    },
    /// Source inputs use a different reflection algorithm.
    AlgorithmMismatch {
        /// The source's reflection algorithm.
        expected: ReflectionAlgorithm,
        /// The input parameters' reflection algorithm.
        actual: ReflectionAlgorithm,
    },
    /// Related handles use different native devices.
    DeviceMismatch,
    /// A different simulator created the supplied source.
    ForeignSource,
}

#[cfg(feature = "dynamic")]
impl fmt::Display for DynamicSimulationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SteamAudio(error) => error.fmt(formatter),
            Self::ParameterValidation(error) => error.fmt(formatter),
            Self::Simulation(error) => error.fmt(formatter),
            Self::DisabledModes(flags) => {
                write!(formatter, "simulation modes are unavailable: {flags:?}")
            }
            Self::UnknownFlags(bits) => write!(formatter, "unknown simulation flags: {bits:#x}"),
            Self::MissingInputs(flags) => {
                write!(formatter, "missing shared simulation inputs: {flags:?}")
            }
            Self::RayTracerMismatch { expected, actual } => write!(
                formatter,
                "ray tracer mismatch: expected {expected:?}, got {actual:?}"
            ),
            Self::AlgorithmMismatch { expected, actual } => write!(
                formatter,
                "reflection algorithm mismatch: expected {expected:?}, got {actual:?}"
            ),
            Self::DeviceMismatch => write!(formatter, "simulation devices do not match"),
            Self::ForeignSource => write!(formatter, "another simulator created this source"),
        }
    }
}

#[cfg(feature = "dynamic")]
impl std::error::Error for DynamicSimulationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SteamAudio(error) => Some(error),
            Self::ParameterValidation(error) => Some(error),
            Self::Simulation(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(all(doctest, feature = "dynamic"))]
/// Simulation binding checks.
///
/// ```compile_fail
/// use audionimbus::{Simulator, ray_tracing::DefaultRayTracer};
/// fn add(simulator: &Simulator<DefaultRayTracer>, source: &audionimbus::dynamic::Source) {
///     simulator.add_source(source);
/// }
/// ```
///
/// ```compile_fail
/// fn add(simulator: &audionimbus::dynamic::Simulator, source: &audionimbus::Source) {
///     simulator.try_add_source(source);
/// }
/// ```
///
/// ```compile_fail
/// use audionimbus::{Simulator, ray_tracing::DefaultRayTracer};
/// fn scene(simulator: &mut Simulator<DefaultRayTracer>, scene: &audionimbus::dynamic::Scene) {
///     simulator.set_scene(scene);
/// }
/// ```
///
/// ```compile_fail
/// fn scene(simulator: &mut audionimbus::dynamic::Simulator, scene: &audionimbus::Scene) {
///     simulator.try_set_scene(scene);
/// }
/// ```
///
/// ```compile_fail
/// fn inputs(source: &audionimbus::Source, inputs: &audionimbus::dynamic::SimulationInputs) {
///     source.set_inputs(inputs);
/// }
/// ```
///
/// ```compile_fail
/// fn inputs(source: &audionimbus::dynamic::Source, inputs: &audionimbus::SimulationInputs) {
///     source.set_inputs(inputs);
/// }
/// ```
///
/// ```compile_fail
/// fn settings(context: &audionimbus::Context, settings: &audionimbus::dynamic::SimulationSettings) {
///     audionimbus::Simulator::try_new(context, settings);
/// }
/// ```
///
/// ```compile_fail
/// fn settings(context: &audionimbus::Context, settings: &audionimbus::SimulationSettings<audionimbus::ray_tracing::DefaultRayTracer>) {
///     audionimbus::dynamic::Simulator::try_new(context, settings);
/// }
/// ```
///
/// ```compile_fail
/// fn outputs(source: &audionimbus::dynamic::Source) {
///     let _: audionimbus::SimulationOutputs<(), (), ()> = source.get_outputs().unwrap();
/// }
/// ```
///
/// ```compile_fail
/// fn outputs(source: &audionimbus::Source) {
///     let _: audionimbus::dynamic::SimulationOutputs = source.get_outputs().unwrap();
/// }
/// ```
///
/// ```compile_fail
/// fn mutate(simulator: &mut audionimbus::dynamic::Simulator) {
///     simulator.raw_ptr_mut();
/// }
/// ```
///
/// ```compile_fail
/// fn mutate(source: &mut audionimbus::dynamic::Source) {
///     source.raw_ptr_mut();
/// }
/// ```
struct SimulationBindings;

#[cfg(test)]
mod tests {
    use super::{
        StaticSimulationSettings as SimulationSettings, StaticSimulator as Simulator,
        StaticSource as Source,
    };
    use crate::*;

    mod source {
        use super::*;

        mod clone {
            use super::*;

            #[test]
            fn test_clone() {
                let context = Context::default();
                let audio_settings = AudioSettings::default();
                let simulator_settings = SimulationSettings::new(&audio_settings);
                let simulator = Simulator::try_new(&context, &simulator_settings).unwrap();
                let source = Source::try_new(&simulator).unwrap();
                let clone = source.clone();
                assert_eq!(source.raw_ptr(), clone.raw_ptr());
                drop(source);
                assert!(!clone.raw_ptr().is_null());
            }
        }
    }

    mod simulator {
        use super::*;

        #[test]
        fn test_simulator_clone() {
            let context = Context::default();
            let audio_settings = AudioSettings::default();
            let settings = SimulationSettings::new(&audio_settings);
            let simulator = Simulator::try_new(&context, &settings).unwrap();
            let clone = simulator.clone();
            assert_eq!(simulator.raw_ptr(), clone.raw_ptr());
            drop(simulator);
            assert!(!clone.raw_ptr().is_null());
        }

        #[test]
        fn test_set_scene_is_noop_when_scene_is_already_committed() {
            let context = Context::default();
            let audio_settings = AudioSettings::default();
            let settings = SimulationSettings::new(&audio_settings);
            let mut simulator = Simulator::try_new(&context, &settings).unwrap();
            let scene = Scene::try_new(&context).unwrap();

            simulator.set_scene(&scene);
            simulator.commit();

            {
                let shared = simulator.shared.lock().unwrap();
                assert!(shared.pending_scene.is_none());
                assert_eq!(shared.committed_scene.as_ref(), Some(&scene));
            }

            simulator.set_scene(&scene);

            let shared = simulator.shared.lock().unwrap();
            assert!(shared.pending_scene.is_none());
            assert_eq!(shared.committed_scene.as_ref(), Some(&scene));
        }
    }
}
