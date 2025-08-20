use crate::air_absorption::AirAbsorptionModel;
use crate::callback::CallbackInformation;
use crate::context::Context;
use crate::deviation::DeviationModel;
use crate::device::open_cl::OpenClDevice;
use crate::device::radeon_rays::RadeonRaysDevice;
use crate::device::true_audio_next::TrueAudioNextDevice;
use crate::directivity::Directivity;
use crate::distance_attenuation::DistanceAttenuationModel;
use crate::effect::{DirectEffectParams, PathEffectParams, ReflectionEffectParams};
use crate::error::{to_option_error, SteamAudioError};
use crate::ffi_wrapper::FFIWrapper;
use crate::geometry;
use crate::geometry::{Scene, SceneParams};
use crate::probe::ProbeBatch;
use std::marker::PhantomData;

// Marker types for capabilities.
#[derive(Debug)]
pub struct Direct;
#[derive(Debug)]
pub struct Reflections;
#[derive(Debug)]
pub struct Pathing;

/// Builder for creating a [`Simulator`].
#[derive(Debug)]
pub struct SimulatorBuilder<D = (), R = (), P = ()> {
    settings: SimulationSettings<'static>,
    _direct: PhantomData<D>,
    _reflections: PhantomData<R>,
    _pathing: PhantomData<P>,
}

/// Manages direct and indirect sound propagation simulation for multiple sources.
///
/// Your application will typically create one simulator object and use it to run simulations with different source and listener parameters between consecutive simulation runs.
/// The simulator can also be reused across scene changes.
#[derive(Debug)]
pub struct Simulator<D = (), R = (), P = ()> {
    inner: audionimbus_sys::IPLSimulator,
    _direct: PhantomData<D>,
    _reflections: PhantomData<R>,
    _pathing: PhantomData<P>,
}

impl Simulator<(), (), ()> {
    /// Creates a new simulator builder with required parameters.
    pub fn builder(
        scene_params: SceneParams<'static>,
        sampling_rate: usize,
        frame_size: usize,
    ) -> SimulatorBuilder<(), (), ()> {
        SimulatorBuilder {
            settings: SimulationSettings {
                scene_params,
                sampling_rate,
                frame_size,
                direct_simulation: None,
                reflections_simulation: None,
                pathing_simulation: None,
            },
            _direct: PhantomData,
            _reflections: PhantomData,
            _pathing: PhantomData,
        }
    }
}

impl<D, R, P> SimulatorBuilder<D, R, P> {
    /// Enables direct simulation.
    pub fn with_direct(
        self,
        direct_settings: DirectSimulationSettings,
    ) -> SimulatorBuilder<Direct, R, P> {
        let SimulatorBuilder {
            mut settings,
            _reflections,
            _pathing,
            ..
        } = self;

        settings.direct_simulation = Some(direct_settings);

        SimulatorBuilder {
            settings,
            _direct: PhantomData,
            _reflections,
            _pathing,
        }
    }

    /// Enables reflections simulation.
    pub fn with_reflections(
        self,
        reflections_settings: ReflectionsSimulationSettings<'static>,
    ) -> SimulatorBuilder<D, Reflections, P> {
        let SimulatorBuilder {
            mut settings,
            _direct,
            _pathing,
            ..
        } = self;

        settings.reflections_simulation = Some(reflections_settings);

        SimulatorBuilder {
            settings,
            _direct,
            _reflections: PhantomData,
            _pathing,
        }
    }

    /// Enables pathing simulation.
    pub fn with_pathing(
        self,
        pathing_settings: PathingSimulationSettings,
    ) -> SimulatorBuilder<D, R, Pathing> {
        let SimulatorBuilder {
            mut settings,
            _direct,
            _reflections,
            ..
        } = self;

        settings.pathing_simulation = Some(pathing_settings);

        SimulatorBuilder {
            settings,
            _direct,
            _reflections,
            _pathing: PhantomData,
        }
    }

