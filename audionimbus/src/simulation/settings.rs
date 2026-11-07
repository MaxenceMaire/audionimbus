//! Simulator creation settings and reflection algorithms.

#[cfg(doc)]
use super::ReflectionsSharedInputs;
#[cfg(feature = "dynamic")]
use super::SimulationFlags;
#[cfg(feature = "dynamic")]
use super::simulator::SimulationChoices;
use super::simulator::SimulatorBinding;
use super::{Direct, Pathing, Reflections};
#[cfg(feature = "dynamic")]
use crate::binding::Dynamic;
use crate::binding::Static;
use crate::device::{OpenClDevice, RadeonRaysDevice, TrueAudioNextDevice};
#[cfg(feature = "dynamic")]
use crate::dynamic::{self, RayTracerKind, ReflectionAlgorithm};
use crate::ray_tracing::{CustomRayTracer, DefaultRayTracer, Embree, RadeonRays, RayTracer};
use crate::{
    AudioSettings, Convolution, Hybrid, Parametric, ReflectionEffectType, Sealed, TrueAudioNext,
};

/// The largest pathing order whose native coefficient count fits in a signed 32-bit integer.
#[cfg(feature = "dynamic")]
pub const MAX_PATHING_ORDER: u32 = i32::MAX.isqrt() as u32 - 1;

/// Storage shared by static and runtime simulator settings.
#[derive(Debug)]
pub struct SimulationSettings<B: SimulatorBinding> {
    /// The native simulation settings.
    settings: audionimbus_sys::IPLSimulationSettings,
    /// The retained OpenCL device.
    open_cl_device: Option<OpenClDevice>,
    /// The retained Radeon Rays device.
    radeon_rays_device: Option<RadeonRaysDevice>,
    /// The retained TrueAudio Next device.
    true_audio_next_device: Option<TrueAudioNextDevice>,
    /// The compile-time or runtime simulation binding.
    binding: B,
}

// SAFETY: The raw pointers in IPLSimulationSettings point to devices owned by the struct and
// remain valid for its lifetime.
unsafe impl<T: RayTracer, D, R, P, RE> Send for StaticSimulationSettings<T, D, R, P, RE> {}

#[cfg(feature = "dynamic")]
unsafe impl Send for DynamicSimulationSettings {}

unsafe impl<T: RayTracer, D, R, P, RE> Sync for StaticSimulationSettings<T, D, R, P, RE> {}

#[cfg(feature = "dynamic")]
unsafe impl Sync for DynamicSimulationSettings {}

impl<B: SimulatorBinding> SimulationSettings<B> {
    /// Returns the audio settings stored in these simulation settings.
    pub const fn audio_settings(&self) -> AudioSettings {
        AudioSettings {
            sampling_rate: self.settings.samplingRate as u32,
            frame_size: self.settings.frameSize as u32,
        }
    }

    /// Converts the settings to the FFI representation.
    pub const fn to_ffi(&self) -> audionimbus_sys::IPLSimulationSettings {
        self.settings
    }
}

impl<B: SimulatorBinding> SimulationSettings<B> {
    /// Retains the devices and copies the native simulator settings.
    fn clone_shared(&self) -> Self {
        Self {
            settings: self.settings,
            open_cl_device: self.open_cl_device.clone(),
            radeon_rays_device: self.radeon_rays_device.clone(),
            true_audio_next_device: self.true_audio_next_device.clone(),
            binding: self.binding.clone(),
        }
    }
}

/// Settings used to create a [`crate::simulation::Simulator`].
///
/// # Examples
///
/// ```
/// # #[cfg(feature = "static")]
/// # fn main() -> Result<(), audionimbus::SteamAudioError> {
/// # use audionimbus::*;
/// # use audionimbus::r#static::SimulationSettings;
/// let audio_settings = AudioSettings::default();
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
/// # Ok::<(), audionimbus::SteamAudioError>(())
/// # }
/// # #[cfg(not(feature = "static"))]
/// # fn main() {}
/// ```
pub type StaticSimulationSettings<T, D = (), R = (), P = (), RE = ()> =
    SimulationSettings<Static<(T, D, R, P, RE)>>;

