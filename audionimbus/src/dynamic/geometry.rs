//! Runtime-selected geometry.

use crate::callback::CustomRayTracingCallbacks;
use crate::device::{EmbreeDevice, OpenClDevice, RadeonRaysDevice};
use crate::{StaticMeshError, SteamAudioError};
use std::fmt;

/// A ray tracer selected at runtime.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RayTracerKind {
    /// Steam Audio's built-in ray tracer.
    Default,

    /// The Intel Embree ray tracer.
    Embree,

    /// The AMD Radeon Rays ray tracer.
    RadeonRays,

    /// Application-provided ray tracing callbacks.
    Custom,
}

/// Settings for a runtime-selected ray tracer.
#[derive(Clone)]
pub enum RayTracerSettings {
    /// Steam Audio's built-in ray tracer.
    Default,

    /// The Intel Embree ray tracer and its device.
    Embree {
        /// The Embree device.
        device: EmbreeDevice,
    },

    /// The AMD Radeon Rays ray tracer and its devices.
    RadeonRays {
        /// The OpenCL device used by Radeon Rays.
        open_cl_device: OpenClDevice,

        /// The Radeon Rays device.
        radeon_rays_device: RadeonRaysDevice,
    },

    /// Application-provided ray tracing callbacks.
    Custom {
        /// The ray tracing callbacks.
        callbacks: CustomRayTracingCallbacks,

        /// The ray batch size for simulator construction.
        ray_batch_size: usize,
    },
}

impl RayTracerSettings {
    /// Returns the selected ray tracer.
    pub const fn kind(&self) -> RayTracerKind {
        match self {
            Self::Default => RayTracerKind::Default,
            Self::Embree { .. } => RayTracerKind::Embree,
            Self::RadeonRays { .. } => RayTracerKind::RadeonRays,
            Self::Custom { .. } => RayTracerKind::Custom,
        }
    }
}

impl fmt::Debug for RayTracerSettings {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RayTracerSettings")
            .field("kind", &self.kind())
            .finish_non_exhaustive()
    }
}

/// A scene with a runtime-selected ray tracer.
///
/// Static and runtime geometry handles cannot be mixed.
///
/// # Examples
///
/// ```no_run
/// use audionimbus::Context;
/// use audionimbus::dynamic::{RayTracerSettings, Scene};
///
/// let context = Context::default();
/// let scene = Scene::try_new(&context, RayTracerSettings::Default)?;
/// scene.commit();
/// let _: Scene = scene;
/// # Ok::<(), audionimbus::dynamic::SceneError>(())
/// ```
pub type Scene = crate::geometry::scene::Scene<crate::binding::Dynamic<RayTracerKind>>;

/// A static mesh with a runtime-selected ray tracer.
///
/// Creates meshes with `try_new`, or loads them with `load` and `load_with_progress_callback`.
/// Serializes default-backend meshes with `try_save`.
/// Clones retain the same native mesh.
pub type StaticMesh =
    crate::geometry::static_mesh::StaticMesh<crate::binding::Dynamic<RayTracerKind>>;

/// An instanced mesh with a runtime-selected ray tracer.
///
/// Creates instances with `try_new`, which rejects different parent and sub-scene ray tracers.
/// Clones retain the native instance and its sub-scene.
pub type InstancedMesh =
    crate::geometry::instanced_mesh::InstancedMesh<crate::binding::Dynamic<RayTracerKind>>;

/// Instanced mesh settings with a runtime-selected ray tracer.
///
/// The public `sub_scene` field contains a runtime [`Scene`].
/// The public `transform` field contains its local-to-world matrix.
pub type InstancedMeshSettings =
    crate::geometry::instanced_mesh::InstancedMeshSettings<crate::binding::Dynamic<RayTracerKind>>;

/// Errors produced by runtime-selected scene operations.
#[cfg(feature = "dynamic")]
#[derive(Debug, Eq, PartialEq)]
pub enum SceneError {
    /// Native scene operation failed.
    SteamAudio(SteamAudioError),

    /// Static mesh creation failed.
    StaticMesh(StaticMeshError),

    /// Two geometry handles use different ray tracers.
    RayTracerMismatch {
        /// The ray tracer required by the receiving handle.
        expected: RayTracerKind,

        /// The ray tracer used by the supplied handle.
        actual: RayTracerKind,
    },

    /// The selected ray tracer does not support serialization.
    SerializationUnsupported(RayTracerKind),

    /// The selected ray tracer does not support OBJ export.
    ObjUnsupported(RayTracerKind),
}

#[cfg(feature = "dynamic")]
impl std::error::Error for SceneError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SteamAudio(error) => Some(error),
            Self::StaticMesh(error) => Some(error),
            Self::RayTracerMismatch { .. }
            | Self::SerializationUnsupported(_)
            | Self::ObjUnsupported(_) => None,
        }
    }
}

#[cfg(feature = "dynamic")]
impl std::fmt::Display for SceneError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SteamAudio(error) => std::fmt::Display::fmt(error, formatter),
            Self::StaticMesh(error) => std::fmt::Display::fmt(error, formatter),
            Self::RayTracerMismatch { expected, actual } => write!(
                formatter,
                "scene uses {expected:?} ray tracing but geometry uses {actual:?}"
            ),
            Self::SerializationUnsupported(ray_tracer) => {
                write!(formatter, "{ray_tracer:?} ray tracing cannot be serialized")
            }
            Self::ObjUnsupported(ray_tracer) => {
                write!(
                    formatter,
                    "{ray_tracer:?} ray tracing cannot be saved as OBJ"
                )
            }
        }
    }
}

