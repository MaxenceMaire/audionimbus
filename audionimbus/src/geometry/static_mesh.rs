use super::scene::{GeometryBinding, Scene};
use super::{Material, Point, Triangle};
#[cfg(feature = "dynamic")]
use crate::binding::Dynamic;
use crate::binding::Static;
use crate::callback::ProgressCallback;
use crate::context::Context;
use crate::error::{SteamAudioError, to_option_error};
use crate::ray_tracing::{DefaultRayTracer, RayTracer};
use crate::serialized_object::SerializedObject;
use std::hash::{Hash, Hasher};

#[cfg(feature = "dynamic")]
use super::scene::RayTracerKind;
#[cfg(feature = "dynamic")]
use super::scene::SceneError;

/// Geometry storage for a compile-time or runtime binding.
#[derive(Debug, PartialEq, Eq)]
pub struct StaticMesh<F> {
    /// The native static mesh pointer.
    inner: audionimbus_sys::IPLStaticMesh,
    /// The compile-time or runtime geometry binding.
    binding: F,
}

impl<F: GeometryBinding> StaticMesh<F> {
    /// Creates a static mesh for a geometry binding.
    ///
    /// # Errors
    ///
    /// - [`StaticMeshError::MaterialIndexCountMismatch`] when triangle and material counts differ.
    /// - [`StaticMeshError::VertexIndexOutOfBounds`] when a triangle refers to a missing vertex.
    /// - [`StaticMeshError::MaterialIndexOutOfBounds`] when a triangle refers to a missing material.
    /// - [`StaticMeshError::SteamAudio`] when native creation fails.
    fn try_new_shared(
        scene: &Scene<F>,
        settings: &StaticMeshSettings,
    ) -> Result<Self, StaticMeshError> {
        validate_static_mesh_settings(settings)?;

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
                scene.raw_ptr_shared(),
                &raw mut static_mesh_settings_ffi,
                &raw mut inner,
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error.into());
        }

        let static_mesh = Self {
            inner,
            binding: scene.binding(),
        };

        Ok(static_mesh)
    }

    /// Loads a static mesh from a serialized object, with an optional progress callback.
    ///
    /// Typically, the serialized object will be created from a byte array loaded from disk or over the network.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if loading fails.
    fn load_with_optional_progress_callback(
        scene: &Scene<F>,
        serialized_object: &SerializedObject,
        progress_callback: Option<ProgressCallback>,
    ) -> Result<Self, SteamAudioError> {
        let (callback_fn, user_data) =
            progress_callback
                .as_ref()
                .map_or((None, std::ptr::null_mut()), |callback| {
                    let (callback_fn, user_data) = callback.as_raw_parts();
                    (Some(callback_fn), user_data)
                });

        let mut inner = std::ptr::null_mut();

        let status = unsafe {
            audionimbus_sys::iplStaticMeshLoad(
                scene.raw_ptr_shared(),
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
            binding: scene.binding(),
        };

        Ok(static_mesh)
    }

    /// Returns the raw static mesh pointer.
    /// Library callers must preserve the checked runtime geometry boundary.
    pub(crate) const fn raw_ptr_shared(&self) -> audionimbus_sys::IPLStaticMesh {
        self.inner
    }

    /// Returns the binding selector.
    /// Library callers must not expose selectors as public configuration choices.
    #[cfg(feature = "dynamic")]
    pub(crate) const fn binding(&self) -> F {
        self.binding
    }

    /// Saves the static mesh without checking its binding capability.
    ///
    /// # Errors
    ///
    /// - [`SteamAudioError::Unspecified`] when the native operation fails.
    /// - [`SteamAudioError::OutOfMemory`] when native allocation fails.
    /// - [`SteamAudioError::Initialization`] when a native dependency fails.
    fn save_shared(&self, context: &Context) -> Result<SerializedObject, SteamAudioError> {
        let serialized_object = SerializedObject::try_new(context)?;
        unsafe {
            audionimbus_sys::iplStaticMeshSave(self.raw_ptr_shared(), serialized_object.raw_ptr());
        }

        Ok(serialized_object)
    }
}

