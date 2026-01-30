use super::{InstancedMesh, Matrix, StaticMesh};
use crate::callback::{CallbackInformation, ProgressCallback};
use crate::context::Context;
use crate::device::embree::EmbreeDevice;
use crate::device::open_cl::OpenClDevice;
use crate::device::radeon_rays::RadeonRaysDevice;
use crate::error::{to_option_error, SteamAudioError};
use crate::geometry::{Direction, Point};
use crate::serialized_object::SerializedObject;
use slotmap::{DefaultKey, SlotMap};
use std::marker::PhantomData;

/// Steam Audio’s built-in ray tracer.
///
/// Supports multi-threading. Runs on all platforms that Steam Audio supports.
#[derive(Debug)]
pub struct DefaultRayTracer;

/// The Intel Embree ray tracer.
///
/// Supports multi-threading.
/// This is a highly optimized implementation, and is likely to be faster than the default ray tracer.
/// However, Embree requires Windows, Linux, or macOS, and a 32-bit x86 or 64-bit x86_64 CPU.
#[derive(Debug)]
pub struct Embree;

/// The AMD Radeon Rays ray tracer.
///
/// This is an OpenCL implementation, and can use either the CPU or any GPU that supports OpenCL 1.2 or later.
/// If using the GPU, it is likely to be significantly faster than the default ray tracer.
/// However, with heavy real-time simulation workloads, it may impact your application’s frame rate.
/// On supported AMD GPUs, you can use the Resource Reservation feature to mitigate this issue.
#[derive(Debug)]
pub struct RadeonRays;

/// Allows you to specify callbacks to your own ray tracer.
///
/// Useful if your application already uses a high-performance ray tracer.
/// This option uses the least amount of memory at run-time, since it does not have to build any ray tracing data structures of its own.
#[derive(Debug)]
pub struct CustomRayTracer;

mod sealed {
    pub trait Sealed {}
}

impl sealed::Sealed for DefaultRayTracer {}
impl sealed::Sealed for Embree {}
impl sealed::Sealed for RadeonRays {}
impl sealed::Sealed for CustomRayTracer {}

/// Marker trait for scenes that can use `save()`.
pub trait SaveableAsSerialized: sealed::Sealed {}
impl SaveableAsSerialized for DefaultRayTracer {}

/// Marker trait for scenes that can use `save_obj()`.
pub trait SaveableAsObj: sealed::Sealed {}
impl SaveableAsObj for DefaultRayTracer {}
impl SaveableAsObj for Embree {}

/// Ray tracer implementation. Can be:
/// - [`DefaultRayTracer`]: Steam Audio’s built-in ray tracer
/// - [`Embree`]: The Intel Embree ray tracer
/// - [`RadeonRays`]: The AMD Radeon Rays ray tracer
/// - [`CustomRayTracer`]: Allows you to specify callbacks to your own ray tracer
pub trait RayTracer: sealed::Sealed {}
impl RayTracer for DefaultRayTracer {}
impl RayTracer for Embree {}
impl RayTracer for RadeonRays {}
impl RayTracer for CustomRayTracer {}

/// A 3D scene, which can contain geometry objects that can interact with acoustic rays.
///
/// The scene object itself doesn’t contain any geometry, but is a container for [`StaticMesh`] and [`InstancedMesh`] objects, which do contain geometry.
#[derive(Debug)]
pub struct Scene<T: RayTracer = DefaultRayTracer> {
    inner: audionimbus_sys::IPLScene,

    /// Used to keep static meshes alive for the lifetime of the scene.
    static_meshes: SlotMap<DefaultKey, StaticMesh>,

    /// Used to keep instanced meshes alive for the lifetime of the scene.
    instanced_meshes: SlotMap<DefaultKey, InstancedMesh>,

    /// Static meshes to be dropped by the next call to [`Self::commit`].
    static_meshes_to_remove: Vec<StaticMesh>,

    /// Instanced meshes to be dropped by the next call to [`Self::commit`].
    instanced_meshes_to_remove: Vec<InstancedMesh>,

    _marker: PhantomData<T>,
}

