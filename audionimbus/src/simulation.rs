//! Spatial audio simulation (direct simulation, reflections, pathing).
//!
//! # Multi-Threading Architecture
//!
//! Simulations are designed to run on separate threads from audio processing:
//!
//! ```text
//! ┌────────────────────────────┐       ┌──────────────────┐       ┌─────────────────┐
//! │ Simulation thread(s)       │       │ Lock-free        │       │ Audio thread    │
//! │                            │──────▶│ communication    │──────▶│ (real-time)     │
//! │                            │       │                  │       │                 │
//! │ Simulator::run_direct      │       │ Triple buffer    │       │ Apply effects   │
//! │ Simulator::run_reflections │       │ or               │       │                 │
//! │ Source::get_outputs        │       │ lock-free queue  │       │                 │
//! └────────────────────────────┘       └──────────────────┘       └─────────────────┘
//!  Can take 50-500ms                    Non-blocking               Must complete in
//!  Can block                                                       a few ms
//! ```
//!
//! ## Important Rules
//!
//! - Never call [`Source::get_outputs`] from an audio thread - it will block and cause audio glitches
//! - Use lock-free communication to pass results from simulation to audio threads
//! - Different simulation types can run in parallel

use crate::baking::{BakedDataIdentifier, BakedDataVariation};
use crate::callback::CallbackInformation;
use crate::context::Context;
use crate::device::open_cl::OpenClDevice;
use crate::device::radeon_rays::RadeonRaysDevice;
use crate::device::true_audio_next::TrueAudioNextDevice;
use crate::effect::reflections::ReflectionEffectType;
use crate::effect::{DirectEffectParams, PathEffectParams, ReflectionEffectParams};
use crate::error::{to_option_error, SteamAudioError};
use crate::ffi_wrapper::FFIWrapper;
use crate::geometry::{CoordinateSystem, Scene};
use crate::model::air_absorption::AirAbsorptionModel;
use crate::model::deviation::DeviationModel;
use crate::model::directivity::Directivity;
use crate::model::distance_attenuation::DistanceAttenuationModel;
use crate::probe::ProbeBatch;
use crate::ray_tracing::{CustomRayTracer, DefaultRayTracer, Embree, RadeonRays, RayTracer};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::{Arc, Mutex, MutexGuard};

/// Marker type indicating that direct sound simulation is enabled.
#[derive(Debug)]
pub struct Direct;

/// Marker type indicating that reflection simulation is enabled.
#[derive(Debug)]
pub struct Reflections;

/// Marker type indicating that pathing simulation is enabled.
#[derive(Debug)]
pub struct Pathing;

/// Manages direct and indirect sound propagation simulation for multiple sources.
///
/// Your application will typically create one simulator object and use it to run simulations with different source and listener parameters between consecutive simulation runs.
/// The simulator can also be reused across scene changes.
///
/// # Examples
///
/// Basic simulation workflow (shown with direct sound; reflections and pathing follow a similar pattern):
///
/// ```
/// # use audionimbus::{
/// #     Context, Simulator, Scene, Source, SourceSettings, SimulationFlags,
/// #     DirectSimulationSettings, SimulationSettings, SimulationInputs, SimulationSharedInputs,
/// #     CoordinateSystem, DirectSimulationParameters, DistanceAttenuationModel,
/// # };
/// # let context = Context::default();
/// // Create a simulator.
/// let settings = SimulationSettings::new(48000, 1024, 2)
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
/// // Create and add a source.
/// let source_settings = SourceSettings {
///     flags: SimulationFlags::DIRECT,
/// };
/// let mut source = Source::try_new(&simulator, &source_settings)?;
///
/// simulator.add_source(&source);
///
/// // Configure simulation parameters.
/// let simulation_inputs = SimulationInputs::new(CoordinateSystem::default())
///     .with_direct(DirectSimulationParameters::new()
///         .with_distance_attenuation(DistanceAttenuationModel::default()));
/// source.set_inputs(SimulationFlags::DIRECT, simulation_inputs);
///
/// // Set shared parameters.
/// let shared_inputs = SimulationSharedInputs {
///     listener: CoordinateSystem::default(),
///     num_rays: 4096,
///     num_bounces: 16,
///     duration: 2.0,
///     order: 1,
///     irradiance_min_distance: 1.0,
///     pathing_visualization_callback: None,
/// };
/// simulator.set_shared_inputs(SimulationFlags::DIRECT, &shared_inputs);
///
/// // Run the simulation.
/// simulator.run_direct();
///
/// // Get results.
/// let outputs = source.get_outputs(SimulationFlags::DIRECT)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug)]
pub struct Simulator<'a, T: RayTracer, D = (), R = (), P = ()> {
    inner: audionimbus_sys::IPLSimulator,

    /// Number of probes after the last commit.
    /// Used to ensure the simulator has probes before running pathing.
    committed_num_probes: usize,

    /// Pending probe batches to be committed.
    pending_probe_batches: HashMap<audionimbus_sys::IPLProbeBatch, usize>,

    /// Whether a scene has been set and committed.
    has_committed_scene: bool,

    /// Whether a scene is pending commit.
    has_pending_scene: bool,

    /// Synchronization lock for direct simulation operations.
    direct_lock: Option<Arc<Mutex<()>>>,

    /// Synchronization lock for reflections simulation operations.
    reflections_lock: Option<Arc<Mutex<()>>>,

    /// Synchronization lock for pathing simulation operations.
    pathing_lock: Option<Arc<Mutex<()>>>,

    _ray_tracer: PhantomData<T>,
    _direct: PhantomData<D>,
    _reflections: PhantomData<R>,
    _pathing: PhantomData<P>,
    _lifetime: PhantomData<&'a ()>,
}

