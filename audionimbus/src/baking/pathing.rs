use super::BakedDataIdentifier;
use crate::callback::{CallbackInformation, ProgressCallback};
use crate::context::Context;
use crate::geometry::Scene;
use crate::probe::ProbeBatch;
use crate::ray_tracing::RayTracer;
use std::marker::PhantomData;

#[cfg(doc)]
use super::BakedDataVariation;

/// A baker of pathing data.
///
/// Pathing is an alternative simulation method to reflections that finds the shortest
/// unoccluded paths from sources to listeners by traveling between probes.
///
/// [`PathBaker`] lets you bake, or precompute pathing data throughout a scene (or part
/// of a scene) beforehand. This precomputation is typically done offline since pathing
/// requires probe generation.
///
/// Pathing data is baked at several points that you specify. Each of these points is
/// called a probe and belongs to a [`ProbeBatch`].
///
/// Just like game engines use light probes to store the variation of lighting throughout
/// a scene, Steam Audio uses acoustic probes to store the variation of acoustic data
/// (in this case, pathing information) throughout a scene.
#[derive(Default)]
pub struct PathBaker<T: RayTracer> {
    _marker: PhantomData<T>,
}

impl<T: RayTracer> PathBaker<T> {
    /// Creates a new [`PathBaker`].
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }

    /// Bakes a single layer of pathing data in a probe batch.
    ///
    /// Only one bake can be in progress at any point in time.
    pub fn bake(
        &self,
        context: &Context,
        probe_batch: &mut ProbeBatch,
        scene: &Scene<T>,
        params: PathBakeParams,
    ) {
        self.bake_with_optional_progress_callback(context, probe_batch, scene, params, None);
    }

    /// Bakes a single layer of pathing data in a probe batch, with a progress callback.
    ///
    /// Only one bake can be in progress at any point in time.
    pub fn bake_with_progress_callback(
        &self,
        context: &Context,
        probe_batch: &mut ProbeBatch,
        scene: &Scene<T>,
        params: PathBakeParams,
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

    /// Bakes a single layer of pathing data in a probe batch, with an optional progress callback.
    ///
    /// Only one bake can be in progress at any point in time.
    fn bake_with_optional_progress_callback(
        &self,
        context: &Context,
        probe_batch: &mut ProbeBatch,
        scene: &Scene<T>,
        params: PathBakeParams,
        progress_callback: Option<CallbackInformation<ProgressCallback>>,
    ) {
        // WORKAROUND: Steam Audio 4.8.0 segfaults when passing `NULL` callback to `iplPathBakerBake`.
        // We pass a no-op callback instead until the fix is released.
        // See: https://github.com/ValveSoftware/steam-audio/issues/523
        // TODO: Remove this workaround when fix is released.
        unsafe extern "C" fn noop_progress_callback(
            _progress: f32,
            _user_data: *mut std::ffi::c_void,
        ) {
        }

        let (callback, user_data) = if let Some(callback_information) = progress_callback {
            (
                callback_information.callback,
                callback_information.user_data,
            )
        } else {
            (
                noop_progress_callback as ProgressCallback,
                std::ptr::null_mut(),
            )
        };

        let mut ffi_params = audionimbus_sys::IPLPathBakeParams {
            scene: scene.raw_ptr(),
            probeBatch: probe_batch.raw_ptr(),
            identifier: params.identifier.into(),
            numSamples: params.num_samples as i32,
            radius: params.radius,
            threshold: params.threshold,
            visRange: params.visibility_range,
            pathRange: params.path_range,
            numThreads: params.num_threads as i32,
        };

        unsafe {
            audionimbus_sys::iplPathBakerBake(
                context.raw_ptr(),
                &mut ffi_params,
                Some(callback),
                user_data,
            );
        }
    }

    /// Cancels any running bakes of pathing data.
    pub fn cancel_bake(&self, context: &Context) {
        unsafe { audionimbus_sys::iplPathBakerCancelBake(context.raw_ptr()) }
    }
}

/// Parameters used to control how pathing data is baked.
#[derive(Debug)]
pub struct PathBakeParams {
    /// An identifier for the data layer that should be baked.
    /// The identifier determines what data is simulated and stored at each probe.
    /// If the probe batch already contains data with this identifier, it will be overwritten.
    pub identifier: BakedDataIdentifier,

    /// Number of point samples to use around each probe when testing whether one probe can see another.
    /// To determine if two probes are mutually visible, numSamples * numSamples rays are traced, from each point sample of the first probe, to every other point sample of the second probe.
    pub num_samples: u32,

    /// When testing for mutual visibility between a pair of probes, each probe is treated as a sphere of this radius (in meters), and point samples are generated within this sphere.
    pub radius: f32,

    /// When tracing rays to test for mutual visibility between a pair of probes, the fraction of rays that are unoccluded must be greater than this threshold for the pair of probes to be considered mutually visible.
    pub threshold: f32,

    /// If the distance between two probes is greater than this value, the probes are not considered mutually visible.
    /// Increasing this value can result in simpler paths, at the cost of increased bake times.
    pub visibility_range: f32,

    /// If the length of the path between two probes is greater than this value, the probes are considered to not have any path between them.
    /// Increasing this value allows sound to propagate over greater distances, at the cost of increased bake times and memory usage.
    pub path_range: f32,

    /// Number of threads to use for baking.
    pub num_threads: u32,
}