#[cfg(feature = "dynamic")]
impl From<SteamAudioError> for SceneError {
    fn from(error: SteamAudioError) -> Self {
        Self::SteamAudio(error)
    }
}

#[cfg(feature = "dynamic")]
impl From<StaticMeshError> for SceneError {
    fn from(error: StaticMeshError) -> Self {
        match error {
            StaticMeshError::SteamAudio(error) => Self::SteamAudio(error),
            error => Self::StaticMesh(error),
        }
    }
}

#[cfg(doctest)]
/// Geometry binding checks.
///
/// Static constructor inference remains unchanged:
///
/// ```no_run
/// use audionimbus::{Context, Scene};
///
/// let context = Context::default();
/// let scene = Scene::try_new(&context)?;
/// scene.commit();
/// let _: Scene = scene;
/// # Ok::<(), audionimbus::SteamAudioError>(())
/// ```
///
/// Static and runtime handles cannot be mixed:
///
/// ```compile_fail
/// use audionimbus::{Context, Material, Point, StaticMeshSettings, Triangle};
/// use audionimbus::dynamic::{RayTracerSettings, Scene, StaticMesh};
///
/// let context = Context::default();
/// let dynamic_scene = Scene::try_new(&context, RayTracerSettings::Default)?;
/// let mesh = StaticMesh::try_new(&dynamic_scene, &StaticMeshSettings {
///     vertices: &[Point::new(0.0, 0.0, 0.0)],
///     triangles: &[Triangle::new(0, 0, 0)],
///     material_indices: &[0],
///     materials: &[Material::CONCRETE],
/// })?;
/// let mut static_scene = audionimbus::Scene::try_new(&context)?;
/// static_scene.add_static_mesh(mesh);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// Runtime choices cannot be used as static marker types:
///
/// ```compile_fail
/// use audionimbus::Scene;
/// use audionimbus::dynamic::RayTracerKind;
///
/// let _: Option<Scene<RayTracerKind>> = None;
/// ```
///
/// Static insertion keeps its existing generic bound:
///
/// ```
/// use audionimbus::{Scene, StaticMesh, RayTracer};
/// fn insert<T: RayTracer>(scene: &mut Scene<T>, mesh: StaticMesh<T>) {
///     scene.add_static_mesh(mesh);
/// }
/// ```
///
/// Runtime insertion accepts runtime meshes:
///
/// ```
/// use audionimbus::dynamic::{Scene, StaticMesh, SceneError};
/// fn insert(scene: &mut Scene, mesh: StaticMesh) -> Result<(), SceneError> {
///     scene.try_add_static_mesh(mesh)?;
///     Ok(())
/// }
/// ```
///
/// Static meshes cannot enter runtime scenes:
///
/// ```compile_fail
/// use audionimbus::{StaticMesh, DefaultRayTracer};
/// use audionimbus::dynamic::Scene;
/// fn insert(scene: &mut Scene, mesh: StaticMesh<DefaultRayTracer>) {
///     scene.try_add_static_mesh(mesh);
/// }
/// ```
///
/// Runtime scenes cannot become static instance settings:
///
/// ```compile_fail
/// use audionimbus::{InstancedMeshSettings, Matrix4};
/// use audionimbus::dynamic::Scene;
/// fn settings(sub_scene: Scene) -> InstancedMeshSettings {
///     InstancedMeshSettings { sub_scene, transform: Matrix4::IDENTITY }
/// }
/// ```
///
/// Static scenes cannot become runtime instance settings:
///
/// ```compile_fail
/// use audionimbus::{Scene, Matrix4};
/// use audionimbus::dynamic::InstancedMeshSettings;
/// fn settings(sub_scene: Scene) -> InstancedMeshSettings {
///     InstancedMeshSettings { sub_scene, transform: Matrix4::IDENTITY }
/// }
/// ```
///
/// Static mesh marker ownership remains visible to auto traits:
///
/// ```compile_fail
/// fn unpin<T: Unpin>() {}
/// unpin::<audionimbus::StaticMesh<std::marker::PhantomPinned>>();
/// ```
///
/// Runtime bindings cannot escape through the shared implementation:
///
/// ```compile_fail
/// fn inspect(scene: &audionimbus::dynamic::Scene) { scene.binding(); }
/// ```
///
/// Runtime handles expose no raw-pointer mutation:
///
/// ```compile_fail
/// fn mutate(scene: &mut audionimbus::dynamic::Scene) { scene.raw_ptr_mut(); }
/// ```
///
/// Static mesh hashing still requires a static ray tracer:
///
/// ```compile_fail
/// fn hash<T: std::hash::Hash>() {}
/// hash::<audionimbus::StaticMesh<String>>();
/// ```
///
/// Static mesh equality still requires marker equality:
///
/// ```compile_fail
/// fn equal<T: Eq>() {}
/// equal::<audionimbus::StaticMesh<audionimbus::DefaultRayTracer>>();
/// ```
struct GeometryBindings;
