use super::{Matrix, Scene};
use crate::error::{to_option_error, SteamAudioError};

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
#[derive(Debug)]
pub struct InstancedMesh(audionimbus_sys::IPLInstancedMesh);

impl InstancedMesh {
    /// Creates a new instanced mesh and returns a handle to it.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if creation fails.
    pub fn try_new(
        scene: &Scene,
        settings: &InstancedMeshSettings,
    ) -> Result<Self, SteamAudioError> {
        let mut instanced_mesh = Self(std::ptr::null_mut());

        let mut instanced_mesh_settings_ffi = audionimbus_sys::IPLInstancedMeshSettings {
            subScene: settings.sub_scene.raw_ptr(),
            transform: settings.transform.into(),
        };

        let status = unsafe {
            audionimbus_sys::iplInstancedMeshCreate(
                scene.raw_ptr(),
                &raw mut instanced_mesh_settings_ffi,
                instanced_mesh.raw_ptr_mut(),
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        Ok(instanced_mesh)
    }

    /// Returns the raw FFI pointer to the underlying instanced mesh.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub const fn raw_ptr(&self) -> audionimbus_sys::IPLInstancedMesh {
        self.0
    }

    /// Returns a mutable reference to the raw FFI pointer.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub const fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLInstancedMesh {
        &mut self.0
    }
}

impl Drop for InstancedMesh {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplInstancedMeshRelease(&raw mut self.0) }
    }
}

unsafe impl Send for InstancedMesh {}
unsafe impl Sync for InstancedMesh {}

impl Clone for InstancedMesh {
    /// Retains an additional reference to the instanced mesh.
    ///
    /// The returned [`InstancedMesh`] shares the same underlying Steam Audio object.
    fn clone(&self) -> Self {
        // SAFETY: The instanced mesh will not be destroyed until all references are released.
        Self(unsafe { audionimbus_sys::iplInstancedMeshRetain(self.0) })
    }
}

/// Settings used to create an instanced mesh.
#[derive(Debug, Clone)]
pub struct InstancedMeshSettings<'a> {
    /// Handle to the scene to be instantiated.
    pub sub_scene: &'a Scene,

    /// Local-to-world transform that places the instance within the parent scene.
    pub transform: Matrix<f32, 4, 4>,
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
            sub_scene: &sub_scene,
            transform,
        };
        let instanced_mesh = InstancedMesh::try_new(&main_scene, &instanced_mesh_settings).unwrap();
        let clone = instanced_mesh.clone();
        assert_eq!(instanced_mesh.raw_ptr(), clone.raw_ptr());
        drop(instanced_mesh);
        assert!(!clone.raw_ptr().is_null());
    }
}
