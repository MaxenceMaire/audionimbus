//! Reflections backing.

use crate::baking::BakeError;
use crate::baking::BakedDataIdentifier;
use crate::baking::bake_lock::BAKE_LOCK;
#[cfg(feature = "dynamic")]
use crate::binding::Dynamic;
use crate::binding::Static;
use crate::callback::ProgressCallback;
use crate::context::Context;
use crate::device::open_cl::OpenClDevice;
use crate::device::radeon_rays::RadeonRaysDevice;
use crate::geometry::Scene;
#[cfg(feature = "dynamic")]
use crate::geometry::scene::RayTracerKind;
use crate::geometry::scene::{GeometryBinding, Scene as BoundScene};
use crate::probe::ProbeBatch;
use crate::ray_tracing::{CustomRayTracer, DefaultRayTracer, Embree, RadeonRays, RayTracer};
#[cfg(feature = "dynamic")]
use std::fmt;

#[cfg(doc)]
use crate::baking::BakedDataVariation;

/// A baker of reflections.
///
/// Simulating reflections in real-time is a very compute-intensive process.
/// [`crate::ReflectionsBaker`] lets you bake, or precompute reflections throughout a scene (or part of a scene) beforehand.
///
/// Reflections are baked at several points that you specify.
/// Each of these points is called a probe and belong to a [`ProbeBatch`].
///
/// Just like game engines use light probes to store the variation of lighting throughout a scene, Steam Audio uses acoustic probes to store the variation of acoustic data (in this case, reflections) throughout a scene.
pub type StaticReflectionsBaker<'a, T> = ReflectionsBaker<'a, Static<T>>;

/// A reflection baker borrowing runtime backend settings.
///
/// ```
/// use audionimbus::dynamic::{RayTracerSettings, ReflectionsBaker};
/// let ray_tracer_settings = RayTracerSettings::Default;
/// let reflections_baker = ReflectionsBaker::new(&ray_tracer_settings);
/// ```
#[cfg(feature = "dynamic")]
pub type DynamicReflectionsBaker<'a> = ReflectionsBaker<'a, Dynamic<RayTracerKind>>;

/// Storage shared by static and runtime reflection bakers.
#[derive(Default)]
pub struct ReflectionsBaker<'a, B: GeometryBinding> {
    /// The custom ray batch size used by native baking.
    ray_batch_size: i32,
    /// The borrowed OpenCL device.
    open_cl_device: Option<&'a OpenClDevice>,
    /// The borrowed Radeon Rays device.
    radeon_rays_device: Option<&'a RadeonRaysDevice>,
    /// The geometry binding selected for this baker.
    binding: B,
}

impl StaticReflectionsBaker<'_, DefaultRayTracer> {
    /// Creates a new [`crate::ReflectionsBaker`].
    pub const fn new() -> Self {
        Self {
            ray_batch_size: 0,
            open_cl_device: None,
            radeon_rays_device: None,
            binding: Static::new(),
        }
    }
}

impl StaticReflectionsBaker<'_, Embree> {
    /// Creates a new [`crate::ReflectionsBaker`].
    pub const fn new() -> Self {
        Self {
            ray_batch_size: 0,
            open_cl_device: None,
            radeon_rays_device: None,
            binding: Static::new(),
        }
    }
}

impl<'a> StaticReflectionsBaker<'a, RadeonRays> {
    /// Creates a new [`crate::ReflectionsBaker`].
    pub const fn new(
        open_cl_device: &'a OpenClDevice,
        radeon_rays_device: &'a RadeonRaysDevice,
    ) -> Self {
        Self {
            ray_batch_size: 0,
            open_cl_device: Some(open_cl_device),
            radeon_rays_device: Some(radeon_rays_device),
            binding: Static::new(),
        }
    }
}

impl StaticReflectionsBaker<'_, CustomRayTracer> {
    /// Creates a new [`crate::ReflectionsBaker`].
    pub const fn new(ray_batch_size: u32) -> Self {
        Self {
            ray_batch_size: ray_batch_size as i32,
            open_cl_device: None,
            radeon_rays_device: None,
            binding: Static::new(),
        }
    }
}

