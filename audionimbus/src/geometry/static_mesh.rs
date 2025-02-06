use super::{Material, Point, Scene, Triangle};
use crate::error::{to_option_error, SteamAudioError};
use crate::serialized_object::SerializedObject;

/// A triangle mesh that doesn’t move or deform in any way.
///
/// The unchanging portions of a scene should typically be collected into a single static mesh object.
/// In addition to the geometry, a static mesh also contains acoustic material information for each triangle.
#[derive(Debug)]
pub struct StaticMesh(audionimbus_sys::IPLStaticMesh);

impl StaticMesh {
    pub fn try_new(
        scene: &Scene,
        static_mesh_settings: &StaticMeshSettings,
    ) -> Result<Self, SteamAudioError> {
        let static_mesh = unsafe {
            let static_mesh: *mut audionimbus_sys::IPLStaticMesh = std::ptr::null_mut();
            let status = audionimbus_sys::iplStaticMeshCreate(
                scene.raw_ptr(),
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

    pub fn raw_ptr(&self) -> audionimbus_sys::IPLStaticMesh {
        self.0
    }

    /// Saves a static mesh to a serialized object.
    ///
    /// Typically, the serialized object will then be saved to disk.
    ///
    /// This function can only be called on a static mesh that is part of a scene created with [`SceneSettings::Default`].
    pub fn save(&self, serialized_object: &mut SerializedObject) {
        unsafe {
            audionimbus_sys::iplStaticMeshSave(self.raw_ptr(), serialized_object.raw_ptr());
        }
    }

    /// Loads a static mesh from a serialized object.
    ///
    /// Typically, the serialized object will be created from a byte array loaded from disk or over the network.
    pub fn load(
        scene: &Scene,
        serialized_object: &mut SerializedObject,
    ) -> Result<Self, SteamAudioError> {
        Self::load_with_optional_progress_callback(scene, serialized_object, None)
    }

    /// Loads a static mesh from a serialized object, with a progress callback.
    ///
    /// Typically, the serialized object will be created from a byte array loaded from disk or over the network.
    pub fn load_with_progress_callback(
        scene: &Scene,
        serialized_object: &SerializedObject,
        progress_callback: unsafe extern "C" fn(_: f32, _: *mut std::ffi::c_void),
        progress_callback_user_data: *mut std::ffi::c_void,
    ) -> Result<Self, SteamAudioError> {
        Self::load_with_optional_progress_callback(
            scene,
            serialized_object,
            Some((progress_callback, progress_callback_user_data)),
        )
    }

    /// Loads a static mesh from a serialized object, with an optional progress callback.
    ///
    /// Typically, the serialized object will be created from a byte array loaded from disk or over the network.
    fn load_with_optional_progress_callback(
        scene: &Scene,
        serialized_object: &SerializedObject,
        progress_callback_information: Option<(
            unsafe extern "C" fn(_: f32, _: *mut std::ffi::c_void),
            *mut std::ffi::c_void,
        )>,
    ) -> Result<Self, SteamAudioError> {
        let (progress_callback, progress_callback_user_data) =
            if let Some((progress_callback, progress_callback_user_data)) =
                progress_callback_information
            {
                (Some(progress_callback), progress_callback_user_data)
            } else {
                (None, std::ptr::null_mut())
            };

        let static_mesh = unsafe {
            let static_mesh: *mut audionimbus_sys::IPLStaticMesh = std::ptr::null_mut();

            let status = audionimbus_sys::iplStaticMeshLoad(
                scene.raw_ptr(),
                serialized_object.raw_ptr(),
                progress_callback,
                progress_callback_user_data,
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
