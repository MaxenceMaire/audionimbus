//! Simulation sources, retained resources, and selected outputs.

use super::inputs::{InputBinding, SimulationInputs, StaticSimulationInputs};
#[cfg(feature = "dynamic")]
use super::simulator::{SimulationChoices, SimulatorShared};
use super::simulator::{Simulator, SimulatorBinding, StaticSimulator};
use super::{
    Direct, DirectCompatible, DirectSimulationParameters, OcclusionAlgorithm,
    ParameterValidationError, Pathing, PathingCompatible, ReflectionEffectCompatible, Reflections,
    ReflectionsCompatible, SimulationFlags, SimulationFlagsProvider,
};
#[cfg(feature = "dynamic")]
use crate::binding::Dynamic;
use crate::binding::Static;
#[cfg(feature = "dynamic")]
use crate::device::TrueAudioNextDevice;
#[cfg(feature = "dynamic")]
use crate::dynamic::{self, ReflectionAlgorithm};
use crate::effect::{
    DirectEffectParams, PathEffectParams, ReflectionEffectParams, ReflectionEffectType,
};
use crate::error::{SteamAudioError, to_option_error};
use crate::model::deviation::DeviationModel;
use crate::probe::ProbeBatch;
use crate::ray_tracing::RayTracer;
use std::hash::{Hash, Hasher};
#[cfg(feature = "dynamic")]
use std::sync::Weak;
use std::sync::{Arc, Mutex, MutexGuard};

/// Source resource storage selected by a simulation binding.
pub trait SourceBinding: Copy {
    /// The retained creator identity and devices.
    type Resources: SourceResources + Default + std::fmt::Debug;
}

impl<D, R, P, RE> SourceBinding for Static<(D, R, P, RE)> {
    type Resources = ();
}

/// Runtime source modes and reflection algorithm.
#[cfg(feature = "dynamic")]
#[derive(Clone, Copy, Debug)]
pub struct SourceChoices {
    /// The source's available simulation modes.
    flags: SimulationFlags,
    /// The source's reflection algorithm.
    reflection_algorithm: Option<ReflectionAlgorithm>,
}

/// Creator identity and resources retained by a runtime source.
#[cfg(feature = "dynamic")]
#[derive(Default, Debug)]
pub struct DynamicSourceResources {
    /// The shared allocation belonging to the creating simulator.
    pub simulator: Weak<Mutex<SimulatorShared<Dynamic<SimulationChoices>>>>,
    /// The direct models referenced by reflection simulation.
    pub reflections_parameters: Option<DirectSimulationParameters>,
    /// The direct models referenced by pathing simulation.
    pub pathing_parameters: Option<DirectSimulationParameters>,
    /// The order requested by the most recent pathing input update.
    pub pathing_order: u32,
    /// The device retained for runtime TrueAudio Next outputs.
    pub true_audio_next_device: Option<TrueAudioNextDevice>,
}

#[cfg(feature = "dynamic")]
impl SourceBinding for Dynamic<SourceChoices> {
    type Resources = DynamicSourceResources;
}

/// Resources retained for selected source inputs.
pub trait SourceResources {
    /// Retains models and records the order referenced by selected native inputs.
    fn set_inputs<I: InputBinding>(&mut self, flags: SimulationFlags, parameters: &I::Parameters);
}

impl SourceResources for () {
    fn set_inputs<I: InputBinding>(&mut self, _: SimulationFlags, _: &I::Parameters) {}
}

#[cfg(feature = "dynamic")]
impl SourceResources for DynamicSourceResources {
    fn set_inputs<I: InputBinding>(&mut self, flags: SimulationFlags, parameters: &I::Parameters) {
        if flags.contains(SimulationFlags::REFLECTIONS) {
            self.reflections_parameters = I::direct(parameters).clone();
        }
        if flags.contains(SimulationFlags::PATHING) {
            self.pathing_parameters = I::direct(parameters).clone();
            self.pathing_order =
                I::pathing(parameters).map_or(0, |parameters| parameters.pathing_order);
        }
    }
}

/// A sound source, for the purposes of simulation.
///
/// This object is used to specify various parameters for direct and indirect sound propagation simulation, and to retrieve the simulation results.
///
/// `Source` is a reference-counted handle to an underlying Steam Audio object.
/// Cloning it is cheap; it produces a new handle pointing to the same underlying object, while
/// incrementing a reference count.
/// The underlying object is destroyed when all handles are dropped.
///
/// Generic over the types of simulation that may be run for this source.
pub type StaticSource<D = (), R = (), P = (), RE = ()> = Source<Static<(D, R, P, RE)>>;

/// A source using a subset of its creating simulator's available modes.
#[cfg(feature = "dynamic")]
pub type DynamicSource = Source<Dynamic<SourceChoices>>;

/// Storage shared by static and runtime sources.
#[derive(Debug)]
pub struct Source<B: SourceBinding> {
    /// The native handle or output storage.
    inner: audionimbus_sys::IPLSource,
    /// The resources shared by cloned handles.
    shared: Arc<Mutex<SourceShared<B::Resources>>>,

    /// The maximum number of occlusion samples specified during creation of the [Simulator].
    max_num_occlusion_samples: Option<u32>,

    /// Reference to the simulator's direct simulation lock.
    /// Used to synchronize access to direct simulation data.
    direct_lock: Option<Arc<Mutex<()>>>,

    /// Reference to the simulator's reflections simulation lock.
    /// Used to synchronize access to reflections simulation data.
    reflections_lock: Option<Arc<Mutex<()>>>,

    /// Reference to the simulator's pathing simulation lock.
    /// Used to synchronize access to pathing simulation data.
    pathing_lock: Option<Arc<Mutex<()>>>,

    /// The compile-time or runtime simulation binding.
    binding: B,
}

