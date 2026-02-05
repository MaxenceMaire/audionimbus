use super::{Matrix, Scene};
use crate::error::{to_option_error, SteamAudioError};

/// A triangle mesh that can be moved (translated), rotated, or scaled, but cannot deform.
///
/// Portions of a scene that undergo rigid-body motion can be represented as instanced meshes.
/// An instanced mesh is essentially a scene (called the “sub-scene”) with a transform applied to it.
/// Adding an instanced mesh to a scene places the sub-scene into the scene with the transform applied.
/// For example, the sub-scene may be a prefab door, and the transform can be used to place it in a doorway and animate it as it opens or closes.
#[derive(Debug)]
pub struct InstancedMesh(audionimbus_sys::IPLInstancedMesh);

impl InstancedMesh {
    /// Creates a new instanced mesh.
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

/// Settings used to create an instanced mesh.
#[derive(Debug, Clone)]
pub struct InstancedMeshSettings<'a> {
    /// Handle to the scene to be instantiated.
    pub sub_scene: &'a Scene,

    /// Local-to-world transform that places the instance within the parent scene.
    pub transform: Matrix<f32, 4, 4>,
}