    pub fn try_build(self, context: &Context) -> Result<Simulator<D, R, P>, SteamAudioError> {
        let mut simulator = Simulator {
            inner: std::ptr::null_mut(),
            _direct: PhantomData,
            _reflections: PhantomData,
            _pathing: PhantomData,
        };

        let status = unsafe {
            audionimbus_sys::iplSimulatorCreate(
                context.raw_ptr(),
                &mut audionimbus_sys::IPLSimulationSettings::from(self.settings),
                simulator.raw_ptr_mut(),
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        Ok(simulator)
    }
}

impl<D, R, P> Simulator<D, R, P> {
    /// Specifies the scene within which all subsequent simulations should be run.
    ///
    /// Call [`Self::commit`] after calling this function for the changes to take effect.
    ///
    /// This function cannot be called while any simulation is running.
    pub fn set_scene(&mut self, scene: &Scene) {
        unsafe { audionimbus_sys::iplSimulatorSetScene(self.raw_ptr(), scene.raw_ptr()) }
    }

    /// Adds a probe batch for use in subsequent simulations.
    /// Sources that require baked data can then use the data contained in the specified probe batch.
    ///
    /// Call [`Self::commit`] after calling this function for the changes to take effect.
    ///
    /// This function cannot be called while any simulation is running.
    pub fn add_probe_batch(&mut self, probe: &ProbeBatch) {
        unsafe {
            audionimbus_sys::iplSimulatorAddProbeBatch(self.raw_ptr(), probe.raw_ptr());
        }
    }

    /// Removes a probe batch from use in subsequent simulations.
    /// Sources that require baked data will then stop using the data contained in the specified probe batch.
    ///
    /// Call [`Self::commit`] after calling this function for the changes to take effect.
    ///
    /// This function cannot be called while any simulation is running.
    pub fn remove_probe_batch(&mut self, probe: &ProbeBatch) {
        unsafe {
            audionimbus_sys::iplSimulatorRemoveProbeBatch(self.raw_ptr(), probe.raw_ptr());
        }
    }

    /// Adds a source to the set of sources processed by a simulator in subsequent simulations.
    ///
    /// Call [`Self::commit`] after calling this function for the changes to take effect.
    pub fn add_source(&mut self, source: &Source) {
        unsafe {
            audionimbus_sys::iplSourceAdd(source.raw_ptr(), self.raw_ptr());
        }
    }

    /// Removes a source from the set of sources processed by a simulator in subsequent simulations.
    ///
    /// Call [`Self::commit`] after calling this function for the changes to take effect.
    pub fn remove_source(&mut self, source: &Source) {
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
        unsafe { audionimbus_sys::iplSimulatorCommit(self.raw_ptr()) }
    }

    /// Specifies simulation parameters that are not associated with any particular source.
    ///
    /// # Arguments
    ///
    /// - `flags`: the types of simulation for which to specify shared inputs. If, for example, direct and reflections simulations are being run on separate threads, you can call this function on the direct simulation thread with [`SimulationFlags::DIRECT`], and on the reflections simulation thread with [`SimulationFlags::REFLECTIONS`], without requiring any synchronization between the calls.
    /// - `shared_inputs`: the shared input parameters to set.
    pub fn set_shared_inputs(
        &mut self,
        simulation_flags: SimulationFlags,
        shared_inputs: &SimulationSharedInputs,
    ) {
        unsafe {
            audionimbus_sys::iplSimulatorSetSharedInputs(
                self.raw_ptr(),
                simulation_flags.into(),
                &mut audionimbus_sys::IPLSimulationSharedInputs::from(shared_inputs),
            );
        }
    }

    pub fn raw_ptr(&self) -> audionimbus_sys::IPLSimulator {
        self.inner
    }

    pub fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLSimulator {
        &mut self.inner
    }
}

impl<R, P> Simulator<Direct, R, P> {
    /// Runs a direct simulation for all sources added to the simulator.
    /// This may include distance attenuation, air absorption, directivity, occlusion, and transmission.
    ///
    /// This function should not be called from the audio processing thread if occlusion and/or transmission are enabled.
    pub fn run_direct(&self) {
        unsafe {
            audionimbus_sys::iplSimulatorRunDirect(self.raw_ptr());
        }
    }
}

impl<D, P> Simulator<D, Reflections, P> {
    /// Runs a reflections simulation for all sources added to the simulator.
    ///
    /// This function can be CPU intensive, and should be called from a separate thread in order to not block either the audio processing thread or the game’s main update thread.
    pub fn run_reflections(&self) {
        unsafe {
            audionimbus_sys::iplSimulatorRunReflections(self.raw_ptr());
        }
    }
}

impl<D, R> Simulator<D, R, Pathing> {
    /// Runs a pathing simulation for all sources added to the simulator.
    ///
    /// This function can be CPU intensive, and should be called from a separate thread in order to not block either the audio processing thread or the game’s main update thread.
    pub fn run_pathing(&self) {
        unsafe {
            audionimbus_sys::iplSimulatorRunPathing(self.raw_ptr());
        }
    }
}

impl<D, R, P> Clone for Simulator<D, R, P> {
    fn clone(&self) -> Self {
        unsafe {
            audionimbus_sys::iplSimulatorRetain(self.inner);
        }

        Self {
            inner: self.inner,
            _direct: PhantomData,
            _reflections: PhantomData,
            _pathing: PhantomData,
        }
    }
}

impl<D, R, P> Drop for Simulator<D, R, P> {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplSimulatorRelease(&mut self.inner) }
    }
}

unsafe impl<D, R, P> Send for Simulator<D, R, P> {}
unsafe impl<D, R, P> Sync for Simulator<D, R, P> {}

/// Settings used to create a simulator.
#[derive(Debug, Copy, Clone)]
pub struct SimulationSettings<'a> {
    /// The scene parameters that will be used for simulations.
    /// The scene parameters cannot change during the lifetime of a simulator object.
    pub scene_params: SceneParams<'a>,

    /// If `Some`, this simulator will be used for direct path simulation.
    pub direct_simulation: Option<DirectSimulationSettings>,

    /// If `Some`, this simulator will be used for reflections simulation.
    /// The reflections effect type cannot change during the lifetime of a simulator object.
    pub reflections_simulation: Option<ReflectionsSimulationSettings<'a>>,

    /// If `Some`, this simulator will be used for pathing.
    pub pathing_simulation: Option<PathingSimulationSettings>,

    /// The sampling rate (in Hz) used for audio processing.
    pub sampling_rate: usize,

    /// The size (in samples) of the audio buffers used for audio processing.
    pub frame_size: usize,
}

