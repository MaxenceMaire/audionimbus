//! Path baking.

use crate::baking::BakedDataIdentifier;
use crate::baking::bake_lock::BAKE_LOCK;
use crate::baking::error::BakeError;
#[cfg(feature = "dynamic")]
use crate::binding::Dynamic;
use crate::binding::Static;
use crate::callback::ProgressCallback;
use crate::context::Context;
#[cfg(feature = "dynamic")]
use crate::geometry::scene::RayTracerKind;
use crate::geometry::scene::StaticScene as Scene;
use crate::geometry::scene::{GeometryBinding, Scene as BoundScene};
use crate::probe::ProbeBatch;
use crate::ray_tracing::RayTracer;
use std::marker::PhantomData;

#[cfg(doc)]
use crate::baking::BakedDataVariation;

/// A baker of pathing data.
///
/// Pathing is an alternative simulation method to reflections that finds the shortest
/// unoccluded paths from sources to listeners by traveling between probes.
///
/// [`crate::baking::pathing::PathBaker`] lets you bake, or precompute pathing data throughout a scene (or part
/// of a scene) beforehand. This precomputation is typically done offline since pathing
/// requires probe generation.
///
/// Pathing data is baked at several points that you specify. Each of these points is
/// called a probe and belongs to a [`ProbeBatch`].
///
/// Just like game engines use light probes to store the variation of lighting throughout
/// a scene, Steam Audio uses acoustic probes to store the variation of acoustic data
/// (in this case, pathing information) throughout a scene.
pub type StaticPathBaker<T> = PathBaker<Static<T>>;

/// A path baker for runtime scenes.
#[cfg(feature = "dynamic")]
pub type DynamicPathBaker = PathBaker<Dynamic<RayTracerKind>>;

/// Storage shared by static and runtime path bakers.
pub struct PathBaker<B: GeometryBinding> {
    /// The scene binding accepted by this baker.
    _marker: PhantomData<B>,
}

impl<T: RayTracer> StaticPathBaker<T> {
    /// Creates a new [`crate::baking::pathing::PathBaker`].
    pub const fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }

    /// Bakes a single layer of pathing data in a probe batch.
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
        params: PathBakeParams,
    ) -> Result<(), BakeError> {
        self.bake_shared(context, probe_batch, scene, params, None)
    }

    /// Bakes a single layer of pathing data in a probe batch, with a progress callback.
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
        params: PathBakeParams,
        progress_callback: ProgressCallback,
    ) -> Result<(), BakeError> {
        self.bake_shared(context, probe_batch, scene, params, Some(progress_callback))
    }
}

impl<B: GeometryBinding> PathBaker<B> {
    /// Bakes a single layer of pathing data in a probe batch, with an optional progress callback.
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
        params: PathBakeParams,
        progress_callback: Option<ProgressCallback>,
    ) -> Result<(), BakeError> {
        // WORKAROUND: Steam Audio segfaults when passing `NULL` callback to `iplPathBakerBake`.
        // We pass a no-op callback instead until the fix is released.
        // See: https://github.com/ValveSoftware/steam-audio/issues/523
        // TODO: Remove this workaround when fix is released.
        /// Ignores native progress notifications.
        unsafe extern "C" fn noop(_: f32, _: *mut std::ffi::c_void) {}

        let _guard = BAKE_LOCK
            .try_lock()
            .map_err(|_| BakeError::BakeInProgress)?;

        let (callback, user_data) = progress_callback.as_ref().map_or(
            (noop as _, std::ptr::null_mut()),
            ProgressCallback::as_raw_parts,
        );

        let mut ffi_params = audionimbus_sys::IPLPathBakeParams {
            scene: scene.raw_ptr_shared(),
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
                &raw mut ffi_params,
                Some(callback),
                user_data,
            );
        }

        Ok(())
    }

    /// Cancels any running bakes of pathing data.
    pub fn cancel_bake(&self, context: &Context) {
        unsafe { audionimbus_sys::iplPathBakerCancelBake(context.raw_ptr()) }
    }
}

impl<T: RayTracer + Default> Default for StaticPathBaker<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "dynamic")]
impl DynamicPathBaker {
    /// Creates a path baker for runtime scenes.
    pub const fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }

    /// Bakes a pathing layer into the probe batch.
    ///
    /// # Errors
    ///
    /// - [`BakeError::BakeInProgress`] if another bake is in progress.
    pub fn bake(
        &self,
        context: &Context,
        probe_batch: &mut ProbeBatch,
        scene: &crate::dynamic::Scene,
        params: PathBakeParams,
    ) -> Result<(), BakeError> {
        self.bake_shared(context, probe_batch, scene, params, None)
    }

    /// Bakes a pathing layer with progress notifications.
    ///
    /// # Errors
    ///
    /// - [`BakeError::BakeInProgress`] if another bake is in progress.
    pub fn bake_with_progress_callback(
        &self,
        context: &Context,
        probe_batch: &mut ProbeBatch,
        scene: &crate::dynamic::Scene,
        params: PathBakeParams,
        progress_callback: ProgressCallback,
    ) -> Result<(), BakeError> {
        self.bake_shared(context, probe_batch, scene, params, Some(progress_callback))
    }
}

#[cfg(feature = "dynamic")]
impl Default for DynamicPathBaker {
    fn default() -> Self {
        Self::new()
    }
}

/// Parameters used to control how pathing data is baked.
#[derive(Debug, Copy, Clone)]
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

#[cfg(test)]
pub mod tests {
    use crate::baking::pathing::baker::StaticPathBaker as PathBaker;
    use crate::geometry::scene::StaticScene as Scene;
    use crate::geometry::static_mesh::StaticStaticMesh as StaticMesh;
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

        // Simple bake
        {
            let context = Context::default();
            let scene = test_scene(&context);
            let mut probe_batch = test_probe_batch(&context, &scene);

            let baker = PathBaker::<DefaultRayTracer>::new();

            let params = PathBakeParams {
                identifier: BakedDataIdentifier::Pathing {
                    variation: BakedDataVariation::Dynamic,
                },
                num_samples: 4,
                radius: 0.5,
                threshold: 0.3,
                visibility_range: 5.0,
                path_range: 10.0,
                num_threads: 1,
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

            let baker = PathBaker::<DefaultRayTracer>::new();

            let params = PathBakeParams {
                identifier: BakedDataIdentifier::Pathing {
                    variation: BakedDataVariation::Dynamic,
                },
                num_samples: 4,
                radius: 0.5,
                threshold: 0.3,
                visibility_range: 5.0,
                path_range: 10.0,
                num_threads: 1,
            };

            assert!(
                baker
                    .bake_with_progress_callback(
                        &context,
                        &mut probe_batch,
                        &scene,
                        params,
                        ProgressCallback::new(|progress| {
                            println!("pathing bake progress: {:.1}%", progress * 100.0);
                        }),
                    )
                    .is_ok()
            );
        }
    }
}