/// Shared ownership of [`crate::Source`] data across clones.
#[derive(Default, Debug)]
struct SourceShared<O> {
    /// The creator identity and resources selected by the source binding.
    resources: O,
    /// Stored direct simulation parameters to keep callback-backed models alive.
    _direct_parameters: Option<DirectSimulationParameters>,

    /// Stored deviation model to keep `callback` and `user_data` alive.
    /// Only used when pathing simulation is enabled.
    deviation_model: Option<DeviationModel>,

    /// When pathing is enabled, a reference to the probe batch within which to find paths.
    /// It keeps the probe batch alive as long as the source is alive.
    _pathing_probes: Option<ProbeBatch>,
}

impl<D, R, P, RE> StaticSource<D, R, P, RE>
where
    D: 'static,
    R: 'static,
    P: 'static,
    RE: 'static,
{
    /// Creates a new source and returns a handle to it.
    ///
    /// Convenience method abstracting the more expressive [`Self::try_new_subset`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use audionimbus::*;
    /// # let context = Context::default();
    /// # let audio_settings = AudioSettings::default();
    /// # let simulation_settings = SimulationSettings::new(&audio_settings)
    /// #     .with_direct(DirectSimulationSettings { max_num_occlusion_samples: 4 });
    /// # let simulator = Simulator::try_new(&context, &simulation_settings)?;
    /// // The source's type parameters determine which simulations may be run.
    /// let source = Source::<Direct, (), ()>::try_new(&simulator)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if creation fails.
    pub fn try_new<T>(simulator: &StaticSimulator<T, D, R, P, RE>) -> Result<Self, SteamAudioError>
    where
        T: RayTracer,
        D: 'static + DirectCompatible<D> + SimulationFlagsProvider,
        R: 'static + ReflectionsCompatible<R> + SimulationFlagsProvider,
        P: 'static + PathingCompatible<P> + SimulationFlagsProvider,
        RE: 'static + ReflectionEffectCompatible<R, RE>,
    {
        Self::try_new_subset::<T, D, R, P, RE>(simulator)
    }

    /// Creates a new source and returns a handle to it.
    ///
    /// Generics specify which types of simulation may be run for this source.
    ///
    /// # Examples
    ///
    /// ```
    /// # use audionimbus::*;
    /// # let context = Context::default();
    /// # let audio_settings = AudioSettings::default();
    /// let simulation_settings = SimulationSettings::new(&audio_settings)
    ///     .with_direct(DirectSimulationSettings { max_num_occlusion_samples: 4 })
    ///     .with_reflections(ConvolutionSettings {
    ///         max_num_rays: 4096,
    ///         num_diffuse_samples: 32,
    ///         max_duration: 2.0,
    ///         max_num_sources: 8,
    ///         num_threads: 1,
    ///         max_order: 1,
    ///     });
    /// let simulator = Simulator::try_new(&context, &simulation_settings)?;
    ///
    /// // The simulator supports both direct and reflections, but this source
    /// // only participates in direct simulation.
    /// let source = Source::<Direct, (), ()>::try_new_subset(&simulator)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if creation fails.
    pub fn try_new_subset<T, SimD, SimR, SimP, SimRE>(
        simulator: &StaticSimulator<T, SimD, SimR, SimP, SimRE>,
    ) -> Result<Self, SteamAudioError>
    where
        T: RayTracer,
        SimD: 'static,
        SimR: 'static,
        SimP: 'static,
        SimRE: 'static,
        D: 'static + DirectCompatible<SimD> + SimulationFlagsProvider,
        R: 'static + ReflectionsCompatible<SimR> + SimulationFlagsProvider,
        P: 'static + PathingCompatible<SimP> + SimulationFlagsProvider,
        RE: 'static + ReflectionEffectCompatible<R, SimRE>,
    {
        Self::try_new_shared(
            simulator,
            D::flags() | R::flags() | P::flags(),
            Static::new(),
            (),
        )
    }

    /// Specifies simulation parameters for a source.
    ///
    /// Convenience method abstracting the more expressive [`Self::set_inputs_subset`].
    ///
    /// Also see:
    /// - [`Self::set_direct_inputs`]
    /// - [`Self::set_reflections_inputs`]
    /// - [`Self::set_pathing_inputs`]
    ///
    /// # Threading Considerations
    ///
    /// This method will block if a simulation is currently running for the
    /// specified type(s).
    /// Call this from the same thread that runs the corresponding simulation(s) to avoid blocking.
    ///
    /// It is safe to call this method concurrently with simulations of different types.
    /// For example, setting `DIRECT` inputs while a `REFLECTIONS` simulation is running
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
    /// #     .with_direct(DirectSimulationSettings { max_num_occlusion_samples: 4 });
    /// # let simulator = Simulator::try_new(&context, &simulation_settings)?;
    /// let source = Source::try_new(&simulator)?;
    /// let inputs = SimulationInputs {
    ///     source: CoordinateSystem::default(),
    ///     parameters: SimulationParameters::new()
    ///         .with_direct(DirectSimulationParameters::new()
    ///             .with_distance_attenuation(DistanceAttenuationModel::default()))
    /// };
    /// source.set_inputs(&inputs)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Arguments
    ///
    /// - `inputs`: the input parameters to set.
    ///
    /// # Errors
    ///
    /// Returns [`ParameterValidationError`] if any parameters exceed the maximums
    /// set during simulator initialization.
    pub fn set_inputs<InD, InR, InP>(
        &self,
        inputs: &StaticSimulationInputs<InD, InR, InP>,
    ) -> Result<(), ParameterValidationError>
    where
        D: DirectCompatible<D> + DirectCompatible<InD> + SimulationFlagsProvider,
        R: ReflectionsCompatible<R> + ReflectionsCompatible<InR> + SimulationFlagsProvider,
        P: PathingCompatible<P> + PathingCompatible<InP> + SimulationFlagsProvider,
        InD: DirectCompatible<D> + SimulationFlagsProvider,
        InR: ReflectionsCompatible<R> + SimulationFlagsProvider,
        InP: PathingCompatible<P> + SimulationFlagsProvider,
    {
        self.set_inputs_subset::<D, R, P, InD, InR, InP>(inputs)
    }

    /// Specifies parts or all of the simulation parameters for a source.
    ///
    /// Only blocks for the requested simulation types, allowing concurrent setting of parameters
    /// across simulation threads.
    ///
    /// For example, `set_inputs_subset::<Direct, (), ()>()` and
    /// `set_inputs_subset::<(), Reflections, ()>()` can be called simultaneously without blocking
    /// each other.
    ///
    /// MUST NOT be called from a real-time audio thread.
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
    /// #         num_threads: 1,
    /// #         max_order: 1,
    /// #     });
    /// # let simulator = Simulator::try_new(&context, &simulation_settings)?;
    /// # let source = Source::try_new(&simulator)?;
    /// # let inputs = SimulationInputs {
    /// #     source: CoordinateSystem::default(),
    /// #     parameters: SimulationParameters::new()
    /// #         .with_direct(DirectSimulationParameters::new()
    /// #             .with_distance_attenuation(DistanceAttenuationModel::default()))
    /// #         .with_reflections(ConvolutionParameters {
    /// #             baked_data_identifier: None,
    /// #         })
    /// # };
    /// // Direct simulation thread...
    /// source.set_inputs_subset::<Direct, (), (), _, _, _>(&inputs)?;
    ///
    /// // Reflections simulation thread... Does not block the direct thread.
    /// source.set_inputs_subset::<(), Reflections, (), _, _, _>(&inputs)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Arguments
    ///
    /// - `inputs`: the input parameters to set.
    ///
    /// # Errors
    ///
    /// Returns [`ParameterValidationError`] if any parameters exceed the maximums
    /// set during simulator initialization.
    pub fn set_inputs_subset<SubD, SubR, SubP, InD, InR, InP>(
        &self,
        inputs: &StaticSimulationInputs<InD, InR, InP>,
    ) -> Result<(), ParameterValidationError>
    where
        SubD: DirectCompatible<D> + DirectCompatible<InD> + SimulationFlagsProvider,
        SubR: ReflectionsCompatible<R> + ReflectionsCompatible<InR> + SimulationFlagsProvider,
        SubP: PathingCompatible<P> + PathingCompatible<InP> + SimulationFlagsProvider,
        InD: DirectCompatible<D> + SimulationFlagsProvider,
        InR: ReflectionsCompatible<R> + SimulationFlagsProvider,
        InP: PathingCompatible<P> + SimulationFlagsProvider,
    {
        self.set_inputs_shared(SubD::flags() | SubR::flags() | SubP::flags(), inputs)
    }

    /// Retrieves simulation results for a source.
    ///
    /// Convenience method abstracting the more expressive [`Self::get_outputs_subset`].
    ///
    /// See the [module-level documentation](crate::simulation) for threading guidelines.
    ///
    /// Also see:
    /// - [`Self::get_direct_outputs`]
    /// - [`Self::get_reflections_outputs`]
    /// - [`Self::get_pathing_outputs`]
    ///
    /// # Errors
    ///
    /// Returns a [`SteamAudioError`] on failure to allocate sufficient memory for the
    /// [`crate::SimulationOutputs`].
    pub fn get_outputs(&self) -> Result<StaticSimulationOutputs<D, R, P, RE>, SteamAudioError>
    where
        D: DirectCompatible<D> + SimulationFlagsProvider,
        R: ReflectionsCompatible<R> + SimulationFlagsProvider,
        P: PathingCompatible<P> + SimulationFlagsProvider,
        RE: ReflectionEffectCompatible<R, RE>,
    {
        self.get_outputs_subset::<D, R, P>()
    }

    /// Retrieves parts or all of the simulation results for a source.
    ///
    /// Only blocks for the requested simulation types, allowing concurrent retrieval across
    /// simulation threads.
    ///
    /// For example, `get_outputs_subset::<Direct, (), ()>()` and
    /// `get_outputs_subset::<(), Reflections, ()>()` can be called simultaneously without blocking
    /// each other.
    ///
    /// MUST NOT be called from a real-time audio thread.
    /// See the [module-level documentation](crate::simulation) for threading guidelines.
    ///
    /// # Errors
    ///
    /// Returns a [`SteamAudioError`] on failure to allocate sufficient memory for the
    /// [`crate::SimulationOutputs`].
    pub fn get_outputs_subset<OutD, OutR, OutP>(
        &self,
    ) -> Result<StaticSimulationOutputs<OutD, OutR, OutP, RE>, SteamAudioError>
    where
        OutD: DirectCompatible<D> + SimulationFlagsProvider,
        OutR: ReflectionsCompatible<R> + SimulationFlagsProvider,
        OutP: PathingCompatible<P> + SimulationFlagsProvider,
        RE: ReflectionEffectCompatible<OutR, RE>,
    {
        self.get_outputs_shared(OutD::flags() | OutR::flags() | OutP::flags(), Static::new())
    }
}