impl<'a, T, D, R, P> Simulator<'a, T, D, R, P>
where
    T: RayTracer,
    D: 'static,
    R: 'static,
    P: 'static,
{
    /// Creates a new [`Simulator`].
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if creation fails.
    ///
    /// # Examples
    ///
    /// ```
    /// # use audionimbus::{Context, Simulator, SimulationSettings, DirectSimulationSettings, ReflectionsSimulationSettings, PathingSimulationSettings};
    /// # let context = Context::default();
    /// let settings = SimulationSettings::new(48000, 1024, 2)
    ///     .with_direct(DirectSimulationSettings {
    ///         max_num_occlusion_samples: 4,
    ///     })
    ///     .with_reflections(ReflectionsSimulationSettings::Convolution {
    ///         max_num_rays: 4096,
    ///         num_diffuse_samples: 32,
    ///         max_duration: 2.0,
    ///         max_num_sources: 8,
    ///         num_threads: 2,
    ///     })
    ///     .with_pathing(PathingSimulationSettings {
    ///         num_visibility_samples: 4,
    ///     });
    /// let simulator = Simulator::try_new(&context, &settings)?;
    /// # Ok::<(), audionimbus::SteamAudioError>(())
    /// ```
    pub fn try_new(
        context: &Context,
        settings: &SimulationSettings<'a, T, D, R, P>,
    ) -> Result<Self, SteamAudioError> {
        let direct_lock = if std::any::TypeId::of::<D>() == std::any::TypeId::of::<Direct>() {
            Some(Arc::new(Mutex::new(())))
        } else {
            None
        };

        let reflections_lock =
            if std::any::TypeId::of::<R>() == std::any::TypeId::of::<Reflections>() {
                Some(Arc::new(Mutex::new(())))
            } else {
                None
            };

        let pathing_lock = if std::any::TypeId::of::<P>() == std::any::TypeId::of::<Pathing>() {
            Some(Arc::new(Mutex::new(())))
        } else {
            None
        };

        let mut simulator = Self {
            inner: std::ptr::null_mut(),
            committed_num_probes: 0,
            pending_probe_batches: HashMap::new(),
            has_committed_scene: false,
            has_pending_scene: false,
            direct_lock,
            reflections_lock,
            pathing_lock,
            _ray_tracer: PhantomData,
            _direct: PhantomData,
            _reflections: PhantomData,
            _pathing: PhantomData,
            _lifetime: PhantomData,
        };

        let status = unsafe {
            audionimbus_sys::iplSimulatorCreate(
                context.raw_ptr(),
                &mut settings.to_ffi(),
                simulator.raw_ptr_mut(),
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
    /// This function cannot be called while any simulation is running.
    pub fn set_scene(&mut self, scene: &Scene<T>) {
        unsafe { audionimbus_sys::iplSimulatorSetScene(self.raw_ptr(), scene.raw_ptr()) }
        self.has_pending_scene = true;
    }

    /// Adds a probe batch for use in subsequent simulations.
    /// Sources that require baked data can then use the data contained in the specified probe batch.
    ///
    /// Call [`Self::commit`] after calling this function for the changes to take effect.
    ///
    /// This function cannot be called while any simulation is running.
    pub fn add_probe_batch(&mut self, probe_batch: &ProbeBatch) {
        let raw_ptr = probe_batch.raw_ptr();

        unsafe {
            audionimbus_sys::iplSimulatorAddProbeBatch(self.raw_ptr(), probe_batch.raw_ptr());
        }

        self.pending_probe_batches
            .insert(raw_ptr, probe_batch.committed_num_probes());
    }

    /// Removes a probe batch from use in subsequent simulations.
    /// Sources that require baked data will then stop using the data contained in the specified probe batch.
    ///
    /// Call [`Self::commit`] after calling this function for the changes to take effect.
    ///
    /// This function cannot be called while any simulation is running.
    pub fn remove_probe_batch(&mut self, probe_batch: &ProbeBatch) {
        let raw_ptr = probe_batch.raw_ptr();

        unsafe {
            audionimbus_sys::iplSimulatorRemoveProbeBatch(self.raw_ptr(), probe_batch.raw_ptr());
        }

        self.pending_probe_batches.remove(&raw_ptr);
    }

    /// Adds a source to the set of sources processed by a simulator in subsequent simulations.
    ///
    /// Call [`Self::commit`] after calling this function for the changes to take effect.
    pub fn add_source(&self, source: &Source) {
        unsafe {
            audionimbus_sys::iplSourceAdd(source.raw_ptr(), self.raw_ptr());
        }
    }

    /// Removes a source from the set of sources processed by a simulator in subsequent simulations.
    ///
    /// Call [`Self::commit`] after calling this function for the changes to take effect.
    pub fn remove_source(&self, source: &Source) {
        unsafe {
            audionimbus_sys::iplSourceRemove(source.raw_ptr(), self.raw_ptr());
        }
    }

    /// Commits changes to the scene or probe batches used for simulation.
    ///
    /// Call this function after calling [`Self::set_scene`], [`Self::add_probe_batch`], or [`Self::remove_probe_batch`] for the changes to take effect.
    ///
    /// This function cannot be called while any simulation is running.
    pub fn commit(&mut self) {
        let _guards: Vec<_> = [
            self.direct_lock.as_ref(),
            self.reflections_lock.as_ref(),
            self.pathing_lock.as_ref(),
        ]
        .iter()
        .filter_map(|lock| lock.as_ref().map(|l| l.lock().unwrap()))
        .collect();

        unsafe { audionimbus_sys::iplSimulatorCommit(self.raw_ptr()) }

        self.committed_num_probes = self.pending_probe_batches.values().sum();

        if self.has_pending_scene {
            self.has_committed_scene = true;
            self.has_pending_scene = false;
        }
    }

    /// Specifies simulation parameters that are not associated with any particular source.
    ///
    /// # Arguments
    ///
    /// - `flags`: the types of simulation for which to specify shared inputs. If, for example, direct and reflections simulations are being run on separate threads, you can call this function on the direct simulation thread with [`SimulationFlags::DIRECT`], and on the reflections simulation thread with [`SimulationFlags::REFLECTIONS`], without requiring any synchronization between the calls.
    /// - `shared_inputs`: the shared input parameters to set.
    pub fn set_shared_inputs(
        &self,
        simulation_flags: SimulationFlags,
        shared_inputs: &SimulationSharedInputs,
    ) {
        let _guards = self.acquire_locks_for_flags(simulation_flags);

        unsafe {
            audionimbus_sys::iplSimulatorSetSharedInputs(
                self.raw_ptr(),
                simulation_flags.into(),
                &mut audionimbus_sys::IPLSimulationSharedInputs::from(shared_inputs),
            );
        }
    }

    /// Acquires locks for the simulation types specified in the given flags.
    fn acquire_locks_for_flags(&self, flags: SimulationFlags) -> Vec<MutexGuard<'_, ()>> {
        let mut guards = Vec::new();

        if flags.contains(SimulationFlags::DIRECT) {
            if let Some(lock) = &self.direct_lock {
                guards.push(lock.lock().unwrap());
            }
        }

        if flags.contains(SimulationFlags::REFLECTIONS) {
            if let Some(lock) = &self.reflections_lock {
                guards.push(lock.lock().unwrap());
            }
        }

        if flags.contains(SimulationFlags::PATHING) {
            if let Some(lock) = &self.pathing_lock {
                guards.push(lock.lock().unwrap());
            }
        }

        guards
    }

    /// Returns the raw FFI pointer to the underlying simulator.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub const fn raw_ptr(&self) -> audionimbus_sys::IPLSimulator {
        self.inner
    }

    /// Returns a mutable reference to the raw FFI pointer.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub const fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLSimulator {
        &mut self.inner
    }
}

impl<T, R, P> Simulator<'_, T, Direct, R, P>
where
    T: RayTracer,
    R: 'static,
    P: 'static,
{
    /// Runs a direct simulation for all sources added to the simulator.
    ///
    /// This may include distance attenuation, air absorption, directivity, occlusion, and transmission.
    ///
    /// # Performance Considerations
    ///
    /// This function should not be called from the audio processing thread if occlusion
    /// and/or transmission are enabled, as these calculations can be CPU-intensive.
    pub fn run_direct(&self) {
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
}

impl<T, D, P> Simulator<'_, T, D, Reflections, P>
where
    T: RayTracer,
    D: 'static,
    P: 'static,
{
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
    /// [`Simulator::set_scene`] and committed via [`Simulator::commit`] before
    /// running simulations.
    pub fn run_reflections(&self) -> Result<(), SimulationError> {
        let _guard = self
            .reflections_lock
            .as_ref()
            .expect("reflections_lock must exist when reflections simulation is enabled")
            .lock()
            .unwrap();

        if !self.has_committed_scene {
            return Err(SimulationError::ReflectionsWithoutScene);
        }

        unsafe {
            audionimbus_sys::iplSimulatorRunReflections(self.raw_ptr());
        }

        Ok(())
    }
}

impl<T, D, R> Simulator<'_, T, D, R, Pathing>
where
    T: RayTracer,
    D: 'static,
    R: 'static,
{
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
    /// via [`Simulator::add_probe_batch`] and committed via [`Simulator::commit`] before running
    /// simulations.
    pub fn run_pathing(&self) -> Result<(), SimulationError> {
        let _guard = self
            .pathing_lock
            .as_ref()
            .expect("pathing_lock must exist when pathing simulation is enabled")
            .lock()
            .unwrap();

        if self.committed_num_probes == 0 {
            return Err(SimulationError::PathingWithoutProbes);
        }

        unsafe {
            audionimbus_sys::iplSimulatorRunPathing(self.raw_ptr());
        }

        Ok(())
    }
}

impl<T: RayTracer, D, R, P> Clone for Simulator<'_, T, D, R, P> {
    fn clone(&self) -> Self {
        unsafe {
            audionimbus_sys::iplSimulatorRetain(self.inner);
        }

        Self {
            inner: self.inner,
            committed_num_probes: self.committed_num_probes,
            pending_probe_batches: self.pending_probe_batches.clone(),
            has_committed_scene: self.has_committed_scene,
            has_pending_scene: self.has_pending_scene,
            direct_lock: self.direct_lock.clone(),
            reflections_lock: self.reflections_lock.clone(),
            pathing_lock: self.pathing_lock.clone(),
            _ray_tracer: PhantomData,
            _direct: PhantomData,
            _reflections: PhantomData,
            _pathing: PhantomData,
            _lifetime: PhantomData,
        }
    }
}

