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
    pub fn try_new(
        scene: &Scene,
        settings: InstancedMeshSettings,
    ) -> Result<Self, SteamAudioError> {
        let mut instanced_mesh = Self(std::ptr::null_mut());

        let mut instanced_mesh_settings_ffi = audionimbus_sys::IPLInstancedMeshSettings {
            subScene: settings.sub_scene.raw_ptr(),
            transform: settings.transform.into(),
        };

        let status = unsafe {
            audionimbus_sys::iplInstancedMeshCreate(
                scene.raw_ptr(),
                &mut instanced_mesh_settings_ffi,
                instanced_mesh.raw_ptr_mut(),
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        Ok(instanced_mesh)
    }

    /// Updates the local-to-world transform of the instanced mesh within its parent scene.
    ///
    /// This function allows the instanced mesh to be moved, rotated, and scaled dynamically.
    ///
    /// After calling this function, [`Scene::commit`] must be called for the changes to take effect.
    pub fn update_transform(&mut self, scene: &Scene, new_transform: &Matrix<f32, 4, 4>) {
        unsafe {
            audionimbus_sys::iplInstancedMeshUpdateTransform(
                self.raw_ptr(),
                scene.raw_ptr(),
                new_transform.into(),
            );
        }
    }

    pub fn raw_ptr(&self) -> audionimbus_sys::IPLInstancedMesh {
        self.0
    }

    pub fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLInstancedMesh {
        &mut self.0
    }
}

impl Clone for InstancedMesh {
    fn clone(&self) -> Self {
        unsafe {
            audionimbus_sys::iplInstancedMeshRetain(self.0);
        }
        Self(self.0)
    }
}

impl Drop for InstancedMesh {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplInstancedMeshRelease(&mut self.0) }
    }
}

unsafe impl Send for InstancedMesh {}
unsafe impl Sync for InstancedMesh {}

/// Settings used to create an instanced mesh.
#[derive(Debug, Clone)]
pub struct InstancedMeshSettings {
    /// Handle to the scene to be instantiated.
    pub sub_scene: Scene,

    /// Local-to-world transform that places the instance within the parent scene.
    pub transform: Matrix<f32, 4, 4>,
}
