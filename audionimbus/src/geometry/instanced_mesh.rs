use super::{Matrix, Scene};
use crate::error::{to_option_error, SteamAudioError};

/// A triangle mesh that can be moved (translated), rotated, or scaled, but cannot deform.
///
/// Portions of a scene that undergo rigid-body motion can be represented as instanced meshes.
/// An instanced mesh is essentially a scene (called the “sub-scene”) with a transform applied to it.
/// Adding an instanced mesh to a scene places the sub-scene into the scene with the transform applied.
/// For example, the sub-scene may be a prefab door, and the transform can be used to place it in a doorway and animate it as it opens or closes.
#[derive(Debug)]
pub struct InstancedMesh(pub audionimbus_sys::IPLInstancedMesh);

impl InstancedMesh {
    pub fn try_new(
        scene: Scene,
        instanced_mesh_settings: &InstancedMeshSettings,
    ) -> Result<Self, SteamAudioError> {
        let instanced_mesh = unsafe {
            let instanced_mesh: *mut audionimbus_sys::IPLInstancedMesh = std::ptr::null_mut();
            let status = audionimbus_sys::iplInstancedMeshCreate(
                *scene,
                &mut audionimbus_sys::IPLInstancedMeshSettings::from(instanced_mesh_settings),
                instanced_mesh,
            );

            if let Some(error) = to_option_error(status) {
                return Err(error);
            }

            *instanced_mesh
        };

        Ok(Self(instanced_mesh))
    }
}

impl std::ops::Deref for InstancedMesh {
    type Target = audionimbus_sys::IPLInstancedMesh;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for InstancedMesh {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Drop for InstancedMesh {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplInstancedMeshRelease(&mut self.0) }
    }
}

/// Settings used to create an instanced mesh.
#[derive(Debug)]
pub struct InstancedMeshSettings {
    /// Handle to the scene to be instantiated.
    sub_scene: Scene,

    /// Local-to-world transform that places the instance within the parent scene.
    transform: Matrix<f32, 4, 4>,
}

impl From<&InstancedMeshSettings> for audionimbus_sys::IPLInstancedMeshSettings {
    fn from(settings: &InstancedMeshSettings) -> Self {
        todo!()
    }
}