impl<T: RayTracer, D, R, P> Drop for Simulator<'_, T, D, R, P> {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplSimulatorRelease(&raw mut self.inner) }
    }
}

unsafe impl<T: RayTracer, D, R, P> Send for Simulator<'_, T, D, R, P> {}
unsafe impl<T: RayTracer, D, R, P> Sync for Simulator<'_, T, D, R, P> {}

/// Settings used to create a [`Simulator`].
///
/// # Examples
///
/// ```
/// # use audionimbus::{Context, Simulator, DirectSimulationSettings, ReflectionsSimulationSettings, PathingSimulationSettings, SimulationSettings};
/// let settings = SimulationSettings::new(48000, 1024, 2)
///     .with_direct(DirectSimulationSettings {
///         max_num_occlusion_samples: 4,
///     })
///     .with_reflections(ReflectionsSimulationSettings::Convolution {
///         max_num_rays: 4096,
///         num_diffuse_samples: 32,
///         max_duration: 2.0,
///         max_num_sources: 8,
///         num_threads: 2,
///     })
///     .with_pathing(PathingSimulationSettings {
///         num_visibility_samples: 4,
///     });
/// # Ok::<(), audionimbus::SteamAudioError>(())
/// ```
#[derive(Debug)]
pub struct SimulationSettings<'a, T: RayTracer, D = (), R = (), P = ()> {
    settings: audionimbus_sys::IPLSimulationSettings,
    _ray_tracer: PhantomData<T>,
    _direct: PhantomData<D>,
    _reflections: PhantomData<R>,
    _pathing: PhantomData<P>,
    _lifetime: PhantomData<&'a ()>,
}

impl SimulationSettings<'_, DefaultRayTracer, (), (), ()> {
    /// Creates new simulation settings with all simulations disabled by default.
    pub const fn new(sampling_rate: u32, frame_size: u32, max_order: u32) -> Self {
        let settings = audionimbus_sys::IPLSimulationSettings {
            flags: audionimbus_sys::IPLSimulationFlags(0),
            sceneType: audionimbus_sys::IPLSceneType::IPL_SCENETYPE_DEFAULT,
            reflectionType:
                audionimbus_sys::IPLReflectionEffectType::IPL_REFLECTIONEFFECTTYPE_CONVOLUTION,
            maxNumOcclusionSamples: 0,
            maxNumRays: 0,
            numDiffuseSamples: 0,
            maxDuration: 0.0,
            maxOrder: max_order as i32,
            maxNumSources: 0,
            numThreads: 0,
            rayBatchSize: 0,
            numVisSamples: 0,
            samplingRate: sampling_rate as i32,
            frameSize: frame_size as i32,
            openCLDevice: std::ptr::null_mut(),
            radeonRaysDevice: std::ptr::null_mut(),
            tanDevice: std::ptr::null_mut(),
        };

        Self {
            settings,
            _ray_tracer: PhantomData,
            _direct: PhantomData,
            _reflections: PhantomData,
            _pathing: PhantomData,
            _lifetime: PhantomData,
        }
    }
}

impl<'a, D, R, P> SimulationSettings<'a, DefaultRayTracer, D, R, P> {
    /// Switches to the Embree ray tracer.
    pub fn with_embree(self) -> SimulationSettings<'a, Embree, (), (), ()> {
        let Self { mut settings, .. } = self;
        settings.sceneType = Embree::scene_type();

        SimulationSettings {
            settings,
            _ray_tracer: PhantomData,
            _direct: PhantomData,
            _reflections: PhantomData,
            _pathing: PhantomData,
            _lifetime: PhantomData,
        }
    }

    /// Switches to the Radeon Rays ray tracer.
    ///
    /// # Arguments
    ///
    /// - `open_cl_device`: The OpenCL device to use.
    /// - `radeon_rays_device`: The Radeon Rays device to use.
    pub fn with_radeon_rays(
        self,
        open_cl_device: &'a OpenClDevice,
        radeon_rays_device: &'a RadeonRaysDevice,
    ) -> SimulationSettings<'a, RadeonRays, (), (), ()> {
        let Self { mut settings, .. } = self;
        settings.sceneType = RadeonRays::scene_type();
        settings.openCLDevice = open_cl_device.raw_ptr();
        settings.radeonRaysDevice = radeon_rays_device.raw_ptr();

        SimulationSettings {
            settings,
            _ray_tracer: PhantomData,
            _direct: PhantomData,
            _reflections: PhantomData,
            _pathing: PhantomData,
            _lifetime: PhantomData,
        }
    }

    /// Switches to a custom ray tracer.
    ///
    /// # Arguments
    ///
    /// - `ray_batch_size`: The number of rays that will be passed to the callbacks every time rays need to be traced.
    pub fn with_custom_ray_tracer(
        self,
        ray_batch_size: u32,
    ) -> SimulationSettings<'a, CustomRayTracer, (), (), ()> {
        let Self { mut settings, .. } = self;
        settings.sceneType = CustomRayTracer::scene_type();
        settings.rayBatchSize = ray_batch_size as i32;

        SimulationSettings {
            settings,
            _ray_tracer: PhantomData,
            _direct: PhantomData,
            _reflections: PhantomData,
            _pathing: PhantomData,
            _lifetime: PhantomData,
        }
    }
}

impl<'a, T: RayTracer, D, R, P> SimulationSettings<'a, T, D, R, P> {
    /// Enables direct simulation.
    pub fn with_direct(
        self,
        direct_settings: DirectSimulationSettings,
    ) -> SimulationSettings<'a, T, Direct, R, P> {
        let Self {
            mut settings,
            _ray_tracer,
            _reflections,
            _pathing,
            _lifetime,
            ..
        } = self;

        settings.flags |= audionimbus_sys::IPLSimulationFlags::IPL_SIMULATIONFLAGS_DIRECT;
        settings.maxNumOcclusionSamples = direct_settings.max_num_occlusion_samples as i32;

        SimulationSettings {
            settings,
            _ray_tracer,
            _direct: PhantomData,
            _reflections,
            _pathing,
            _lifetime,
        }
    }

    /// Enables reflections simulation.
    pub fn with_reflections(
        self,
        reflections_settings: ReflectionsSimulationSettings<'static>,
    ) -> SimulationSettings<'a, T, D, Reflections, P> {
        let Self {
            mut settings,
            _ray_tracer,
            _direct,
            _pathing,
            _lifetime,
            ..
        } = self;

        settings.flags |= audionimbus_sys::IPLSimulationFlags::IPL_SIMULATIONFLAGS_REFLECTIONS;

        let (
            reflection_effect_type,
            max_num_rays,
            num_diffuse_samples,
            max_duration,
            max_num_sources,
            num_threads,
        ) = match reflections_settings {
            ReflectionsSimulationSettings::Convolution {
                max_num_rays,
                num_diffuse_samples,
                max_duration,
                max_num_sources,
                num_threads,
            } => (
                audionimbus_sys::IPLReflectionEffectType::IPL_REFLECTIONEFFECTTYPE_CONVOLUTION,
                max_num_rays,
                num_diffuse_samples,
                max_duration,
                max_num_sources,
                num_threads,
            ),
            ReflectionsSimulationSettings::Parametric {
                max_num_rays,
                num_diffuse_samples,
                max_duration,
                max_num_sources,
                num_threads,
            } => (
                audionimbus_sys::IPLReflectionEffectType::IPL_REFLECTIONEFFECTTYPE_PARAMETRIC,
                max_num_rays,
                num_diffuse_samples,
                max_duration,
                max_num_sources,
                num_threads,
            ),
            ReflectionsSimulationSettings::Hybrid {
                max_num_rays,
                num_diffuse_samples,
                max_duration,
                max_num_sources,
                num_threads,
            } => (
                audionimbus_sys::IPLReflectionEffectType::IPL_REFLECTIONEFFECTTYPE_HYBRID,
                max_num_rays,
                num_diffuse_samples,
                max_duration,
                max_num_sources,
                num_threads,
            ),
            ReflectionsSimulationSettings::TrueAudioNext {
                max_num_rays,
                num_diffuse_samples,
                max_duration,
                max_num_sources,
                num_threads,
                open_cl_device,
                true_audio_next_device,
            } => {
                settings.openCLDevice = open_cl_device.raw_ptr();
                settings.tanDevice = true_audio_next_device.raw_ptr();

                (
                    audionimbus_sys::IPLReflectionEffectType::IPL_REFLECTIONEFFECTTYPE_TAN,
                    max_num_rays,
                    num_diffuse_samples,
                    max_duration,
                    max_num_sources,
                    num_threads,
                )
            }
        };

        settings.reflectionType = reflection_effect_type;
        settings.maxNumRays = max_num_rays as i32;
        settings.numDiffuseSamples = num_diffuse_samples as i32;
        settings.maxDuration = max_duration;
        settings.maxNumSources = max_num_sources as i32;
        settings.numThreads = num_threads as i32;

        SimulationSettings {
            settings,
            _ray_tracer,
            _direct,
            _reflections: PhantomData,
            _pathing,
            _lifetime,
        }
    }

    /// Enables pathing simulation.
    pub fn with_pathing(
        self,
        pathing_settings: PathingSimulationSettings,
    ) -> SimulationSettings<'a, T, D, R, Pathing> {
        let Self {
            mut settings,
            _ray_tracer,
            _direct,
            _reflections,
            _lifetime,
            ..
        } = self;

        settings.flags |= audionimbus_sys::IPLSimulationFlags::IPL_SIMULATIONFLAGS_PATHING;
        settings.numVisSamples = pathing_settings.num_visibility_samples as i32;

        SimulationSettings {
            settings,
            _ray_tracer,
            _direct,
            _reflections,
            _pathing: PhantomData,
            _lifetime,
        }
    }

    /// Converts the settings to the FFI representation.
    pub const fn to_ffi(&self) -> audionimbus_sys::IPLSimulationSettings {
        self.settings
    }
}