impl<T: RayTracer> StaticReflectionsBaker<'_, T> {
    /// Bakes a single layer of reflections data in a probe batch.
    ///
    /// Only one bake can be in progress at any point in time.
    ///
    /// # Errors
    ///
    /// Returns [`BakeError`] if another bake operation is already in progress.
    pub fn bake(
        &self,
        context: &Context,
        probe_batch: &mut ProbeBatch,
        scene: &Scene<T>,
        params: ReflectionsBakeParams,
    ) -> Result<(), BakeError> {
        self.bake_shared(context, probe_batch, scene, params, None)
    }

    /// Bakes a single layer of reflections data in a probe batch, with a progress callback.
    ///
    /// Only one bake can be in progress at any point in time.
    ///
    /// # Errors
    ///
    /// Returns [`BakeError`] if another bake operation is already in progress.
    pub fn bake_with_progress_callback(
        &self,
        context: &Context,
        probe_batch: &mut ProbeBatch,
        scene: &Scene<T>,
        params: ReflectionsBakeParams,
        progress_callback: ProgressCallback,
    ) -> Result<(), BakeError> {
        self.bake_shared(context, probe_batch, scene, params, Some(progress_callback))
    }
}

impl<B: GeometryBinding> ReflectionsBaker<'_, B> {
    /// Bakes a single layer of reflections data in a probe batch, with an optional progress callback.
    ///
    /// Only one bake can be in progress at any point in time.
    ///
    /// # Errors
    ///
    /// Returns [`BakeError`] if another bake operation is already in progress.
    fn bake_shared(
        &self,
        context: &Context,
        probe_batch: &mut ProbeBatch,
        scene: &BoundScene<B>,
        params: ReflectionsBakeParams,
        progress_callback: Option<ProgressCallback>,
    ) -> Result<(), BakeError> {
        let _guard = BAKE_LOCK
            .try_lock()
            .map_err(|_| BakeError::BakeInProgress)?;

        let (callback, user_data) = progress_callback.as_ref().map_or(
            (None, std::ptr::null_mut()),
            |callback_information| {
                let (callback, user_data) = callback_information.as_raw_parts();
                (Some(callback), user_data)
            },
        );

        let mut ffi_params = audionimbus_sys::IPLReflectionsBakeParams {
            scene: scene.raw_ptr_shared(),
            probeBatch: probe_batch.raw_ptr(),
            sceneType: self.binding.scene_type(),
            identifier: params.identifier.into(),
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
                &raw mut ffi_params,
                callback,
                user_data,
            );
        }

        Ok(())
    }

    /// Cancels any running bakes of reflections data.
    pub fn cancel_bake(&self, context: &Context) {
        unsafe { audionimbus_sys::iplReflectionsBakerCancelBake(context.raw_ptr()) }
    }
}

#[cfg(feature = "dynamic")]
impl<'a> DynamicReflectionsBaker<'a> {
    /// Creates a baker using the supplied backend settings.
    pub fn new(ray_tracer_settings: &'a crate::dynamic::RayTracerSettings) -> Self {
        use crate::dynamic::RayTracerSettings;

        let (ray_batch_size, open_cl_device, radeon_rays_device) = match ray_tracer_settings {
            RayTracerSettings::Default | RayTracerSettings::Embree { .. } => (0, None, None),
            RayTracerSettings::RadeonRays {
                open_cl_device,
                radeon_rays_device,
            } => (0, Some(open_cl_device), Some(radeon_rays_device)),
            RayTracerSettings::Custom { ray_batch_size, .. } => {
                (*ray_batch_size as i32, None, None)
            }
        };

        Self {
            ray_batch_size,
            open_cl_device,
            radeon_rays_device,
            binding: crate::binding::Dynamic(ray_tracer_settings.kind()),
        }
    }

    /// Bakes a reflection layer into the probe batch.
    ///
    /// # Errors
    ///
    /// - [`crate::dynamic::BakeError::RayTracerMismatch`] if the scene uses another backend.
    /// - [`crate::dynamic::BakeError::DeviceMismatch`] if Radeon Rays devices differ.
    /// - [`crate::dynamic::BakeError::Bake`] if another bake is in progress.
    pub fn bake(
        &self,
        context: &Context,
        probe_batch: &mut ProbeBatch,
        scene: &crate::dynamic::Scene,
        params: ReflectionsBakeParams,
    ) -> Result<(), crate::dynamic::BakeError> {
        self.bake_with_optional_progress_callback(context, probe_batch, scene, params, None)
    }

