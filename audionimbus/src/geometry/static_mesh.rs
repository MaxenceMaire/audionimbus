use super::{Material, Point, Scene, Triangle};
use crate::callback::ProgressCallback;
use crate::error::{SteamAudioError, to_option_error};
use crate::ray_tracing::{DefaultRayTracer, RayTracer};
use crate::serialized_object::SerializedObject;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

/// A triangle mesh that doesn’t move or deform in any way.
///
/// The unchanging portions of a scene should typically be collected into a single static mesh object.
/// In addition to the geometry, a static mesh also contains acoustic material information for each triangle.
///
/// [`StaticMesh`] is generic over the [`RayTracer`] implementation used to create the scene it
/// belongs to.
///
/// `StaticMesh` is a reference-counted handle to an underlying Steam Audio object.
/// Cloning it is cheap; it produces a new handle pointing to the same underlying object, while
/// incrementing a reference count.
/// The underlying object is destroyed when all handles are dropped.
#[derive(Debug, PartialEq, Eq)]
pub struct StaticMesh<T> {
    inner: audionimbus_sys::IPLStaticMesh,
    _marker: PhantomData<T>,
}

impl<T: RayTracer> StaticMesh<T> {
    /// Creates a new static mesh and returns a handle to it.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if creation fails.
    pub fn try_new(
        scene: &Scene<T>,
        settings: &StaticMeshSettings,
    ) -> Result<Self, SteamAudioError> {
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
        scene: &Scene<T>,
        serialized_object: &SerializedObject,
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
        scene: &Scene<T>,
        serialized_object: &SerializedObject,
        progress_callback: ProgressCallback,
    ) -> Result<Self, SteamAudioError> {
        Self::load_with_optional_progress_callback(
            scene,
            serialized_object,
            Some(progress_callback),
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
        scene: &Scene<T>,
        serialized_object: &SerializedObject,
        progress_callback: Option<ProgressCallback>,
    ) -> Result<Self, SteamAudioError> {
        let (callback_fn, user_data) =
            progress_callback.map_or((None, std::ptr::null_mut()), |callback| {
                let (callback_fn, user_data) = callback.as_raw_parts();
                (Some(callback_fn), user_data)
            });

        let mut inner = std::ptr::null_mut();

        let status = unsafe {
            audionimbus_sys::iplStaticMeshLoad(
                scene.raw_ptr(),
                serialized_object.raw_ptr(),
                callback_fn,
                user_data,
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
unsafe impl<T: RayTracer> Sync for StaticMesh<T> {}

impl<T: RayTracer> Clone for StaticMesh<T> {
    /// Retains an additional reference to the static mesh.
    ///
    /// The returned [`StaticMesh`] shares the same underlying Steam Audio object.
    fn clone(&self) -> Self {
        // SAFETY: The static mesh will not be destroyed until all references are released.
        Self {
            inner: unsafe { audionimbus_sys::iplStaticMeshRetain(self.inner) },
            _marker: PhantomData,
        }
    }
}

impl<T: RayTracer> Hash for StaticMesh<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::ptr::hash(self.raw_ptr(), state);
    }
}

/// Settings used to create a static mesh.
#[derive(Debug)]
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

#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn test_static_mesh_clone() {
        let context = Context::default();
        let scene = Scene::<DefaultRayTracer>::try_new(&context).unwrap();

        let vertices = vec![
            geometry::Point::new(0.0, 0.0, 0.0),
            geometry::Point::new(1.0, 0.0, 0.0),
            geometry::Point::new(1.0, 1.0, 0.0),
            geometry::Point::new(0.0, 1.0, 0.0),
        ];

        let triangles = vec![
            geometry::Triangle::new(0, 1, 2),
            geometry::Triangle::new(0, 2, 2),
        ];

        let materials = vec![geometry::Material {
            absorption: [0.1, 0.1, 0.1],
            scattering: 0.5,
            transmission: [0.2, 0.2, 0.2],
        }];

        let material_indices = vec![0, 0];

        let settings = geometry::StaticMeshSettings {
            vertices: &vertices,
            triangles: &triangles,
            material_indices: &material_indices,
            materials: &materials,
        };

        let static_mesh = StaticMesh::<DefaultRayTracer>::try_new(&scene, &settings).unwrap();
        let clone = static_mesh.clone();
        assert_eq!(static_mesh.raw_ptr(), clone.raw_ptr());
        drop(static_mesh);
        assert!(!clone.raw_ptr().is_null());
    }
}