/// Creation settings with runtime simulation modes and algorithms.
#[cfg(feature = "dynamic")]
pub type DynamicSimulationSettings = SimulationSettings<Dynamic<SimulationChoices>>;

impl StaticSimulationSettings<DefaultRayTracer, (), (), (), ()> {
    /// Creates new simulation settings with all simulations disabled by default.
    pub fn new(audio_settings: &AudioSettings) -> Self {
        let settings = audionimbus_sys::IPLSimulationSettings {
            flags: audionimbus_sys::IPLSimulationFlags(0),
            sceneType: audionimbus_sys::IPLSceneType::IPL_SCENETYPE_DEFAULT,
            reflectionType:
                audionimbus_sys::IPLReflectionEffectType::IPL_REFLECTIONEFFECTTYPE_CONVOLUTION,
            maxNumOcclusionSamples: 0,
            maxNumRays: 0,
            numDiffuseSamples: 0,
            maxDuration: 0.0,
            maxOrder: 0,
            maxNumSources: 0,
            numThreads: 0,
            rayBatchSize: 0,
            numVisSamples: 0,
            samplingRate: audio_settings.sampling_rate as i32,
            frameSize: audio_settings.frame_size as i32,
            openCLDevice: std::ptr::null_mut(),
            radeonRaysDevice: std::ptr::null_mut(),
            tanDevice: std::ptr::null_mut(),
        };

        Self {
            settings,
            open_cl_device: None,
            radeon_rays_device: None,
            true_audio_next_device: None,
            binding: Static::new(),
        }
    }
}

