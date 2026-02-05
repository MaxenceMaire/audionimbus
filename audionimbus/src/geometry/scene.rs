use super::{InstancedMesh, Matrix, StaticMesh};
use crate::callback::{CallbackInformation, ProgressCallback};
use crate::context::Context;
use crate::device::embree::EmbreeDevice;
use crate::device::radeon_rays::RadeonRaysDevice;
use crate::error::{to_option_error, SteamAudioError};
use crate::geometry::{Direction, Point};
use crate::ray_tracing::{CustomRayTracer, DefaultRayTracer, Embree, RadeonRays, RayTracer};
use crate::serialized_object::SerializedObject;
use crate::Sealed;
use slotmap::{DefaultKey, SlotMap};
use std::marker::PhantomData;

/// Marker trait for scenes that can use `save()`.
pub trait SaveableAsSerialized: Sealed {}
impl SaveableAsSerialized for DefaultRayTracer {}

/// Marker trait for scenes that can use `save_obj()`.
pub trait SaveableAsObj: Sealed {}
impl SaveableAsObj for DefaultRayTracer {}
impl SaveableAsObj for Embree {}

/// A 3D scene, which can contain geometry objects that can interact with acoustic rays.
///
/// The scene object itself doesn’t contain any geometry, but is a container for [`StaticMesh`] and [`InstancedMesh`] objects, which do contain geometry.
///
/// [`Scene`] is generic over the [`RayTracer`] implementation.
#[derive(Debug)]
pub struct Scene<T: RayTracer = DefaultRayTracer> {
    inner: audionimbus_sys::IPLScene,

    /// Used to keep static meshes alive for the lifetime of the scene.
    static_meshes: SlotMap<DefaultKey, StaticMesh<T>>,

    /// Used to keep instanced meshes alive for the lifetime of the scene.
    instanced_meshes: SlotMap<DefaultKey, InstancedMesh>,

    /// Static meshes to be dropped by the next call to [`Self::commit`].
    static_meshes_to_remove: Vec<StaticMesh<T>>,

    /// Instanced meshes to be dropped by the next call to [`Self::commit`].
    instanced_meshes_to_remove: Vec<InstancedMesh>,

    _marker: PhantomData<T>,
}

impl<T: RayTracer> Scene<T> {
    /// Creates an empty scene.
    fn empty() -> Self {
        Self {
            inner: std::ptr::null_mut(),
            static_meshes: SlotMap::new(),
            instanced_meshes: SlotMap::new(),
            static_meshes_to_remove: Vec::new(),
            instanced_meshes_to_remove: Vec::new(),
            _marker: PhantomData,
        }
    }