/// Settings used for direct path simulation.
#[derive(Debug, Copy, Clone)]
pub struct DirectSimulationSettings {
    /// The maximum number of point samples to consider when calculating occlusion using the volumetric occlusion algorithm.
    /// Different sources can use different numbers of samples, and the number of samples can change between simulation runs, but this is the maximum value.
    /// Increasing this value results in smoother occlusion transitions, at the cost of increased CPU usage.
    pub max_num_occlusion_samples: u32,
}

/// Settings used for reflections simulation.
#[derive(Debug, Copy, Clone)]
pub enum ReflectionsSimulationSettings<'a> {
    /// Multi-channel convolution reverb.
    Convolution {
        /// The maximum number of rays to trace from the listener when simulating reflections.
        /// You can use different numbers of rays between simulation runs, but this is the maximum value.
        /// Increasing this value results in more accurate reflections, at the cost of increased CPU usage.
        max_num_rays: u32,

        /// The number of directions to sample when generating diffusely reflected rays.
        /// Increasing this value may increase the accuracy of diffuse reflections.
        num_diffuse_samples: u32,

        /// The maximum length (in seconds) of impulse responses generated by reflection simulations.
        /// You can change this value betweeen simulation runs, but this is the maximum value.
        /// Increasing this value results in longer, more accurate reverb tails, at the cost of increased CPU and memory usage.
        max_duration: f32,

        /// The maximum number of sources for which reflection simulations will be run at any given time.
        max_num_sources: u32,

        /// The number of threads used for real-time reflection simulations.
        num_threads: u32,
    },

    /// Parametric (or artificial) reverb, using feedback delay networks.
    Parametric {
        /// The maximum number of rays to trace from the listener when simulating reflections.
        /// You can use different numbers of rays between simulation runs, but this is the maximum value.
        /// Increasing this value results in more accurate reflections, at the cost of increased CPU usage.
        max_num_rays: u32,

        /// The number of directions to sample when generating diffusely reflected rays.
        /// Increasing this value may increase the accuracy of diffuse reflections.
        num_diffuse_samples: u32,

        /// The maximum length (in seconds) of impulse responses generated by reflection simulations.
        /// You can change this value betweeen simulation runs, but this is the maximum value.
        /// Increasing this value results in longer, more accurate reverb tails, at the cost of increased CPU and memory usage.
        max_duration: f32,

        /// The maximum number of sources for which reflection simulations will be run at any given time.
        max_num_sources: u32,

        /// The number of threads used for real-time reflection simulations.
        num_threads: u32,
    },

    /// A hybrid of convolution and parametric reverb.
    Hybrid {
        /// The maximum number of rays to trace from the listener when simulating reflections.
        /// You can use different numbers of rays between simulation runs, but this is the maximum value.
        /// Increasing this value results in more accurate reflections, at the cost of increased CPU usage.
        max_num_rays: u32,

        /// The number of directions to sample when generating diffusely reflected rays.
        /// Increasing this value may increase the accuracy of diffuse reflections.
        num_diffuse_samples: u32,

        /// The maximum length (in seconds) of impulse responses generated by reflection simulations.
        /// You can change this value betweeen simulation runs, but this is the maximum value.
        /// Increasing this value results in longer, more accurate reverb tails, at the cost of increased CPU and memory usage.
        max_duration: f32,

        /// The maximum number of sources for which reflection simulations will be run at any given time.
        max_num_sources: u32,

        /// The number of threads used for real-time reflection simulations.
        num_threads: u32,
    },

    /// Multi-channel convolution reverb, using AMD TrueAudio Next for GPU acceleration.
    TrueAudioNext {
        /// The maximum number of rays to trace from the listener when simulating reflections.
        /// You can use different numbers of rays between simulation runs, but this is the maximum value.
        /// Increasing this value results in more accurate reflections, at the cost of increased CPU usage.
        max_num_rays: u32,

        /// The number of directions to sample when generating diffusely reflected rays.
        /// Increasing this value may increase the accuracy of diffuse reflections.
        num_diffuse_samples: u32,

        /// The maximum length (in seconds) of impulse responses generated by reflection simulations.
        /// You can change this value betweeen simulation runs, but this is the maximum value.
        /// Increasing this value results in longer, more accurate reverb tails, at the cost of increased CPU and memory usage.
        max_duration: f32,

        /// The maximum number of sources for which reflection simulations will be run at any given time.
        max_num_sources: u32,

        /// The number of threads used for real-time reflection simulations.
        num_threads: u32,

        /// The OpenCL device being used.
        open_cl_device: &'a OpenClDevice,

        /// The TrueAudio Next device being used.
        true_audio_next_device: &'a TrueAudioNextDevice,
    },
}

/// Settings used for pathing simulation.
#[derive(Debug, Copy, Clone)]
pub struct PathingSimulationSettings {
    /// The number of point samples to consider when calculating probe-to-probe visibility for pathing simulations.
    /// Baked paths may end up being occluded by dynamic objects, in which case you can configure the simulator to look for alternate paths in real time.
    /// This process will involve checking visibility between probes.
    pub num_visibility_samples: u32,
}

bitflags::bitflags! {
    /// Flags indicating which types of simulation should be enabled.
    #[derive(Copy, Clone, Debug)]
    pub struct SimulationFlags: u32 {
        /// Enable direct simulation.
        /// This includes distance attenuation, air absorption, directivity, occlusion, and transmission.
        const DIRECT = 1 << 0;

        /// Enable reflections simulation.
        /// This includes both real-time and baked simulation.
        const REFLECTIONS = 1 << 1;

        /// Enable pathing simulation.
        const PATHING = 1 << 2;
    }
}

impl From<SimulationFlags> for audionimbus_sys::IPLSimulationFlags {
    fn from(simulation_flags: SimulationFlags) -> Self {
        Self(simulation_flags.bits() as _)
    }
}

/// A sound source, for the purposes of simulation.
///
/// This object is used to specify various parameters for direct and indirect sound propagation simulation, and to retrieve the simulation results.
#[derive(Debug)]
pub struct Source {
    inner: audionimbus_sys::IPLSource,