impl<D, R, P, RE> StaticSource<D, R, P, RE> {
    /// Returns a mutable reference to the raw FFI pointer.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub const fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLSource {
        &mut self.inner
    }
}

impl<R, P, RE> StaticSource<Direct, R, P, RE>
where
    R: 'static,
    P: 'static,
    RE: 'static + ReflectionEffectCompatible<R, RE>,
    (): ReflectionsCompatible<R>,
    (): PathingCompatible<P>,
{
    /// Specifies direct simulation parameters for a source.
    ///
    /// Convenience method abstracting the more expressive [`Self::set_inputs_subset`].
    /// See the [module-level documentation](crate::simulation) for threading guidelines.
    ///
    /// # Examples
    ///
    /// ```
    /// # use audionimbus::*;
    /// # let context = Context::default();
    /// # let audio_settings = AudioSettings::default();
    /// # let simulation_settings = SimulationSettings::new(&audio_settings)
    /// #     .with_direct(DirectSimulationSettings { max_num_occlusion_samples: 4 });
    /// # let simulator = Simulator::try_new(&context, &simulation_settings)?;
    /// # let source = Source::try_new(&simulator)?;
    /// let inputs = SimulationInputs {
    ///     source: CoordinateSystem::default(),
    ///     parameters: SimulationParameters::new()
    ///         .with_direct(DirectSimulationParameters::new()
    ///             .with_distance_attenuation(DistanceAttenuationModel::default())),
    /// };
    /// source.set_direct_inputs(&inputs)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Arguments
    ///
    /// - `inputs`: the input parameters to set.
    ///
    /// # Errors
    ///
    /// Returns [`ParameterValidationError`] if any parameters exceed the maximums
    /// set during simulator initialization.
    pub fn set_direct_inputs<InR, InP>(
        &self,
        inputs: &StaticSimulationInputs<Direct, InR, InP>,
    ) -> Result<(), ParameterValidationError>
    where
        InR: ReflectionsCompatible<R> + SimulationFlagsProvider,
        InP: PathingCompatible<P> + SimulationFlagsProvider,
        (): ReflectionsCompatible<InR>,
        (): PathingCompatible<InP>,
    {
        self.set_inputs_subset::<Direct, (), (), Direct, InR, InP>(inputs)
    }

    /// Retrieves direct simulation results for a source.
    ///
    /// Convenience method abstracting the more expressive [`Self::get_outputs_subset`].
    /// See the [module-level documentation](crate::simulation) for threading guidelines.
    ///
    /// # Errors
    ///
    /// Returns a [`SteamAudioError`] on failure to allocate sufficient memory for the
    /// [`crate::SimulationOutputs`].
    pub fn get_direct_outputs(&self) -> Result<DirectEffectParams, SteamAudioError> {
        self.get_outputs_subset::<Direct, (), ()>()
            .map(|outputs| outputs.direct())
    }
}