    /// Creates a scene from FFI settings.
    fn from_ffi_create(
        context: &Context,
        settings: &mut audionimbus_sys::IPLSceneSettings,
    ) -> Result<Self, SteamAudioError> {
        let mut scene = Self::empty();

        let status = unsafe {
            audionimbus_sys::iplSceneCreate(context.raw_ptr(), settings, scene.raw_ptr_mut())
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        Ok(scene)
    }

    /// Loads a scene from FFI settings and serialized object.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if loading fails.
    fn from_ffi_load(
        context: &Context,
        settings: &mut audionimbus_sys::IPLSceneSettings,
        serialized_object: &SerializedObject,
        progress_callback: Option<CallbackInformation<ProgressCallback>>,
    ) -> Result<Self, SteamAudioError> {
        let mut scene = Self::empty();

        let (callback, user_data) = progress_callback.map_or((None, std::ptr::null_mut()), |cb| {
            (Some(cb.callback), cb.user_data)
        });

        let status = unsafe {
            audionimbus_sys::iplSceneLoad(
                context.raw_ptr(),
                settings,
                serialized_object.raw_ptr(),
                callback,
                user_data,
                scene.raw_ptr_mut(),
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        Ok(scene)
    }
}

impl Scene<DefaultRayTracer> {
    /// Creates a new scene.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if creation fails.
    pub fn try_new(context: &Context) -> Result<Self, SteamAudioError> {
        Self::from_ffi_create(context, &mut Self::ffi_settings())
    }

    /// Loads a scene from a serialized object.
    ///
    /// Typically, the serialized object will be created from a byte array loaded from disk or over the network.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if loading fails.
    pub fn load(
        context: &Context,
        serialized_object: &SerializedObject,
    ) -> Result<Self, SteamAudioError> {
        Self::from_ffi_load(context, &mut Self::ffi_settings(), serialized_object, None)
    }

    /// Loads a scene from a serialized object.
    ///
    /// Typically, the serialized object will be created from a byte array loaded from disk or over the network.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if loading fails.
    pub fn load_with_progress(
        context: &Context,
        serialized_object: &SerializedObject,
        progress_callback: CallbackInformation<ProgressCallback>,
    ) -> Result<Self, SteamAudioError> {
        Self::from_ffi_load(
            context,
            &mut Self::ffi_settings(),
            serialized_object,
            Some(progress_callback),
        )
    }

    /// Returns FFI scene settings for the default ray tracer implementation.
    fn ffi_settings() -> audionimbus_sys::IPLSceneSettings {
        audionimbus_sys::IPLSceneSettings {
            type_: DefaultRayTracer::scene_type(),
            closestHitCallback: None,
            anyHitCallback: None,
            batchedClosestHitCallback: None,
            batchedAnyHitCallback: None,
            userData: std::ptr::null_mut(),
            embreeDevice: std::ptr::null_mut(),
            radeonRaysDevice: std::ptr::null_mut(),
        }
    }
}

impl Scene<Embree> {
    /// Creates a new scene with the Embree ray tracer.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if creation fails.
    pub fn try_with_embree(
        context: &Context,
        device: &EmbreeDevice,
    ) -> Result<Self, SteamAudioError> {
        Self::from_ffi_create(context, &mut Self::ffi_settings(device))
    }

    /// Loads a scene from a serialized object using Embree.
    ///
    /// Typically, the serialized object will be created from a byte array loaded from disk or over the network.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if loading fails.
    pub fn load_embree(
        context: &Context,
        device: &EmbreeDevice,
        serialized_object: &SerializedObject,
    ) -> Result<Self, SteamAudioError> {
        let mut scene = Self {
            inner: std::ptr::null_mut(),
            static_meshes: SlotMap::new(),
            instanced_meshes: SlotMap::new(),
            static_meshes_to_remove: Vec::new(),
            instanced_meshes_to_remove: Vec::new(),
            _marker: PhantomData,
        };

        let status = unsafe {
            audionimbus_sys::iplSceneLoad(
                context.raw_ptr(),
                &mut Self::ffi_settings(device),
                serialized_object.raw_ptr(),
                None,
                std::ptr::null_mut(),
                scene.raw_ptr_mut(),
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        Ok(scene)
    }

    /// Loads a scene from a serialized object using Embree with a progress callback.
    ///
    /// Typically, the serialized object will be created from a byte array loaded from disk or over the network.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if loading fails.
    pub fn load_embree_with_progress(
        context: &Context,
        device: &EmbreeDevice,
        serialized_object: &SerializedObject,
        progress_callback: CallbackInformation<ProgressCallback>,
    ) -> Result<Self, SteamAudioError> {
        Self::from_ffi_load(
            context,
            &mut Self::ffi_settings(device),
            serialized_object,
            Some(progress_callback),
        )
    }

    /// Returns FFI scene settings with the Embree ray tracer.
    fn ffi_settings(device: &EmbreeDevice) -> audionimbus_sys::IPLSceneSettings {
        audionimbus_sys::IPLSceneSettings {
            type_: Embree::scene_type(),
            closestHitCallback: None,
            anyHitCallback: None,
            batchedClosestHitCallback: None,
            batchedAnyHitCallback: None,
            userData: std::ptr::null_mut(),
            embreeDevice: device.raw_ptr(),
            radeonRaysDevice: std::ptr::null_mut(),
        }
    }
}

impl Scene<RadeonRays> {
    /// Creates a new scene with the Radeon Rays ray tracer.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if creation fails.
    pub fn try_with_radeon_rays(
        context: &Context,
        device: &RadeonRaysDevice,
    ) -> Result<Self, SteamAudioError> {
        Self::from_ffi_create(context, &mut Self::ffi_settings(device))
    }

    /// Loads a scene from a serialized object using Radeon Rays.
    ///
    /// Typically, the serialized object will be created from a byte array loaded from disk or over the network.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if loading fails.
    pub fn load_radeon_rays(
        context: &Context,
        device: &RadeonRaysDevice,
        serialized_object: &SerializedObject,
    ) -> Result<Self, SteamAudioError> {
        Self::from_ffi_load(
            context,
            &mut Self::ffi_settings(device),
            serialized_object,
            None,
        )
    }

    /// Loads a scene from a serialized object using Radeon Rays with a progerss callback.
    ///
    /// Typically, the serialized object will be created from a byte array loaded from disk or over the network.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if loading fails.
    pub fn load_radeon_rays_with_progress(
        context: &Context,
        device: &RadeonRaysDevice,
        serialized_object: &SerializedObject,
        progress_callback: CallbackInformation<ProgressCallback>,
    ) -> Result<Self, SteamAudioError> {
        Self::from_ffi_load(
            context,
            &mut Self::ffi_settings(device),
            serialized_object,
            Some(progress_callback),
        )
    }

    /// Returns FFI scene settings with the Radeon Rays ray tracer.
    fn ffi_settings(device: &RadeonRaysDevice) -> audionimbus_sys::IPLSceneSettings {
        audionimbus_sys::IPLSceneSettings {
            type_: RadeonRays::scene_type(),
            closestHitCallback: None,
            anyHitCallback: None,
            batchedClosestHitCallback: None,
            batchedAnyHitCallback: None,
            userData: std::ptr::null_mut(),
            embreeDevice: std::ptr::null_mut(),
            radeonRaysDevice: device.raw_ptr(),
        }
    }
}

impl Scene<CustomRayTracer> {
    /// Creates a new scene with a custom ray tracer.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if creation fails.
    pub fn try_with_custom(
        context: &Context,
        callbacks: &CustomCallbacks,
    ) -> Result<Self, SteamAudioError> {
        Self::from_ffi_create(context, &mut Self::ffi_settings(callbacks))
    }

    /// Loads a scene from a serialized object using a custom ray tracer.
    ///
    /// Typically, the serialized object will be created from a byte array loaded from disk or over the network.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if loading fails.
    pub fn load_custom(
        context: &Context,
        callbacks: &CustomCallbacks,
        serialized_object: &SerializedObject,
    ) -> Result<Self, SteamAudioError> {
        Self::from_ffi_load(
            context,
            &mut Self::ffi_settings(callbacks),
            serialized_object,
            None,
        )
    }

    /// Loads a scene from a serialized object using a custom ray tracer with a progress callback.
    ///
    /// Typically, the serialized object will be created from a byte array loaded from disk or over the network.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if loading fails.
    pub fn load_custom_with_progress(
        context: &Context,
        callbacks: &CustomCallbacks,
        serialized_object: &SerializedObject,
        progress_callback: CallbackInformation<ProgressCallback>,
    ) -> Result<Self, SteamAudioError> {
        Self::from_ffi_load(
            context,
            &mut Self::ffi_settings(callbacks),
            serialized_object,
            Some(progress_callback),
        )
    }

    /// Returns FFI scene settings with custom callbacks.
    fn ffi_settings(callbacks: &CustomCallbacks) -> audionimbus_sys::IPLSceneSettings {
        audionimbus_sys::IPLSceneSettings {
            type_: CustomRayTracer::scene_type(),
            closestHitCallback: Some(callbacks.closest_hit_callback),
            anyHitCallback: Some(callbacks.any_hit_callback),
            batchedClosestHitCallback: Some(callbacks.batched_closest_hit_callback),
            batchedAnyHitCallback: Some(callbacks.batched_any_hit_callback),
            userData: callbacks.user_data,
            embreeDevice: std::ptr::null_mut(),
            radeonRaysDevice: std::ptr::null_mut(),
        }
    }
}

impl<T: RayTracer> Scene<T> {
    /// Adds a static mesh to a scene and returns a handle to it.
    ///
    /// After calling this function, [`Self::commit`] must be called for the changes to take effect.
    ///
    /// # Returns
    ///
    /// A [`StaticMeshHandle`] that can be used to reference or remove this mesh.
    ///
    /// # Example
    ///
    /// ```
    /// # use audionimbus::*;
    /// # let context = Context::default();
    /// # let mut scene = Scene::try_new(&context)?;
    /// # let vertices = vec![Point::new(0.0, 0.0, 0.0)];
    /// # let triangles = vec![Triangle::new(0, 1, 2)];
    /// # let materials = vec![Material::default()];
    /// # let material_indices = vec![0];
    /// let static_mesh = StaticMesh::try_new(
    ///     &scene,
    ///     &StaticMeshSettings {
    ///         vertices: &vertices,
    ///         triangles: &triangles,
    ///         material_indices: &material_indices,
    ///         materials: &materials,
    ///     },
    /// )?;
    ///
    /// let handle = scene.add_static_mesh(static_mesh);
    /// scene.commit();
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn add_static_mesh(&mut self, static_mesh: StaticMesh<T>) -> StaticMeshHandle {
        unsafe {
            audionimbus_sys::iplStaticMeshAdd(static_mesh.raw_ptr(), self.raw_ptr());
        }
        let key = self.static_meshes.insert(static_mesh);
        StaticMeshHandle(key)
    }

    /// Removes a static mesh from a scene using its handle.
    ///
    /// After calling this function, [`Self::commit`] must be called for the changes to take effect.
    ///
    /// # Returns
    ///
    /// * `true` if the mesh was found and removed successfully.
    /// * `false` if the handle was invalid (mesh already removed or handle never existed).
    ///
    /// # Example
    ///
    /// ```
    /// # use audionimbus::*;
    /// # let context = Context::default();
    /// # let mut scene = Scene::try_new(&context)?;
    /// # let vertices = vec![Point::new(0.0, 0.0, 0.0)];
    /// # let triangles = vec![Triangle::new(0, 1, 2)];
    /// # let materials = vec![Material::default()];
    /// # let material_indices = vec![0];
    /// # let static_mesh = StaticMesh::try_new(
    /// #     &scene,
    /// #     &StaticMeshSettings {
    /// #         vertices: &vertices,
    /// #         triangles: &triangles,
    /// #         material_indices: &material_indices,
    /// #         materials: &materials,
    /// #     },
    /// # )?;
    /// # let handle = scene.add_static_mesh(static_mesh);
    /// # scene.commit();
    /// assert!(scene.remove_static_mesh(handle));
    /// scene.commit();
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn remove_static_mesh(&mut self, handle: StaticMeshHandle) -> bool {
        let handle = handle.0;

        let Some(static_mesh) = self.static_meshes.remove(handle) else {
            return false;
        };

        unsafe {
            audionimbus_sys::iplStaticMeshRemove(static_mesh.raw_ptr(), self.raw_ptr());
        }

        self.static_meshes_to_remove.push(static_mesh);

        true
    }

    /// Adds an instanced mesh to a scene and returns a handle to it.
    ///
    /// After calling this function, [`Self::commit`] must be called for the changes to take effect.
    ///
    /// # Returns
    ///
    /// An [`InstancedMeshHandle`] that can be used to reference, update, or remove this mesh.
    ///
    /// # Example
    ///
    /// ```
    /// # use audionimbus::*;
    /// # let context = Context::default();
    /// # let mut sub_scene = Scene::try_new(&context)?;
    /// # let vertices = vec![Point::new(0.0, 0.0, 0.0)];
    /// # let triangles = vec![Triangle::new(0, 1, 2)];
    /// # let materials = vec![Material::default()];
    /// # let material_indices = vec![0];
    /// # let static_mesh = StaticMesh::try_new(
    /// #     &sub_scene,
    /// #     &StaticMeshSettings {
    /// #         vertices: &vertices,
    /// #         triangles: &triangles,
    /// #         material_indices: &material_indices,
    /// #         materials: &materials,
    /// #     },
    /// # )?;
    /// # let _ = sub_scene.add_static_mesh(static_mesh);
    /// # sub_scene.commit();
    /// # let mut scene = Scene::try_new(&context)?;
    /// # let transform = Matrix4::IDENTITY;
    /// let instanced_mesh = InstancedMesh::try_new(
    ///     &scene,
    ///     &InstancedMeshSettings {
    ///         sub_scene: &sub_scene,
    ///         transform: Matrix4::IDENTITY,
    ///     },
    /// )?;
    ///
    /// let handle = scene.add_instanced_mesh(instanced_mesh);
    /// scene.commit();
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn add_instanced_mesh(&mut self, instanced_mesh: InstancedMesh) -> InstancedMeshHandle {
        unsafe {
            audionimbus_sys::iplInstancedMeshAdd(instanced_mesh.raw_ptr(), self.raw_ptr());
        }
        let key = self.instanced_meshes.insert(instanced_mesh);
        InstancedMeshHandle(key)
    }

    /// Removes an instanced mesh from a scene using its handle.
    ///
    /// After calling this function, [`Self::commit`] must be called for the changes to take effect.
    ///
    /// # Returns
    ///
    /// * `true` if the mesh was found and removed successfully.
    /// * `false` if the handle was invalid (mesh already removed or handle never existed).
    ///
    /// # Example
    ///
    /// ```
    /// # use audionimbus::*;
    /// # let context = Context::default();
    /// # let mut sub_scene = Scene::try_new(&context)?;
    /// # let vertices = vec![Point::new(0.0, 0.0, 0.0)];
    /// # let triangles = vec![Triangle::new(0, 1, 2)];
    /// # let materials = vec![Material::default()];
    /// # let material_indices = vec![0];
    /// # let static_mesh = StaticMesh::try_new(
    /// #     &sub_scene,
    /// #     &StaticMeshSettings {
    /// #         vertices: &vertices,
    /// #         triangles: &triangles,
    /// #         material_indices: &material_indices,
    /// #         materials: &materials,
    /// #     },
    /// # )?;
    /// # let _ = sub_scene.add_static_mesh(static_mesh);
    /// # sub_scene.commit();
    /// # let mut scene = Scene::try_new(&context)?;
    /// # let transform = Matrix4::IDENTITY;
    /// # let instanced_mesh = InstancedMesh::try_new(
    /// #     &scene,
    /// #     &InstancedMeshSettings {
    /// #        sub_scene: &sub_scene,
    /// #         transform: Matrix4::IDENTITY,
    /// #     },
    /// # )?;
    /// # let handle = scene.add_instanced_mesh(instanced_mesh);
    /// # scene.commit();
    /// assert!(scene.remove_instanced_mesh(handle));
    /// scene.commit();
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn remove_instanced_mesh(&mut self, handle: InstancedMeshHandle) -> bool {
        let handle = handle.0;

        let Some(instanced_mesh) = self.instanced_meshes.remove(handle) else {
            return false;
        };

        unsafe {
            audionimbus_sys::iplInstancedMeshRemove(instanced_mesh.raw_ptr(), self.raw_ptr());
        }

        self.instanced_meshes_to_remove.push(instanced_mesh);

        true
    }

    /// Updates the local-to-world transform of an instanced mesh within its parent scene.
    ///
    /// This function allows the instanced mesh to be moved, rotated, and scaled dynamically.
    ///
    /// After calling this function, [`Self::commit`] must be called for the changes to take effect.
    ///
    /// # Returns
    ///
    /// * `true` if the mesh was found and updated successfully.
    /// * `false` if the handle was invalid (mesh already removed or handle never existed).
    ///
    /// # Example
    ///
    /// ```
    /// # use audionimbus::*;
    /// # let context = Context::default();
    /// # let mut sub_scene = Scene::try_new(&context)?;
    /// # let vertices = vec![Point::new(0.0, 0.0, 0.0)];
    /// # let triangles = vec![Triangle::new(0, 1, 2)];
    /// # let materials = vec![Material::default()];
    /// # let material_indices = vec![0];
    /// # let static_mesh = StaticMesh::try_new(
    /// #     &sub_scene,
    /// #     &StaticMeshSettings {
    /// #         vertices: &vertices,
    /// #         triangles: &triangles,
    /// #         material_indices: &material_indices,
    /// #         materials: &materials,
    /// #     },
    /// # )?;
    /// # let _ = sub_scene.add_static_mesh(static_mesh);
    /// # sub_scene.commit();
    /// # let mut scene = Scene::try_new(&context)?;
    /// # let transform = Matrix4::IDENTITY;
    /// # let instanced_mesh = InstancedMesh::try_new(
    /// #     &scene,
    /// #     &InstancedMeshSettings {
    /// #         sub_scene: &sub_scene,
    /// #         transform: Matrix4::IDENTITY,
    /// #     },
    /// # )?;
    /// # let handle = scene.add_instanced_mesh(instanced_mesh);
    /// # scene.commit();
    /// let new_transform = Matrix::new([
    ///     [1.0, 0.0, 0.0, 5.0],
    ///     [0.0, 1.0, 0.0, 0.0],
    ///     [0.0, 0.0, 1.0, 0.0],
    ///     [0.0, 0.0, 0.0, 1.0],
    /// ]);
    /// assert!(scene.update_instanced_mesh_transform(handle, new_transform));
    /// scene.commit();
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn update_instanced_mesh_transform(
        &mut self,
        handle: InstancedMeshHandle,
        transform: Matrix<f32, 4, 4>,
    ) -> bool {
        let Some(instanced_mesh) = self.instanced_meshes.get(handle.0) else {
            return false;
        };

        unsafe {
            audionimbus_sys::iplInstancedMeshUpdateTransform(
                instanced_mesh.raw_ptr(),
                self.raw_ptr(),
                transform.into(),
            );
        }

        true
    }

    /// Commits any changes to the scene.
    ///
    /// This function should be called after any calls to the following functions, for the changes to take effect:
    /// - [`Self::add_static_mesh`]
    /// - [`Self::remove_static_mesh`]
    /// - [`Self::add_instanced_mesh`]
    /// - [`Self::remove_instanced_mesh`]
    /// - [`Self::update_instanced_mesh_transform`]
    ///
    /// For best performance, call this function once after all changes have been made for a given frame.
    ///
    /// This function cannot be called concurrently with any simulation functions.
    ///
    /// # Example
    ///
    /// ```
    /// # use audionimbus::*;
    /// # let context = Context::default();
    /// # let mut scene = Scene::try_new(&context)?;
    /// # let vertices = vec![Point::new(0.0, 0.0, 0.0)];
    /// # let triangles = vec![Triangle::new(0, 1, 2)];
    /// # let materials = vec![Material::default()];
    /// # let material_indices = vec![0];
    /// let static_mesh = StaticMesh::try_new(
    ///     &scene,
    ///     &StaticMeshSettings {
    ///         vertices: &vertices,
    ///         triangles: &triangles,
    ///         material_indices: &material_indices,
    ///         materials: &materials,
    ///     },
    /// )?;
    ///
    /// let handle = scene.add_static_mesh(static_mesh);
    /// scene.commit();
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn commit(&mut self) {
        unsafe {
            audionimbus_sys::iplSceneCommit(self.raw_ptr());
        }

        self.static_meshes_to_remove.clear();
        self.instanced_meshes_to_remove.clear();
    }

    /// Returns the raw FFI pointer to the underlying scene.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub const fn raw_ptr(&self) -> audionimbus_sys::IPLScene {
        self.inner
    }

    /// Returns a mutable reference to the raw FFI pointer.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub const fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLScene {
        &mut self.inner
    }
}

impl<T: RayTracer + SaveableAsSerialized> Scene<T> {
    /// Saves a scene to a serialized object.
    ///
    /// Typically, the serialized object will then be saved to disk.
    ///
    /// This function can only be called on a scene created with the `DefaultRayTracer` ray tracer.
    pub fn save(&self) -> SerializedObject {
        let serialized_object = SerializedObject(std::ptr::null_mut());

        unsafe {
            audionimbus_sys::iplSceneSave(self.raw_ptr(), serialized_object.raw_ptr());
        }

        serialized_object
    }
}

impl<T: RayTracer + SaveableAsObj> Scene<T> {
    /// Saves a scene to an OBJ file.
    ///
    /// An OBJ file is a widely-supported 3D model file format, that can be displayed using a variety of software on most PC platforms.
    /// The OBJ file generated by this function can be useful for detecting problems that occur when exporting scene data from your application to Steam Audio.
    ///
    /// This function can only be called on a scene created with the [`DefaultRayTracer`] or [`Embree`] ray tracers.
    ///
    /// `file_basename` is the absolute or relative path to the OBJ file to generate.
    pub fn save_obj(&self, filename: String) {
        let filename_c_string =
            std::ffi::CString::new(filename).expect("failed to create a CString from the filename");

        unsafe { audionimbus_sys::iplSceneSaveOBJ(self.raw_ptr(), filename_c_string.as_ptr()) }
    }
}

impl<T: RayTracer> Drop for Scene<T> {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplSceneRelease(&raw mut self.inner) }
    }
}

unsafe impl<T: RayTracer> Send for Scene<T> {}

/// Callbacks used for a custom ray tracer.
pub struct CustomCallbacks {
    /// Callback for finding the closest hit along a ray.
    pub closest_hit_callback: unsafe extern "C" fn(
        ray: *const audionimbus_sys::IPLRay,
        min_distance: f32,
        max_distance: f32,
        hit: *mut audionimbus_sys::IPLHit,
        user_data: *mut std::ffi::c_void,
    ),