impl Scene<DefaultRayTracer> {
    /// Creates a new scene.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if creation fails.
    pub fn try_new(context: &Context) -> Result<Self, SteamAudioError> {
        let mut scene = Self {
            inner: std::ptr::null_mut(),
            static_meshes: SlotMap::new(),
            instanced_meshes: SlotMap::new(),
            static_meshes_to_remove: Vec::new(),
            instanced_meshes_to_remove: Vec::new(),
            _marker: PhantomData,
        };

        let status = unsafe {
            audionimbus_sys::iplSceneCreate(
                context.raw_ptr(),
                &mut Self::ffi_settings(),
                scene.raw_ptr_mut(),
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        Ok(scene)
    }

    /// Loads a scene from a serialized object.
    ///
    /// Typically, the serialized object will be created from a byte array loaded from disk or over the network.
    pub fn load(
        context: &Context,
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
                &mut Self::ffi_settings(),
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

    /// Loads a scene from a serialized object.
    ///
    /// Typically, the serialized object will be created from a byte array loaded from disk or over the network.
    pub fn load_with_callback(
        context: &Context,
        serialized_object: &SerializedObject,
        progress_callback: CallbackInformation<ProgressCallback>,
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
                &mut Self::ffi_settings(),
                serialized_object.raw_ptr(),
                Some(progress_callback.callback),
                progress_callback.user_data,
                scene.raw_ptr_mut(),
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        Ok(scene)
    }

    /// Returns FFI scene settings for the default ray tracer implementation.
    fn ffi_settings() -> audionimbus_sys::IPLSceneSettings {
        audionimbus_sys::IPLSceneSettings {
            type_: audionimbus_sys::IPLSceneType::IPL_SCENETYPE_DEFAULT,
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
    /// Creates a new scene.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if creation fails.
    pub fn try_new(context: &Context, device: EmbreeDevice) -> Result<Self, SteamAudioError> {
        let mut scene = Self {
            inner: std::ptr::null_mut(),
            static_meshes: SlotMap::new(),
            instanced_meshes: SlotMap::new(),
            static_meshes_to_remove: Vec::new(),
            instanced_meshes_to_remove: Vec::new(),
            _marker: PhantomData,
        };

        let status = unsafe {
            audionimbus_sys::iplSceneCreate(
                context.raw_ptr(),
                &mut Self::ffi_settings(device),
                scene.raw_ptr_mut(),
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        Ok(scene)
    }

    /// Loads a scene from a serialized object.
    ///
    /// Typically, the serialized object will be created from a byte array loaded from disk or over the network.
    pub fn load(
        context: &Context,
        device: EmbreeDevice,
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

    /// Loads a scene from a serialized object.
    ///
    /// Typically, the serialized object will be created from a byte array loaded from disk or over the network.
    pub fn load_with_callback(
        context: &Context,
        device: EmbreeDevice,
        serialized_object: &SerializedObject,
        progress_callback: CallbackInformation<ProgressCallback>,
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
                Some(progress_callback.callback),
                progress_callback.user_data,
                scene.raw_ptr_mut(),
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        Ok(scene)
    }

    /// Returns FFI scene settings with the Embree ray tracer.
    fn ffi_settings(device: EmbreeDevice) -> audionimbus_sys::IPLSceneSettings {
        audionimbus_sys::IPLSceneSettings {
            type_: audionimbus_sys::IPLSceneType::IPL_SCENETYPE_EMBREE,
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
    /// Creates a new scene.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if creation fails.
    pub fn try_new(context: &Context, device: RadeonRaysDevice) -> Result<Self, SteamAudioError> {
        let mut scene = Self {
            inner: std::ptr::null_mut(),
            static_meshes: SlotMap::new(),
            instanced_meshes: SlotMap::new(),
            static_meshes_to_remove: Vec::new(),
            instanced_meshes_to_remove: Vec::new(),
            _marker: PhantomData,
        };

        let status = unsafe {
            audionimbus_sys::iplSceneCreate(
                context.raw_ptr(),
                &mut Self::ffi_settings(device),
                scene.raw_ptr_mut(),
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        Ok(scene)
    }

    /// Loads a scene from a serialized object.
    ///
    /// Typically, the serialized object will be created from a byte array loaded from disk or over the network.
    pub fn load(
        context: &Context,
        device: RadeonRaysDevice,
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

    /// Loads a scene from a serialized object.
    ///
    /// Typically, the serialized object will be created from a byte array loaded from disk or over the network.
    pub fn load_with_callback(
        context: &Context,
        device: RadeonRaysDevice,
        serialized_object: &SerializedObject,
        progress_callback: CallbackInformation<ProgressCallback>,
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
                Some(progress_callback.callback),
                progress_callback.user_data,
                scene.raw_ptr_mut(),
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        Ok(scene)
    }

    /// Returns FFI scene settings with the Radeon Rays ray tracer.
    fn ffi_settings(device: RadeonRaysDevice) -> audionimbus_sys::IPLSceneSettings {
        audionimbus_sys::IPLSceneSettings {
            type_: audionimbus_sys::IPLSceneType::IPL_SCENETYPE_RADEONRAYS,
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
    /// Creates a new scene.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if creation fails.
    pub fn try_new(
        context: &Context,
        callbacks: &CustomCallbacks,
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
            audionimbus_sys::iplSceneCreate(
                context.raw_ptr(),
                &mut Self::ffi_settings(callbacks),
                scene.raw_ptr_mut(),
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        Ok(scene)
    }

    /// Loads a scene from a serialized object.
    ///
    /// Typically, the serialized object will be created from a byte array loaded from disk or over the network.
    pub fn load(
        context: &Context,
        callbacks: &CustomCallbacks,
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
                &mut Self::ffi_settings(callbacks),
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

    /// Loads a scene from a serialized object.
    ///
    /// Typically, the serialized object will be created from a byte array loaded from disk or over the network.
    pub fn load_with_callback(
        context: &Context,
        callbacks: &CustomCallbacks,
        serialized_object: &SerializedObject,
        progress_callback: CallbackInformation<ProgressCallback>,
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
                &mut Self::ffi_settings(callbacks),
                serialized_object.raw_ptr(),
                Some(progress_callback.callback),
                progress_callback.user_data,
                scene.raw_ptr_mut(),
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        Ok(scene)
    }

    /// Returns FFI scene settings with custom callbacks.
    fn ffi_settings(callbacks: &CustomCallbacks) -> audionimbus_sys::IPLSceneSettings {
        audionimbus_sys::IPLSceneSettings {
            type_: audionimbus_sys::IPLSceneType::IPL_SCENETYPE_CUSTOM,
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
    /// # let mut scene = Scene::try_new(&context, &SceneSettings::default())?;
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
    pub fn add_static_mesh(&mut self, static_mesh: StaticMesh) -> StaticMeshHandle {
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
    /// # let mut scene = Scene::try_new(&context, &SceneSettings::default())?;
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
    /// # let mut sub_scene = Scene::try_new(&context, &SceneSettings::default())?;
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
    /// # let mut scene = Scene::try_new(&context, &SceneSettings::default())?;
    /// # let transform = Matrix4::IDENTITY;
    /// let instanced_mesh = InstancedMesh::try_new(
    ///     &scene,
    ///     InstancedMeshSettings {
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
    /// # let mut sub_scene = Scene::try_new(&context, &SceneSettings::default())?;
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
    /// # let mut scene = Scene::try_new(&context, &SceneSettings::default())?;
    /// # let transform = Matrix4::IDENTITY;
    /// # let instanced_mesh = InstancedMesh::try_new(
    /// #     &scene,
    /// #     InstancedMeshSettings {
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
    /// # let mut sub_scene = Scene::try_new(&context, &SceneSettings::default())?;
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
    /// # let mut scene = Scene::try_new(&context, &SceneSettings::default())?;
    /// # let transform = Matrix4::IDENTITY;
    /// # let instanced_mesh = InstancedMesh::try_new(
    /// #     &scene,
    /// #     InstancedMeshSettings {
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
    /// # let mut scene = Scene::try_new(&context, &SceneSettings::default())?;
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
    pub fn raw_ptr(&self) -> audionimbus_sys::IPLScene {
        self.inner
    }

    /// Returns a mutable reference to the raw FFI pointer.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLScene {
        &mut self.inner
    }
}

impl<T: RayTracer + SaveableAsSerialized> Scene<T> {
    /// Saves a scene to a serialized object.
    ///
    /// Typically, the serialized object will then be saved to disk.
    ///
    /// This function can only be called on a scene created with [`SceneSettings::Default`].
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
    /// This function can only be called on a scene created with [`SceneSettings::Default`] or [`SceneSettings::Embree`].
    ///
    /// `file_basename` is the absolute or relative path to the OBJ file to generate.
    pub fn save_obj(&self, filename: String) {
        let filename_c_string =
            std::ffi::CString::new(filename).expect("failed to create a CString from the filename");

        unsafe { audionimbus_sys::iplSceneSaveOBJ(self.raw_ptr(), filename_c_string.as_ptr()) }
    }
}

impl Clone for Scene {
    fn clone(&self) -> Self {
        unsafe {
            audionimbus_sys::iplSceneRetain(self.inner);
        }

        Self {
            inner: self.inner,
            static_meshes: self.static_meshes.clone(),
            instanced_meshes: self.instanced_meshes.clone(),
            static_meshes_to_remove: self.static_meshes_to_remove.clone(),
            instanced_meshes_to_remove: self.instanced_meshes_to_remove.clone(),
            _marker: PhantomData,
        }
    }
}

impl<T: RayTracer> Drop for Scene<T> {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplSceneRelease(&mut self.inner) }
    }
}

unsafe impl Send for Scene {}
unsafe impl Sync for Scene {}

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

/// The scene parameters.
#[derive(Default, Copy, Clone, Debug)]
pub enum SceneParams<'a> {
    /// Steam Audio’s built-in ray tracer.
    #[default]
    Default,

    /// The Intel Embree ray tracer.
    Embree,

    /// The AMD Radeon Rays ray tracer.
    RadeonRays {
        /// The OpenCL device being used.
        open_cl_device: &'a OpenClDevice,

        /// The Radeon Rays device being used.
        radeon_rays_device: &'a RadeonRaysDevice,
    },

    /// Custom ray tracer.
    Custom {
        /// The number of rays that will be passed to the callbacks every time rays need to be traced.
        ray_batch_size: u32,
    },
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
/// # let mut scene = Scene::try_new(&context, &SceneSettings::default())?;
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
/// # let mut sub_scene = Scene::try_new(&context, &SceneSettings::default())?;
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
/// # let mut scene = Scene::try_new(&context, &SceneSettings::default())?;
/// # let transform = Matrix4::IDENTITY;
/// let instanced_mesh = InstancedMesh::try_new(
///     &scene,
///     InstancedMeshSettings {
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
        let scene_settings = SceneSettings::default();
        let scene_result = Scene::try_new(&context, &scene_settings);
        assert!(scene_result.is_ok());
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