impl<D, P, RE> StaticSource<D, Reflections, P, RE>
where
    D: 'static,
    P: 'static,
    RE: 'static + ReflectionEffectType + ReflectionEffectCompatible<Reflections, RE>,
    (): DirectCompatible<D>,
    (): PathingCompatible<P>,
{
    /// Specifies reflections simulation parameters for a source.
    ///
    /// Convenience method abstracting the more expressive [`Self::set_inputs_subset`].
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
    /// #         num_threads: 2,
    /// #         max_order: 1,
    /// #     });
    /// # let simulator = Simulator::try_new(&context, &simulation_settings)?;
    /// # let source = Source::try_new(&simulator)?;
    /// let inputs = SimulationInputs {
    ///     source: CoordinateSystem::default(),
    ///     parameters: SimulationParameters::new()
    ///         .with_reflections(ConvolutionParameters {
    ///             baked_data_identifier: None,
    ///         })
    /// };
    /// source.set_reflections_inputs(&inputs)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Arguments
    ///
    /// - `inputs`: the input parameters to set.
    ///
    /// # Errors
    ///
    /// Returns [`ParameterValidationError`] if any parameters exceed the maximums
    /// set during simulator initialization.
    pub fn set_reflections_inputs<InD, InP>(
        &self,
        inputs: &StaticSimulationInputs<InD, Reflections, InP>,
    ) -> Result<(), ParameterValidationError>
    where
        InD: DirectCompatible<D> + SimulationFlagsProvider,
        InP: PathingCompatible<P> + SimulationFlagsProvider,
        (): DirectCompatible<InD>,
        (): PathingCompatible<InP>,
    {
        self.set_inputs_subset::<(), Reflections, (), InD, Reflections, InP>(inputs)
    }

    /// Retrieves reflections simulation results for a source.
    ///
    /// Convenience method abstracting the more expressive [`Self::get_outputs_subset`].
    /// See the [module-level documentation](crate::simulation) for threading guidelines.
    ///
    /// # Errors
    ///
    /// Returns a [`SteamAudioError`] on failure to allocate sufficient memory for the
    /// [`crate::SimulationOutputs`].
    pub fn get_reflections_outputs(&self) -> Result<ReflectionEffectParams<RE>, SteamAudioError> {
        self.get_outputs_subset::<(), Reflections, ()>()
            .map(|outputs| outputs.reflections())
    }
}

impl<D, R, RE> StaticSource<D, R, Pathing, RE>
where
    D: 'static,
    R: 'static,
    RE: 'static + ReflectionEffectCompatible<R, RE>,
    (): DirectCompatible<D>,
    (): ReflectionsCompatible<R>,
{
    /// Specifies pathing simulation parameters for a source.
    ///
    /// Convenience method abstracting the more expressive [`Self::set_inputs_subset`].
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
    /// # let source = Source::try_new(&simulator)?;
    /// # let pathing_probes = ProbeBatch::try_new(&context).unwrap();
    /// let inputs = SimulationInputs {
    ///     source: CoordinateSystem::default(),
    ///     parameters: SimulationParameters::new()
    ///         .with_pathing(PathingSimulationParameters {
    ///             pathing_probes,
    ///             visibility_radius: 1.0,
    ///             visibility_threshold: 10.0,
    ///             visibility_range: 10.0,
    ///             pathing_order: 1,
    ///             enable_validation: true,
    ///             find_alternate_paths: true,
    ///             deviation: DeviationModel::default(),
    ///         }),
    /// };
    /// source.set_pathing_inputs(&inputs)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Arguments
    ///
    /// - `inputs`: the input parameters to set.
    ///
    /// # Errors
    ///
    /// Returns [`ParameterValidationError`] if any parameters exceed the maximums
    /// set during simulator initialization.
    pub fn set_pathing_inputs<InD, InR>(
        &self,
        inputs: &StaticSimulationInputs<InD, InR, Pathing>,
    ) -> Result<(), ParameterValidationError>
    where
        InD: DirectCompatible<D> + SimulationFlagsProvider,
        InR: ReflectionsCompatible<R> + SimulationFlagsProvider,
        (): DirectCompatible<InD>,
        (): ReflectionsCompatible<InR>,
    {
        self.set_inputs_subset::<(), (), Pathing, InD, InR, Pathing>(inputs)
    }

    /// Retrieves pathing simulation results for a source.
    ///
    /// Convenience method abstracting the more expressive [`Self::get_outputs_subset`].
    /// See the [module-level documentation](crate::simulation) for threading guidelines.
    ///
    /// # Errors
    ///
    /// Returns a [`SteamAudioError`] on failure to allocate sufficient memory for the
    /// [`crate::SimulationOutputs`].
    pub fn get_pathing_outputs(&self) -> Result<PathEffectParams, SteamAudioError> {
        self.get_outputs_subset::<(), (), Pathing>()
            .map(|outputs| outputs.pathing())
    }
}