    /// Reference to the simulator's direct simulation lock.
    /// Used to synchronize access to direct simulation data.
    direct_lock: Option<Arc<Mutex<()>>>,

    /// Reference to the simulator's reflections simulation lock.
    /// Used to synchronize access to reflections simulation data.
    reflections_lock: Option<Arc<Mutex<()>>>,

    /// Reference to the simulator's pathing simulation lock.
    /// Used to synchronize access to pathing simulation data.
    pathing_lock: Option<Arc<Mutex<()>>>,
}

impl Source {
    /// Creates a new source.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if creation fails.
    pub fn try_new<T, D, R, P>(
        simulator: &Simulator<T, D, R, P>,
        source_settings: &SourceSettings,
    ) -> Result<Self, SteamAudioError>
    where
        T: RayTracer,
        D: 'static,
        R: 'static,
        P: 'static,
    {
        let mut inner = std::ptr::null_mut();

        let status = unsafe {
            audionimbus_sys::iplSourceCreate(
                simulator.raw_ptr(),
                &mut audionimbus_sys::IPLSourceSettings::from(source_settings),
                &mut inner,
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        let direct_lock = simulator.direct_lock.clone();
        let reflections_lock = simulator.reflections_lock.clone();
        let pathing_lock = simulator.pathing_lock.clone();

        let source = Self {
            inner,
            direct_lock,
            reflections_lock,
            pathing_lock,
        };

        Ok(source)
    }

    /// Specifies simulation parameters for a source.
    ///
    /// # Threading Considerations
    ///
    /// This method will block if a simulation is currently running for the
    /// specified type(s). Call this from the same thread that runs the corresponding
    /// simulation(s) to avoid blocking.
    ///
    /// It is safe to call this method concurrently with simulations of different types.
    /// For example, setting `DIRECT` inputs while a `REFLECTIONS` simulation is running
    /// will not block.
    ///
    /// # Arguments
    ///
    /// - `flags`: the types of simulation for which to specify inputs. If, for example, direct and reflections simulations are being run on separate threads, you can call this function on the direct simulation thread with [`SimulationFlags::DIRECT`], and on the reflections simulation thread with [`SimulationFlags::REFLECTIONS`], without requiring any synchronization between the calls.
    /// - `inputs`: the input parameters to set.
    pub fn set_inputs(&mut self, simulation_flags: SimulationFlags, inputs: SimulationInputs) {
        let _guards = self.acquire_locks_for_flags(simulation_flags);

        unsafe {
            audionimbus_sys::iplSourceSetInputs(
                self.raw_ptr(),
                simulation_flags.into(),
                &mut inputs.into(),
            );
        }
    }

    /// Retrieves simulation results for a source.
    ///
    /// # ⚠️ Critical Threading Considerations
    ///
    /// This method MUST NOT be called from a real-time audio thread!
    ///
    /// This method will block while a simulation is running for the specified type(s).
    /// Since simulations can take 50-500ms to complete, calling this from an audio thread
    /// will likely cause audio dropouts, glitches, and severe performance problems.
    ///
    /// Recommended multi-threading pattern:
    /// - Simulation thread: run simulation, then call `get_outputs()` to retrieve results
    /// - Communication layer: send results to audio thread via non-blocking mechanism (e.g.,
    ///   triple buffering or lock-free queue)
    /// - Audio thread: receive results and apply effects
    ///
    /// # Arguments
    ///
    /// - `flags`: the types of simulation for which to retrieve results.
    ///
    /// # Errors
    ///
    /// Returns a [`SteamAudioError`] on failure to allocate sufficient memory for the
    /// [`SimulationOutputs`].
    pub fn get_outputs(
        &mut self,
        simulation_flags: SimulationFlags,
    ) -> Result<SimulationOutputs, SteamAudioError> {
        let _guards = self.acquire_locks_for_flags(simulation_flags);

        let simulation_outputs = SimulationOutputs::try_allocate()?;

        unsafe {
            audionimbus_sys::iplSourceGetOutputs(
                self.raw_ptr(),
                simulation_flags.into(),
                simulation_outputs.raw_ptr(),
            );
        }

        Ok(simulation_outputs)
    }

    /// Acquires locks for the simulation types specified in the given flags.
    fn acquire_locks_for_flags(&self, flags: SimulationFlags) -> Vec<MutexGuard<'_, ()>> {
        let mut guards = Vec::new();

        if flags.contains(SimulationFlags::DIRECT) {
            if let Some(lock) = &self.direct_lock {
                guards.push(lock.lock().unwrap());
            }
        }

        if flags.contains(SimulationFlags::REFLECTIONS) {
            if let Some(lock) = &self.reflections_lock {
                guards.push(lock.lock().unwrap());
            }
        }

        if flags.contains(SimulationFlags::PATHING) {
            if let Some(lock) = &self.pathing_lock {
                guards.push(lock.lock().unwrap());
            }
        }

        guards
    }

    /// Returns the raw FFI pointer to the underlying source.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub const fn raw_ptr(&self) -> audionimbus_sys::IPLSource {
        self.inner
    }

    /// Returns a mutable reference to the raw FFI pointer.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub const fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLSource {
        &mut self.inner
    }
}

impl Clone for Source {
    fn clone(&self) -> Self {
        unsafe {
            audionimbus_sys::iplSourceRetain(self.inner);
        }

        Self {
            inner: self.inner,
            direct_lock: self.direct_lock.clone(),
            reflections_lock: self.reflections_lock.clone(),
            pathing_lock: self.pathing_lock.clone(),
        }
    }
}

impl Drop for Source {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplSourceRelease(&raw mut self.inner) }
    }
}

unsafe impl Send for Source {}
unsafe impl Sync for Source {}

/// Settings used to create a source.
#[derive(Debug)]
pub struct SourceSettings {
    /// The types of simulation that may be run for this source.
    pub flags: SimulationFlags,
}

impl From<&SourceSettings> for audionimbus_sys::IPLSourceSettings {
    fn from(settings: &SourceSettings) -> Self {
        Self {
            flags: settings.flags.into(),
        }
    }
}

/// Simulation parameters for a source.
#[derive(Debug, Copy, Clone)]
pub struct SimulationInputs<'a> {
    /// The position and orientation of this source.
    pub source: CoordinateSystem,

    /// If `Some`, enables direct simulation. This includes distance attenuation, air absorption, directivity, occlusion, and transmission.
    pub direct_simulation: Option<DirectSimulationParameters>,

    /// If `Some`, enables reflections simulation. This includes both real-time and baked simulation.
    pub reflections_simulation: Option<ReflectionsSimulationParameters>,

    /// If `Some`, enables pathing simulation.
    pub pathing_simulation: Option<PathingSimulationParameters<'a>>,
}

impl<'a> SimulationInputs<'a> {
    /// Crates new [`SimulationInputs`] with all simulations disabled by default.
    pub fn new(source: CoordinateSystem) -> Self {
        Self {
            source,
            direct_simulation: None,
            reflections_simulation: None,
            pathing_simulation: None,
        }
    }

    /// Enables direct simulation with the specified parameters.
    pub fn with_direct(mut self, params: DirectSimulationParameters) -> Self {
        self.direct_simulation = Some(params);
        self
    }

    /// Enables reflections simulation with the specified parameters.
    pub fn with_reflections(mut self, params: ReflectionsSimulationParameters) -> Self {
        self.reflections_simulation = Some(params);
        self
    }

    /// Enables pathing simulation with the specified parameters.
    pub fn with_pathing(mut self, params: PathingSimulationParameters<'a>) -> Self {
        self.pathing_simulation = Some(params);
        self
    }
}

/// Direct simulation parameters for a source.
#[derive(Default, Debug, Copy, Clone)]
pub struct DirectSimulationParameters {
    /// If `Some`, enables distance attenuation calculations with the specified model.
    pub distance_attenuation: Option<DistanceAttenuationModel>,

    /// If `Some`, enables air absorption calculations with the specified model.
    pub air_absorption: Option<AirAbsorptionModel>,

    /// If `Some`, enables directivity calculations with the specified directivity pattern.
    pub directivity: Option<Directivity>,

