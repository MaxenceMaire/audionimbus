use super::{Material, Point, Scene, Triangle};
use crate::callback::{CallbackInformation, ProgressCallback};
use crate::error::{to_option_error, SteamAudioError};
use crate::serialized_object::SerializedObject;

/// A triangle mesh that doesn’t move or deform in any way.
///
/// The unchanging portions of a scene should typically be collected into a single static mesh object.
/// In addition to the geometry, a static mesh also contains acoustic material information for each triangle.
#[derive(Debug)]
pub struct StaticMesh(audionimbus_sys::IPLStaticMesh);

impl StaticMesh {
    /// Creates a new static mesh.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if creation fails.
    pub fn try_new(scene: &Scene, settings: &StaticMeshSettings) -> Result<Self, SteamAudioError> {
        let mut static_mesh = Self(std::ptr::null_mut());

        let mut vertices: Vec<audionimbus_sys::IPLVector3> = settings
            .vertices
            .iter()
            .map(|v| audionimbus_sys::IPLVector3::from(*v))
            .collect();

        let mut triangles: Vec<audionimbus_sys::IPLTriangle> = settings
            .triangles
            .iter()
            .map(|t| audionimbus_sys::IPLTriangle::from(*t))
            .collect();

        let mut material_indices: Vec<i32> = settings
            .material_indices
            .iter()
            .map(|&i| i as i32)
            .collect();

        let mut materials: Vec<audionimbus_sys::IPLMaterial> = settings
            .materials
            .iter()
            .map(|m| audionimbus_sys::IPLMaterial::from(*m))
            .collect();

        let mut static_mesh_settings_ffi = audionimbus_sys::IPLStaticMeshSettings {
            numVertices: vertices.len() as i32,
            numTriangles: triangles.len() as i32,
            numMaterials: materials.len() as i32,
            vertices: vertices.as_mut_ptr(),
            triangles: triangles.as_mut_ptr(),
            materialIndices: material_indices.as_mut_ptr(),
            materials: materials.as_mut_ptr(),
        };

        let status = unsafe {
            audionimbus_sys::iplStaticMeshCreate(
                scene.raw_ptr(),
                &mut static_mesh_settings_ffi,
                static_mesh.raw_ptr_mut(),
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        Ok(static_mesh)
    }

    /// Returns the raw FFI pointer to the underlying static mesh.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub fn raw_ptr(&self) -> audionimbus_sys::IPLStaticMesh {
        self.0
    }

    /// Returns a mutable reference to the raw FFI pointer.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLStaticMesh {
        &mut self.0
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
        progress_callback_information: CallbackInformation<ProgressCallback>,
    ) -> Result<Self, SteamAudioError> {
        Self::load_with_optional_progress_callback(
            scene,
            serialized_object,
            Some(progress_callback_information),
        )
    }

    /// Loads a static mesh from a serialized object, with an optional progress callback.
    ///
    /// Typically, the serialized object will be created from a byte array loaded from disk or over the network.
    fn load_with_optional_progress_callback(
        scene: &Scene,
        serialized_object: &SerializedObject,
        progress_callback_information: Option<CallbackInformation<ProgressCallback>>,
    ) -> Result<Self, SteamAudioError> {
        let (progress_callback, progress_callback_user_data) = if let Some(CallbackInformation {
            callback,
            user_data,
        }) = progress_callback_information
        {
            (Some(callback), user_data)
        } else {
            (None, std::ptr::null_mut())
        };

        let mut static_mesh = Self(std::ptr::null_mut());

        let status = unsafe {
            audionimbus_sys::iplStaticMeshLoad(
                scene.raw_ptr(),
                serialized_object.raw_ptr(),
                progress_callback,
                progress_callback_user_data,
                static_mesh.raw_ptr_mut(),
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        Ok(static_mesh)
    }
}

impl Clone for StaticMesh {
    fn clone(&self) -> Self {
        unsafe {
            audionimbus_sys::iplStaticMeshRetain(self.0);
        }
        Self(self.0)
    }
}

impl Drop for StaticMesh {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplStaticMeshRelease(&mut self.0) }
    }
}

unsafe impl Send for StaticMesh {}
unsafe impl Sync for StaticMesh {}

/// Settings used to create a static mesh.
#[derive(Default, Debug)]
pub struct StaticMeshSettings<'a> {
    /// Array containing vertices.
    pub vertices: &'a [Point],

    /// Array containing (indexed) triangles.
    pub triangles: &'a [Triangle],

    /// Array containing, for each triangle, the index of the associated material.
    pub material_indices: &'a [usize],

    /// Array of materials.
    pub materials: &'a [Material],
}