    /// Bakes a reflection layer with progress notifications.
    ///
    /// # Errors
    ///
    /// - [`crate::dynamic::BakeError::RayTracerMismatch`] if the scene uses another backend.
    /// - [`crate::dynamic::BakeError::DeviceMismatch`] if Radeon Rays devices differ.
    /// - [`crate::dynamic::BakeError::Bake`] if another bake is in progress.
    pub fn bake_with_progress_callback(
        &self,
        context: &Context,
        probe_batch: &mut ProbeBatch,
        scene: &crate::dynamic::Scene,
        params: ReflectionsBakeParams,
        progress_callback: ProgressCallback,
    ) -> Result<(), crate::dynamic::BakeError> {
        self.bake_with_optional_progress_callback(
            context,
            probe_batch,
            scene,
            params,
            Some(progress_callback),
        )
    }

    /// Validates the scene before baking a reflection layer.
    ///
    /// # Errors
    ///
    /// - [`crate::dynamic::BakeError::RayTracerMismatch`] if the scene uses another backend.
    /// - [`crate::dynamic::BakeError::DeviceMismatch`] if Radeon Rays devices differ.
    /// - [`crate::dynamic::BakeError::Bake`] if another bake is in progress.
    fn bake_with_optional_progress_callback(
        &self,
        context: &Context,
        probe_batch: &mut ProbeBatch,
        scene: &crate::dynamic::Scene,
        params: ReflectionsBakeParams,
        progress_callback: Option<ProgressCallback>,
    ) -> Result<(), crate::dynamic::BakeError> {
        let expected = self.binding.0;
        let actual = scene.ray_tracer();
        if expected != actual {
            return Err(crate::dynamic::BakeError::RayTracerMismatch { expected, actual });
        }

        if !scene
            .shared
            .lock()
            .unwrap()
            .uses_devices(self.open_cl_device, self.radeon_rays_device)
        {
            return Err(crate::dynamic::BakeError::DeviceMismatch);
        }

        self.bake_shared(context, probe_batch, scene, params, progress_callback)
            .map_err(crate::dynamic::BakeError::Bake)
    }
}

/// Parameters used to control how reflections data is baked.
#[derive(Debug, Copy, Clone)]
pub struct ReflectionsBakeParams {
    /// An identifier for the data layer that should be baked.
    /// The identifier determines what data is simulated and stored at each probe.
    /// If the probe batch already contains data with this identifier, it will be overwritten.
    pub identifier: BakedDataIdentifier,

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

#[cfg(test)]
pub mod tests {
    use crate::*;

    /// Creates the scene used by the bake cases.
    fn test_scene(context: &Context) -> Scene<DefaultRayTracer> {
        let mut scene = Scene::try_new(context).unwrap();

        // Create a simple room mesh.
        let vertices = vec![
            // Floor
            Vector3::new(-5.0, 0.0, -5.0),
            Vector3::new(5.0, 0.0, -5.0),
            Vector3::new(5.0, 0.0, 5.0),
            Vector3::new(-5.0, 0.0, 5.0),
            // Ceiling
            Vector3::new(-5.0, 3.0, -5.0),
            Vector3::new(5.0, 3.0, -5.0),
            Vector3::new(5.0, 3.0, 5.0),
            Vector3::new(-5.0, 3.0, 5.0),
        ];

        let triangles = [
            // Floor
            [0, 1, 2],
            [0, 2, 3],
            // Ceiling
            [4, 6, 5],
            [4, 7, 6],
            // Walls
            [0, 4, 5],
            [0, 5, 1],
            [1, 5, 6],
            [1, 6, 2],
            [2, 6, 7],
            [2, 7, 3],
            [3, 7, 4],
            [3, 4, 0],
        ]
        .iter()
        .map(|indices| Triangle::new(indices[0], indices[1], indices[2]))
        .collect::<Vec<_>>();

        let material_indices = vec![0; triangles.len()];
        let materials = vec![Material::default()];

        let settings = StaticMeshSettings {
            vertices: &vertices,
            triangles: &triangles,
            material_indices: &material_indices,
            materials: &materials,
        };
        let static_mesh = StaticMesh::try_new(&scene, &settings).unwrap();
        scene.add_static_mesh(static_mesh);
        scene.commit();

        scene
    }

