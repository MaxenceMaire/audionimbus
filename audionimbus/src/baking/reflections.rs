use super::BakedDataIdentifier;
use crate::callback::{CallbackInformation, ProgressCallback};
use crate::context::Context;
use crate::device::open_cl::OpenClDevice;
use crate::device::radeon_rays::RadeonRaysDevice;
use crate::geometry::Scene;
use crate::probe::ProbeBatch;
use crate::ray_tracing::{CustomRayTracer, DefaultRayTracer, Embree, RadeonRays, RayTracer};
use std::marker::PhantomData;

#[cfg(doc)]
use super::BakedDataVariation;

/// A baker of reflections.
///
/// Simulating reflections in real-time is a very compute-intensive process.
/// [`ReflectionsBaker`] lets you bake, or precompute reflections throughout a scene (or part of a scene) beforehand.
///
/// Reflections are baked at several points that you specify.
/// Each of these points is called a probe and belong to a [`ProbeBatch`].
///
/// Just like game engines use light probes to store the variation of lighting throughout a scene, Steam Audio uses acoustic probes to store the variation of acoustic data (in this case, reflections) throughout a scene.
pub struct ReflectionsBaker<'a, T: RayTracer> {
    ray_batch_size: i32,
    open_cl_device: Option<&'a OpenClDevice>,
    radeon_rays_device: Option<&'a RadeonRaysDevice>,
    _marker: PhantomData<T>,
}

impl<'a> ReflectionsBaker<'a, DefaultRayTracer> {
    /// Creates a new [`ReflectionsBaker`].
    pub fn new() -> Self {
        Self {
            ray_batch_size: 0,
            open_cl_device: None,
            radeon_rays_device: None,
            _marker: PhantomData,
        }
    }
}

impl<'a> ReflectionsBaker<'a, Embree> {
    /// Creates a new [`ReflectionsBaker`].
    pub fn new() -> Self {
        Self {
            ray_batch_size: 0,
            open_cl_device: None,
            radeon_rays_device: None,
            _marker: PhantomData,
        }
    }
}

impl<'a> ReflectionsBaker<'a, RadeonRays> {
    /// Creates a new [`ReflectionsBaker`].
    pub fn new(open_cl_device: &'a OpenClDevice, radeon_rays_device: &'a RadeonRaysDevice) -> Self {
        Self {
            ray_batch_size: 0,
            open_cl_device: Some(open_cl_device),
            radeon_rays_device: Some(radeon_rays_device),
            _marker: PhantomData,
        }
    }
}

impl<'a> ReflectionsBaker<'a, CustomRayTracer> {
    /// Creates a new [`ReflectionsBaker`].
    pub fn new(ray_batch_size: u32) -> Self {
        Self {
            ray_batch_size: ray_batch_size as i32,
            open_cl_device: None,
            radeon_rays_device: None,
            _marker: PhantomData,
        }
    }
}

impl<'a, T: RayTracer> ReflectionsBaker<'a, T> {
    /// Bakes a single layer of reflections data in a probe batch.
    ///
    /// Only one bake can be in progress at any point in time.
    pub fn bake(
        &self,
        context: &Context,
        probe_batch: &mut ProbeBatch,
        scene: &Scene<T>,
        params: ReflectionsBakeParams,
    ) {
        self.bake_with_optional_progress_callback(context, probe_batch, scene, params, None);
    }

    /// Bakes a single layer of reflections data in a probe batch, with a progress callback.
    ///
    /// Only one bake can be in progress at any point in time.
    pub fn bake_with_progress_callback(
        &self,
        context: &Context,
        probe_batch: &mut ProbeBatch,
        scene: &Scene<T>,
        params: ReflectionsBakeParams,
        progress_callback: CallbackInformation<ProgressCallback>,
    ) {
        self.bake_with_optional_progress_callback(
            context,
            probe_batch,
            scene,
            params,
            Some(progress_callback),
        );
    }