impl<B: SourceBinding> Drop for Source<B> {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplSourceRelease(&raw mut self.inner) }
    }
}

unsafe impl<D, R, P, RE> Send for StaticSource<D, R, P, RE> {}

#[cfg(feature = "dynamic")]
unsafe impl Send for dynamic::Source {}

unsafe impl<D, R, P, RE> Sync for StaticSource<D, R, P, RE> {}

#[cfg(feature = "dynamic")]
unsafe impl Sync for dynamic::Source {}

impl<B: SourceBinding> Clone for Source<B> {
    /// Retains an additional reference to the source.
    ///
    /// The returned [`crate::Source`] shares the same underlying Steam Audio object.
    fn clone(&self) -> Self {
        // SAFETY: The source will not be destroyed until all references are released.
        Self {
            binding: self.binding,
            inner: unsafe { audionimbus_sys::iplSourceRetain(self.inner) },
            max_num_occlusion_samples: self.max_num_occlusion_samples,
            shared: Arc::clone(&self.shared),
            direct_lock: self.direct_lock.clone(),
            reflections_lock: self.reflections_lock.clone(),
            pathing_lock: self.pathing_lock.clone(),
        }
    }
}

impl<B: SourceBinding> PartialEq for Source<B> {
    fn eq(&self, other: &Self) -> bool {
        self.raw_ptr() == other.raw_ptr()
    }
}

impl<B: SourceBinding> Eq for Source<B> {}

impl<B: SourceBinding> Hash for Source<B> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::ptr::hash(self.raw_ptr(), state);
    }
}

impl<B: SourceBinding> Source<B> {
    /// Creates the native source with its simulator locks and retained resources.
    ///
    /// # Errors
    ///
    /// - [`SteamAudioError::Unspecified`] if the native operation fails.
    /// - [`SteamAudioError::OutOfMemory`] if allocation fails.
    /// - [`SteamAudioError::Initialization`] if a native dependency fails.
    fn try_new_shared<S: SimulatorBinding>(
        simulator: &Simulator<S>,
        simulation_flags: SimulationFlags,
        binding: B,
        resources: B::Resources,
    ) -> Result<Self, SteamAudioError> {
        let mut inner = std::ptr::null_mut();

        let status = unsafe {
            audionimbus_sys::iplSourceCreate(
                simulator.raw_ptr(),
                &mut audionimbus_sys::IPLSourceSettings {
                    flags: simulation_flags.into(),
                },
                &mut inner,
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        let (max_num_occlusion_samples, [direct_lock, reflections_lock, pathing_lock]) =
            simulator.source_parts();

        let source = Self {
            inner,
            binding,
            shared: Arc::new(Mutex::new(SourceShared {
                resources,
                ..Default::default()
            })),
            max_num_occlusion_samples,
            direct_lock,
            reflections_lock,
            pathing_lock,
        };

        Ok(source)
    }

    /// Validates and publishes source inputs without releasing unselected resources.
    ///
    /// # Errors
    ///
    /// - [`ParameterValidationError::OcclusionSamplesExceedsMax`] if occlusion samples exceed the creation limit.
    #[inline(always)]
    fn set_inputs_shared<I: InputBinding>(
        &self,
        simulation_flags: SimulationFlags,
        inputs: &SimulationInputs<I>,
    ) -> Result<(), ParameterValidationError> {
        self.validate_inputs(inputs)?;

        let (mut ffi_inputs, mut deviation_model) = inputs.to_ffi();
        ffi_inputs.deviationModel = &raw mut deviation_model;

        let direct_parameters = if simulation_flags.contains(SimulationFlags::DIRECT) {
            Some(I::direct(&inputs.parameters).clone())
        } else {
            None
        };

        let mut shared = self.shared.lock().unwrap();
        let _guards = self.acquire_locks_for_flags(simulation_flags);
        shared
            .resources
            .set_inputs::<I>(simulation_flags, &inputs.parameters);
        if simulation_flags.contains(SimulationFlags::PATHING) {
            (shared.deviation_model, shared._pathing_probes) = I::pathing(&inputs.parameters)
                .map(|p| (Some(p.deviation.clone()), Some(p.pathing_probes.clone())))
                .unwrap_or_default();
        }

        unsafe {
            audionimbus_sys::iplSourceSetInputs(
                self.raw_ptr(),
                simulation_flags.into(),
                &mut ffi_inputs,
            );
        }

        if let Some(direct_parameters) = direct_parameters {
            shared._direct_parameters = direct_parameters;
        }

        Ok(())
    }

    /// Acquires locks for the simulation types specified in the given flags.
    fn acquire_locks_for_flags(&self, flags: SimulationFlags) -> Vec<MutexGuard<'_, ()>> {
        let mut guards = Vec::new();

        if flags.contains(SimulationFlags::DIRECT)
            && let Some(lock) = &self.direct_lock
        {
            guards.push(lock.lock().unwrap());
        }

        if flags.contains(SimulationFlags::REFLECTIONS)
            && let Some(lock) = &self.reflections_lock
        {
            guards.push(lock.lock().unwrap());
        }

        if flags.contains(SimulationFlags::PATHING)
            && let Some(lock) = &self.pathing_lock
        {
            guards.push(lock.lock().unwrap());
        }

        guards
    }

    /// Validates simulation parameters against the limits set during simulator initialization.
    fn validate_inputs<I: InputBinding>(
        &self,
        inputs: &SimulationInputs<I>,
    ) -> Result<(), ParameterValidationError> {
        let Some(direct_params) = I::direct(&inputs.parameters) else {
            return Ok(());
        };

        let Some(occlusion) = &direct_params.occlusion else {
            return Ok(());
        };

        let OcclusionAlgorithm::Volumetric {
            num_occlusion_samples,
            ..
        } = occlusion.algorithm
        else {
            return Ok(());
        };

        let Some(max) = self.max_num_occlusion_samples else {
            return Ok(());
        };

        if num_occlusion_samples > max {
            return Err(ParameterValidationError::OcclusionSamplesExceedsMax {
                requested: num_occlusion_samples,
                max,
            });
        }

        Ok(())
    }

    /// Retrieves selected native outputs and retains their source.
    ///
    /// # Errors
    ///
    /// - [`SteamAudioError::OutOfMemory`] if allocation fails.
    fn get_outputs_shared<O>(
        &self,
        flags: SimulationFlags,
        binding: O,
    ) -> Result<SimulationOutputs<O>, SteamAudioError> {
        let _guards = self.acquire_locks_for_flags(flags);
        let outputs = SimulationOutputs::try_allocate(self.raw_ptr(), binding)?;
        unsafe {
            audionimbus_sys::iplSourceGetOutputs(self.raw_ptr(), flags.into(), outputs.inner);
        }

        Ok(outputs)
    }
}

impl<B: SourceBinding> Source<B> {
    /// Returns the raw FFI pointer to the underlying source.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub const fn raw_ptr(&self) -> audionimbus_sys::IPLSource {
        self.inner
    }
}