    /// Callback for finding whether a ray hits anything.
    pub any_hit_callback: unsafe extern "C" fn(
        ray: *const audionimbus_sys::IPLRay,
        min_distance: f32,
        max_distance: f32,
        occluded: *mut u8,
        user_data: *mut std::ffi::c_void,
    ),

    /// Callback for finding the closest hit along a batch of rays.
    pub batched_closest_hit_callback: unsafe extern "C" fn(
        num_rays: i32,
        rays: *const audionimbus_sys::IPLRay,
        min_distances: *const f32,
        max_distances: *const f32,
        hits: *mut audionimbus_sys::IPLHit,
        user_data: *mut std::ffi::c_void,
    ),

    /// Callback for finding whether a batch of rays hits anything.
    pub batched_any_hit_callback: unsafe extern "C" fn(
        num_rays: i32,
        rays: *const audionimbus_sys::IPLRay,
        min_distances: *const f32,
        max_distances: *const f32,
        occluded: *mut u8,
        user_data: *mut std::ffi::c_void,
    ),

    /// Arbitrary user-provided data for use by ray tracing callbacks.
    pub user_data: *mut std::ffi::c_void,
}

/// Calculates the relative direction from the listener to a sound source.
///
/// The returned direction vector is expressed in the listener’s coordinate system.
///
/// # Arguments
///
/// - `context`: the context used to initialize AudioNimbus.
/// - `source_position`: world-space coordinates of the source.
/// - `listener_position`: world-space coordinates of the listener.
/// - `listener_ahead`: world-space unit-length vector pointing ahead relative to the listener.
/// - `listener_up`: world-space unit-length vector pointing up relative to the listener.
///
/// # Returns
///
/// A unit-length vector in the listener’s coordinate space, pointing from the listener to the source.
pub fn relative_direction(
    context: &Context,
    source_position: Point,
    listener_position: Point,
    listener_ahead: Direction,
    listener_up: Direction,
) -> Direction {
    let relative_direction = unsafe {
        audionimbus_sys::iplCalculateRelativeDirection(
            context.raw_ptr(),
            source_position.into(),
            listener_position.into(),
            listener_ahead.into(),
            listener_up.into(),
        )
    };

    relative_direction.into()
}

/// A handle to a static mesh within a scene.
///
/// This handle is returned when adding a static mesh to a scene via [`Scene::add_static_mesh`].
/// It can be used to reference the mesh for operations like removal.
///
/// # Example
///
/// ```
/// # use audionimbus::*;
/// # let context = Context::default();
/// # let mut scene = Scene::try_new(&context)?;
/// # let vertices = vec![Point::new(0.0, 0.0, 0.0)];
/// # let triangles = vec![Triangle::new(0, 1, 2)];
/// # let materials = vec![Material::default()];
/// # let material_indices = vec![0];
/// let static_mesh = StaticMesh::try_new(
///     &scene,
///     &StaticMeshSettings {
///         vertices: &vertices,
///         triangles: &triangles,
///         material_indices: &material_indices,
///         materials: &materials,
///     },
/// )?;
///
/// let handle = scene.add_static_mesh(static_mesh);
/// scene.commit();
///
/// // Later, remove the mesh using the handle.
/// scene.remove_static_mesh(handle);
/// scene.commit();
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct StaticMeshHandle(DefaultKey);

/// A handle to an instanced mesh within a scene.
///
/// This handle is returned when adding an instanced mesh to a scene via [`Scene::add_instanced_mesh`].
/// It can be used to reference the mesh for operations like removal or transform updates.
///
/// # Example
///
/// ```
/// # use audionimbus::*;
/// # let context = Context::default();
/// # let mut sub_scene = Scene::try_new(&context)?;
/// # let vertices = vec![Point::new(0.0, 0.0, 0.0)];
/// # let triangles = vec![Triangle::new(0, 1, 2)];
/// # let materials = vec![Material::default()];
/// # let material_indices = vec![0];
/// # let static_mesh = StaticMesh::try_new(
/// #     &sub_scene,
/// #     &StaticMeshSettings {
/// #         vertices: &vertices,
/// #         triangles: &triangles,
/// #         material_indices: &material_indices,
/// #         materials: &materials,
/// #     },
/// # )?;
/// # let _ = sub_scene.add_static_mesh(static_mesh);
/// # sub_scene.commit();
/// # let mut scene = Scene::try_new(&context)?;
/// # let transform = Matrix4::IDENTITY;
/// let instanced_mesh = InstancedMesh::try_new(
///     &scene,
///     &InstancedMeshSettings {
///         sub_scene: &sub_scene,
///         transform: Matrix4::IDENTITY,
///     },
/// )?;
///
/// let handle = scene.add_instanced_mesh(instanced_mesh);
/// scene.commit();
///
/// // Later, update the transform.
/// let new_transform = Matrix::new([
///     [1.0, 0.0, 0.0, 5.0],
///     [0.0, 1.0, 0.0, 0.0],
///     [0.0, 0.0, 1.0, 0.0],
///     [0.0, 0.0, 0.0, 1.0],
/// ]);
/// assert!(scene.update_instanced_mesh_transform(handle, new_transform));
/// scene.commit();
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct InstancedMeshHandle(DefaultKey);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Vector3;

    #[test]
    fn test_default_scene() {
        let context = Context::default();
        assert!(Scene::try_new(&context).is_ok());
    }

    #[test]
    fn test_relative_direction() {
        let context = Context::default();

        let source_position = Point::new(1.0, 0.0, 0.0);
        let listener_position = Point::new(0.0, 0.0, 0.0);
        let listener_ahead = Direction::new(0.0, 0.0, 1.0);
        let listener_up = Direction::new(0.0, 1.0, 0.0);

        let direction = relative_direction(
            &context,
            source_position,
            listener_position,
            listener_ahead,
            listener_up,
        );

        assert_eq!(
            direction,
            Vector3 {
                x: -1.0,
                y: 0.0,
                z: 0.0
            }
        );
    }
}