/// Settings used for direct path simulation.
#[derive(Debug, Copy, Clone)]
pub struct DirectSimulationSettings {
    /// The maximum number of point samples to consider when calculating occlusion using the volumetric occlusion algorithm.
    /// Different sources can use different numbers of samples, and the number of samples can change between simulation runs, but this is the maximum value.
    /// Increasing this value results in smoother occlusion transitions, at the cost of increased CPU usage.
    pub max_num_occlusion_samples: usize,
}

/// Settings used for reflections simulation.
#[derive(Debug, Copy, Clone)]
pub enum ReflectionsSimulationSettings<'a> {
    /// Multi-channel convolution reverb.
    Convolution {
        /// The maximum number of rays to trace from the listener when simulating reflections.
        /// You can use different numbers of rays between simulation runs, but this is the maximum value.
        /// Increasing this value results in more accurate reflections, at the cost of increased CPU usage.
        max_num_rays: usize,

        /// The number of directions to sample when generating diffusely reflected rays.
        /// Increasing this value may increase the accuracy of diffuse reflections.
        num_diffuse_samples: usize,

        /// The maximum length (in seconds) of impulse responses generated by reflection simulations.
        /// You can change this value betweeen simulation runs, but this is the maximum value.
        /// Increasing this value results in longer, more accurate reverb tails, at the cost of increased CPU and memory usage.
        max_duration: f32,

        /// The maximum Ambisonic order of impulse responses generated by reflection simulations.
        /// You can change this value between simulation runs, but this is the maximum value.
        /// Increasing this value results in more accurate directional variations in the impulse responses, at the cost of increased CPU and memory usage.
        max_order: usize,

        /// The maximum number of sources for which reflection simulations will be run at any given time.
        max_num_sources: usize,

        /// The number of threads used for real-time reflection simulations.
        num_threads: usize,
    },

    /// Parametric (or artificial) reverb, using feedback delay networks.
    Parametric {
        /// The maximum number of rays to trace from the listener when simulating reflections.
        /// You can use different numbers of rays between simulation runs, but this is the maximum value.
        /// Increasing this value results in more accurate reflections, at the cost of increased CPU usage.
        max_num_rays: usize,

        /// The number of directions to sample when generating diffusely reflected rays.
        /// Increasing this value may increase the accuracy of diffuse reflections.
        num_diffuse_samples: usize,

        /// The maximum length (in seconds) of impulse responses generated by reflection simulations.
        /// You can change this value betweeen simulation runs, but this is the maximum value.
        /// Increasing this value results in longer, more accurate reverb tails, at the cost of increased CPU and memory usage.
        max_duration: f32,

        /// The maximum Ambisonic order of impulse responses generated by reflection simulations.
        /// You can change this value between simulation runs, but this is the maximum value.
        /// Increasing this value results in more accurate directional variations in the impulse responses, at the cost of increased CPU and memory usage.
        max_order: usize,

        /// The maximum number of sources for which reflection simulations will be run at any given time.
        max_num_sources: usize,

        /// The number of threads used for real-time reflection simulations.
        num_threads: usize,
    },

    /// A hybrid of convolution and parametric reverb.
    Hybrid {
        /// The maximum number of rays to trace from the listener when simulating reflections.
        /// You can use different numbers of rays between simulation runs, but this is the maximum value.
        /// Increasing this value results in more accurate reflections, at the cost of increased CPU usage.
        max_num_rays: usize,

        /// The number of directions to sample when generating diffusely reflected rays.
        /// Increasing this value may increase the accuracy of diffuse reflections.
        num_diffuse_samples: usize,

        /// The maximum length (in seconds) of impulse responses generated by reflection simulations.
        /// You can change this value betweeen simulation runs, but this is the maximum value.
        /// Increasing this value results in longer, more accurate reverb tails, at the cost of increased CPU and memory usage.
        max_duration: f32,

        /// The maximum Ambisonic order of impulse responses generated by reflection simulations.
        /// You can change this value between simulation runs, but this is the maximum value.
        /// Increasing this value results in more accurate directional variations in the impulse responses, at the cost of increased CPU and memory usage.
        max_order: usize,

        /// The maximum number of sources for which reflection simulations will be run at any given time.
        max_num_sources: usize,

        /// The number of threads used for real-time reflection simulations.
        num_threads: usize,
    },

    /// Multi-channel convolution reverb, using AMD TrueAudio Next for GPU acceleration.
    TrueAudioNext {
        /// The maximum number of rays to trace from the listener when simulating reflections.
        /// You can use different numbers of rays between simulation runs, but this is the maximum value.
        /// Increasing this value results in more accurate reflections, at the cost of increased CPU usage.
        max_num_rays: usize,

        /// The number of directions to sample when generating diffusely reflected rays.
        /// Increasing this value may increase the accuracy of diffuse reflections.
        num_diffuse_samples: usize,

        /// The maximum length (in seconds) of impulse responses generated by reflection simulations.
        /// You can change this value betweeen simulation runs, but this is the maximum value.
        /// Increasing this value results in longer, more accurate reverb tails, at the cost of increased CPU and memory usage.
        max_duration: f32,

        /// The maximum Ambisonic order of impulse responses generated by reflection simulations.
        /// You can change this value between simulation runs, but this is the maximum value.
        /// Increasing this value results in more accurate directional variations in the impulse responses, at the cost of increased CPU and memory usage.
        max_order: usize,

        /// The maximum number of sources for which reflection simulations will be run at any given time.
        max_num_sources: usize,

        /// The number of threads used for real-time reflection simulations.
        num_threads: usize,

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
    pub num_visibility_samples: usize,
}