/// A triangle mesh that doesn’t move or deform in any way.
///
/// The unchanging portions of a scene should typically be collected into a single static mesh object.
/// In addition to the geometry, a static mesh also contains acoustic material information for each triangle.
///
/// [`crate::geometry::StaticMesh`] is generic over the [`RayTracer`] implementation used to create the scene it
/// belongs to.
///
/// `StaticMesh` is a reference-counted handle to an underlying Steam Audio object.
/// Cloning it is cheap; it produces a new handle pointing to the same underlying object, while
/// incrementing a reference count.
/// The underlying object is destroyed when all handles are dropped.
pub type StaticStaticMesh<T> = StaticMesh<Static<T>>;

/// A static mesh with a runtime-selected ray tracer.
///
/// Creates meshes with `try_new`, or loads them with `load` and `load_with_progress_callback`.
/// Serializes default-backend meshes with `try_save`.
/// Clones retain the same native mesh.
#[cfg(feature = "dynamic")]
pub type DynamicStaticMesh = StaticMesh<Dynamic<RayTracerKind>>;

impl<T: RayTracer> StaticStaticMesh<T> {
    /// Creates a new static mesh and returns a handle to it.
    ///
    /// # Errors
    ///
    /// Returns [`StaticMeshError`] if the settings are invalid or creation fails.
    pub fn try_new(
        scene: &crate::geometry::scene::StaticScene<T>,
        settings: &StaticMeshSettings,
    ) -> Result<Self, StaticMeshError> {
        Self::try_new_shared(scene, settings)
    }

    /// Loads a static mesh from a serialized object.
    ///
    /// Typically, the serialized object will be created from a byte array loaded from disk or over the network.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if loading fails.
    pub fn load(
        scene: &crate::geometry::scene::StaticScene<T>,
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
        scene: &crate::geometry::scene::StaticScene<T>,
        serialized_object: &SerializedObject,
        progress_callback: ProgressCallback,
    ) -> Result<Self, SteamAudioError> {
        Self::load_with_optional_progress_callback(
            scene,
            serialized_object,
            Some(progress_callback),
        )
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

impl StaticStaticMesh<DefaultRayTracer> {
    /// Saves the static mesh and returns its serialized representation.
    ///
    /// Typically, the serialized object will then be saved to disk.
    ///
    /// This function can only be called on a static mesh that is part of a scene created with the [`DefaultRayTracer`] ray tracer.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if the destination serialized object cannot be created.
    pub fn try_save(&self, context: &Context) -> Result<SerializedObject, SteamAudioError> {
        self.save_shared(context)
    }
}

#[cfg(feature = "dynamic")]
impl DynamicStaticMesh {
    /// Creates a new static mesh and returns a handle to it.
    ///
    /// # Errors
    ///
    /// - [`StaticMeshError::MaterialIndexCountMismatch`] if triangle and material counts differ.
    /// - [`StaticMeshError::VertexIndexOutOfBounds`] if a triangle refers to a missing vertex.
    /// - [`StaticMeshError::MaterialIndexOutOfBounds`] if a triangle refers to a missing material.
    /// - [`StaticMeshError::SteamAudio`] if native creation fails.
    pub fn try_new(
        scene: &crate::dynamic::Scene,
        settings: &StaticMeshSettings,
    ) -> Result<Self, StaticMeshError> {
        Self::try_new_shared(scene, settings)
    }

    /// Loads a static mesh from a serialized object.
    ///
    /// # Errors
    ///
    /// - [`SteamAudioError::Unspecified`] if the native operation fails.
    /// - [`SteamAudioError::OutOfMemory`] if native memory allocation fails.
    /// - [`SteamAudioError::Initialization`] if an external dependency cannot initialize.
    pub fn load(
        scene: &crate::dynamic::Scene,
        serialized_object: &SerializedObject,
    ) -> Result<Self, SteamAudioError> {
        Self::load_with_optional_progress_callback(scene, serialized_object, None)
    }

    /// Loads a static mesh with a progress callback.
    ///
    /// # Errors
    ///
    /// - [`SteamAudioError::Unspecified`] if the native operation fails.
    /// - [`SteamAudioError::OutOfMemory`] if native memory allocation fails.
    /// - [`SteamAudioError::Initialization`] if an external dependency cannot initialize.
    pub fn load_with_progress_callback(
        scene: &crate::dynamic::Scene,
        serialized_object: &SerializedObject,
        progress_callback: ProgressCallback,
    ) -> Result<Self, SteamAudioError> {
        Self::load_with_optional_progress_callback(
            scene,
            serialized_object,
            Some(progress_callback),
        )
    }

    /// Saves the static mesh and returns its serialized representation.
    ///
    /// # Errors
    ///
    /// - [`SceneError::SerializationUnsupported`] when the mesh does not use the default ray tracer.
    /// - [`SceneError::SteamAudio`] when the destination serialized object cannot be created.
    pub fn try_save(&self, context: &Context) -> Result<SerializedObject, SceneError> {
        if self.binding.0 != RayTracerKind::Default {
            return Err(SceneError::SerializationUnsupported(self.binding.0));
        }

        self.save_shared(context).map_err(SceneError::from)
    }
}

impl<F> Drop for StaticMesh<F> {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplStaticMeshRelease(&raw mut self.inner) }
    }
}