    /// If `Some`, enables occlusion simulation.
    pub occlusion: Option<Occlusion>,
}

impl DirectSimulationParameters {
    /// Creates new [`DirectSimulationParameters`] with all calculations disabled by default.
    pub fn new() -> Self {
        Self {
            distance_attenuation: None,
            air_absorption: None,
            directivity: None,
            occlusion: None,
        }
    }

    /// Enables distance attenuation with the specified model.
    pub fn with_distance_attenuation(mut self, model: DistanceAttenuationModel) -> Self {
        self.distance_attenuation = Some(model);
        self
    }

    /// Enables air absorption with the specified model.
    pub fn with_air_absorption(mut self, model: AirAbsorptionModel) -> Self {
        self.air_absorption = Some(model);
        self
    }

    /// Enables directivity with the specified pattern.
    pub fn with_directivity(mut self, directivity: Directivity) -> Self {
        self.directivity = Some(directivity);
        self
    }

    /// Enables occlusion with the specified parameters.
    pub fn with_occlusion(mut self, occlusion: Occlusion) -> Self {
        self.occlusion = Some(occlusion);
        self
    }
}

/// Occlusion parameters.
#[derive(Debug, Copy, Clone)]
pub struct Occlusion {
    /// If `Some`, enables transmission simulation.
    pub transmission: Option<TransmissionParameters>,

    /// The occlusion algorithm to use.
    pub algorithm: OcclusionAlgorithm,
}

impl Occlusion {
    /// Creates a new [`Occlusion`] with transmission simulation disabled by default.
    pub fn new(algorithm: OcclusionAlgorithm) -> Self {
        Self {
            transmission: None,
            algorithm,
        }
    }

    /// Enables transmission simulation with the specified parameters.
    pub fn with_transmission(mut self, params: TransmissionParameters) -> Self {
        self.transmission = Some(params);
        self
    }
}

/// Transmission parameters.
#[derive(Debug, Copy, Clone)]
pub struct TransmissionParameters {
    /// If simulating transmission, this is the maximum number of surfaces, starting from the closest surface to the listener, whose transmission coefficients will be considered when calculating the total amount of sound transmitted.
    /// Increasing this value will result in more accurate results when multiple surfaces lie between the source and the listener, at the cost of increased CPU usage.
    pub num_transmission_rays: u32,
}

/// Reflections simulation parameters for a source.
#[derive(Debug, Copy, Clone)]
pub enum ReflectionsSimulationParameters {
    /// Multi-channel convolution reverb.
    Convolution {
        /// The optional identifier used to specify which layer of baked data to use for simulating reflections for this source.
        baked_data_identifier: Option<BakedDataIdentifier>,
    },

    /// Parametric (or artificial) reverb, using feedback delay networks.
    Parametric {
        /// The reverb decay times for each frequency band are scaled by these values.
        /// Set to `[1.0, 1.0, 1.0]` to use the simulated values without modification.
        reverb_scale: [f32; 3],

        /// The optional identifier used to specify which layer of baked data to use for simulating reflections for this source.
        baked_data_identifier: Option<BakedDataIdentifier>,
    },

    /// A hybrid of convolution and parametric reverb.
    Hybrid {
        /// The reverb decay times for each frequency band are scaled by these values.
        /// Set to `[1.0, 1.0, 1.0]` to use the simulated values without modification.
        reverb_scale: [f32; 3],

        /// This is the length (in seconds) of impulse response to use for convolution reverb.
        /// The rest of the impulse response will be used for parametric reverb estimation only.
        /// Increasing this value results in more accurate reflections, at the cost of increased CPU usage.
        hybrid_reverb_transition_time: f32,

        /// This is the amount of overlap between the convolution and parametric parts.
        /// To ensure smooth transitions from the early convolution part to the late parametric part, the two are cross-faded towards the end of the convolution part.
        /// For example, if `hybrid_reverb_transition_time` is 1.0, and `hybrid_reverb_overlap_percent` is 0.25, then the first 0.75 seconds are pure convolution, the next 0.25 seconds are a blend between convolution and parametric, and the portion of the tail beyond 1.0 second is pure parametric.
        hybrid_reverb_overlap_percent: f32,

        /// The optional identifier used to specify which layer of baked data to use for simulating reflections for this source.
        baked_data_identifier: Option<BakedDataIdentifier>,
    },

    /// Multi-channel convolution reverb, using AMD TrueAudio Next for GPU acceleration.
    TrueAudioNext {
        /// The optional identifier used to specify which layer of baked data to use for simulating reflections for this source.
        baked_data_identifier: Option<BakedDataIdentifier>,
    },
}

/// Pathing simulation parameters for a source.
#[derive(Debug, Copy, Clone)]
pub struct PathingSimulationParameters<'a> {
    /// The probe batch within which to find paths from this source to the listener.
    pub pathing_probes: &'a ProbeBatch,

    /// When testing for mutual visibility between a pair of probes, each probe is treated as a sphere of this radius (in meters), and point samples are generated within this sphere.
    pub visibility_radius: f32,

    /// When tracing rays to test for mutual visibility between a pair of probes, the fraction of rays that are unoccluded must be greater than this threshold for the pair of probes to be considered mutually visible.
    pub visibility_threshold: f32,

    /// If the distance between two probes is greater than this value, the probes are not considered mutually visible.
    /// Increasing this value can result in simpler paths, at the cost of increased CPU usage.
    pub visibility_range: f32,

    /// The Ambisonic order used for representing path directionality.
    /// Higher values result in more precise spatialization of paths, at the cost of increased CPU usage.
    pub pathing_order: u32,

    /// If `true`, baked paths are tested for visibility.
    /// This is useful if your scene has dynamic objects that might occlude baked paths.
    pub enable_validation: bool,

    /// If `true`, and [`Self::enable_validation`] is `true`, then if a baked path is occluded by dynamic geometry, path finding is re-run in real-time to find alternate paths that take into account the dynamic geometry.
    pub find_alternate_paths: bool,

    /// The deviation model to use for this source.
    pub deviation: DeviationModel,
}

/// Intermediate representation of direct simulation parameters for FFI conversion.
struct DirectSimulationData {
    flags: audionimbus_sys::IPLDirectSimulationFlags,
    distance_attenuation_model: DistanceAttenuationModel,
    air_absorption_model: AirAbsorptionModel,
    directivity: Directivity,
    occlusion_type: audionimbus_sys::IPLOcclusionType,
    occlusion_radius: f32,
    num_occlusion_samples: i32,
    num_transmission_rays: i32,
}

impl DirectSimulationData {
    /// Converts optional direct simulation parameters into concrete FFI-compatible data.
    fn from_params(params: Option<DirectSimulationParameters>) -> Self {
        let Some(params) = params else {
            return Self::default();
        };

        let mut flags = audionimbus_sys::IPLDirectSimulationFlags(0);

        let distance_attenuation_model =
            Self::process_distance_attenuation(params.distance_attenuation, &mut flags);

        let air_absorption_model = Self::process_air_absorption(params.air_absorption, &mut flags);

        let directivity = Self::process_directivity(params.directivity, &mut flags);

        let (occlusion_type, occlusion_radius, num_occlusion_samples, num_transmission_rays) =
            Self::process_occlusion(params.occlusion, &mut flags);

        Self {
            flags,
            distance_attenuation_model,
            air_absorption_model,
            directivity,
            occlusion_type,
            occlusion_radius,
            num_occlusion_samples,
            num_transmission_rays,
        }
    }

    /// Processes optional distance attenuation settings and updates flags accordingly.
    fn process_distance_attenuation(
        distance_attenuation: Option<DistanceAttenuationModel>,
        flags: &mut audionimbus_sys::IPLDirectSimulationFlags,
    ) -> DistanceAttenuationModel {
        if let Some(model) = distance_attenuation {
            *flags |= audionimbus_sys::IPLDirectSimulationFlags::IPL_DIRECTSIMULATIONFLAGS_DISTANCEATTENUATION;
            model
        } else {
            DistanceAttenuationModel::default()
        }
    }