impl From<SimulationSettings<'_>> for audionimbus_sys::IPLSimulationSettings {
    fn from(settings: SimulationSettings) -> Self {
        let SimulationSettings {
            scene_params,
            direct_simulation,
            reflections_simulation,
            pathing_simulation,
            sampling_rate,
            frame_size,
        } = settings;

        let mut ray_batch_size = usize::default();
        let mut open_cl_device = &OpenClDevice::null();
        let mut radeon_rays_device = &RadeonRaysDevice::null();
        let scene_type = match scene_params {
            SceneParams::Default => audionimbus_sys::IPLSceneType::IPL_SCENETYPE_DEFAULT,
            SceneParams::Embree => audionimbus_sys::IPLSceneType::IPL_SCENETYPE_EMBREE,
            SceneParams::RadeonRays {
                open_cl_device: ocl_device,
                radeon_rays_device: rr_device,
            } => {
                open_cl_device = ocl_device;
                radeon_rays_device = rr_device;
                audionimbus_sys::IPLSceneType::IPL_SCENETYPE_RADEONRAYS
            }
            SceneParams::Custom {
                ray_batch_size: rb_size,
            } => {
                ray_batch_size = rb_size;
                audionimbus_sys::IPLSceneType::IPL_SCENETYPE_CUSTOM
            }
        };

        let mut flags = audionimbus_sys::IPLSimulationFlags(0);

        let mut max_num_occlusion_samples = usize::default();
        if let Some(direct_simulation_settings) = direct_simulation {
            flags |= audionimbus_sys::IPLSimulationFlags::IPL_SIMULATIONFLAGS_DIRECT;
            max_num_occlusion_samples = direct_simulation_settings.max_num_occlusion_samples;
        }

        let mut reflection_type =
            audionimbus_sys::IPLReflectionEffectType::IPL_REFLECTIONEFFECTTYPE_CONVOLUTION;
        let mut max_num_rays = usize::default();
        let mut num_diffuse_samples = usize::default();
        let mut max_duration = f32::default();
        let mut max_order = usize::default();
        let mut max_num_sources = usize::default();
        let mut num_threads = usize::default();
        let mut true_audio_next_device = &TrueAudioNextDevice::null();
        if let Some(reflections_simulation_settings) = reflections_simulation {
            flags |= audionimbus_sys::IPLSimulationFlags::IPL_SIMULATIONFLAGS_REFLECTIONS;

            (
                reflection_type,
                max_num_rays,
                num_diffuse_samples,
                max_duration,
                max_order,
                max_num_sources,
                num_threads,
            ) = match reflections_simulation_settings {
                ReflectionsSimulationSettings::Convolution {
                    max_num_rays,
                    num_diffuse_samples,
                    max_duration,
                    max_order,
                    max_num_sources,
                    num_threads,
                } => (
                    audionimbus_sys::IPLReflectionEffectType::IPL_REFLECTIONEFFECTTYPE_CONVOLUTION,
                    max_num_rays,
                    num_diffuse_samples,
                    max_duration,
                    max_order,
                    max_num_sources,
                    num_threads,
                ),
                ReflectionsSimulationSettings::Parametric {
                    max_num_rays,
                    num_diffuse_samples,
                    max_duration,
                    max_order,
                    max_num_sources,
                    num_threads,
                } => (
                    audionimbus_sys::IPLReflectionEffectType::IPL_REFLECTIONEFFECTTYPE_PARAMETRIC,
                    max_num_rays,
                    num_diffuse_samples,
                    max_duration,
                    max_order,
                    max_num_sources,
                    num_threads,
                ),
                ReflectionsSimulationSettings::Hybrid {
                    max_num_rays,
                    num_diffuse_samples,
                    max_duration,
                    max_order,
                    max_num_sources,
                    num_threads,
                } => (
                    audionimbus_sys::IPLReflectionEffectType::IPL_REFLECTIONEFFECTTYPE_HYBRID,
                    max_num_rays,
                    num_diffuse_samples,
                    max_duration,
                    max_order,
                    max_num_sources,
                    num_threads,
                ),
                ReflectionsSimulationSettings::TrueAudioNext {
                    max_num_rays,
                    num_diffuse_samples,
                    max_duration,
                    max_order,
                    max_num_sources,
                    num_threads,
                    open_cl_device: ocl_device,
                    true_audio_next_device: tan_device,
                } => {
                    open_cl_device = ocl_device;
                    true_audio_next_device = tan_device;

                    (
                        audionimbus_sys::IPLReflectionEffectType::IPL_REFLECTIONEFFECTTYPE_TAN,
                        max_num_rays,
                        num_diffuse_samples,
                        max_duration,
                        max_order,
                        max_num_sources,
                        num_threads,
                    )
                }
            };
        }

        let mut num_visibility_samples = usize::default();
        if let Some(pathing_simulation_settings) = pathing_simulation {
            flags |= audionimbus_sys::IPLSimulationFlags::IPL_SIMULATIONFLAGS_PATHING;
            num_visibility_samples = pathing_simulation_settings.num_visibility_samples;
        }

        Self {
            flags,
            sceneType: scene_type,
            reflectionType: reflection_type,
            maxNumOcclusionSamples: max_num_occlusion_samples as i32,
            maxNumRays: max_num_rays as i32,
            numDiffuseSamples: num_diffuse_samples as i32,
            maxDuration: max_duration,
            maxOrder: max_order as i32,
            maxNumSources: max_num_sources as i32,
            numThreads: num_threads as i32,
            rayBatchSize: ray_batch_size as i32,
            numVisSamples: num_visibility_samples as i32,
            samplingRate: sampling_rate as i32,
            frameSize: frame_size as i32,
            openCLDevice: open_cl_device.raw_ptr(),
            radeonRaysDevice: radeon_rays_device.raw_ptr(),
            tanDevice: true_audio_next_device.raw_ptr(),
        }
    }
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
pub struct Source(audionimbus_sys::IPLSource);