#[cfg(feature = "dynamic")]
impl DynamicSource {
    /// Creates a source for a subset of the simulator's available modes.
    ///
    /// # Errors
    ///
    /// - [`dynamic::SimulationError::UnknownFlags`] for unknown mode bits.
    /// - [`dynamic::SimulationError::DisabledModes`] for unavailable modes.
    /// - [`dynamic::SimulationError::SteamAudio`] if native creation fails.
    pub fn try_new(
        simulator: &dynamic::Simulator,
        flags: SimulationFlags,
    ) -> Result<Self, dynamic::SimulationError> {
        let unknown = flags.bits() & !SimulationFlags::all().bits();
        if unknown != 0 {
            return Err(dynamic::SimulationError::UnknownFlags(unknown));
        }
        let disabled = flags & !simulator.flags();
        if !disabled.is_empty() {
            return Err(dynamic::SimulationError::DisabledModes(disabled));
        }
        let reflection_algorithm = if flags.contains(SimulationFlags::REFLECTIONS) {
            simulator.reflection_algorithm()
        } else {
            None
        };
        let dynamic_source_resources = simulator.source_resources();

        Self::try_new_shared(
            simulator,
            flags,
            Dynamic(SourceChoices {
                flags,
                reflection_algorithm,
            }),
            dynamic_source_resources,
        )
        .map_err(dynamic::SimulationError::SteamAudio)
    }

    /// Returns the source's available simulation modes.
    pub const fn flags(&self) -> SimulationFlags {
        self.binding.0.flags
    }

    /// Returns the source's reflection algorithm.
    pub const fn reflection_algorithm(&self) -> Option<ReflectionAlgorithm> {
        self.binding.0.reflection_algorithm
    }

    /// Updates participation in every available source mode.
    ///
    /// Missing parameters disable their modes.
    ///
    /// # Errors
    ///
    /// - [`dynamic::SimulationError::UnknownFlags`] for unknown mode bits.
    /// - [`dynamic::SimulationError::DisabledModes`] for unavailable modes.
    /// - [`dynamic::SimulationError::AlgorithmMismatch`] for incompatible reflection parameters.
    /// - [`dynamic::SimulationError::ParameterValidation`] if inputs exceed creation limits.
    pub fn set_inputs(
        &self,
        inputs: &dynamic::SimulationInputs,
    ) -> Result<(), dynamic::SimulationError> {
        let disabled = inputs.parameters.flags() & !self.flags();
        if !disabled.is_empty() {
            return Err(dynamic::SimulationError::DisabledModes(disabled));
        }

        self.set_inputs_subset(self.flags(), inputs)
    }

    /// Updates selected modes and preserves unselected resources.
    ///
    /// Missing parameters disable selected modes.
    ///
    /// # Errors
    ///
    /// - [`dynamic::SimulationError::UnknownFlags`] for unknown mode bits.
    /// - [`dynamic::SimulationError::DisabledModes`] for unavailable modes.
    /// - [`dynamic::SimulationError::AlgorithmMismatch`] for incompatible reflection parameters.
    /// - [`dynamic::SimulationError::ParameterValidation`] if inputs exceed creation limits.
    pub fn set_inputs_subset(
        &self,
        flags: SimulationFlags,
        inputs: &dynamic::SimulationInputs,
    ) -> Result<(), dynamic::SimulationError> {
        let unknown = flags.bits() & !SimulationFlags::all().bits();
        if unknown != 0 {
            return Err(dynamic::SimulationError::UnknownFlags(unknown));
        }
        let disabled = flags & !self.flags();
        if !disabled.is_empty() {
            return Err(dynamic::SimulationError::DisabledModes(disabled));
        }
        let requested = inputs.parameters.flags() & flags;
        if requested.contains(SimulationFlags::REFLECTIONS)
            && let Some(parameters) = &inputs.parameters.reflections
            && let Some(expected) = self.reflection_algorithm()
        {
            let actual = parameters.algorithm();
            if expected != actual {
                return Err(dynamic::SimulationError::AlgorithmMismatch { expected, actual });
            }
        }

        self.set_inputs_shared(flags, inputs)
            .map_err(dynamic::SimulationError::ParameterValidation)
    }