    /// Creates the probe batch used by the bake cases.
    fn test_probe_batch(context: &Context, scene: &Scene) -> ProbeBatch {
        let mut probe_batch = ProbeBatch::try_new(context).unwrap();

        let params = ProbeGenerationParams::Centroid {
            transform: Matrix4::new([
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ]),
        };
        let mut probe_array = ProbeArray::try_new(context).unwrap();
        probe_array.generate_probes(scene, &params);

        probe_batch.add_probe_array(&probe_array);
        probe_batch.commit();

        probe_batch
    }

    // This test runs at the module level to avoid concurrent execution
    // with other bake tests, which would cause BakeError::BakeInProgress.
    /// Exercises native baking with and without progress callbacks.
    pub fn test_bake() {
        // Run test cases sequentially to avoid BakeError::BakeInProgress.

        // Convolution
        {
            let context = Context::default();
            let scene = test_scene(&context);
            let mut probe_batch = test_probe_batch(&context, &scene);

            let baker = ReflectionsBaker::<DefaultRayTracer>::new();

            let params = ReflectionsBakeParams {
                identifier: BakedDataIdentifier::Reflections {
                    variation: BakedDataVariation::Reverb,
                },
                bake_flags: ReflectionsBakeFlags::BAKE_CONVOLUTION,
                num_rays: 1024,
                num_diffuse_samples: 32,
                num_bounces: 8,
                simulated_duration: 2.0,
                saved_duration: 2.0,
                order: 1,
                num_threads: 2,
                irradiance_min_distance: 1.0,
                bake_batch_size: 8,
            };

            assert!(
                baker
                    .bake(&context, &mut probe_batch, &scene, params)
                    .is_ok()
            );
        }

        // Parametric
        {
            let context = Context::default();
            let scene = test_scene(&context);
            let mut probe_batch = test_probe_batch(&context, &scene);

            let baker = ReflectionsBaker::<DefaultRayTracer>::new();

            let params = ReflectionsBakeParams {
                identifier: BakedDataIdentifier::Reflections {
                    variation: BakedDataVariation::Reverb,
                },
                bake_flags: ReflectionsBakeFlags::BAKE_PARAMETRIC,
                num_rays: 512,
                num_diffuse_samples: 16,
                num_bounces: 4,
                simulated_duration: 1.0,
                saved_duration: 1.0,
                order: 1,
                num_threads: 1,
                irradiance_min_distance: 0.5,
                bake_batch_size: 4,
            };

            assert!(
                baker
                    .bake(&context, &mut probe_batch, &scene, params)
                    .is_ok()
            );
        }

        // Both flags
        {
            let context = Context::default();
            let scene = test_scene(&context);
            let mut probe_batch = test_probe_batch(&context, &scene);

            let baker = ReflectionsBaker::<DefaultRayTracer>::new();

            let params = ReflectionsBakeParams {
                identifier: BakedDataIdentifier::Reflections {
                    variation: BakedDataVariation::Reverb,
                },
                bake_flags: ReflectionsBakeFlags::BAKE_CONVOLUTION
                    | ReflectionsBakeFlags::BAKE_PARAMETRIC,
                num_rays: 512,
                num_diffuse_samples: 16,
                num_bounces: 4,
                simulated_duration: 1.0,
                saved_duration: 1.0,
                order: 1,
                num_threads: 1,
                irradiance_min_distance: 0.5,
                bake_batch_size: 4,
            };

            assert!(
                baker
                    .bake(&context, &mut probe_batch, &scene, params)
                    .is_ok()
            );
        }

        // Static source
        {
            let context = Context::default();
            let scene = test_scene(&context);
            let mut probe_batch = test_probe_batch(&context, &scene);

            let baker = ReflectionsBaker::<DefaultRayTracer>::new();

            let params = ReflectionsBakeParams {
                identifier: BakedDataIdentifier::Reflections {
                    variation: BakedDataVariation::StaticSource {
                        endpoint_influence: Sphere {
                            center: Vector3::new(0.0, 1.5, 0.0),
                            radius: 1.0,
                        },
                    },
                },
                bake_flags: ReflectionsBakeFlags::BAKE_CONVOLUTION,
                num_rays: 512,
                num_diffuse_samples: 16,
                num_bounces: 4,
                simulated_duration: 1.0,
                saved_duration: 1.0,
                order: 1,
                num_threads: 1,
                irradiance_min_distance: 0.5,
                bake_batch_size: 4,
            };

            assert!(
                baker
                    .bake(&context, &mut probe_batch, &scene, params)
                    .is_ok()
            );
        }

        // With progress callback
        {
            let context = Context::default();
            let scene = test_scene(&context);
            let mut probe_batch = test_probe_batch(&context, &scene);

            let baker = ReflectionsBaker::<DefaultRayTracer>::new();

            let params = ReflectionsBakeParams {
                identifier: BakedDataIdentifier::Reflections {
                    variation: BakedDataVariation::Reverb,
                },
                bake_flags: ReflectionsBakeFlags::BAKE_CONVOLUTION,
                num_rays: 512,
                num_diffuse_samples: 16,
                num_bounces: 4,
                simulated_duration: 1.0,
                saved_duration: 1.0,
                order: 1,
                num_threads: 1,
                irradiance_min_distance: 0.5,
                bake_batch_size: 4,
            };

            assert!(
                baker
                    .bake_with_progress_callback(
                        &context,
                        &mut probe_batch,
                        &scene,
                        params,
                        ProgressCallback::new(|progress| {
                            println!("baking progress: {:.1}%", progress * 100.0);
                        }),
                    )
                    .is_ok()
            );
        }
    }
}

/// Errors from runtime reflection baking.
#[cfg(feature = "dynamic")]
#[derive(Debug, Eq, PartialEq)]
pub enum DynamicBakeError {
    /// Another native bake is already in progress.
    Bake(crate::BakeError),
    /// The scene and reflection baker use different ray tracers.
    RayTracerMismatch {
        /// The baker's ray tracer.
        expected: RayTracerKind,
        /// The scene's ray tracer.
        actual: RayTracerKind,
    },
    /// The scene and reflection baker use different devices.
    DeviceMismatch,
}

#[cfg(feature = "dynamic")]
impl fmt::Display for DynamicBakeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bake(error) => error.fmt(formatter),
            Self::RayTracerMismatch { expected, actual } => write!(
                formatter,
                "bake ray tracer mismatch: expected {expected:?}, got {actual:?}"
            ),
            Self::DeviceMismatch => write!(formatter, "bake devices do not match"),
        }
    }
}