impl<D, R, P, RE> StaticSimulationSettings<DefaultRayTracer, D, R, P, RE> {
    /// Switches to the Embree ray tracer.
    pub fn with_embree(self) -> StaticSimulationSettings<Embree, D, R, P, RE> {
        let Self {
            mut settings,
            open_cl_device,
            radeon_rays_device,
            true_audio_next_device,
            ..
        } = self;
        settings.sceneType = Embree::scene_type();

        StaticSimulationSettings {
            binding: Static::new(),
            settings,
            open_cl_device,
            radeon_rays_device,
            true_audio_next_device,
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
        open_cl_device: OpenClDevice,
        radeon_rays_device: RadeonRaysDevice,
    ) -> StaticSimulationSettings<RadeonRays, D, R, P, RE> {
        let Self {
            mut settings,
            true_audio_next_device,
            ..
        } = self;
        settings.sceneType = RadeonRays::scene_type();
        settings.openCLDevice = open_cl_device.raw_ptr();
        settings.radeonRaysDevice = radeon_rays_device.raw_ptr();

        StaticSimulationSettings {
            binding: Static::new(),
            settings,
            open_cl_device: Some(open_cl_device),
            radeon_rays_device: Some(radeon_rays_device),
            true_audio_next_device,
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
    ) -> StaticSimulationSettings<CustomRayTracer, D, R, P, RE> {
        let Self {
            mut settings,
            open_cl_device,
            radeon_rays_device,
            true_audio_next_device,
            ..
        } = self;
        settings.sceneType = CustomRayTracer::scene_type();
        settings.rayBatchSize = ray_batch_size as i32;

        StaticSimulationSettings {
            binding: Static::new(),
            settings,
            open_cl_device,
            radeon_rays_device,
            true_audio_next_device,
        }
    }
}

impl<T: RayTracer, D, R, P, RE> StaticSimulationSettings<T, D, R, P, RE> {
    /// Enables direct simulation.
    pub fn with_direct(
        self,
        direct_settings: DirectSimulationSettings,
    ) -> StaticSimulationSettings<T, Direct, R, P, RE> {
        let Self {
            mut settings,
            open_cl_device,
            radeon_rays_device,
            true_audio_next_device,
            ..
        } = self;

        settings.flags |= audionimbus_sys::IPLSimulationFlags::IPL_SIMULATIONFLAGS_DIRECT;
        settings.maxNumOcclusionSamples = direct_settings.max_num_occlusion_samples as i32;

        StaticSimulationSettings {
            binding: Static::new(),
            settings,
            open_cl_device,
            radeon_rays_device,
            true_audio_next_device,
        }
    }

    /// Enables reflections simulation.
    pub fn with_reflections<A: ReflectionsAlgorithm>(
        self,
        algorithm_settings: A,
    ) -> StaticSimulationSettings<T, D, Reflections, P, A::EffectType> {
        algorithm_settings.apply(self)
    }

    /// Enables pathing simulation.
    pub fn with_pathing(
        self,
        pathing_settings: PathingSimulationSettings,
    ) -> StaticSimulationSettings<T, D, R, Pathing, RE> {
        let Self {
            mut settings,
            open_cl_device,
            radeon_rays_device,
            true_audio_next_device,
            ..
        } = self;

        settings.flags |= audionimbus_sys::IPLSimulationFlags::IPL_SIMULATIONFLAGS_PATHING;
        settings.numVisSamples = pathing_settings.num_visibility_samples as i32;

        StaticSimulationSettings {
            binding: Static::new(),
            settings,
            open_cl_device,
            radeon_rays_device,
            true_audio_next_device,
        }
    }
}

#[cfg(feature = "dynamic")]
impl DynamicSimulationSettings {
    /// Creates settings with the default ray tracer and no simulation modes.
    pub fn new(audio_settings: &AudioSettings) -> Self {
        let simulation_settings = StaticSimulationSettings::new(audio_settings);

        Self {
            settings: simulation_settings.settings,
            open_cl_device: None,
            radeon_rays_device: None,
            true_audio_next_device: None,
            binding: Dynamic(SimulationChoices {
                flags: SimulationFlags::empty(),
                ray_tracer: RayTracerKind::Default,
                reflection_algorithm: None,
                ray_open_cl_device: None,
                max_pathing_order: 0,
                #[cfg(any(feature = "fmod", feature = "wwise"))]
                creation_settings: simulation_settings.settings,
            }),
        }
    }

    /// Selects the ray tracer for subsequent simulator creation.
    pub fn with_ray_tracer(mut self, ray_tracer_settings: dynamic::RayTracerSettings) -> Self {
        self.binding.0.ray_tracer = ray_tracer_settings.kind();
        self.binding.0.ray_open_cl_device = None;
        self.radeon_rays_device = None;
        self.settings.radeonRaysDevice = std::ptr::null_mut();
        self.settings.rayBatchSize = 0;
        self.settings.sceneType = match ray_tracer_settings {
            dynamic::RayTracerSettings::Default => DefaultRayTracer::scene_type(),
            dynamic::RayTracerSettings::Embree { .. } => Embree::scene_type(),
            dynamic::RayTracerSettings::RadeonRays {
                open_cl_device,
                radeon_rays_device,
            } => {
                self.settings.radeonRaysDevice = radeon_rays_device.raw_ptr();
                self.binding.0.ray_open_cl_device = Some(open_cl_device);
                self.radeon_rays_device = Some(radeon_rays_device);
                RadeonRays::scene_type()
            }
            dynamic::RayTracerSettings::Custom { ray_batch_size, .. } => {
                self.settings.rayBatchSize = ray_batch_size as i32;
                CustomRayTracer::scene_type()
            }
        };
        if self.true_audio_next_device.is_none() {
            self.open_cl_device = self.binding.0.ray_open_cl_device.clone();
            self.settings.openCLDevice = self
                .open_cl_device
                .as_ref()
                .map_or(std::ptr::null_mut(), OpenClDevice::raw_ptr);
        }

        self
    }

    /// Enables direct simulation.
    pub fn with_direct(mut self, direct_settings: DirectSimulationSettings) -> Self {
        self.binding.0.flags |= SimulationFlags::DIRECT;
        self.settings.flags |= audionimbus_sys::IPLSimulationFlags::IPL_SIMULATIONFLAGS_DIRECT;
        self.settings.maxNumOcclusionSamples = direct_settings.max_num_occlusion_samples as i32;

        self
    }

    /// Enables the selected reflection algorithm.
    pub fn with_reflections(
        mut self,
        reflections_settings: impl Into<ReflectionsSettings>,
    ) -> Self {
        let reflections_settings = reflections_settings.into();
        self.binding.0.flags |= SimulationFlags::REFLECTIONS;
        self.binding.0.reflection_algorithm = Some(reflections_settings.algorithm());
        let simulation_settings = StaticSimulationSettings::new(&self.audio_settings());
        let (reflection_settings, open_cl_device, true_audio_next_device) =
            match reflections_settings {
                ReflectionsSettings::Convolution(reflections_settings) => {
                    let simulation_settings =
                        simulation_settings.with_reflections(reflections_settings);
                    (
                        simulation_settings.settings,
                        simulation_settings.open_cl_device,
                        simulation_settings.true_audio_next_device,
                    )
                }
                ReflectionsSettings::Parametric(reflections_settings) => {
                    let simulation_settings =
                        simulation_settings.with_reflections(reflections_settings);
                    (
                        simulation_settings.settings,
                        simulation_settings.open_cl_device,
                        simulation_settings.true_audio_next_device,
                    )
                }
                ReflectionsSettings::Hybrid(reflections_settings) => {
                    let simulation_settings =
                        simulation_settings.with_reflections(reflections_settings);
                    (
                        simulation_settings.settings,
                        simulation_settings.open_cl_device,
                        simulation_settings.true_audio_next_device,
                    )
                }
                ReflectionsSettings::TrueAudioNext(reflections_settings) => {
                    let simulation_settings =
                        simulation_settings.with_reflections(reflections_settings);
                    (
                        simulation_settings.settings,
                        simulation_settings.open_cl_device,
                        simulation_settings.true_audio_next_device,
                    )
                }
            };
        self.settings.flags |= reflection_settings.flags;
        self.settings.reflectionType = reflection_settings.reflectionType;
        self.settings.maxNumRays = reflection_settings.maxNumRays;
        self.settings.numDiffuseSamples = reflection_settings.numDiffuseSamples;
        self.settings.maxDuration = reflection_settings.maxDuration;
        self.settings.maxNumSources = reflection_settings.maxNumSources;
        self.settings.numThreads = reflection_settings.numThreads;
        self.settings.maxOrder = if self.binding.0.flags.contains(SimulationFlags::PATHING) {
            self.settings.maxOrder.max(reflection_settings.maxOrder)
        } else {
            reflection_settings.maxOrder
        };
        self.settings.tanDevice = reflection_settings.tanDevice;
        self.true_audio_next_device = true_audio_next_device;
        self.open_cl_device = open_cl_device.or_else(|| self.binding.0.ray_open_cl_device.clone());
        self.settings.openCLDevice = self
            .open_cl_device
            .as_ref()
            .map_or(std::ptr::null_mut(), OpenClDevice::raw_ptr);

        self
    }

    /// Enables pathing simulation with the specified maximum Ambisonic order.
    ///
    /// Reflection and pathing simulation share the larger creation order.
    ///
    /// # Arguments
    ///
    /// - `pathing_settings`: Visibility settings for pathing simulation.
    /// - `max_order`: The maximum Ambisonic order requested by pathing inputs.
    ///
    /// # Panics
    ///
    /// Panics if the coefficient count, `(max_order + 1)^2`, exceeds `i32::MAX`.
    pub fn with_pathing(
        mut self,
        pathing_settings: PathingSimulationSettings,
        max_order: u32,
    ) -> Self {
        assert!(
            max_order <= MAX_PATHING_ORDER,
            "pathing coefficient count exceeds i32::MAX"
        );
        self.binding.0.flags |= SimulationFlags::PATHING;
        self.settings.flags |= audionimbus_sys::IPLSimulationFlags::IPL_SIMULATIONFLAGS_PATHING;
        self.settings.numVisSamples = pathing_settings.num_visibility_samples as i32;
        self.settings.maxOrder = self.settings.maxOrder.max(max_order as i32);

        self
    }

    /// Returns the available simulation modes.
    pub const fn flags(&self) -> SimulationFlags {
        self.binding.0.flags
    }

    /// Returns the selected reflection algorithm.
    pub const fn reflection_algorithm(&self) -> Option<ReflectionAlgorithm> {
        self.binding.0.reflection_algorithm
    }
}

impl<T: RayTracer + Clone, D: Clone, R: Clone, P: Clone, RE: Clone> Clone
    for StaticSimulationSettings<T, D, R, P, RE>
{
    /// Retains the devices used by these settings.
    fn clone(&self) -> Self {
        self.clone_shared()
    }
}

#[cfg(feature = "dynamic")]
impl Clone for DynamicSimulationSettings {
    fn clone(&self) -> Self {
        self.clone_shared()
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

impl Default for DirectSimulationSettings {
    fn default() -> Self {
        Self {
            max_num_occlusion_samples: 32,
        }
    }
}

/// Algorithm used for reflections simulation.
///
/// See:
/// - [`ConvolutionSettings`]
/// - [`ParametricSettings`]
/// - [`HybridSettings`]
/// - [`TrueAudioNextSettings`]
pub trait ReflectionsAlgorithm: Sealed {
    /// The type of the algorithm.
    type EffectType: ReflectionEffectType;

    /// Applies the algorithm settings to the specified simulation settings.
    fn apply<T: RayTracer, D, R, P, RE>(
        self,
        settings: StaticSimulationSettings<T, D, R, P, RE>,
    ) -> StaticSimulationSettings<T, D, Reflections, P, Self::EffectType>;
}

/// Settings for multi-channel convolution reverb.
#[derive(Debug, Copy, Clone)]
pub struct ConvolutionSettings {
    /// The maximum number of rays to trace from the listener when simulating reflections.
    /// You can use different numbers of rays between simulation runs, but this is the maximum value.
    /// Increasing this value results in more accurate reflections, at the cost of increased CPU usage.
    pub max_num_rays: u32,

    /// The number of directions to sample when generating diffusely reflected rays.
    /// Increasing this value may increase the accuracy of diffuse reflections.
    pub num_diffuse_samples: u32,

    /// The maximum length (in seconds) of impulse responses generated by reflection simulations.
    /// You can change this value betweeen simulation runs, but this is the maximum value.
    /// Increasing this value results in longer, more accurate reverb tails, at the cost of increased CPU and memory usage.
    pub max_duration: f32,

    /// The maximum number of sources for which reflection simulations will be run at any given time.
    pub max_num_sources: u32,

    /// The number of threads used for real-time reflection simulations.
    pub num_threads: u32,

    /// The maximum Ambisonic order of impulse responses generated by reflection simulations.
    ///
    /// This value is fixed for the lifetime of the simulator and determines how much memory
    /// is allocated.
    /// Higher orders result in more accurate directional variation in the simulated reflections,
    /// at the cost of increased CPU and memory usage.
    ///
    /// The actual order used per simulation run can be set independently via
    /// [`ReflectionsSharedInputs::order`], as long as it does not exceed this value.
    pub max_order: u32,
}

impl ReflectionsAlgorithm for ConvolutionSettings {
    type EffectType = Convolution;

    fn apply<T: RayTracer, D, R, P, RE>(
        self,
        simulation_settings: StaticSimulationSettings<T, D, R, P, RE>,
    ) -> StaticSimulationSettings<T, D, Reflections, P, Self::EffectType> {
        let StaticSimulationSettings {
            mut settings,
            open_cl_device,
            radeon_rays_device,
            true_audio_next_device,
            ..
        } = simulation_settings;

        settings.flags |= audionimbus_sys::IPLSimulationFlags::IPL_SIMULATIONFLAGS_REFLECTIONS;
        settings.reflectionType =
            audionimbus_sys::IPLReflectionEffectType::IPL_REFLECTIONEFFECTTYPE_CONVOLUTION;
        settings.maxNumRays = self.max_num_rays as i32;
        settings.numDiffuseSamples = self.num_diffuse_samples as i32;
        settings.maxDuration = self.max_duration;
        settings.maxNumSources = self.max_num_sources as i32;
        settings.numThreads = self.num_threads as i32;
        settings.maxOrder = self.max_order as i32;

        StaticSimulationSettings {
            binding: Static::new(),
            settings,
            open_cl_device,
            radeon_rays_device,
            true_audio_next_device,
        }
    }
}

impl Default for ConvolutionSettings {
    fn default() -> Self {
        Self {
            max_num_rays: 256,
            num_diffuse_samples: 256,
            max_duration: 2.0,
            max_num_sources: 64,
            num_threads: 4,
            max_order: 2,
        }
    }
}

/// Settings for parametric (or artificial) reverb, using feedback delay networks.
#[derive(Debug, Copy, Clone)]
pub struct ParametricSettings {
    /// The maximum number of rays to trace from the listener when simulating reflections.
    /// You can use different numbers of rays between simulation runs, but this is the maximum value.
    /// Increasing this value results in more accurate reflections, at the cost of increased CPU usage.
    pub max_num_rays: u32,

    /// The number of directions to sample when generating diffusely reflected rays.
    /// Increasing this value may increase the accuracy of diffuse reflections.
    pub num_diffuse_samples: u32,

    /// The maximum length (in seconds) of impulse responses generated by reflection simulations.
    /// You can change this value betweeen simulation runs, but this is the maximum value.
    /// Increasing this value results in longer, more accurate reverb tails, at the cost of increased CPU and memory usage.
    pub max_duration: f32,

    /// The maximum number of sources for which reflection simulations will be run at any given time.
    pub max_num_sources: u32,

    /// The number of threads used for real-time reflection simulations.
    pub num_threads: u32,

    /// The maximum Ambisonic order of impulse responses generated by reflection simulations.
    ///
    /// This value is fixed for the lifetime of the simulator and determines how much memory
    /// is allocated.
    /// Higher orders result in more accurate directional variation in the simulated reflections,
    /// at the cost of increased CPU and memory usage.
    ///
    /// The actual order used per simulation run can be set independently via
    /// [`ReflectionsSharedInputs::order`], as long as it does not exceed this value.
    pub max_order: u32,
}

impl ReflectionsAlgorithm for ParametricSettings {
    type EffectType = Parametric;

    fn apply<T: RayTracer, D, R, P, RE>(
        self,
        simulation_settings: StaticSimulationSettings<T, D, R, P, RE>,
    ) -> StaticSimulationSettings<T, D, Reflections, P, Self::EffectType> {
        let StaticSimulationSettings {
            mut settings,
            open_cl_device,
            radeon_rays_device,
            true_audio_next_device,
            ..
        } = simulation_settings;

        settings.flags |= audionimbus_sys::IPLSimulationFlags::IPL_SIMULATIONFLAGS_REFLECTIONS;
        settings.reflectionType =
            audionimbus_sys::IPLReflectionEffectType::IPL_REFLECTIONEFFECTTYPE_PARAMETRIC;
        settings.maxNumRays = self.max_num_rays as i32;
        settings.numDiffuseSamples = self.num_diffuse_samples as i32;
        settings.maxDuration = self.max_duration;
        settings.maxNumSources = self.max_num_sources as i32;
        settings.numThreads = self.num_threads as i32;
        settings.maxOrder = self.max_order as i32;

        StaticSimulationSettings {
            binding: Static::new(),
            settings,
            open_cl_device,
            radeon_rays_device,
            true_audio_next_device,
        }
    }
}

/// Settings for a hybrid of convolution and parametric reverb.
#[derive(Debug, Copy, Clone)]
pub struct HybridSettings {
    /// The maximum number of rays to trace from the listener when simulating reflections.
    /// You can use different numbers of rays between simulation runs, but this is the maximum value.
    /// Increasing this value results in more accurate reflections, at the cost of increased CPU usage.
    pub max_num_rays: u32,

    /// The number of directions to sample when generating diffusely reflected rays.
    /// Increasing this value may increase the accuracy of diffuse reflections.
    pub num_diffuse_samples: u32,

    /// The maximum length (in seconds) of impulse responses generated by reflection simulations.
    /// You can change this value betweeen simulation runs, but this is the maximum value.
    /// Increasing this value results in longer, more accurate reverb tails, at the cost of increased CPU and memory usage.
    pub max_duration: f32,

    /// The maximum number of sources for which reflection simulations will be run at any given time.
    pub max_num_sources: u32,

    /// The number of threads used for real-time reflection simulations.
    pub num_threads: u32,

    /// The maximum Ambisonic order of impulse responses generated by reflection simulations.
    ///
    /// This value is fixed for the lifetime of the simulator and determines how much memory
    /// is allocated.
    /// Higher orders result in more accurate directional variation in the simulated reflections,
    /// at the cost of increased CPU and memory usage.
    ///
    /// The actual order used per simulation run can be set independently via
    /// [`ReflectionsSharedInputs::order`], as long as it does not exceed this value.
    pub max_order: u32,
}

impl ReflectionsAlgorithm for HybridSettings {
    type EffectType = Hybrid;

    fn apply<T: RayTracer, D, R, P, RE>(
        self,
        simulation_settings: StaticSimulationSettings<T, D, R, P, RE>,
    ) -> StaticSimulationSettings<T, D, Reflections, P, Self::EffectType> {
        let StaticSimulationSettings {
            mut settings,
            open_cl_device,
            radeon_rays_device,
            true_audio_next_device,
            ..
        } = simulation_settings;

        settings.flags |= audionimbus_sys::IPLSimulationFlags::IPL_SIMULATIONFLAGS_REFLECTIONS;
        settings.reflectionType =
            audionimbus_sys::IPLReflectionEffectType::IPL_REFLECTIONEFFECTTYPE_HYBRID;
        settings.maxNumRays = self.max_num_rays as i32;
        settings.numDiffuseSamples = self.num_diffuse_samples as i32;
        settings.maxDuration = self.max_duration;
        settings.maxNumSources = self.max_num_sources as i32;
        settings.numThreads = self.num_threads as i32;
        settings.maxOrder = self.max_order as i32;

        StaticSimulationSettings {
            binding: Static::new(),
            settings,
            open_cl_device,
            radeon_rays_device,
            true_audio_next_device,
        }
    }
}

/// Settings for a multi-channel convolution reverb, using AMD TrueAudio Next for GPU acceleration.
#[derive(Debug, Clone)]
pub struct TrueAudioNextSettings {
    /// The maximum number of rays to trace from the listener when simulating reflections.
    /// You can use different numbers of rays between simulation runs, but this is the maximum value.
    /// Increasing this value results in more accurate reflections, at the cost of increased CPU usage.
    pub max_num_rays: u32,

    /// The number of directions to sample when generating diffusely reflected rays.
    /// Increasing this value may increase the accuracy of diffuse reflections.
    pub num_diffuse_samples: u32,

    /// The maximum length (in seconds) of impulse responses generated by reflection simulations.
    /// You can change this value betweeen simulation runs, but this is the maximum value.
    /// Increasing this value results in longer, more accurate reverb tails, at the cost of increased CPU and memory usage.
    pub max_duration: f32,

    /// The maximum number of sources for which reflection simulations will be run at any given time.
    pub max_num_sources: u32,

    /// The number of threads used for real-time reflection simulations.
    pub num_threads: u32,

    /// The OpenCL device being used.
    pub open_cl_device: OpenClDevice,

    /// The TrueAudio Next device being used.
    pub true_audio_next_device: TrueAudioNextDevice,

    /// The maximum Ambisonic order of impulse responses generated by reflection simulations.
    ///
    /// This value is fixed for the lifetime of the simulator and determines how much memory
    /// is allocated.
    /// Higher orders result in more accurate directional variation in the simulated reflections,
    /// at the cost of increased CPU and memory usage.
    ///
    /// The actual order used per simulation run can be set independently via
    /// [`ReflectionsSharedInputs::order`], as long as it does not exceed this value.
    pub max_order: u32,
}

impl ReflectionsAlgorithm for TrueAudioNextSettings {
    type EffectType = TrueAudioNext;

    fn apply<T: RayTracer, D, R, P, RE>(
        self,
        simulation_settings: StaticSimulationSettings<T, D, R, P, RE>,
    ) -> StaticSimulationSettings<T, D, Reflections, P, Self::EffectType> {
        let StaticSimulationSettings {
            mut settings,
            radeon_rays_device,
            ..
        } = simulation_settings;

        settings.flags |= audionimbus_sys::IPLSimulationFlags::IPL_SIMULATIONFLAGS_REFLECTIONS;
        settings.reflectionType =
            audionimbus_sys::IPLReflectionEffectType::IPL_REFLECTIONEFFECTTYPE_TAN;
        settings.openCLDevice = self.open_cl_device.raw_ptr();
        settings.tanDevice = self.true_audio_next_device.raw_ptr();
        settings.maxNumRays = self.max_num_rays as i32;
        settings.numDiffuseSamples = self.num_diffuse_samples as i32;
        settings.maxDuration = self.max_duration;
        settings.maxNumSources = self.max_num_sources as i32;
        settings.numThreads = self.num_threads as i32;
        settings.maxOrder = self.max_order as i32;

        StaticSimulationSettings {
            binding: Static::new(),
            settings,
            open_cl_device: Some(self.open_cl_device),
            radeon_rays_device,
            true_audio_next_device: Some(self.true_audio_next_device),
        }
    }
}

impl Sealed for ConvolutionSettings {}

impl Sealed for ParametricSettings {}

impl Sealed for HybridSettings {}

impl Sealed for TrueAudioNextSettings {}

/// Simulation settings for a runtime-selected reflection algorithm.
#[cfg(feature = "dynamic")]
#[derive(Clone, Debug)]
pub enum ReflectionsSettings {
    /// Multi-channel convolution settings.
    Convolution(ConvolutionSettings),
    /// Parametric reverb settings.
    Parametric(ParametricSettings),
    /// Combined convolution and parametric settings.
    Hybrid(HybridSettings),
    /// GPU convolution settings and devices.
    TrueAudioNext(TrueAudioNextSettings),
}

#[cfg(feature = "dynamic")]
impl ReflectionsSettings {
    /// Returns the selected algorithm.
    pub const fn algorithm(&self) -> ReflectionAlgorithm {
        match self {
            Self::Convolution(_) => ReflectionAlgorithm::Convolution,
            Self::Parametric(_) => ReflectionAlgorithm::Parametric,
            Self::Hybrid(_) => ReflectionAlgorithm::Hybrid,
            Self::TrueAudioNext(_) => ReflectionAlgorithm::TrueAudioNext,
        }
    }
}

#[cfg(feature = "dynamic")]
impl From<ConvolutionSettings> for ReflectionsSettings {
    fn from(convolution_settings: ConvolutionSettings) -> Self {
        Self::Convolution(convolution_settings)
    }
}

#[cfg(feature = "dynamic")]
impl From<ParametricSettings> for ReflectionsSettings {
    fn from(parametric_settings: ParametricSettings) -> Self {
        Self::Parametric(parametric_settings)
    }
}

#[cfg(feature = "dynamic")]
impl From<HybridSettings> for ReflectionsSettings {
    fn from(hybrid_settings: HybridSettings) -> Self {
        Self::Hybrid(hybrid_settings)
    }
}

#[cfg(feature = "dynamic")]
impl From<TrueAudioNextSettings> for ReflectionsSettings {
    fn from(true_audio_next_settings: TrueAudioNextSettings) -> Self {
        Self::TrueAudioNext(true_audio_next_settings)
    }
}

/// Settings used for pathing simulation.
#[derive(Debug, Copy, Clone)]
pub struct PathingSimulationSettings {
    /// The number of point samples to consider when calculating probe-to-probe visibility for pathing simulations.
    /// Baked paths may end up being occluded by dynamic objects, in which case you can configure the simulator to look for alternate paths in real time.
    /// This process will involve checking visibility between probes.
    pub num_visibility_samples: u32,
}

impl Default for PathingSimulationSettings {
    fn default() -> Self {
        Self {
            num_visibility_samples: 16,
        }
    }
}

impl<B: SimulatorBinding> SimulationSettings<B> {
    /// Returns the binding and devices retained for simulator construction.
    ///
    /// Binding selectors must stay inaccessible to downstream callers.
    pub(crate) fn simulator_parts(
        &self,
    ) -> (
        &B,
        &Option<OpenClDevice>,
        &Option<RadeonRaysDevice>,
        &Option<TrueAudioNextDevice>,
    ) {
        (
            &self.binding,
            &self.open_cl_device,
            &self.radeon_rays_device,
            &self.true_audio_next_device,
        )
    }
}