    /// Retrieves outputs for every available source mode.
    ///
    /// # Errors
    ///
    /// - [`dynamic::SimulationError::UnknownFlags`] for unknown mode bits.
    /// - [`dynamic::SimulationError::DisabledModes`] for unavailable modes.
    /// - [`dynamic::SimulationError::SteamAudio`] if output allocation fails.
    pub fn get_outputs(&self) -> Result<dynamic::SimulationOutputs, dynamic::SimulationError> {
        self.get_outputs_subset(self.flags())
    }

    /// Retrieves outputs for selected available source modes.
    ///
    /// # Errors
    ///
    /// - [`dynamic::SimulationError::UnknownFlags`] for unknown mode bits.
    /// - [`dynamic::SimulationError::DisabledModes`] for unavailable modes.
    /// - [`dynamic::SimulationError::SteamAudio`] if output allocation fails.
    pub fn get_outputs_subset(
        &self,
        flags: SimulationFlags,
    ) -> Result<dynamic::SimulationOutputs, dynamic::SimulationError> {
        let unknown = flags.bits() & !SimulationFlags::all().bits();
        if unknown != 0 {
            return Err(dynamic::SimulationError::UnknownFlags(unknown));
        }
        let disabled = flags & !self.flags();
        if !disabled.is_empty() {
            return Err(dynamic::SimulationError::DisabledModes(disabled));
        }
        let shared = self.shared.lock().unwrap();
        let binding = Dynamic(OutputSelection {
            flags,
            pathing_order: shared.resources.pathing_order,
            reflection_algorithm: self.reflection_algorithm(),
            true_audio_next_device: shared.resources.true_audio_next_device.clone(),
        });

        self.get_outputs_shared(flags, binding)
            .map_err(dynamic::SimulationError::SteamAudio)
    }

    /// Retrieves direct output without an aggregate output allocation.
    ///
    /// # Errors
    ///
    /// - [`dynamic::SimulationError::DisabledModes`] if direct simulation is unavailable.
    pub fn get_direct_outputs(&self) -> Result<DirectEffectParams, dynamic::SimulationError> {
        if !self.flags().contains(SimulationFlags::DIRECT) {
            return Err(dynamic::SimulationError::DisabledModes(
                SimulationFlags::DIRECT,
            ));
        }

        Ok(self.outputs_shared(SimulationFlags::DIRECT).direct.into())
    }

    /// Retrieves reflection output without an aggregate output allocation.
    ///
    /// # Errors
    ///
    /// - [`dynamic::SimulationError::DisabledModes`] if reflection simulation is unavailable.
    pub fn get_reflections_outputs(
        &self,
    ) -> Result<dynamic::ReflectionEffectParams, dynamic::SimulationError> {
        let Some(algorithm) = self.reflection_algorithm() else {
            return Err(dynamic::SimulationError::DisabledModes(
                SimulationFlags::REFLECTIONS,
            ));
        };
        let mut reflections = self
            .outputs_shared(SimulationFlags::REFLECTIONS)
            .reflections;
        let shared = self.shared.lock().unwrap();
        reflections.tanDevice = shared
            .resources
            .true_audio_next_device
            .as_ref()
            .map_or(std::ptr::null_mut(), TrueAudioNextDevice::raw_ptr);

        Ok(unsafe {
            dynamic::ReflectionEffectParams::from_ffi_unchecked(
                reflections,
                self.raw_ptr(),
                Dynamic(algorithm),
            )
        })
    }

    /// Retrieves pathing output without an aggregate output allocation.
    ///
    /// # Errors
    ///
    /// - [`dynamic::SimulationError::DisabledModes`] if pathing simulation is unavailable.
    pub fn get_pathing_outputs(&self) -> Result<PathEffectParams, dynamic::SimulationError> {
        if !self.flags().contains(SimulationFlags::PATHING) {
            return Err(dynamic::SimulationError::DisabledModes(
                SimulationFlags::PATHING,
            ));
        }

        let shared = self.shared.lock().unwrap();
        let mut pathing = self.outputs_shared(SimulationFlags::PATHING).pathing;
        pathing.order = shared.resources.pathing_order as i32;

        Ok(pathing.into())
    }
}

impl<B: SourceBinding> Source<B> {
    /// Retrieves selected native outputs into stack storage.
    #[cfg(feature = "dynamic")]
    fn outputs_shared(&self, flags: SimulationFlags) -> audionimbus_sys::IPLSimulationOutputs {
        let _guards = self.acquire_locks_for_flags(flags);
        let mut outputs = unsafe { std::mem::zeroed() };
        unsafe {
            audionimbus_sys::iplSourceGetOutputs(self.raw_ptr(), flags.into(), &raw mut outputs);
        }

        outputs
    }
}

/// Runtime output selection and retained convolution device.
#[cfg(feature = "dynamic")]
#[derive(Debug)]
pub struct OutputSelection {
    /// The modes requested from the source.
    flags: SimulationFlags,
    /// The order requested by the source's pathing inputs.
    pathing_order: u32,
    /// The reflection algorithm selected at source creation.
    reflection_algorithm: Option<ReflectionAlgorithm>,
    /// The device needed to process TrueAudio Next output.
    true_audio_next_device: Option<TrueAudioNextDevice>,
}

