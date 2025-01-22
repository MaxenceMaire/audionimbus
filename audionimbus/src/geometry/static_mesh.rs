use super::{Material, Point, Scene, Triangle};
use crate::error::{to_option_error, SteamAudioError};

/// A triangle mesh that doesn’t move or deform in any way.
///
/// The unchanging portions of a scene should typically be collected into a single static mesh object.
/// In addition to the geometry, a static mesh also contains acoustic material information for each triangle.
#[derive(Debug)]
pub struct StaticMesh(pub audionimbus_sys::IPLStaticMesh);

impl StaticMesh {
    pub fn try_new(
        scene: Scene,
        static_mesh_settings: &StaticMeshSettings,
    ) -> Result<Self, SteamAudioError> {
        let static_mesh = unsafe {
            let static_mesh: *mut audionimbus_sys::IPLStaticMesh = std::ptr::null_mut();
            let status = audionimbus_sys::iplStaticMeshCreate(
                *scene,
                &mut audionimbus_sys::IPLStaticMeshSettings::from(static_mesh_settings),
                static_mesh,
            );

            if let Some(error) = to_option_error(status) {
                return Err(error);
            }

            *static_mesh
        };

        Ok(Self(static_mesh))
    }
}

impl std::ops::Deref for StaticMesh {
    type Target = audionimbus_sys::IPLStaticMesh;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for StaticMesh {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Drop for StaticMesh {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplStaticMeshRelease(&mut self.0) }
    }
}

/// Settings used to create a static mesh.
#[derive(Default, Debug)]
pub struct StaticMeshSettings {
    /// Number of vertices.
    pub num_vertices: i32,

    /// Number of triangles.
    pub num_triangles: i32,

    /// Number of materials.
    pub num_materials: i32,

    /// Array containing vertices.
    pub vertices: Vec<Point>,

    /// Array containing (indexed) triangles.
    pub triangles: Vec<Triangle>,

    /// Array containing, for each triangle, the index of the associated material.
    pub material_indices: Vec<i32>,

    /// Array of materials.
    pub materials: Vec<Material>,
}

impl From<&StaticMeshSettings> for audionimbus_sys::IPLStaticMeshSettings {
    fn from(settings: &StaticMeshSettings) -> Self {
        todo!()
    }
}