#[cfg(feature = "dynamic")]
impl std::error::Error for DynamicBakeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Bake(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(all(doctest, feature = "dynamic"))]
/// Baker binding checks.
///
/// ```compile_fail
/// use audionimbus::{Context, ProbeBatch, ReflectionsBaker, ReflectionsBakeParams, ray_tracing::DefaultRayTracer};
/// fn bake(context: &Context, probes: &mut ProbeBatch, scene: &audionimbus::dynamic::Scene, params: ReflectionsBakeParams) {
///     ReflectionsBaker::<DefaultRayTracer>::new().bake(context, probes, scene, params);
/// }
/// ```
///
/// ```compile_fail
/// use audionimbus::{Context, ProbeBatch, ReflectionsBakeParams, Scene};
/// fn bake(baker: &audionimbus::dynamic::ReflectionsBaker<'_>, context: &Context, probes: &mut ProbeBatch, scene: &Scene, params: ReflectionsBakeParams) {
///     baker.bake(context, probes, scene, params);
/// }
/// ```
///
/// ```compile_fail
/// use audionimbus::{Context, ProbeBatch, PathBaker, PathBakeParams};
/// fn bake(context: &Context, probes: &mut ProbeBatch, scene: &audionimbus::dynamic::Scene, params: PathBakeParams) {
///     PathBaker::new().bake(context, probes, scene, params);
/// }
/// ```
///
/// ```compile_fail
/// use audionimbus::{Context, ProbeBatch, PathBakeParams, Scene};
/// fn bake(context: &Context, probes: &mut ProbeBatch, scene: &Scene, params: PathBakeParams) {
///     audionimbus::dynamic::PathBaker::new().bake(context, probes, scene, params);
/// }
/// ```
///
/// ```compile_fail
/// use audionimbus::{ReflectionsBaker, dynamic::RayTracerKind};
/// let _: Option<ReflectionsBaker<'static, RayTracerKind>> = None;
/// ```
///
/// ```compile_fail
/// use audionimbus::{PathBaker, dynamic::RayTracerKind};
/// let _: Option<PathBaker<RayTracerKind>> = None;
/// ```
struct BakingBindings;