/// Simulation results for a source.
pub type StaticSimulationOutputs<D, R, P, RE = ()> = SimulationOutputs<Static<(D, R, P, RE)>>;

/// Selected source outputs and their retained resources.
#[cfg(feature = "dynamic")]
pub type DynamicSimulationOutputs = SimulationOutputs<Dynamic<OutputSelection>>;

/// Storage shared by static and runtime simulation outputs.
#[derive(Debug)]
pub struct SimulationOutputs<B> {
    /// The native handle or output storage.
    inner: *mut audionimbus_sys::IPLSimulationOutputs,

    /// Retained pointer of the [`crate::Source`] it originated from, to ensure the IR pointer remains
    /// valid.
    _source: audionimbus_sys::IPLSource,

    /// The compile-time or runtime simulation binding.
    _binding: B,
}

impl<B> SimulationOutputs<B> {
    /// Allocates native outputs and retains their originating source.
    ///
    /// # Errors
    ///
    /// - [`SteamAudioError::OutOfMemory`] if allocation fails.
    fn try_allocate(
        source: audionimbus_sys::IPLSource,
        binding: B,
    ) -> Result<Self, SteamAudioError> {
        let ptr = unsafe {
            let layout = std::alloc::Layout::new::<audionimbus_sys::IPLSimulationOutputs>();
            let ptr = std::alloc::alloc(layout).cast::<audionimbus_sys::IPLSimulationOutputs>();
            if ptr.is_null() {
                return Err(SteamAudioError::OutOfMemory);
            }
            std::ptr::write(ptr, std::mem::zeroed());
            ptr
        };

        let source = unsafe { audionimbus_sys::iplSourceRetain(source) };

        Ok(Self {
            inner: ptr,
            _binding: binding,
            _source: source,
        })
    }

    /// Returns the native simulation outputs.
    pub const fn raw_ptr(&self) -> *mut audionimbus_sys::IPLSimulationOutputs {
        self.inner
    }
}

impl<D, R, P, RE> StaticSimulationOutputs<D, R, P, RE> {
    /// Returns mutable access to the native output pointer.
    pub const fn raw_ptr_mut(&mut self) -> &mut *mut audionimbus_sys::IPLSimulationOutputs {
        &mut self.inner
    }
}

impl<R, P, RE> StaticSimulationOutputs<Direct, R, P, RE> {
    /// Returns direct simulation output.
    pub fn direct(&self) -> DirectEffectParams {
        unsafe { (*self.inner).direct.into() }
    }
}

impl<D, P, RE> StaticSimulationOutputs<D, Reflections, P, RE>
where
    RE: ReflectionEffectType,
{
    /// Returns reflection simulation output.
    pub fn reflections(&self) -> ReflectionEffectParams<RE> {
        unsafe {
            ReflectionEffectParams::from_ffi_unchecked(
                (*self.inner).reflections,
                self._source,
                Static::new(),
            )
        }
    }
}

impl<D, R, RE> StaticSimulationOutputs<D, R, Pathing, RE> {
    /// Returns pathing simulation output.
    pub fn pathing(&self) -> PathEffectParams {
        unsafe { (*self.inner).pathing.into() }
    }
}

unsafe impl<D, R, P, RE> Send for StaticSimulationOutputs<D, R, P, RE> {}

#[cfg(feature = "dynamic")]
unsafe impl Send for dynamic::SimulationOutputs {}

impl<B> Drop for SimulationOutputs<B> {
    fn drop(&mut self) {
        unsafe {
            let layout = std::alloc::Layout::new::<audionimbus_sys::IPLSimulationOutputs>();
            std::alloc::dealloc(self.inner.cast::<u8>(), layout);

            audionimbus_sys::iplSourceRelease(&mut self._source);
        }
    }
}

#[cfg(feature = "dynamic")]
impl DynamicSimulationOutputs {
    /// Returns direct output when direct simulation was requested.
    pub fn direct(&self) -> Option<DirectEffectParams> {
        self._binding
            .0
            .flags
            .contains(SimulationFlags::DIRECT)
            .then(|| unsafe { (*self.inner).direct.into() })
    }

    /// Returns reflection output when reflection simulation was requested.
    pub fn reflections(&self) -> Option<dynamic::ReflectionEffectParams> {
        if !self._binding.0.flags.contains(SimulationFlags::REFLECTIONS) {
            return None;
        }
        let algorithm = self._binding.0.reflection_algorithm?;
        let mut reflections = unsafe { (*self.inner).reflections };
        reflections.tanDevice = self
            ._binding
            .0
            .true_audio_next_device
            .as_ref()
            .map_or(std::ptr::null_mut(), TrueAudioNextDevice::raw_ptr);

        Some(unsafe {
            dynamic::ReflectionEffectParams::from_ffi_unchecked(
                reflections,
                self._source,
                Dynamic(algorithm),
            )
        })
    }

    /// Returns pathing output when pathing simulation was requested.
    pub fn pathing(&self) -> Option<PathEffectParams> {
        if !self._binding.0.flags.contains(SimulationFlags::PATHING) {
            return None;
        }
        let mut pathing = unsafe { (*self.inner).pathing };
        pathing.order = self._binding.0.pathing_order as i32;

        Some(pathing.into())
    }
}

#[cfg(feature = "dynamic")]
impl DynamicSource {
    /// Returns the identity of the creating simulator allocation.
    ///
    /// Creator storage must stay internal to preserve simulator registration lock ordering.
    pub(crate) fn creator(&self) -> Weak<Mutex<SimulatorShared<Dynamic<SimulationChoices>>>> {
        self.shared.lock().unwrap().resources.simulator.clone()
    }
}