unsafe impl<F: GeometryBinding> Send for StaticMesh<F> {}
unsafe impl<F: GeometryBinding> Sync for StaticMesh<F> {}

impl<F: GeometryBinding> Clone for StaticMesh<F> {
    /// Retains an additional reference to the static mesh.
    ///
    /// The returned [`StaticMesh`] shares the same underlying Steam Audio object.
    fn clone(&self) -> Self {
        // SAFETY: The static mesh will not be destroyed until all references are released.
        Self {
            inner: unsafe { audionimbus_sys::iplStaticMeshRetain(self.inner) },
            binding: self.binding,
        }
    }
}

impl<F: GeometryBinding> Hash for StaticMesh<F> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::ptr::hash(self.inner, state);
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

/// Validates static mesh settings.
///
/// # Errors
///
/// Returns [`StaticMeshError`] when material counts differ or an index is out of bounds.
fn validate_static_mesh_settings(settings: &StaticMeshSettings) -> Result<(), StaticMeshError> {
    if settings.material_indices.len() != settings.triangles.len() {
        return Err(StaticMeshError::MaterialIndexCountMismatch {
            num_triangles: settings.triangles.len(),
            num_material_indices: settings.material_indices.len(),
        });
    }

    for (triangle, indices) in settings.triangles.iter().enumerate() {
        for &vertex_index in &indices.indices {
            if !usize::try_from(vertex_index)
                .is_ok_and(|vertex_index| vertex_index < settings.vertices.len())
            {
                return Err(StaticMeshError::VertexIndexOutOfBounds {
                    triangle,
                    vertex_index,
                    num_vertices: settings.vertices.len(),
                });
            }
        }
    }

    for (triangle, &material_index) in settings.material_indices.iter().enumerate() {
        if material_index >= settings.materials.len() {
            return Err(StaticMeshError::MaterialIndexOutOfBounds {
                triangle,
                material_index,
                num_materials: settings.materials.len(),
            });
        }
    }

    Ok(())
}

/// Errors produced while creating a [`crate::geometry::StaticMesh`].
#[derive(Debug, PartialEq, Eq)]
pub enum StaticMeshError {
    /// The number of material indices differs from the number of triangles.
    MaterialIndexCountMismatch {
        num_triangles: usize,
        num_material_indices: usize,
    },

    /// A triangle refers to a vertex that does not exist.
    VertexIndexOutOfBounds {
        triangle: usize,
        vertex_index: i32,
        num_vertices: usize,
    },

    /// A triangle refers to a material that does not exist.
    MaterialIndexOutOfBounds {
        triangle: usize,
        material_index: usize,
        num_materials: usize,
    },

    /// Native static mesh creation failed.
    SteamAudio(SteamAudioError),
}

impl std::error::Error for StaticMeshError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SteamAudio(error) => Some(error),
            _ => None,
        }
    }
}

