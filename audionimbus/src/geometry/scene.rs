use crate::Sealed;
use crate::callback::{CustomRayTracingCallbacks, ProgressCallback};
use crate::context::Context;
use crate::device::embree::EmbreeDevice;
use crate::device::radeon_rays::RadeonRaysDevice;
use crate::error::{SteamAudioError, to_option_error};
use crate::geometry::{Direction, InstancedMesh, Matrix, Point, StaticMesh};
use crate::ray_tracing::{
    CustomCallbackUserData, CustomRayTracer, DefaultRayTracer, Embree, RadeonRays, RayTracer,
};
use crate::serialized_object::SerializedObject;
use slotmap::{DefaultKey, SlotMap};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};

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
///
/// `Scene` is a reference-counted handle to an underlying Steam Audio object.
/// Cloning it is cheap; it produces a new handle pointing to the same underlying object, while
/// incrementing a reference count.
/// The underlying object is destroyed when all handles are dropped.
#[derive(Debug)]
pub struct Scene<T: RayTracer = DefaultRayTracer> {
    inner: audionimbus_sys::IPLScene,
    pub(crate) shared: Arc<Mutex<SceneShared<T>>>,
    _marker: PhantomData<T>,
}

/// Simulator-specific registration state for a scene hierarchy.
#[derive(Clone, Debug)]
pub(crate) struct SceneSimulationRegistration {
    /// The simulation locks that must be held while committing this scene.
    pub(crate) locks: Vec<Arc<Mutex<()>>>,

    /// Number of references from this simulator into this scene subtree.
    pub(crate) ref_count: usize,
}

/// Shared ownership of [`Scene`] data across clones.
#[derive(Debug)]
pub(crate) struct SceneShared<T: RayTracer> {
    /// Used to keep static meshes alive for the lifetime of the scene.
    static_meshes: SlotMap<DefaultKey, StaticMesh<T>>,

    /// Used to keep instanced meshes alive for the lifetime of the scene.
    instanced_meshes: SlotMap<DefaultKey, InstancedMesh<T>>,

    /// Static meshes to be dropped by the next call to [`Self::commit`].
    static_meshes_to_remove: Vec<StaticMesh<T>>,

    /// Instanced meshes to be dropped by the next call to [`Self::commit`].
    instanced_meshes_to_remove: Vec<InstancedMesh<T>>,

    /// Simulator registrations that currently require this scene to block commits.
    simulation_registrations: HashMap<audionimbus_sys::IPLSimulator, SceneSimulationRegistration>,

    /// Keeps the device alive for the lifetime of the scene.
    _device: T::Device,

    /// Keeps the callback user data alive for custom ray tracers.
    _callback_user_data: T::CallbackUserData,
}

impl<T: RayTracer> SceneShared<T> {
    /// Creates empty shared scene state.
    fn new(device: T::Device, callback_user_data: T::CallbackUserData) -> Self {
        Self {
            static_meshes: SlotMap::new(),
            instanced_meshes: SlotMap::new(),
            static_meshes_to_remove: Vec::new(),
            instanced_meshes_to_remove: Vec::new(),
            simulation_registrations: HashMap::new(),
            _device: device,
            _callback_user_data: callback_user_data,
        }
    }

    /// Returns handles to all sub-scenes reachable from this scene.
    ///
    /// Scenes from pending removals are included.
    fn sub_scenes(&self) -> Vec<Scene<T>> {
        self.instanced_meshes
            .values()
            .map(|mesh| mesh.sub_scene.clone())
            .chain(
                self.instanced_meshes_to_remove
                    .iter()
                    .map(|mesh| mesh.sub_scene.clone()),
            )
            .collect()
    }

    /// Returns clones of the simulator registrations for this scene.
    fn registration_snapshots(
        &self,
    ) -> Vec<(audionimbus_sys::IPLSimulator, SceneSimulationRegistration)> {
        self.simulation_registrations
            .iter()
            .map(|(simulator, registration)| (*simulator, registration.clone()))
            .collect()
    }

