use super::instanced_mesh::InstancedMesh;
use super::static_mesh::StaticMesh;
use crate::Sealed;
use crate::binding::Static;
use crate::callback::{CustomRayTracingCallbacks, ProgressCallback};
use crate::context::Context;
use crate::device::embree::EmbreeDevice;
use crate::device::radeon_rays::RadeonRaysDevice;
use crate::error::{SteamAudioError, to_option_error};
use crate::geometry::{Direction, Matrix, Point};
use crate::ray_tracing::{
    CustomCallbackUserData, CustomRayTracer, DefaultRayTracer, Embree, RadeonRays, RayTracer,
};
use crate::serialized_object::SerializedObject;
use slotmap::{DefaultKey, SlotMap};
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};

#[cfg(feature = "dynamic")]
use crate::binding::Dynamic;
#[cfg(feature = "dynamic")]
use crate::device::OpenClDevice;
#[cfg(feature = "dynamic")]
use crate::dynamic::{RayTracerKind, RayTracerSettings, SceneError};

/// Marker trait for scenes that can use `try_save()`.
pub trait SaveableAsSerialized: Sealed {}
impl SaveableAsSerialized for DefaultRayTracer {}

/// Marker trait for scenes that can use `save_obj()`.
pub trait SaveableAsObj: Sealed {}
impl SaveableAsObj for DefaultRayTracer {}
impl SaveableAsObj for Embree {}

/// Storage selected by a geometry API binding.
pub trait GeometryBinding: Copy {
    /// Resources retained by each scene in the binding.
    type Resources: Debug + Send + Sync;
}

impl<T: RayTracer> GeometryBinding for Static<T> {
    type Resources = (T::Device, T::CallbackUserData);
}

/// Resources retained by a runtime-selected scene.
#[cfg(feature = "dynamic")]
#[derive(Debug)]
pub enum DynamicSceneResources {
    /// No backend resources.
    Default,

    /// The Embree device.
    Embree(EmbreeDevice),

    /// The OpenCL and Radeon Rays devices.
    RadeonRays {
        /// The OpenCL device.
        open_cl_device: OpenClDevice,

        /// The Radeon Rays device.
        radeon_rays_device: RadeonRaysDevice,
    },

    /// Custom callback storage and batch size.
    Custom {
        /// The callback user data referenced by native code.
        callback_user_data: CustomCallbackUserData,

        /// The ray batch size for simulator construction.
        ray_batch_size: usize,
    },
}

#[cfg(feature = "dynamic")]
impl GeometryBinding for Dynamic<RayTracerKind> {
    type Resources = DynamicSceneResources;
}

/// Geometry storage for a compile-time or runtime binding.
#[derive(Debug)]
pub struct Scene<F: GeometryBinding> {
    /// The native scene pointer.
    inner: audionimbus_sys::IPLScene,
    /// The graph registrations whose mutation must preserve scene commit synchronization.
    pub(crate) shared: Arc<Mutex<SceneShared<F>>>,
    /// The compile-time or runtime geometry binding.
    binding: F,
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
pub struct SceneShared<F: GeometryBinding> {
    /// Used to keep static meshes alive for the lifetime of the scene.
    static_meshes: SlotMap<DefaultKey, StaticMesh<F>>,

    /// Used to keep instanced meshes alive for the lifetime of the scene.
    instanced_meshes: SlotMap<DefaultKey, InstancedMesh<F>>,

    /// Static meshes to be dropped by the next call to [`Scene::commit`].
    static_meshes_to_remove: Vec<StaticMesh<F>>,

    /// Instanced meshes to be dropped by the next call to [`Scene::commit`].
    instanced_meshes_to_remove: Vec<InstancedMesh<F>>,

    /// Simulator registrations that currently require this scene to block commits.
    simulation_registrations: HashMap<audionimbus_sys::IPLSimulator, SceneSimulationRegistration>,

    /// Backend resources retained for the lifetime of the scene.
    _resources: F::Resources,
}

impl<F: GeometryBinding> SceneShared<F> {
    /// Creates empty shared scene state.
    fn new(resources: F::Resources) -> Self {
        Self {
            static_meshes: SlotMap::new(),
            instanced_meshes: SlotMap::new(),
            static_meshes_to_remove: Vec::new(),
            instanced_meshes_to_remove: Vec::new(),
            simulation_registrations: HashMap::new(),
            _resources: resources,
        }
    }