impl std::fmt::Display for StaticMeshError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::MaterialIndexCountMismatch {
                num_triangles,
                num_material_indices,
            } => write!(
                f,
                "static mesh has {num_triangles} triangles but {num_material_indices} material indices"
            ),
            Self::VertexIndexOutOfBounds {
                triangle,
                vertex_index,
                num_vertices,
            } => write!(
                f,
                "triangle {triangle} has vertex index {vertex_index}, but the mesh has {num_vertices} vertices"
            ),
            Self::MaterialIndexOutOfBounds {
                triangle,
                material_index,
                num_materials,
            } => write!(
                f,
                "triangle {triangle} has material index {material_index}, but the mesh has {num_materials} materials"
            ),
            Self::SteamAudio(error) => error.fmt(f),
        }
    }
}

impl From<SteamAudioError> for StaticMeshError {
    fn from(error: SteamAudioError) -> Self {
        Self::SteamAudio(error)
    }
}

#[cfg(test)]
mod tests {
    use crate::geometry::scene::StaticScene as Scene;
    use crate::geometry::static_mesh::StaticStaticMesh as StaticMesh;
    use crate::*;

    mod validate_static_mesh_settings {
        use super::super::validate_static_mesh_settings;
        use crate::geometry;

        fn valid_static_mesh_settings() -> geometry::StaticMeshSettings<'static> {
            static VERTICES: [geometry::Point; 3] = [
                geometry::Point::new(0.0, 0.0, 0.0),
                geometry::Point::new(1.0, 0.0, 0.0),
                geometry::Point::new(0.0, 1.0, 0.0),
            ];
            static TRIANGLES: [geometry::Triangle; 1] = [geometry::Triangle::new(0, 1, 2)];
            static MATERIAL_INDICES: [usize; 1] = [0];
            static MATERIALS: [geometry::Material; 1] = [geometry::Material::GENERIC];

            geometry::StaticMeshSettings {
                vertices: &VERTICES,
                triangles: &TRIANGLES,
                material_indices: &MATERIAL_INDICES,
                materials: &MATERIALS,
            }
        }

        #[test]
        fn accepts_valid_static_mesh_settings() {
            assert_eq!(
                validate_static_mesh_settings(&valid_static_mesh_settings()),
                Ok(())
            );
        }

        #[test]
        fn rejects_mismatched_material_index_count() {
            let mut settings = valid_static_mesh_settings();
            settings.material_indices = &[];

            assert_eq!(
                validate_static_mesh_settings(&settings),
                Err(geometry::StaticMeshError::MaterialIndexCountMismatch {
                    num_triangles: 1,
                    num_material_indices: 0,
                })
            );
        }

        #[test]
        fn rejects_negative_vertex_index() {
            let triangles = [geometry::Triangle::new(-1, 1, 2)];
            let mut settings = valid_static_mesh_settings();
            settings.triangles = &triangles;

            assert_eq!(
                validate_static_mesh_settings(&settings),
                Err(geometry::StaticMeshError::VertexIndexOutOfBounds {
                    triangle: 0,
                    vertex_index: -1,
                    num_vertices: 3,
                })
            );
        }

        #[test]
        fn rejects_out_of_range_vertex_index() {
            let triangles = [geometry::Triangle::new(0, 1, 3)];
            let mut settings = valid_static_mesh_settings();
            settings.triangles = &triangles;

            assert_eq!(
                validate_static_mesh_settings(&settings),
                Err(geometry::StaticMeshError::VertexIndexOutOfBounds {
                    triangle: 0,
                    vertex_index: 3,
                    num_vertices: 3,
                })
            );
        }

        #[test]
        fn rejects_out_of_range_material_index() {
            let material_indices = [1];
            let mut settings = valid_static_mesh_settings();
            settings.material_indices = &material_indices;

            assert_eq!(
                validate_static_mesh_settings(&settings),
                Err(geometry::StaticMeshError::MaterialIndexOutOfBounds {
                    triangle: 0,
                    material_index: 1,
                    num_materials: 1,
                })
            );
        }
    }

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
