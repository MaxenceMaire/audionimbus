use super::{Material, Point, Scene, Triangle};
use crate::error::{to_option_error, SteamAudioError};
use crate::ray_tracing::{DefaultRayTracer, RayTracer};
use crate::serialized_object::SerializedObject;
use std::marker::PhantomData;

/// A triangle mesh that doesn’t move or deform in any way.
///
/// The unchanging portions of a scene should typically be collected into a single static mesh object.
/// In addition to the geometry, a static mesh also contains acoustic material information for each triangle.
///
/// [`StaticMesh`] is generic over the [`RayTracer`] implementation used to create the scene it
/// belongs to.
#[derive(Debug)]
pub struct StaticMesh<T> {
    inner: audionimbus_sys::IPLStaticMesh,
    _marker: PhantomData<T>,
}

impl<T: RayTracer> StaticMesh<T> {
    /// Creates a new static mesh.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if creation fails.
    pub fn try_new(scene: &Scene, settings: &StaticMeshSettings) -> Result<Self, SteamAudioError> {
        let mut inner = std::ptr::null_mut();

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
                &raw mut static_mesh_settings_ffi,
                &raw mut inner,
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        let static_mesh = Self {
            inner,
            _marker: PhantomData,
        };

        Ok(static_mesh)
    }

    /// Loads a static mesh from a serialized object.
    ///
    /// Typically, the serialized object will be created from a byte array loaded from disk or over the network.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if loading fails.
    pub fn load(
        scene: &Scene,
        serialized_object: &mut SerializedObject,
    ) -> Result<Self, SteamAudioError> {
        Self::load_with_optional_progress_callback(scene, serialized_object, None)
    }

    /// Loads a static mesh from a serialized object, with a progress callback.
    ///
    /// Typically, the serialized object will be created from a byte array loaded from disk or over the network.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if loading fails.
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
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if loading fails.
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

        let mut inner = std::ptr::null_mut();

        let status = unsafe {
            audionimbus_sys::iplStaticMeshLoad(
                scene.raw_ptr(),
                serialized_object.raw_ptr(),
                progress_callback,
                progress_callback_user_data,
                &raw mut inner,
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        let static_mesh = Self {
            inner,
            _marker: PhantomData,
        };

        Ok(static_mesh)
    }

    /// Returns the raw FFI pointer to the underlying static mesh.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub const fn raw_ptr(&self) -> audionimbus_sys::IPLStaticMesh {
        self.inner
    }

    /// Returns a mutable reference to the raw FFI pointer.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub const fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLStaticMesh {
        &mut self.inner
    }
}

impl StaticMesh<DefaultRayTracer> {
    /// Saves a static mesh to a serialized object.
    ///
    /// Typically, the serialized object will then be saved to disk.
    ///
    /// This function can only be called on a static mesh that is part of a scene created with the [`DefaultRayTracer`] ray tracer.
    pub fn save(&self, serialized_object: &mut SerializedObject) {
        unsafe {
            audionimbus_sys::iplStaticMeshSave(self.raw_ptr(), serialized_object.raw_ptr());
        }
    }
}

impl<T> Drop for StaticMesh<T> {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplStaticMeshRelease(&raw mut self.inner) }
    }
}

unsafe impl<T: RayTracer> Send for StaticMesh<T> {}

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