    /// Bakes a single layer of reflections data in a probe batch, with an optional progress callback.
    ///
    /// Only one bake can be in progress at any point in time.
    fn bake_with_optional_progress_callback(
        &self,
        context: &Context,
        probe_batch: &mut ProbeBatch,
        scene: &Scene<T>,
        params: ReflectionsBakeParams,
        progress_callback: Option<CallbackInformation<ProgressCallback>>,
    ) {
        let (callback, user_data) = if let Some(callback_information) = progress_callback {
            (
                Some(callback_information.callback),
                callback_information.user_data,
            )
        } else {
            (None, std::ptr::null_mut())
        };

        let mut ffi_params = audionimbus_sys::IPLReflectionsBakeParams {
            scene: scene.raw_ptr(),
            probeBatch: probe_batch.raw_ptr(),
            sceneType: T::scene_type(),
            identifier: (*params.identifier).into(),
            bakeFlags: params.bake_flags.into(),
            numRays: params.num_rays as i32,
            numDiffuseSamples: params.num_diffuse_samples as i32,
            numBounces: params.num_bounces as i32,
            simulatedDuration: params.simulated_duration,
            savedDuration: params.saved_duration,
            order: params.order as i32,
            numThreads: params.num_threads as i32,
            rayBatchSize: self.ray_batch_size,
            irradianceMinDistance: params.irradiance_min_distance,
            bakeBatchSize: params.bake_batch_size as i32,
            openCLDevice: self
                .open_cl_device
                .map_or(std::ptr::null_mut(), OpenClDevice::raw_ptr),
            radeonRaysDevice: self
                .radeon_rays_device
                .map_or(std::ptr::null_mut(), RadeonRaysDevice::raw_ptr),
        };

        unsafe {
            audionimbus_sys::iplReflectionsBakerBake(
                context.raw_ptr(),
                &mut ffi_params,
                callback,
                user_data,
            );
        }
    }

    /// Cancels any running bakes of reflections data.
    pub fn cancel_bake(&self, context: &Context) {
        unsafe { audionimbus_sys::iplReflectionsBakerCancelBake(context.raw_ptr()) }
    }
}

impl<'a> Default for ReflectionsBaker<'a, DefaultRayTracer> {
    fn default() -> Self {
        Self::new()
    }
}

/// Parameters used to control how reflections data is baked.
#[derive(Debug, Copy, Clone)]
pub struct ReflectionsBakeParams<'a> {
    /// An identifier for the data layer that should be baked.
    /// The identifier determines what data is simulated and stored at each probe.
    /// If the probe batch already contains data with this identifier, it will be overwritten.
    pub identifier: &'a BakedDataIdentifier,

    /// The types of data to save for each probe.
    pub bake_flags: ReflectionsBakeFlags,

    /// The number of rays to trace from each listener position when baking.
    /// Increasing this number results in improved accuracy, at the cost of increased bake times.
    pub num_rays: u32,

    /// The number of directions to consider when generating diffusely-reflected rays when baking.
    /// Increasing this number results in slightly improved accuracy of diffuse reflections.
    pub num_diffuse_samples: u32,

    /// The number of times each ray is reflected off of solid geometry.
    /// Increasing this number results in longer reverb tails and improved accuracy, at the cost of increased bake times.
    pub num_bounces: u32,

    /// The length (in seconds) of the impulse responses to simulate.
    /// Increasing this number allows the baked data to represent longer reverb tails (and hence larger spaces), at the cost of increased memory usage while baking.
    pub simulated_duration: f32,

    /// The length (in seconds) of the impulse responses to save at each probe.
    /// Increasing this number allows the baked data to represent longer reverb tails (and hence larger spaces), at the cost of increased disk space usage and memory usage at run-time.
    ///
    /// It may be useful to set [`Self::saved_duration`] to be less than [`Self::simulated_duration`], especially if you plan to use hybrid reverb for rendering baked reflections.
    /// This way, the parametric reverb data is estimated using a longer IR, resulting in more accurate estimation, but only the early part of the IR can be saved for subsequent rendering.
    pub saved_duration: f32,

    /// Ambisonic order of the baked IRs.
    pub order: u32,

    /// Number of threads to use for baking.
    pub num_threads: u32,

    /// When calculating how much sound energy reaches a surface directly from a source, any source that is closer than [`Self::irradiance_min_distance`] to the surface is assumed to be at a distance of [`Self::irradiance_min_distance`], for the purposes of energy calculations.
    pub irradiance_min_distance: f32,

    /// If using Radeon Rays or if [`Self::identifier`] uses [`BakedDataVariation::StaticListener`], this is the number of probes for which data is baked simultaneously.
    pub bake_batch_size: u32,
}

bitflags::bitflags! {
    /// Flags for specifying what types of reflections data to bake.
    #[derive(Copy, Clone, Debug)]
    pub struct ReflectionsBakeFlags: u32 {
        /// Bake impulse responses for [`ReflectionEffectSettings::Convolution`], [`ReflectionEffectSettings::Hybrid`], or [`ReflectionEffectSettings::TrueAudioNext`].
        const BAKE_CONVOLUTION = 1 << 0;

        /// Bake parametric reverb for [`ReflectionEffectSettings::Parametric`] or [`ReflectionEffectSettings::Hybrid`].
        const BAKE_PARAMETRIC = 1 << 1;
    }
}

impl From<ReflectionsBakeFlags> for audionimbus_sys::IPLReflectionsBakeFlags {
    fn from(reflections_bake_flags: ReflectionsBakeFlags) -> Self {
        Self(reflections_bake_flags.bits() as _)
    }
}