impl Source {
    pub fn try_new<D, R, P>(
        simulator: &Simulator<D, R, P>,
        source_settings: &SourceSettings,
    ) -> Result<Self, SteamAudioError> {
        let mut source = Self(std::ptr::null_mut());

        let status = unsafe {
            audionimbus_sys::iplSourceCreate(
                simulator.raw_ptr(),
                &mut audionimbus_sys::IPLSourceSettings::from(source_settings),
                source.raw_ptr_mut(),
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        Ok(source)
    }

    /// Specifies simulation parameters for a source.
    ///
    /// # Arguments
    ///
    /// - `flags`: the types of simulation for which to specify inputs. If, for example, direct and reflections simulations are being run on separate threads, you can call this function on the direct simulation thread with [`SimulationFlags::DIRECT`], and on the reflections simulation thread with [`SimulationFlags::REFLECTIONS`], without requiring any synchronization between the calls.
    /// - `inputs`: the input parameters to set.
    pub fn set_inputs(&mut self, simulation_flags: SimulationFlags, inputs: SimulationInputs) {
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
    /// # Arguments
    ///
    /// - `flags`: the types of simulation for which to retrieve results.
    pub fn get_outputs(&self, simulation_flags: SimulationFlags) -> SimulationOutputs {
        let simulation_outputs = SimulationOutputs::default();

        unsafe {
            audionimbus_sys::iplSourceGetOutputs(
                self.raw_ptr(),
                simulation_flags.into(),
                simulation_outputs.raw_ptr(),
            );
        }

        simulation_outputs
    }

    pub fn raw_ptr(&self) -> audionimbus_sys::IPLSource {
        self.0
    }

    pub fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLSource {
        &mut self.0
    }
}

impl Clone for Source {
    fn clone(&self) -> Self {
        unsafe {
            audionimbus_sys::iplSourceRetain(self.0);
        }
        Self(self.0)
    }
}

impl Drop for Source {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplSourceRelease(&mut self.0) }
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
    pub source: geometry::CoordinateSystem,

    /// If `Some`, enables direct simulation. This includes distance attenuation, air absorption, directivity, occlusion, and transmission.
    pub direct_simulation: Option<DirectSimulationParameters>,

    /// If `Some`, enables reflections simulation. This includes both real-time and baked simulation.
    pub reflections_simulation: Option<ReflectionsSimulationParameters>,

    /// If `Some`, enables pathing simulation.
    pub pathing_simulation: Option<PathingSimulationParameters<'a>>,
}

/// Direct simulation parameters for a source.
#[derive(Debug, Copy, Clone)]
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

/// Occlusion parameters.
#[derive(Debug, Copy, Clone)]
pub struct Occlusion {
    /// If `Some`, enables transmission simulation.
    pub transmission: Option<TransmissionParameters>,

    /// The occlusion algorithm to use.
    pub algorithm: OcclusionAlgorithm,
}

/// Transmission parameters.
#[derive(Debug, Copy, Clone)]
pub struct TransmissionParameters {
    /// If simulating transmission, this is the maximum number of surfaces, starting from the closest surface to the listener, whose transmission coefficients will be considered when calculating the total amount of sound transmitted.
    /// Increasing this value will result in more accurate results when multiple surfaces lie between the source and the listener, at the cost of increased CPU usage.
    pub num_transmission_rays: usize,
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
    pub pathing_order: usize,

    /// If `true`, baked paths are tested for visibility.
    /// This is useful if your scene has dynamic objects that might occlude baked paths.
    pub enable_validation: bool,

    /// If `true`, and [`Self::enable_validation`] is `true`, then if a baked path is occluded by dynamic geometry, path finding is re-run in real-time to find alternate paths that take into account the dynamic geometry.
    pub find_alternate_paths: bool,

    /// The deviation model to use for this source.
    pub deviation: DeviationModel,
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

        let (
            direct_flags,
            distance_attenuation_model,
            air_absorption_model,
            directivity,
            occlusion_type,
            occlusion_radius,
            num_occlusion_samples,
            num_transmission_rays,
        ) = if let Some(direct_simulation_parameters) = direct_simulation {
            flags |= audionimbus_sys::IPLSimulationFlags::IPL_SIMULATIONFLAGS_DIRECT;

            let DirectSimulationParameters {
                distance_attenuation,
                air_absorption,
                directivity,
                occlusion,
            } = direct_simulation_parameters;

            let mut direct_flags = audionimbus_sys::IPLDirectSimulationFlags(0);

            let distance_attenuation_model = if let Some(distance_attenuation_model) =
                distance_attenuation
            {
                direct_flags |= audionimbus_sys::IPLDirectSimulationFlags::IPL_DIRECTSIMULATIONFLAGS_DISTANCEATTENUATION;
                distance_attenuation_model
            } else {
                DistanceAttenuationModel::default()
            };

            let air_absorption_model = if let Some(air_absorption_model) = air_absorption {
                direct_flags |= audionimbus_sys::IPLDirectSimulationFlags::IPL_DIRECTSIMULATIONFLAGS_AIRABSORPTION;
                air_absorption_model
            } else {
                AirAbsorptionModel::default()
            };

            let directivity = if let Some(directivity) = directivity {
                direct_flags |= audionimbus_sys::IPLDirectSimulationFlags::IPL_DIRECTSIMULATIONFLAGS_DIRECTIVITY;
                directivity
            } else {
                Directivity::default()
            };

            let (occlusion_type, occlusion_radius, num_occlusion_samples, num_transmission_rays) =
                if let Some(occlusion) = occlusion {
                    direct_flags |=
                    audionimbus_sys::IPLDirectSimulationFlags::IPL_DIRECTSIMULATIONFLAGS_OCCLUSION;

                    let (occlusion_type, occlusion_radius, num_occlusion_samples) =
                        match occlusion.algorithm {
                            OcclusionAlgorithm::Raycast => (
                                audionimbus_sys::IPLOcclusionType::IPL_OCCLUSIONTYPE_RAYCAST,
                                f32::default(),
                                usize::default(),
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

                    let num_transmission_rays = if let Some(transmission_parameters) =
                        occlusion.transmission
                    {
                        direct_flags |= audionimbus_sys::IPLDirectSimulationFlags::IPL_DIRECTSIMULATIONFLAGS_TRANSMISSION;
                        transmission_parameters.num_transmission_rays
                    } else {
                        usize::default()
                    };

                    (
                        occlusion_type,
                        occlusion_radius,
                        num_occlusion_samples,
                        num_transmission_rays,
                    )
                } else {
                    (
                        audionimbus_sys::IPLOcclusionType::IPL_OCCLUSIONTYPE_RAYCAST,
                        f32::default(),
                        usize::default(),
                        usize::default(),
                    )
                };

            (
                direct_flags,
                distance_attenuation_model,
                air_absorption_model,
                directivity,
                occlusion_type,
                occlusion_radius,
                num_occlusion_samples,
                num_transmission_rays,
            )
        } else {
            (
                audionimbus_sys::IPLDirectSimulationFlags(0),
                DistanceAttenuationModel::default(),
                AirAbsorptionModel::default(),
                Directivity::default(),
                audionimbus_sys::IPLOcclusionType::IPL_OCCLUSIONTYPE_RAYCAST,
                f32::default(),
                usize::default(),
                usize::default(),
            )
        };

        let (
            baked,
            baked_data_identifier,
            reverb_scale,
            hybrid_reverb_transition_time,
            hybrid_reverb_overlap_percent,
        ) = if let Some(reflections_simulation_parameters) = reflections_simulation {
            flags |= audionimbus_sys::IPLSimulationFlags::IPL_SIMULATIONFLAGS_REFLECTIONS;

            let (
                baked_data_identifier,
                reverb_scale,
                hybrid_reverb_transition_time,
                hybrid_reverb_overlap_percent,
            ) = match reflections_simulation_parameters {
                ReflectionsSimulationParameters::Convolution {
                    baked_data_identifier,
                } => (
                    baked_data_identifier,
                    <[f32; 3]>::default(),
                    f32::default(),
                    f32::default(),
                ),
                ReflectionsSimulationParameters::Parametric {
                    reverb_scale,
                    baked_data_identifier,
                } => (
                    baked_data_identifier,
                    reverb_scale,
                    f32::default(),
                    f32::default(),
                ),
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
                ReflectionsSimulationParameters::TrueAudioNext {
                    baked_data_identifier,
                } => (
                    baked_data_identifier,
                    <[f32; 3]>::default(),
                    f32::default(),
                    f32::default(),
                ),
            };

            let (baked, baked_data_identifier) =
                if let Some(baked_data_identifier) = baked_data_identifier {
                    (audionimbus_sys::IPLbool::IPL_TRUE, baked_data_identifier)
                } else {
                    (
                        audionimbus_sys::IPLbool::IPL_FALSE,
                        BakedDataIdentifier::Reflections {
                            variation: BakedDataVariation::Reverb,
                        },
                    )
                };

            (
                baked,
                baked_data_identifier,
                reverb_scale,
                hybrid_reverb_transition_time,
                hybrid_reverb_overlap_percent,
            )
        } else {
            (
                audionimbus_sys::IPLbool::IPL_FALSE,
                BakedDataIdentifier::Reflections {
                    variation: BakedDataVariation::Reverb,
                },
                <[f32; 3]>::default(),
                f32::default(),
                f32::default(),
            )
        };

        let (
            pathing_probes,
            visibility_radius,
            visibility_threshold,
            visibility_range,
            pathing_order,
            enable_validation,
            find_alternate_paths,
            deviation_model,
        ) = if let Some(pathing_simulation_parameters) = pathing_simulation {
            flags |= audionimbus_sys::IPLSimulationFlags::IPL_SIMULATIONFLAGS_PATHING;

            (
                pathing_simulation_parameters.pathing_probes.raw_ptr(),
                pathing_simulation_parameters.visibility_radius,
                pathing_simulation_parameters.visibility_threshold,
                pathing_simulation_parameters.visibility_range,
                pathing_simulation_parameters.pathing_order,
                pathing_simulation_parameters.enable_validation,
                pathing_simulation_parameters.find_alternate_paths,
                // FIXME: Potential memory leak: this prevents dangling pointers, but there is no guarantee it will be freed by the C library.
                Box::into_raw(Box::new((&pathing_simulation_parameters.deviation).into())),
            )
        } else {
            (
                std::ptr::null_mut(),
                f32::default(),
                f32::default(),
                f32::default(),
                usize::default(),
                bool::default(),
                bool::default(),
                std::ptr::null_mut(),
            )
        };

        Self {
            flags,
            directFlags: direct_flags,
            source: source.into(),
            distanceAttenuationModel: (&distance_attenuation_model).into(),
            airAbsorptionModel: (&air_absorption_model).into(),
            directivity: (&directivity).into(),
            occlusionType: occlusion_type,
            occlusionRadius: occlusion_radius,
            numOcclusionSamples: num_occlusion_samples as i32,
            reverbScale: reverb_scale,
            hybridReverbTransitionTime: hybrid_reverb_transition_time,
            hybridReverbOverlapPercent: hybrid_reverb_overlap_percent,
            baked,
            bakedDataIdentifier: baked_data_identifier.into(),
            pathingProbes: pathing_probes,
            visRadius: visibility_radius,
            visThreshold: visibility_threshold,
            visRange: visibility_range,
            pathingOrder: pathing_order as i32,
            enableValidation: if enable_validation {
                audionimbus_sys::IPLbool::IPL_TRUE
            } else {
                audionimbus_sys::IPLbool::IPL_FALSE
            },
            findAlternatePaths: if find_alternate_paths {
                audionimbus_sys::IPLbool::IPL_TRUE
            } else {
                audionimbus_sys::IPLbool::IPL_FALSE
            },
            numTransmissionRays: num_transmission_rays as i32,
            deviationModel: deviation_model,
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
        num_occlusion_samples: usize,
    },
}

/// Identifies a “layer” of data stored in a probe batch.
/// Each probe batch may store multiple layers of data, such as reverb, static source reflections, or pathing.
/// Each layer can be accessed using an identifier.
#[derive(Copy, Clone, Debug)]
pub enum BakedDataIdentifier {
    /// Reflections.
    /// The source and listener positions used to compute the reflections data stored at each probe depends on the \c IPLBakedDataVariation selected.
    Reflections {
        /// The way in which source and listener positions depend on probe position.
        variation: BakedDataVariation,
    },

    /// Pathing.
    /// The probe batch stores data about the shortest paths between any pair of probes in the batch.
    Pathing {
        /// The way in which source and listener positions depend on probe position.
        variation: BakedDataVariation,
    },
}

impl From<BakedDataIdentifier> for audionimbus_sys::IPLBakedDataIdentifier {
    fn from(baked_data_identifier: BakedDataIdentifier) -> Self {
        let (type_, variation) = match baked_data_identifier {
            BakedDataIdentifier::Reflections { variation } => (
                audionimbus_sys::IPLBakedDataType::IPL_BAKEDDATATYPE_REFLECTIONS,
                variation,
            ),
            BakedDataIdentifier::Pathing { variation } => (
                audionimbus_sys::IPLBakedDataType::IPL_BAKEDDATATYPE_PATHING,
                variation,
            ),
        };

        let (variation, endpoint_influence) = match variation {
            BakedDataVariation::Reverb => (
                audionimbus_sys::IPLBakedDataVariation::IPL_BAKEDDATAVARIATION_REVERB,
                geometry::Sphere::default().into(),
            ),
            BakedDataVariation::StaticSource { endpoint_influence } => (
                audionimbus_sys::IPLBakedDataVariation::IPL_BAKEDDATAVARIATION_STATICSOURCE,
                endpoint_influence.into(),
            ),
            BakedDataVariation::StaticListener { endpoint_influence } => (
                audionimbus_sys::IPLBakedDataVariation::IPL_BAKEDDATAVARIATION_STATICLISTENER,
                endpoint_influence.into(),
            ),
            BakedDataVariation::Dynamic => (
                audionimbus_sys::IPLBakedDataVariation::IPL_BAKEDDATAVARIATION_DYNAMIC,
                geometry::Sphere::default().into(),
            ),
        };

        Self {
            type_,
            variation,
            endpointInfluence: endpoint_influence,
        }
    }
}

/// The different ways in which the source and listener positions used to generate baked data can vary as a function of probe position.
#[derive(Copy, Clone, Debug)]
pub enum BakedDataVariation {
    /// At each probe, baked data is calculated with both the source and the listener at the probe position.
    /// This is useful for modeling traditional reverbs, which depend only on the listener’s position (or only on the source’s position).
    Reverb,

    /// At each probe, baked data is calculated with the source at some fixed position (specified separately), and the listener at the probe position.
    /// This is used for modeling reflections from a static source to any point within the probe batch.
    StaticSource {
        /// The static source used to generate baked data.
        /// Baked data is only stored for probes that lie within the radius of this sphere.
        endpoint_influence: geometry::Sphere,
    },

    /// At each probe, baked data is calculated with the source at the probe position, and the listener at some fixed position (specified separately).
    /// This is used for modeling reflections from a moving source to a static listener.
    StaticListener {
        /// The static listener used to generate baked data.
        /// Baked data is only stored for probes that lie within the radius of this sphere.
        endpoint_influence: geometry::Sphere,
    },

    /// Baked data is calculated for each pair of probes.
    /// For example, this is used for calculating paths between every pair of probes in a batch.
    Dynamic,
}

/// Simulation parameters that are not specific to any source.
#[derive(Debug)]
pub struct SimulationSharedInputs {
    /// The position and orientation of the listener.
    pub listener: geometry::CoordinateSystem,

    /// The number of rays to trace from the listener.
    /// Increasing this value results in more accurate reflections, at the cost of increased CPU usage.
    pub num_rays: usize,

    /// The number of times each ray traced from the listener is reflected when it encounters a solid object.
    /// Increasing this value results in longer, more accurate reverb tails, at the cost of increased CPU usage during simulation.
    pub num_bounces: usize,

    /// The duration (in seconds) of the impulse responses generated when simulating reflections.
    /// Increasing this value results in longer, more accurate reverb tails, at the cost of increased CPU usage during audio processing.
    pub duration: f32,

    /// The Ambisonic order of the impulse responses generated when simulating reflections.
    /// Increasing this value results in more accurate directional variation of reflected sound, at the cost of increased CPU usage during audio processing.
    pub order: usize,

    /// When calculating how much sound energy reaches a surface directly from a source, any source that is closer than [`Self::irradiance_min_distance`] to the surface is assumed to be at a distance of [`Self::irradiance_min_distance`], for the purposes of energy calculations.
    pub irradiance_min_distance: f32,

    /// Optional callback for visualizing valid path segments during call to [`Simulator::run_pathing`].
    pub pathing_visualization_callback: Option<CallbackInformation<PathingVisualizationCallback>>,
}

impl From<&SimulationSharedInputs> for audionimbus_sys::IPLSimulationSharedInputs {
    fn from(simulation_shared_inputs: &SimulationSharedInputs) -> Self {
        let (pathing_visualization_callback, pathing_user_data) =
            if let Some(callback_information) =
                &simulation_shared_inputs.pathing_visualization_callback
            {
                (
                    Some(callback_information.callback),
                    callback_information.user_data,
                )
            } else {
                (None, std::ptr::null_mut())
            };

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
    userData: *mut std::ffi::c_void,
);

/// Simulation results for a source.
#[derive(Debug)]
pub struct SimulationOutputs(*mut audionimbus_sys::IPLSimulationOutputs);

impl SimulationOutputs {
    pub fn direct(&self) -> FFIWrapper<'_, DirectEffectParams, Self> {
        unsafe { FFIWrapper::new((*self.0).direct.into()) }
    }

    pub fn reflections(&self) -> FFIWrapper<'_, ReflectionEffectParams, Self> {
        unsafe { FFIWrapper::new((*self.0).reflections.into()) }
    }

    pub fn pathing(&self) -> FFIWrapper<'_, PathEffectParams, Self> {
        unsafe { FFIWrapper::new((*self.0).pathing.into()) }
    }

    pub fn raw_ptr(&self) -> *mut audionimbus_sys::IPLSimulationOutputs {
        self.0
    }

    pub fn raw_ptr_mut(&mut self) -> &mut *mut audionimbus_sys::IPLSimulationOutputs {
        &mut self.0
    }
}

impl Default for SimulationOutputs {
    fn default() -> Self {
        let ptr = unsafe {
            let layout = std::alloc::Layout::new::<audionimbus_sys::IPLSimulationOutputs>();
            let ptr = std::alloc::alloc(layout) as *mut audionimbus_sys::IPLSimulationOutputs;
            if ptr.is_null() {
                panic!("failed to allocate memory for IPLSimulationOutputs");
            }
            std::ptr::write(ptr, std::mem::zeroed());
            ptr
        };

        SimulationOutputs(ptr)
    }
}

impl Drop for SimulationOutputs {
    fn drop(&mut self) {
        unsafe {
            let layout = std::alloc::Layout::new::<audionimbus_sys::IPLSimulationOutputs>();
            std::alloc::dealloc(self.0 as *mut u8, layout);
        }
    }
}