    /// Processes optional air absorption settings and updates flags accordingly.
    fn process_air_absorption(
        air_absorption: Option<AirAbsorptionModel>,
        flags: &mut audionimbus_sys::IPLDirectSimulationFlags,
    ) -> AirAbsorptionModel {
        if let Some(model) = air_absorption {
            *flags |=
                audionimbus_sys::IPLDirectSimulationFlags::IPL_DIRECTSIMULATIONFLAGS_AIRABSORPTION;
            model
        } else {
            AirAbsorptionModel::default()
        }
    }

    /// Processes optional directivity settings and updates flags accordingly.
    fn process_directivity(
        directivity: Option<Directivity>,
        flags: &mut audionimbus_sys::IPLDirectSimulationFlags,
    ) -> Directivity {
        if let Some(directivity) = directivity {
            *flags |=
                audionimbus_sys::IPLDirectSimulationFlags::IPL_DIRECTSIMULATIONFLAGS_DIRECTIVITY;
            directivity
        } else {
            Directivity::default()
        }
    }

    /// Processes optional occlusion settings and updates flags accordingly.
    fn process_occlusion(
        occlusion: Option<Occlusion>,
        flags: &mut audionimbus_sys::IPLDirectSimulationFlags,
    ) -> (audionimbus_sys::IPLOcclusionType, f32, i32, i32) {
        let Some(occlusion) = occlusion else {
            return (
                audionimbus_sys::IPLOcclusionType::IPL_OCCLUSIONTYPE_RAYCAST,
                0.0,
                0,
                0,
            );
        };

        *flags |= audionimbus_sys::IPLDirectSimulationFlags::IPL_DIRECTSIMULATIONFLAGS_OCCLUSION;

        let (occlusion_type, occlusion_radius, num_occlusion_samples) = match occlusion.algorithm {
            OcclusionAlgorithm::Raycast => (
                audionimbus_sys::IPLOcclusionType::IPL_OCCLUSIONTYPE_RAYCAST,
                0.0,
                0,
            ),
            OcclusionAlgorithm::Volumetric {
                radius,
                num_occlusion_samples,
            } => (
                audionimbus_sys::IPLOcclusionType::IPL_OCCLUSIONTYPE_VOLUMETRIC,
                radius,
                num_occlusion_samples,
            ),
        };

        let num_transmission_rays = if let Some(transmission) = occlusion.transmission {
            *flags |=
                audionimbus_sys::IPLDirectSimulationFlags::IPL_DIRECTSIMULATIONFLAGS_TRANSMISSION;
            transmission.num_transmission_rays
        } else {
            0
        };

        (
            occlusion_type,
            occlusion_radius,
            num_occlusion_samples as i32,
            num_transmission_rays as i32,
        )
    }
}

impl Default for DirectSimulationData {
    fn default() -> Self {
        Self {
            flags: audionimbus_sys::IPLDirectSimulationFlags(0),
            distance_attenuation_model: DistanceAttenuationModel::default(),
            air_absorption_model: AirAbsorptionModel::default(),
            directivity: Directivity::default(),
            occlusion_type: audionimbus_sys::IPLOcclusionType::IPL_OCCLUSIONTYPE_RAYCAST,
            occlusion_radius: 0.0,
            num_occlusion_samples: 0,
            num_transmission_rays: 0,
        }
    }
}

/// Intermediate representation of reflections simulation parameters for FFI conversion.
struct ReflectionsSimulationData {
    baked: audionimbus_sys::IPLbool,
    baked_data_identifier: BakedDataIdentifier,
    reverb_scale: [f32; 3],
    hybrid_reverb_transition_time: f32,
    hybrid_reverb_overlap_percent: f32,
}

impl ReflectionsSimulationData {
    /// Converts optional reflections simulation parameters into concrete FFI-compatible data.
    fn from_params(params: Option<ReflectionsSimulationParameters>) -> Self {
        let Some(params) = params else {
            return Self::default();
        };

        let (baked_data_id_opt, reverb_scale, transition_time, overlap_percent) = match params {
            ReflectionsSimulationParameters::Convolution {
                baked_data_identifier,
            }
            | ReflectionsSimulationParameters::TrueAudioNext {
                baked_data_identifier,
            } => (baked_data_identifier, [0.0; 3], 0.0, 0.0),
            ReflectionsSimulationParameters::Parametric {
                reverb_scale,
                baked_data_identifier,
            } => (baked_data_identifier, reverb_scale, 0.0, 0.0),
            ReflectionsSimulationParameters::Hybrid {
                reverb_scale,
                hybrid_reverb_transition_time,
                hybrid_reverb_overlap_percent,
                baked_data_identifier,
            } => (
                baked_data_identifier,
                reverb_scale,
                hybrid_reverb_transition_time,
                hybrid_reverb_overlap_percent,
            ),
        };

        let (baked, baked_data_identifier) = baked_data_id_opt.map_or(
            (
                audionimbus_sys::IPLbool::IPL_FALSE,
                BakedDataIdentifier::Reflections {
                    variation: BakedDataVariation::Reverb,
                },
            ),
            |id| (audionimbus_sys::IPLbool::IPL_TRUE, id),
        );

        Self {
            baked,
            baked_data_identifier,
            reverb_scale,
            hybrid_reverb_transition_time: transition_time,
            hybrid_reverb_overlap_percent: overlap_percent,
        }
    }
}

impl Default for ReflectionsSimulationData {
    fn default() -> Self {
        Self {
            baked: audionimbus_sys::IPLbool::IPL_FALSE,
            baked_data_identifier: BakedDataIdentifier::Reflections {
                variation: BakedDataVariation::Reverb,
            },
            reverb_scale: [0.0; 3],
            hybrid_reverb_transition_time: 0.0,
            hybrid_reverb_overlap_percent: 0.0,
        }
    }
}

/// Intermediate representation of pathing simulation parameters for FFI conversion.
struct PathingSimulationData {
    pathing_probes: audionimbus_sys::IPLProbeBatch,
    visibility_radius: f32,
    visibility_threshold: f32,
    visibility_range: f32,
    pathing_order: i32,
    enable_validation: audionimbus_sys::IPLbool,
    find_alternate_paths: audionimbus_sys::IPLbool,
    deviation_model: *mut audionimbus_sys::IPLDeviationModel,
}

impl PathingSimulationData {
    /// Converts optional pathing simulation parameters into concrete FFI-compatible data.
    fn from_params(params: Option<PathingSimulationParameters>) -> Self {
        let Some(params) = params else {
            return Self::default();
        };

        Self {
            pathing_probes: params.pathing_probes.raw_ptr(),
            visibility_radius: params.visibility_radius,
            visibility_threshold: params.visibility_threshold,
            visibility_range: params.visibility_range,
            pathing_order: params.pathing_order as i32,
            enable_validation: if params.enable_validation {
                audionimbus_sys::IPLbool::IPL_TRUE
            } else {
                audionimbus_sys::IPLbool::IPL_FALSE
            },
            find_alternate_paths: if params.find_alternate_paths {
                audionimbus_sys::IPLbool::IPL_TRUE
            } else {
                audionimbus_sys::IPLbool::IPL_FALSE
            },
            // FIXME: Potential memory leak: this prevents dangling pointers, but there is no guarantee it will be freed by the C library.
            deviation_model: Box::into_raw(Box::new((&params.deviation).into())),
        }
    }
}

impl Default for PathingSimulationData {
    fn default() -> Self {
        Self {
            pathing_probes: std::ptr::null_mut(),
            visibility_radius: 0.0,
            visibility_threshold: 0.0,
            visibility_range: 0.0,
            pathing_order: 0,
            enable_validation: audionimbus_sys::IPLbool::IPL_FALSE,
            find_alternate_paths: audionimbus_sys::IPLbool::IPL_FALSE,
            deviation_model: std::ptr::null_mut(),
        }
    }
}

