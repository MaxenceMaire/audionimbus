use super::Matrix;
use super::scene::{GeometryBinding, Scene};
use crate::binding::Static;
use crate::error::{SteamAudioError, to_option_error};
use crate::ray_tracing::RayTracer;
use std::hash::{Hash, Hasher};

#[cfg(feature = "dynamic")]
use crate::dynamic::SceneError;

/// Geometry storage for a compile-time or runtime binding.
#[derive(Debug)]
pub struct InstancedMesh<F: GeometryBinding> {
    /// The native instanced mesh pointer.
    inner: audionimbus_sys::IPLInstancedMesh,
    /// The sub-scene whose replacement would break native instance ownership.
    pub(crate) sub_scene: Scene<F>,
}

impl<F: GeometryBinding> InstancedMesh<F> {
    /// Creates an instanced mesh without checking runtime binding settings.
    ///
    /// # Errors
    ///
    /// - [`SteamAudioError::Unspecified`] when the native operation fails.
    /// - [`SteamAudioError::OutOfMemory`] when native allocation fails.
    /// - [`SteamAudioError::Initialization`] when a native dependency fails.
    fn try_new_shared(
        scene: &Scene<F>,
        settings: &InstancedMeshSettings<F>,
    ) -> Result<Self, SteamAudioError> {
        let mut inner = std::ptr::null_mut();

        let mut instanced_mesh_settings_ffi = audionimbus_sys::IPLInstancedMeshSettings {
            subScene: settings.sub_scene.raw_ptr_shared(),
            transform: settings.transform.into(),
        };

        let status = unsafe {
            audionimbus_sys::iplInstancedMeshCreate(
                scene.raw_ptr_shared(),
                &raw mut instanced_mesh_settings_ffi,
                &raw mut inner,
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        let instanced_mesh = Self {
            inner,
            sub_scene: settings.sub_scene.clone(),
        };

        Ok(instanced_mesh)
    }

    /// Returns the raw instanced mesh pointer.
    /// Library callers must preserve the checked runtime geometry boundary.
    pub(crate) const fn raw_ptr_shared(&self) -> audionimbus_sys::IPLInstancedMesh {
        self.inner
    }
}

/// A triangle mesh that can be moved (translated), rotated, or scaled, but cannot deform.
///
/// Portions of a scene that undergo rigid-body motion can be represented as instanced meshes.
/// An instanced mesh is essentially a scene (called the “sub-scene”) with a transform applied to it.
/// Adding an instanced mesh to a scene places the sub-scene into the scene with the transform applied.
/// For example, the sub-scene may be a prefab door, and the transform can be used to place it in a doorway and animate it as it opens or closes.
///
/// `InstancedMesh` is a reference-counted handle to an underlying Steam Audio object.
/// Cloning it is cheap; it produces a new handle pointing to the same underlying object, while
/// incrementing a reference count.
/// The underlying object is destroyed when all handles are dropped.
pub type TypedInstancedMesh<T = crate::ray_tracing::DefaultRayTracer> = InstancedMesh<Static<T>>;

impl<T: RayTracer> TypedInstancedMesh<T> {
    /// Creates a new instanced mesh and returns a handle to it.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if creation fails.
    pub fn try_new(
        scene: &super::Scene<T>,
        settings: &TypedInstancedMeshSettings<T>,
    ) -> Result<Self, SteamAudioError> {
        Self::try_new_shared(scene, settings)
    }

    /// Returns the raw FFI pointer to the underlying instanced mesh.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub const fn raw_ptr(&self) -> audionimbus_sys::IPLInstancedMesh {
        self.inner
    }

    /// Returns a mutable reference to the raw FFI pointer.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub const fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLInstancedMesh {
        &mut self.inner
    }
}

#[cfg(feature = "dynamic")]
impl crate::dynamic::InstancedMesh {
    /// Creates a new instanced mesh and returns a handle to it.
    ///
    /// # Errors
    ///
    /// - [`SceneError::RayTracerMismatch`] when the parent and sub-scene use different ray tracers.
    /// - [`SceneError::SteamAudio`] when native creation fails.
    pub fn try_new(
        scene: &crate::dynamic::Scene,
        settings: &crate::dynamic::InstancedMeshSettings,
    ) -> Result<Self, SceneError> {
        let expected = scene.binding().0;
        let actual = settings.sub_scene.binding().0;
        if actual != expected {
            return Err(SceneError::RayTracerMismatch { expected, actual });
        }

        Self::try_new_shared(scene, settings).map_err(SceneError::from)
    }
}

impl<F: GeometryBinding> Drop for InstancedMesh<F> {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplInstancedMeshRelease(&raw mut self.inner) }
    }
}

unsafe impl<F: GeometryBinding> Send for InstancedMesh<F> {}
unsafe impl<F: GeometryBinding> Sync for InstancedMesh<F> {}

impl<F: GeometryBinding> Clone for InstancedMesh<F> {
    /// Retains an additional reference to the instanced mesh.
    ///
    /// The returned [`InstancedMesh`] shares the same underlying Steam Audio object.
    fn clone(&self) -> Self {
        // SAFETY: The instanced mesh will not be destroyed until all references are released.
        Self {
            inner: unsafe { audionimbus_sys::iplInstancedMeshRetain(self.inner) },
            sub_scene: self.sub_scene.clone(),
        }
    }
}

impl<F: GeometryBinding> PartialEq for InstancedMesh<F> {
    fn eq(&self, other: &Self) -> bool {
        self.raw_ptr_shared() == other.raw_ptr_shared()
    }
}

impl<F: GeometryBinding> Eq for InstancedMesh<F> {}

impl<F: GeometryBinding> Hash for InstancedMesh<F> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::ptr::hash(self.raw_ptr_shared(), state);
    }
}

/// Settings used to create an instanced mesh.
#[derive(Debug)]
pub struct InstancedMeshSettings<F: GeometryBinding> {
    /// Handle to the scene to be instantiated.
    pub sub_scene: Scene<F>,

    /// Local-to-world transform that places the instance within the parent scene.
    pub transform: Matrix<f32, 4, 4>,
}

/// Instanced mesh settings for a statically selected ray tracer.
pub type TypedInstancedMeshSettings<T = crate::ray_tracing::DefaultRayTracer> =
    InstancedMeshSettings<Static<T>>;

impl<T: RayTracer + Clone> Clone for TypedInstancedMeshSettings<T> {
    fn clone(&self) -> Self {
        Self {
            sub_scene: self.sub_scene.clone(),
            transform: self.transform,
        }
    }
}

#[cfg(feature = "dynamic")]
impl Clone for crate::dynamic::InstancedMeshSettings {
    fn clone(&self) -> Self {
        Self {
            sub_scene: self.sub_scene.clone(),
            transform: self.transform,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn test_instanced_mesh_clone() {
        let context = Context::default();
        let main_scene = Scene::try_new(&context).unwrap();
        let sub_scene = Scene::try_new(&context).unwrap();

        let transform = Matrix::new([
            [1.0, 0.0, 0.0, 5.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]);

        let instanced_mesh_settings = geometry::InstancedMeshSettings {
            sub_scene,
            transform,
        };
        let instanced_mesh = InstancedMesh::try_new(&main_scene, &instanced_mesh_settings).unwrap();
        let clone = instanced_mesh.clone();
        assert_eq!(instanced_mesh.raw_ptr(), clone.raw_ptr());
        drop(instanced_mesh);
        assert!(!clone.raw_ptr().is_null());
    }
}