    /// Collects all registered simulation locks, sorted and deduplicated by pointer.
    fn simulation_locks(&self) -> Vec<Arc<Mutex<()>>> {
        let mut locks: Vec<_> = self
            .simulation_registrations
            .values()
            .flat_map(|registration| registration.locks.iter().cloned())
            .collect();
        locks.sort_unstable_by_key(|lock| Arc::as_ptr(lock) as usize);
        locks.dedup_by_key(|lock| Arc::as_ptr(lock) as usize);
        locks
    }
}

impl<T: RayTracer> Scene<T> {
    /// Creates an empty scene with the specified device and returns a handle to it.
    fn empty(device: T::Device, callback_user_data: T::CallbackUserData) -> Self {
        Self {
            inner: std::ptr::null_mut(),
            shared: Arc::new(Mutex::new(SceneShared::new(device, callback_user_data))),
            _marker: PhantomData,
        }
    }

    /// Creates a scene from FFI settings and returns a handle to it.
    fn from_ffi_create(
        context: &Context,
        settings: &mut audionimbus_sys::IPLSceneSettings,
        callback_user_data: T::CallbackUserData,
        device: T::Device,
    ) -> Result<Self, SteamAudioError> {
        let mut scene = Self::empty(device, callback_user_data);

        let status = unsafe {
            audionimbus_sys::iplSceneCreate(context.raw_ptr(), settings, scene.raw_ptr_mut())
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        Ok(scene)
    }

    /// Loads a scene from FFI settings and serialized object and returns a handle to it.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if loading fails.
    fn from_ffi_load(
        context: &Context,
        settings: &mut audionimbus_sys::IPLSceneSettings,
        callback_user_data: T::CallbackUserData,
        device: T::Device,
        serialized_object: &SerializedObject,
        progress_callback: Option<ProgressCallback>,
    ) -> Result<Self, SteamAudioError> {
        let mut scene = Self::empty(device, callback_user_data);

        let (callback_fn, user_data) =
            progress_callback.map_or((None, std::ptr::null_mut()), |callback| {
                let (callback_fn, user_data) = callback.as_raw_parts();
                (Some(callback_fn), user_data)
            });

        let status = unsafe {
            audionimbus_sys::iplSceneLoad(
                context.raw_ptr(),
                settings,
                serialized_object.raw_ptr(),
                callback_fn,
                user_data,
                scene.raw_ptr_mut(),
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        Ok(scene)
    }

    /// Registers a simulator with this scene and all reachable sub-scenes.
    pub(crate) fn register_simulator(
        &self,
        simulator: audionimbus_sys::IPLSimulator,
        locks: &[Arc<Mutex<()>>],
        count: usize,
    ) {
        if count == 0 {
            return;
        }

        let sub_scenes = {
            let mut shared = self.shared.lock().unwrap();
            shared
                .simulation_registrations
                .entry(simulator)
                .and_modify(|registration| registration.ref_count += count)
                .or_insert_with(|| SceneSimulationRegistration {
                    locks: locks.to_vec(),
                    ref_count: count,
                });
            shared.sub_scenes()
        };

        for sub_scene in sub_scenes {
            sub_scene.register_simulator(simulator, locks, count);
        }
    }

    /// Unregisters a simulator from this scene and all reachable sub-scenes.
    pub(crate) fn unregister_simulator(
        &self,
        simulator: audionimbus_sys::IPLSimulator,
        count: usize,
    ) {
        if count == 0 {
            return;
        }

        let sub_scenes = {
            let mut shared = self.shared.lock().unwrap();
            let Some(registration) = shared.simulation_registrations.get_mut(&simulator) else {
                return;
            };

            if registration.ref_count <= count {
                shared.simulation_registrations.remove(&simulator);
            } else {
                registration.ref_count -= count;
            }

            shared.sub_scenes()
        };

        for sub_scene in sub_scenes {
            sub_scene.unregister_simulator(simulator, count);
        }
    }
}

impl Scene<DefaultRayTracer> {
    /// Creates a new scene and returns a handle to it.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if creation fails.
    pub fn try_new(context: &Context) -> Result<Self, SteamAudioError> {
        Self::from_ffi_create(context, &mut Self::ffi_settings(), (), ())
    }

    /// Loads a scene from a serialized object and returns a handle to it.
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
        Self::from_ffi_load(
            context,
            &mut Self::ffi_settings(),
            (),
            (),
            serialized_object,
            None,
        )
    }

    /// Loads a scene from a serialized object and returns a handle to it.
    ///
    /// Typically, the serialized object will be created from a byte array loaded from disk or over the network.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if loading fails.
    pub fn load_with_progress(
        context: &Context,
        serialized_object: &SerializedObject,
        progress_callback: ProgressCallback,
    ) -> Result<Self, SteamAudioError> {
        Self::from_ffi_load(
            context,
            &mut Self::ffi_settings(),
            (),
            (),
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
    /// Creates a new scene with the Embree ray tracer and returns a handle to it.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if creation fails.
    pub fn try_with_embree(
        context: &Context,
        device: EmbreeDevice,
    ) -> Result<Self, SteamAudioError> {
        let scene = Self::from_ffi_create(context, &mut Self::ffi_settings(&device), (), device)?;
        Ok(scene)
    }

    /// Loads a scene from a serialized object using Embree and returns a handle to it.
    ///
    /// Typically, the serialized object will be created from a byte array loaded from disk or over the network.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if loading fails.
    pub fn load_embree(
        context: &Context,
        device: EmbreeDevice,
        serialized_object: &SerializedObject,
    ) -> Result<Self, SteamAudioError> {
        let scene = Self::from_ffi_load(
            context,
            &mut Self::ffi_settings(&device),
            (),
            device,
            serialized_object,
            None,
        )?;
        Ok(scene)
    }

    /// Loads a scene from a serialized object using Embree with a progress callback, and returns a
    /// handle to it.
    ///
    /// Typically, the serialized object will be created from a byte array loaded from disk or over the network.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if loading fails.
    pub fn load_embree_with_progress(
        context: &Context,
        device: EmbreeDevice,
        serialized_object: &SerializedObject,
        progress_callback: ProgressCallback,
    ) -> Result<Self, SteamAudioError> {
        let scene = Self::from_ffi_load(
            context,
            &mut Self::ffi_settings(&device),
            (),
            device,
            serialized_object,
            Some(progress_callback),
        )?;
        Ok(scene)
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
    /// Creates a new scene with the Radeon Rays ray tracer and returns a handle to it.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if creation fails.
    pub fn try_with_radeon_rays(
        context: &Context,
        device: RadeonRaysDevice,
    ) -> Result<Self, SteamAudioError> {
        let scene = Self::from_ffi_create(context, &mut Self::ffi_settings(&device), (), device)?;
        Ok(scene)
    }

    /// Loads a scene from a serialized object using Radeon Rays and returns a handle to it.
    ///
    /// Typically, the serialized object will be created from a byte array loaded from disk or over the network.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if loading fails.
    pub fn load_radeon_rays(
        context: &Context,
        device: RadeonRaysDevice,
        serialized_object: &SerializedObject,
    ) -> Result<Self, SteamAudioError> {
        let scene = Self::from_ffi_load(
            context,
            &mut Self::ffi_settings(&device),
            (),
            device,
            serialized_object,
            None,
        )?;
        Ok(scene)
    }

    /// Loads a scene from a serialized object using Radeon Rays with a progress callback, and
    /// returns a handle to it.
    ///
    /// Typically, the serialized object will be created from a byte array loaded from disk or over the network.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if loading fails.
    pub fn load_radeon_rays_with_progress(
        context: &Context,
        device: RadeonRaysDevice,
        serialized_object: &SerializedObject,
        progress_callback: ProgressCallback,
    ) -> Result<Self, SteamAudioError> {
        let scene = Self::from_ffi_load(
            context,
            &mut Self::ffi_settings(&device),
            (),
            device,
            serialized_object,
            Some(progress_callback),
        )?;
        Ok(scene)
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
    /// Creates a new scene with a custom ray tracer and returns a handle to it.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if creation fails.
    pub fn try_with_custom(
        context: &Context,
        callbacks: CustomRayTracingCallbacks,
    ) -> Result<Self, SteamAudioError> {
        let (mut settings, user_data) = callbacks.as_ffi_settings();
        Self::from_ffi_create(
            context,
            &mut settings,
            CustomCallbackUserData(user_data),
            (),
        )
    }

    /// Loads a scene from a serialized object using a custom ray tracer and returns a handle to it.
    ///
    /// Typically, the serialized object will be created from a byte array loaded from disk or over the network.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if loading fails.
    pub fn load_custom(
        context: &Context,
        callbacks: CustomRayTracingCallbacks,
        serialized_object: &SerializedObject,
    ) -> Result<Self, SteamAudioError> {
        let (mut settings, user_data) = callbacks.as_ffi_settings();
        Self::from_ffi_load(
            context,
            &mut settings,
            CustomCallbackUserData(user_data),
            (),
            serialized_object,
            None,
        )
    }

    /// Loads a scene from a serialized object using a custom ray tracer with a progress callback,
    /// and returns a handle to it.
    ///
    /// Typically, the serialized object will be created from a byte array loaded from disk or over the network.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if loading fails.
    pub fn load_custom_with_progress(
        context: &Context,
        callbacks: CustomRayTracingCallbacks,
        serialized_object: &SerializedObject,
        progress_callback: ProgressCallback,
    ) -> Result<Self, SteamAudioError> {
        let (mut settings, user_data) = callbacks.as_ffi_settings();
        Self::from_ffi_load(
            context,
            &mut settings,
            CustomCallbackUserData(user_data),
            (),
            serialized_object,
            Some(progress_callback),
        )
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

        let key = self
            .shared
            .lock()
            .unwrap()
            .static_meshes
            .insert(static_mesh);

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
        let mut shared = self.shared.lock().unwrap();

        let Some(static_mesh) = shared.static_meshes.remove(handle.0) else {
            return false;
        };

        unsafe {
            audionimbus_sys::iplStaticMeshRemove(static_mesh.raw_ptr(), self.raw_ptr());
        }

        shared.static_meshes_to_remove.push(static_mesh);

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
    ///         sub_scene,
    ///         transform: Matrix4::IDENTITY,
    ///     },
    /// )?;
    ///
    /// let handle = scene.add_instanced_mesh(instanced_mesh);
    /// scene.commit();
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn add_instanced_mesh(&mut self, instanced_mesh: InstancedMesh<T>) -> InstancedMeshHandle {
        unsafe {
            audionimbus_sys::iplInstancedMeshAdd(instanced_mesh.raw_ptr(), self.raw_ptr());
        }

        let sub_scene = instanced_mesh.sub_scene.clone();
        let (key, registrations) = {
            let mut shared = self.shared.lock().unwrap();
            let key = shared.instanced_meshes.insert(instanced_mesh);
            (key, shared.registration_snapshots())
        };

        for (simulator, registration) in registrations {
            sub_scene.register_simulator(simulator, &registration.locks, registration.ref_count);
        }

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
    /// #        sub_scene,
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
        let mut shared = self.shared.lock().unwrap();

        let Some(instanced_mesh) = shared.instanced_meshes.remove(handle.0) else {
            return false;
        };

        unsafe {
            audionimbus_sys::iplInstancedMeshRemove(instanced_mesh.raw_ptr(), self.raw_ptr());
        }

        shared.instanced_meshes_to_remove.push(instanced_mesh);

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
    /// #         sub_scene,
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
        let shared = self.shared.lock().unwrap();
        let Some(instanced_mesh) = shared.instanced_meshes.get(handle.0) else {
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
    /// This function cannot be called while any simulation that uses this scene hierarchy is
    /// running. Either will block until the other finishes.
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
    pub fn commit(&self) {
        let locks = {
            let shared = self.shared.lock().unwrap();
            shared.simulation_locks()
        };

        let _guards = locks
            .iter()
            .map(|lock| lock.lock().unwrap())
            .collect::<Vec<_>>();

        unsafe {
            audionimbus_sys::iplSceneCommit(self.raw_ptr());
        }

        let (removed_sub_scenes, registrations) = {
            let mut shared = self.shared.lock().unwrap();
            let removed_sub_scenes = shared
                .instanced_meshes_to_remove
                .iter()
                .map(|mesh| mesh.sub_scene.clone())
                .collect::<Vec<_>>();
            let registrations = shared.registration_snapshots();
            shared.static_meshes_to_remove.clear();
            shared.instanced_meshes_to_remove.clear();
            (removed_sub_scenes, registrations)
        };

        for removed_sub_scene in removed_sub_scenes {
            for (simulator, registration) in &registrations {
                removed_sub_scene.unregister_simulator(*simulator, registration.ref_count);
            }
        }
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
unsafe impl<T: RayTracer> Sync for Scene<T> {}

impl<T: RayTracer> Clone for Scene<T> {
    /// Retains an additional reference to the scene.
    ///
    /// The returned [`Scene`] shares the same underlying Steam Audio object.
    fn clone(&self) -> Self {
        // SAFETY: The scene will not be destroyed until all references are released.
        Self {
            inner: unsafe { audionimbus_sys::iplSceneRetain(self.inner) },
            shared: Arc::clone(&self.shared),
            _marker: PhantomData,
        }
    }
}

impl<T: RayTracer> PartialEq for Scene<T> {
    fn eq(&self, other: &Self) -> bool {
        self.raw_ptr() == other.raw_ptr()
    }
}

impl<T: RayTracer> Eq for Scene<T> {}

impl<T: RayTracer> Hash for Scene<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::ptr::hash(self.raw_ptr(), state);
    }
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
///         sub_scene,
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
    use crate::{
        AnyHitCallback, AudioSettings, BatchedAnyHitCallback, BatchedClosestHitCallback,
        ClosestHitCallback, CustomRayTracingCallbacks, Direct, DirectSimulationSettings,
        InstancedMesh, InstancedMeshSettings, Matrix4, SimulationSettings, Simulator, Vector3,
    };

    fn registration_ref_count<D, R, P, RE>(
        scene: &Scene<DefaultRayTracer>,
        simulator: &Simulator<DefaultRayTracer, D, R, P, RE>,
    ) -> usize
    where
        D: 'static,
        R: 'static,
        P: 'static,
        RE: 'static,
    {
        scene
            .shared
            .lock()
            .unwrap()
            .simulation_registrations
            .get(&simulator.raw_ptr())
            .map_or(0, |registration| registration.ref_count)
    }

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

    #[test]
    fn test_custom_ray_tracer() {
        let context = Context::default();

        let closest_hit = ClosestHitCallback::new(|_ray, _min_dist, _max_dist| None);

        let any_hit = AnyHitCallback::new(|_ray, _min_dist, _max_dist| false);

        let batched_closest_hit =
            BatchedClosestHitCallback::new(|rays, _min_dists, _max_dists| vec![None; rays.len()]);

        let batched_any_hit =
            BatchedAnyHitCallback::new(|rays, _min_dists, _max_dists| vec![false; rays.len()]);

        let callbacks = CustomRayTracingCallbacks::new(
            closest_hit,
            any_hit,
            batched_closest_hit,
            batched_any_hit,
        );

        assert!(Scene::<CustomRayTracer>::try_with_custom(&context, callbacks).is_ok());
    }

    #[test]
    fn test_scene_clone() {
        let context = Context::default();
        let scene = Scene::<DefaultRayTracer>::try_new(&context).unwrap();
        let clone = scene.clone();
        assert_eq!(scene.raw_ptr(), clone.raw_ptr());
        drop(scene);
        assert!(!clone.raw_ptr().is_null());
    }

    #[test]
    fn test_add_instanced_mesh_propagates_existing_simulator_registration() {
        let context = Context::default();
        let audio_settings = AudioSettings::default();
        let settings =
            SimulationSettings::new(&audio_settings).with_direct(DirectSimulationSettings {
                max_num_occlusion_samples: 4,
            });
        let mut simulator: Simulator<DefaultRayTracer, Direct, (), (), ()> =
            Simulator::try_new(&context, &settings).unwrap();
        let mut root_scene = Scene::try_new(&context).unwrap();
        let sub_scene = Scene::try_new(&context).unwrap();

        simulator.set_scene(&root_scene);
        simulator.commit();

        let instanced_mesh = InstancedMesh::try_new(
            &root_scene,
            &InstancedMeshSettings {
                sub_scene: sub_scene.clone(),
                transform: Matrix4::IDENTITY,
            },
        )
        .unwrap();
        root_scene.add_instanced_mesh(instanced_mesh);

        assert_eq!(registration_ref_count(&sub_scene, &simulator), 1);
    }

    #[test]
    fn test_shared_sub_scene_registration_ref_count_tracks_instances() {
        let context = Context::default();
        let audio_settings = AudioSettings::default();
        let settings =
            SimulationSettings::new(&audio_settings).with_direct(DirectSimulationSettings {
                max_num_occlusion_samples: 4,
            });
        let mut simulator: Simulator<DefaultRayTracer, Direct, (), (), ()> =
            Simulator::try_new(&context, &settings).unwrap();
        let mut root_scene = Scene::try_new(&context).unwrap();
        let shared_sub_scene = Scene::try_new(&context).unwrap();

        let first_mesh = InstancedMesh::try_new(
            &root_scene,
            &InstancedMeshSettings {
                sub_scene: shared_sub_scene.clone(),
                transform: Matrix4::IDENTITY,
            },
        )
        .unwrap();
        let first_handle = root_scene.add_instanced_mesh(first_mesh);

        let second_mesh = InstancedMesh::try_new(
            &root_scene,
            &InstancedMeshSettings {
                sub_scene: shared_sub_scene.clone(),
                transform: Matrix4::IDENTITY,
            },
        )
        .unwrap();
        let second_handle = root_scene.add_instanced_mesh(second_mesh);

        root_scene.commit();

        simulator.set_scene(&root_scene);
        simulator.commit();

        assert_eq!(registration_ref_count(&shared_sub_scene, &simulator), 2);

        assert!(root_scene.remove_instanced_mesh(first_handle));
        assert_eq!(registration_ref_count(&shared_sub_scene, &simulator), 2);

        root_scene.commit();
        assert_eq!(registration_ref_count(&shared_sub_scene, &simulator), 1);

        assert!(root_scene.remove_instanced_mesh(second_handle));
        root_scene.commit();
        assert_eq!(registration_ref_count(&shared_sub_scene, &simulator), 0);
    }

    #[test]
    fn test_simulator_commit_switches_scene_registrations_across_sub_scene_hierarchies() {
        let context = Context::default();
        let audio_settings = AudioSettings::default();
        let settings =
            SimulationSettings::new(&audio_settings).with_direct(DirectSimulationSettings {
                max_num_occlusion_samples: 4,
            });
        let mut simulator: Simulator<DefaultRayTracer, Direct, (), (), ()> =
            Simulator::try_new(&context, &settings).unwrap();

        let mut old_root_scene = Scene::try_new(&context).unwrap();
        let old_sub_scene = Scene::try_new(&context).unwrap();
        let old_instanced_mesh = InstancedMesh::try_new(
            &old_root_scene,
            &InstancedMeshSettings {
                sub_scene: old_sub_scene.clone(),
                transform: Matrix4::IDENTITY,
            },
        )
        .unwrap();
        old_root_scene.add_instanced_mesh(old_instanced_mesh);
        old_root_scene.commit();

        let mut new_root_scene = Scene::try_new(&context).unwrap();
        let new_sub_scene = Scene::try_new(&context).unwrap();
        let new_instanced_mesh = InstancedMesh::try_new(
            &new_root_scene,
            &InstancedMeshSettings {
                sub_scene: new_sub_scene.clone(),
                transform: Matrix4::IDENTITY,
            },
        )
        .unwrap();
        new_root_scene.add_instanced_mesh(new_instanced_mesh);
        new_root_scene.commit();

        simulator.set_scene(&old_root_scene);
        assert_eq!(registration_ref_count(&old_root_scene, &simulator), 1);
        assert_eq!(registration_ref_count(&old_sub_scene, &simulator), 1);

        simulator.commit();
        assert_eq!(registration_ref_count(&old_root_scene, &simulator), 1);
        assert_eq!(registration_ref_count(&old_sub_scene, &simulator), 1);

        simulator.set_scene(&new_root_scene);
        assert_eq!(registration_ref_count(&old_root_scene, &simulator), 1);
        assert_eq!(registration_ref_count(&old_sub_scene, &simulator), 1);
        assert_eq!(registration_ref_count(&new_root_scene, &simulator), 1);
        assert_eq!(registration_ref_count(&new_sub_scene, &simulator), 1);

        simulator.commit();
        assert_eq!(registration_ref_count(&old_root_scene, &simulator), 0);
        assert_eq!(registration_ref_count(&old_sub_scene, &simulator), 0);
        assert_eq!(registration_ref_count(&new_root_scene, &simulator), 1);
        assert_eq!(registration_ref_count(&new_sub_scene, &simulator), 1);
    }
}