    /// Returns handles to all sub-scenes reachable from this scene.
    ///
    /// Scenes from pending removals are included.
    fn sub_scenes(&self) -> Vec<Scene<F>> {
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

impl<F: GeometryBinding> Scene<F> {
    /// Creates an empty scene with the specified device and returns a handle to it.
    fn empty(binding: F, resources: F::Resources) -> Self {
        Self {
            inner: std::ptr::null_mut(),
            shared: Arc::new(Mutex::new(SceneShared::new(resources))),
            binding,
        }
    }

    /// Creates a scene from FFI settings and returns a handle to it.
    fn from_ffi_create(
        context: &Context,
        settings: &mut audionimbus_sys::IPLSceneSettings,
        binding: F,
        resources: F::Resources,
    ) -> Result<Self, SteamAudioError> {
        let mut scene = Self::empty(binding, resources);

        let status = unsafe {
            audionimbus_sys::iplSceneCreate(context.raw_ptr(), settings, &raw mut scene.inner)
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
        binding: F,
        resources: F::Resources,
        serialized_object: &SerializedObject,
        progress_callback: Option<ProgressCallback>,
    ) -> Result<Self, SteamAudioError> {
        let mut scene = Self::empty(binding, resources);

        let (callback_fn, user_data) =
            progress_callback
                .as_ref()
                .map_or((None, std::ptr::null_mut()), |callback| {
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
                &raw mut scene.inner,
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

/// A 3D scene, which can contain geometry objects that can interact with acoustic rays.
///
/// The scene object itself doesn’t contain any geometry, but is a container for [`crate::StaticMesh`] and [`crate::InstancedMesh`] objects, which do contain geometry.
///
/// [`crate::Scene`] is generic over the [`RayTracer`] implementation.
///
/// `Scene` is a reference-counted handle to an underlying Steam Audio object.
/// Cloning it is cheap; it produces a new handle pointing to the same underlying object, while
/// incrementing a reference count.
/// The underlying object is destroyed when all handles are dropped.
pub type TypedScene<T = DefaultRayTracer> = Scene<Static<T>>;

impl TypedScene<DefaultRayTracer> {
    /// Creates a new scene and returns a handle to it.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if creation fails.
    pub fn try_new(context: &Context) -> Result<Self, SteamAudioError> {
        Self::from_ffi_create(context, &mut Self::ffi_settings(), Static::new(), ((), ()))
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
            Static::new(),
            ((), ()),
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
            Static::new(),
            ((), ()),
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

impl TypedScene<Embree> {
    /// Creates a new scene with the Embree ray tracer and returns a handle to it.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if creation fails.
    pub fn try_with_embree(
        context: &Context,
        device: EmbreeDevice,
    ) -> Result<Self, SteamAudioError> {
        let scene = Self::from_ffi_create(
            context,
            &mut Self::ffi_settings(&device),
            Static::new(),
            (device, ()),
        )?;
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
            Static::new(),
            (device, ()),
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
            Static::new(),
            (device, ()),
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

impl TypedScene<RadeonRays> {
    /// Creates a new scene with the Radeon Rays ray tracer and returns a handle to it.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if creation fails.
    pub fn try_with_radeon_rays(
        context: &Context,
        device: RadeonRaysDevice,
    ) -> Result<Self, SteamAudioError> {
        let scene = Self::from_ffi_create(
            context,
            &mut Self::ffi_settings(&device),
            Static::new(),
            (device, ()),
        )?;
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
            Static::new(),
            (device, ()),
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
            Static::new(),
            (device, ()),
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

impl TypedScene<CustomRayTracer> {
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
            Static::new(),
            ((), CustomCallbackUserData(user_data)),
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
            Static::new(),
            ((), CustomCallbackUserData(user_data)),
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
            Static::new(),
            ((), CustomCallbackUserData(user_data)),
            serialized_object,
            Some(progress_callback),
        )
    }
}

#[cfg(feature = "dynamic")]
impl RayTracerSettings {
    /// Converts runtime settings into native settings and retained resources.
    fn into_scene_parts(self) -> (audionimbus_sys::IPLSceneSettings, DynamicSceneResources) {
        match self {
            Self::Default => (
                Scene::<Static<DefaultRayTracer>>::ffi_settings(),
                DynamicSceneResources::Default,
            ),
            Self::Embree { device } => {
                let settings = Scene::<Static<Embree>>::ffi_settings(&device);
                (settings, DynamicSceneResources::Embree(device))
            }
            Self::RadeonRays {
                open_cl_device,
                radeon_rays_device,
            } => {
                let settings = Scene::<Static<RadeonRays>>::ffi_settings(&radeon_rays_device);
                (
                    settings,
                    DynamicSceneResources::RadeonRays {
                        open_cl_device,
                        radeon_rays_device,
                    },
                )
            }
            Self::Custom {
                callbacks,
                ray_batch_size,
            } => {
                let (settings, callback_user_data) = callbacks.as_ffi_settings();
                (
                    settings,
                    DynamicSceneResources::Custom {
                        callback_user_data: CustomCallbackUserData(callback_user_data),
                        ray_batch_size,
                    },
                )
            }
        }
    }
}

#[cfg(feature = "dynamic")]
impl crate::dynamic::Scene {
    /// Creates a scene with a runtime-selected ray tracer.
    ///
    /// # Errors
    ///
    /// - [`SceneError::SteamAudio`] when native creation fails.
    pub fn try_new(context: &Context, settings: RayTracerSettings) -> Result<Self, SceneError> {
        let ray_tracer = settings.kind();
        let (mut settings, resources) = settings.into_scene_parts();
        Self::from_ffi_create(context, &mut settings, Dynamic(ray_tracer), resources)
            .map_err(SceneError::from)
    }

    /// Loads a scene with a runtime-selected ray tracer.
    ///
    /// # Errors
    ///
    /// - [`SceneError::SteamAudio`] when loading fails.
    pub fn load(
        context: &Context,
        settings: RayTracerSettings,
        serialized_object: &SerializedObject,
    ) -> Result<Self, SceneError> {
        Self::load_with_optional_progress(context, settings, serialized_object, None)
    }

    /// Loads a scene with a runtime-selected ray tracer and a progress callback.
    ///
    /// # Errors
    ///
    /// - [`SceneError::SteamAudio`] when loading fails.
    pub fn load_with_progress(
        context: &Context,
        settings: RayTracerSettings,
        serialized_object: &SerializedObject,
        progress_callback: ProgressCallback,
    ) -> Result<Self, SceneError> {
        Self::load_with_optional_progress(
            context,
            settings,
            serialized_object,
            Some(progress_callback),
        )
    }

    /// Loads a scene with an optional progress callback.
    ///
    /// # Errors
    ///
    /// - [`SceneError::SteamAudio`] when loading fails.
    fn load_with_optional_progress(
        context: &Context,
        settings: RayTracerSettings,
        serialized_object: &SerializedObject,
        progress_callback: Option<ProgressCallback>,
    ) -> Result<Self, SceneError> {
        let ray_tracer = settings.kind();
        let (mut settings, resources) = settings.into_scene_parts();
        Self::from_ffi_load(
            context,
            &mut settings,
            Dynamic(ray_tracer),
            resources,
            serialized_object,
            progress_callback,
        )
        .map_err(SceneError::from)
    }

    /// Returns the selected ray tracer.
    pub const fn ray_tracer(&self) -> RayTracerKind {
        self.binding.0
    }

    /// Adds a static mesh to the scene.
    ///
    /// # Errors
    ///
    /// - [`SceneError::RayTracerMismatch`] when the mesh uses a different ray tracer.
    ///
    /// # Panics
    ///
    /// Panics if the scene graph mutex is poisoned.
    pub fn try_add_static_mesh(
        &mut self,
        static_mesh: crate::dynamic::StaticMesh,
    ) -> Result<StaticMeshHandle, SceneError> {
        let expected = self.ray_tracer();
        let actual = static_mesh.binding().0;
        if actual != expected {
            return Err(SceneError::RayTracerMismatch { expected, actual });
        }

        Ok(self.add_static_mesh_shared(static_mesh))
    }

    /// Adds an instanced mesh to the scene.
    ///
    /// # Errors
    ///
    /// - [`SceneError::RayTracerMismatch`] when the mesh uses a different ray tracer.
    ///
    /// # Panics
    ///
    /// Panics if the scene graph mutex is poisoned.
    pub fn try_add_instanced_mesh(
        &mut self,
        instanced_mesh: crate::dynamic::InstancedMesh,
    ) -> Result<InstancedMeshHandle, SceneError> {
        let expected = self.ray_tracer();
        let actual = instanced_mesh.sub_scene.ray_tracer();
        if actual != expected {
            return Err(SceneError::RayTracerMismatch { expected, actual });
        }

        Ok(self.add_instanced_mesh_shared(instanced_mesh))
    }

    /// Saves the scene and returns its serialized representation.
    ///
    /// # Errors
    ///
    /// - [`SceneError::SerializationUnsupported`] when the scene does not use the default ray tracer.
    /// - [`SceneError::SteamAudio`] when the destination serialized object cannot be created.
    pub fn try_save(&self, context: &Context) -> Result<SerializedObject, SceneError> {
        if self.ray_tracer() != RayTracerKind::Default {
            return Err(SceneError::SerializationUnsupported(self.ray_tracer()));
        }

        self.save_shared(context).map_err(SceneError::from)
    }

    /// Saves the scene as an OBJ file.
    ///
    /// # Errors
    ///
    /// - [`SceneError::ObjUnsupported`] when the ray tracer cannot export OBJ files.
    ///
    /// # Panics
    ///
    /// Panics if the filename contains a null byte.
    pub fn try_save_obj(&self, filename: String) -> Result<(), SceneError> {
        if !matches!(
            self.ray_tracer(),
            RayTracerKind::Default | RayTracerKind::Embree
        ) {
            return Err(SceneError::ObjUnsupported(self.ray_tracer()));
        }

        self.save_obj_shared(filename);
        Ok(())
    }
}

impl<F: GeometryBinding> Scene<F> {
    /// Adds geometry after its binding compatibility is established.
    ///
    /// # Panics
    ///
    /// Panics if the scene graph mutex is poisoned.
    fn add_static_mesh_shared(&mut self, static_mesh: StaticMesh<F>) -> StaticMeshHandle {
        unsafe {
            audionimbus_sys::iplStaticMeshAdd(static_mesh.raw_ptr_shared(), self.raw_ptr_shared());
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
    /// After calling this function, [`Scene::commit`] must be called for the changes to take effect.
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
    /// # let vertices = vec![
    /// #     Point::new(0.0, 0.0, 0.0),
    /// #     Point::new(1.0, 0.0, 0.0),
    /// #     Point::new(0.0, 1.0, 0.0),
    /// # ];
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
            audionimbus_sys::iplStaticMeshRemove(
                static_mesh.raw_ptr_shared(),
                self.raw_ptr_shared(),
            );
        }

        shared.static_meshes_to_remove.push(static_mesh);

        true
    }

    /// Adds geometry after its binding compatibility is established.
    ///
    /// # Panics
    ///
    /// Panics if the scene graph mutex is poisoned.
    fn add_instanced_mesh_shared(
        &mut self,
        instanced_mesh: InstancedMesh<F>,
    ) -> InstancedMeshHandle {
        unsafe {
            audionimbus_sys::iplInstancedMeshAdd(
                instanced_mesh.raw_ptr_shared(),
                self.raw_ptr_shared(),
            );
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
    /// After calling this function, [`Scene::commit`] must be called for the changes to take effect.
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
    /// # let vertices = vec![
    /// #     Point::new(0.0, 0.0, 0.0),
    /// #     Point::new(1.0, 0.0, 0.0),
    /// #     Point::new(0.0, 1.0, 0.0),
    /// # ];
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
            audionimbus_sys::iplInstancedMeshRemove(
                instanced_mesh.raw_ptr_shared(),
                self.raw_ptr_shared(),
            );
        }

        shared.instanced_meshes_to_remove.push(instanced_mesh);

        true
    }

    /// Updates the local-to-world transform of an instanced mesh within its parent scene.
    ///
    /// This function allows the instanced mesh to be moved, rotated, and scaled dynamically.
    ///
    /// After calling this function, [`Scene::commit`] must be called for the changes to take effect.
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
    /// # let vertices = vec![
    /// #     Point::new(0.0, 0.0, 0.0),
    /// #     Point::new(1.0, 0.0, 0.0),
    /// #     Point::new(0.0, 1.0, 0.0),
    /// # ];
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
                instanced_mesh.raw_ptr_shared(),
                self.raw_ptr_shared(),
                transform.into(),
            );
        }

        true
    }

    /// Commits any changes to the scene.
    ///
    /// This function should be called after any calls to the following functions, for the changes to take effect:
    /// - [`add_static_mesh`](crate::Scene)
    /// - [`Self::remove_static_mesh`]
    /// - [`add_instanced_mesh`](crate::Scene)
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
    /// # let vertices = vec![
    /// #     Point::new(0.0, 0.0, 0.0),
    /// #     Point::new(1.0, 0.0, 0.0),
    /// #     Point::new(0.0, 1.0, 0.0),
    /// # ];
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
            audionimbus_sys::iplSceneCommit(self.raw_ptr_shared());
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

    /// Returns the raw scene pointer.
    /// Library callers must preserve the checked runtime geometry boundary.
    pub(crate) const fn raw_ptr_shared(&self) -> audionimbus_sys::IPLScene {
        self.inner
    }

    /// Returns the binding selector.
    /// Library callers must not expose selectors as public configuration choices.
    pub(crate) const fn binding(&self) -> F {
        self.binding
    }

    /// Saves the scene without checking its binding capability.
    ///
    /// # Errors
    ///
    /// - [`SteamAudioError::Unspecified`] when the native operation fails.
    /// - [`SteamAudioError::OutOfMemory`] when native allocation fails.
    /// - [`SteamAudioError::Initialization`] when a native dependency fails.
    fn save_shared(&self, context: &Context) -> Result<SerializedObject, SteamAudioError> {
        let serialized_object = SerializedObject::try_new(context)?;
        unsafe {
            audionimbus_sys::iplSceneSave(self.raw_ptr_shared(), serialized_object.raw_ptr());
        }

        Ok(serialized_object)
    }

    /// Saves the scene as OBJ without checking its binding capability.
    ///
    /// # Panics
    ///
    /// Panics if the filename contains a null byte.
    fn save_obj_shared(&self, filename: String) {
        let filename_c_string =
            std::ffi::CString::new(filename).expect("failed to create a CString from the filename");

        unsafe {
            audionimbus_sys::iplSceneSaveOBJ(self.raw_ptr_shared(), filename_c_string.as_ptr());
        }
    }
}

impl<T: RayTracer> TypedScene<T> {
    /// Adds a static mesh to a scene and returns a handle to it.
    ///
    /// After calling this function, [`Scene::commit`] must be called for the changes to take effect.
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
    /// # let vertices = vec![
    /// #     Point::new(0.0, 0.0, 0.0),
    /// #     Point::new(1.0, 0.0, 0.0),
    /// #     Point::new(0.0, 1.0, 0.0),
    /// # ];
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
    pub fn add_static_mesh(&mut self, static_mesh: super::StaticMesh<T>) -> StaticMeshHandle {
        self.add_static_mesh_shared(static_mesh)
    }

    /// Adds an instanced mesh to a scene and returns a handle to it.
    ///
    /// After calling this function, [`Scene::commit`] must be called for the changes to take effect.
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
    /// # let vertices = vec![
    /// #     Point::new(0.0, 0.0, 0.0),
    /// #     Point::new(1.0, 0.0, 0.0),
    /// #     Point::new(0.0, 1.0, 0.0),
    /// # ];
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
    pub fn add_instanced_mesh(
        &mut self,
        instanced_mesh: super::InstancedMesh<T>,
    ) -> InstancedMeshHandle {
        self.add_instanced_mesh_shared(instanced_mesh)
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

impl<T: RayTracer + SaveableAsSerialized> TypedScene<T> {
    /// Saves the scene and returns its serialized representation.
    ///
    /// Typically, the serialized object will then be saved to disk.
    ///
    /// This function can only be called on a scene created with the `DefaultRayTracer` ray tracer.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if the destination serialized object cannot be created.
    pub fn try_save(&self, context: &Context) -> Result<SerializedObject, SteamAudioError> {
        self.save_shared(context)
    }
}

impl<T: RayTracer + SaveableAsObj> TypedScene<T> {
    /// Saves a scene to an OBJ file.
    ///
    /// An OBJ file is a widely-supported 3D model file format, that can be displayed using a variety of software on most PC platforms.
    /// The OBJ file generated by this function can be useful for detecting problems that occur when exporting scene data from your application to Steam Audio.
    ///
    /// This function can only be called on a scene created with the [`DefaultRayTracer`] or [`Embree`] ray tracers.
    ///
    /// `file_basename` is the absolute or relative path to the OBJ file to generate.
    pub fn save_obj(&self, filename: String) {
        self.save_obj_shared(filename);
    }
}

impl<F: GeometryBinding> Drop for Scene<F> {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplSceneRelease(&raw mut self.inner) }
    }
}

unsafe impl<F: GeometryBinding> Send for Scene<F> {}
unsafe impl<F: GeometryBinding> Sync for Scene<F> {}

impl<F: GeometryBinding> Clone for Scene<F> {
    /// Retains an additional reference to the scene.
    ///
    /// The returned [`Scene`] shares the same underlying Steam Audio object.
    fn clone(&self) -> Self {
        // SAFETY: The scene will not be destroyed until all references are released.
        Self {
            inner: unsafe { audionimbus_sys::iplSceneRetain(self.inner) },
            shared: Arc::clone(&self.shared),
            binding: self.binding,
        }
    }
}

impl<F: GeometryBinding> PartialEq for Scene<F> {
    fn eq(&self, other: &Self) -> bool {
        self.raw_ptr_shared() == other.raw_ptr_shared()
    }
}

impl<F: GeometryBinding> Eq for Scene<F> {}

impl<F: GeometryBinding> Hash for Scene<F> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::ptr::hash(self.raw_ptr_shared(), state);
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
/// This handle is returned when adding a static mesh to a scene via [`add_static_mesh`](crate::Scene).
/// It can be used to reference the mesh for operations like removal.
///
/// # Example
///
/// ```
/// # use audionimbus::*;
/// # let context = Context::default();
/// # let mut scene = Scene::try_new(&context)?;
/// # let vertices = vec![
/// #     Point::new(0.0, 0.0, 0.0),
/// #     Point::new(1.0, 0.0, 0.0),
/// #     Point::new(0.0, 1.0, 0.0),
/// # ];
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
/// This handle is returned when adding an instanced mesh to a scene via [`add_instanced_mesh`](crate::Scene).
/// It can be used to reference the mesh for operations like removal or transform updates.
///
/// # Example
///
/// ```
/// # use audionimbus::*;
/// # let context = Context::default();
/// # let mut sub_scene = Scene::try_new(&context)?;
/// # let vertices = vec![
/// #     Point::new(0.0, 0.0, 0.0),
/// #     Point::new(1.0, 0.0, 0.0),
/// #     Point::new(0.0, 1.0, 0.0),
/// # ];
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

#[cfg(feature = "dynamic")]
impl SceneShared<Dynamic<RayTracerKind>> {
    /// Returns whether Radeon Rays resources match the supplied simulator devices.
    pub fn uses_devices(
        &self,
        open_cl_device: Option<&OpenClDevice>,
        radeon_rays_device: Option<&RadeonRaysDevice>,
    ) -> bool {
        match &self._resources {
            DynamicSceneResources::RadeonRays {
                open_cl_device: scene_open_cl_device,
                radeon_rays_device: scene_radeon_rays_device,
            } => {
                open_cl_device == Some(scene_open_cl_device)
                    && radeon_rays_device == Some(scene_radeon_rays_device)
            }
            _ => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AnyHitCallback, AudioSettings, BatchedAnyHitCallback, BatchedClosestHitCallback,
        ClosestHitCallback, CustomRayTracingCallbacks, Direct, DirectSimulationSettings,
        InstancedMesh, InstancedMeshSettings, Matrix4, SimulationSettings, Simulator, Vector3,
    };

    fn registration_ref_count<D, R, P, RE>(
        scene: &crate::Scene<DefaultRayTracer>,
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
        assert!(crate::Scene::try_new(&context).is_ok());
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

        assert!(crate::Scene::<CustomRayTracer>::try_with_custom(&context, callbacks).is_ok());
    }

    #[test]
    fn test_scene_clone() {
        let context = Context::default();
        let scene = crate::Scene::<DefaultRayTracer>::try_new(&context).unwrap();
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
        let mut root_scene = crate::Scene::try_new(&context).unwrap();
        let sub_scene = crate::Scene::try_new(&context).unwrap();

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
        let mut root_scene = crate::Scene::try_new(&context).unwrap();
        let shared_sub_scene = crate::Scene::try_new(&context).unwrap();

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

        let mut old_root_scene = crate::Scene::try_new(&context).unwrap();
        let old_sub_scene = crate::Scene::try_new(&context).unwrap();
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

        let mut new_root_scene = crate::Scene::try_new(&context).unwrap();
        let new_sub_scene = crate::Scene::try_new(&context).unwrap();
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