impl From<SimulationInputs<'_>> for audionimbus_sys::IPLSimulationInputs {
    fn from(simulation_inputs: SimulationInputs) -> Self {
        let SimulationInputs {
            source,
            direct_simulation,
            reflections_simulation,
            pathing_simulation,
        } = simulation_inputs;

        let mut flags = audionimbus_sys::IPLSimulationFlags(0);

        if direct_simulation.is_some() {
            flags |= audionimbus_sys::IPLSimulationFlags::IPL_SIMULATIONFLAGS_DIRECT;
        }
        let direct_data = DirectSimulationData::from_params(direct_simulation);

        if reflections_simulation.is_some() {
            flags |= audionimbus_sys::IPLSimulationFlags::IPL_SIMULATIONFLAGS_REFLECTIONS;
        }
        let reflections_data = ReflectionsSimulationData::from_params(reflections_simulation);

        if pathing_simulation.is_some() {
            flags |= audionimbus_sys::IPLSimulationFlags::IPL_SIMULATIONFLAGS_PATHING;
        }
        let pathing_data = PathingSimulationData::from_params(pathing_simulation);

        Self {
            flags,
            directFlags: direct_data.flags,
            source: source.into(),
            distanceAttenuationModel: (&direct_data.distance_attenuation_model).into(),
            airAbsorptionModel: (&direct_data.air_absorption_model).into(),
            directivity: (&direct_data.directivity).into(),
            occlusionType: direct_data.occlusion_type,
            occlusionRadius: direct_data.occlusion_radius,
            numOcclusionSamples: direct_data.num_occlusion_samples,
            numTransmissionRays: direct_data.num_transmission_rays,
            reverbScale: reflections_data.reverb_scale,
            hybridReverbTransitionTime: reflections_data.hybrid_reverb_transition_time,
            hybridReverbOverlapPercent: reflections_data.hybrid_reverb_overlap_percent,
            baked: reflections_data.baked,
            bakedDataIdentifier: reflections_data.baked_data_identifier.into(),
            pathingProbes: pathing_data.pathing_probes,
            visRadius: pathing_data.visibility_radius,
            visThreshold: pathing_data.visibility_threshold,
            visRange: pathing_data.visibility_range,
            pathingOrder: pathing_data.pathing_order,
            enableValidation: pathing_data.enable_validation,
            findAlternatePaths: pathing_data.find_alternate_paths,
            deviationModel: pathing_data.deviation_model,
        }
    }
}

/// The different algorithms for simulating occlusion.
#[derive(Copy, Clone, Debug)]
pub enum OcclusionAlgorithm {
    /// Raycast occlusion.
    /// A single ray is traced from the listener to the source.
    /// If the ray hits a solid object before it reaches the source, the source is considered occluded.
    Raycast,

    /// A volumetric occlusion algorithm that can model partial occlusion.
    /// The source is modeled as a sphere with a configurable radius.
    /// Multiple points are sampled within the volume of this sphere.
    /// Rays are then traced from each sample point to both the source and the listener.
    /// A sample point is considered occluded if either of these two rays is occluded.
    /// The occlusion value for the source is calculated as the fraction of sample points that are unoccluded.
    /// This algorithm allows for smoother transitions in and out of occlusion.
    Volumetric {
        /// The radius of the sphere the source is modeled as.
        radius: f32,

        /// The number of point samples to consider when tracing rays.
        /// This value can change between simulation runs.
        num_occlusion_samples: u32,
    },
}

/// Simulation parameters that are not specific to any source.
#[derive(Debug)]
pub struct SimulationSharedInputs {
    /// The position and orientation of the listener.
    pub listener: CoordinateSystem,

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

    /// Optional callback for visualizing valid path segments during call to [`Simulator::run_pathing`].
    pub pathing_visualization_callback: Option<CallbackInformation<PathingVisualizationCallback>>,
}

impl From<&SimulationSharedInputs> for audionimbus_sys::IPLSimulationSharedInputs {
    fn from(simulation_shared_inputs: &SimulationSharedInputs) -> Self {
        let (pathing_visualization_callback, pathing_user_data) = simulation_shared_inputs
            .pathing_visualization_callback
            .as_ref()
            .map_or((None, std::ptr::null_mut()), |callback_information| {
                (
                    Some(callback_information.callback),
                    callback_information.user_data,
                )
            });

        Self {
            listener: simulation_shared_inputs.listener.into(),
            numRays: simulation_shared_inputs.num_rays as i32,
            numBounces: simulation_shared_inputs.num_bounces as i32,
            duration: simulation_shared_inputs.duration,
            order: simulation_shared_inputs.order as i32,
            irradianceMinDistance: simulation_shared_inputs.irradiance_min_distance,
            pathingVisCallback: pathing_visualization_callback,
            pathingUserData: pathing_user_data,
        }
    }
}

/// Callback for visualizing valid path segments during the call to [`Simulator::run_pathing`].
///
/// You can use this to provide the user with visual feedback, like drawing each segment of a path.
///
/// # Arguments
///
/// - `from`: position of starting probe.
/// - `to`: position of ending probe.
/// - `occluded`: occlusion status of ray segment between `from` to `to`.
/// - `user_data`: pointer to arbitrary user-specified data provided when calling the function that will call this callback.
pub type PathingVisualizationCallback = unsafe extern "C" fn(
    from: audionimbus_sys::IPLVector3,
    to: audionimbus_sys::IPLVector3,
    occluded: audionimbus_sys::IPLbool,
    user_data: *mut std::ffi::c_void,
);

/// Simulation results for a source.
#[derive(Debug)]
pub struct SimulationOutputs(*mut audionimbus_sys::IPLSimulationOutputs);

unsafe impl Send for SimulationOutputs {}

impl SimulationOutputs {
    fn try_allocate() -> Result<Self, SteamAudioError> {
        let ptr = unsafe {
            let layout = std::alloc::Layout::new::<audionimbus_sys::IPLSimulationOutputs>();
            let ptr = std::alloc::alloc(layout).cast::<audionimbus_sys::IPLSimulationOutputs>();
            if ptr.is_null() {
                return Err(SteamAudioError::OutOfMemory);
            }
            std::ptr::write(ptr, std::mem::zeroed());
            ptr
        };

        Ok(Self(ptr))
    }

    pub fn direct(&self) -> FFIWrapper<'_, DirectEffectParams, Self> {
        unsafe { FFIWrapper::new((*self.0).direct.into()) }
    }

    pub fn reflections<T: ReflectionEffectType>(
        &self,
    ) -> FFIWrapper<'_, ReflectionEffectParams<T>, Self> {
        unsafe { FFIWrapper::new((*self.0).reflections.into()) }
    }

    pub fn pathing(&self) -> FFIWrapper<'_, PathEffectParams, Self> {
        unsafe { FFIWrapper::new((*self.0).pathing.into()) }
    }

    pub const fn raw_ptr(&self) -> *mut audionimbus_sys::IPLSimulationOutputs {
        self.0
    }

    pub const fn raw_ptr_mut(&mut self) -> &mut *mut audionimbus_sys::IPLSimulationOutputs {
        &mut self.0
    }
}

impl Drop for SimulationOutputs {
    fn drop(&mut self) {
        unsafe {
            let layout = std::alloc::Layout::new::<audionimbus_sys::IPLSimulationOutputs>();
            std::alloc::dealloc(self.0.cast::<u8>(), layout);
        }
    }
}

/// Errors that can occur during simulation operations.
#[derive(Eq, PartialEq, Debug)]
pub enum SimulationError {
    /// Attempted to run pathing simulation without any probe batches committed.
    ///
    /// Pathing requires at least one probe batch to be added to the simulator
    /// via [`Simulator::add_probe_batch`] and committed via [`Simulator::commit`]
    /// before running simulations.
    PathingWithoutProbes,

    /// Attempted to run reflection simulation without a scene set.
    ///
    /// Reflection simulation requires a [`Scene`] to be set on the simulator via
    /// [`Simulator::set_scene`] and committed via [`Simulator::commit`] before
    /// running simulations.
    ReflectionsWithoutScene,
}

impl std::error::Error for SimulationError {}

impl std::fmt::Display for SimulationError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match &self {
            Self::PathingWithoutProbes => {
                write!(f, "running pathing on a simulator with no probes")
            }
            Self::ReflectionsWithoutScene => {
                write!(f, "running reflections on a simulator with no scene set")
            }
        }
    }
}
