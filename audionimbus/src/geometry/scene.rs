use super::{InstancedMesh, Matrix, StaticMesh};
use crate::callback::{CallbackInformation, ProgressCallback};
use crate::context::Context;
use crate::device::embree::EmbreeDevice;
use crate::device::open_cl::OpenClDevice;
use crate::device::radeon_rays::RadeonRaysDevice;
use crate::error::{to_option_error, SteamAudioError};
use crate::geometry::{Direction, Point};
use crate::serialized_object::SerializedObject;

/// A 3D scene, which can contain geometry objects that can interact with acoustic rays.
///
/// The scene object itself doesn’t contain any geometry, but is a container for [`StaticMesh`] and [`InstancedMesh`] objects, which do contain geometry.
#[derive(Debug)]
pub struct Scene {
    inner: audionimbus_sys::IPLScene,

    /// Used for validation when calling [`Self::save`].
    uses_default_ray_tracer: bool,
}

impl Scene {
    pub fn try_new(context: &Context, settings: &SceneSettings) -> Result<Self, SteamAudioError> {
        let mut scene = Self {
            inner: std::ptr::null_mut(),
            uses_default_ray_tracer: matches!(settings, SceneSettings::Default),
        };

        let status = unsafe {
            audionimbus_sys::iplSceneCreate(
                context.raw_ptr(),
                &mut audionimbus_sys::IPLSceneSettings::from(settings),
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
        settings: &SceneSettings,
        serialized_object: &SerializedObject,
    ) -> Result<Self, SteamAudioError> {
        let mut scene = Self {
            inner: std::ptr::null_mut(),
            uses_default_ray_tracer: matches!(settings, SceneSettings::Default),
        };

        let status = unsafe {
            audionimbus_sys::iplSceneLoad(
                context.raw_ptr(),
                &mut audionimbus_sys::IPLSceneSettings::from(settings),
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
        settings: &SceneSettings,
        serialized_object: &SerializedObject,
        progress_callback: CallbackInformation<ProgressCallback>,
    ) -> Result<Self, SteamAudioError> {
        let mut scene = Self {
            inner: std::ptr::null_mut(),
            uses_default_ray_tracer: matches!(settings, SceneSettings::Default),
        };

        let status = unsafe {
            audionimbus_sys::iplSceneLoad(
                context.raw_ptr(),
                &mut audionimbus_sys::IPLSceneSettings::from(settings),
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

    /// Adds a static mesh to a scene.
    ///
    /// After calling this function, [`Self::commit`] must be called for the changes to take effect.
    pub fn add_static_mesh(&mut self, static_mesh: &StaticMesh) {
        unsafe {
            audionimbus_sys::iplStaticMeshAdd(static_mesh.raw_ptr(), self.raw_ptr());
        }
    }

    /// Removes a static mesh from a scene.
    ///
    /// After calling this function, [`Self::commit`] must be called for the changes to take effect.
    pub fn remove_static_mesh(&mut self, static_mesh: &StaticMesh) {
        unsafe {
            audionimbus_sys::iplStaticMeshRemove(static_mesh.raw_ptr(), self.raw_ptr());
        }
    }

    /// Adds an instanced mesh to a scene.
    ///
    /// After calling this function, [`Self::commit`] must be called for the changes to take effect.
    pub fn add_instanced_mesh(&mut self, instanced_mesh: &InstancedMesh) {
        unsafe {
            audionimbus_sys::iplInstancedMeshAdd(instanced_mesh.raw_ptr(), self.raw_ptr());
        }
    }

    /// Removes an instanced mesh from a scene.
    ///
    /// After calling this function, [`Self::commit`] must be called for the changes to take effect.
    pub fn remove_instanced_mesh(&mut self, instanced_mesh: &InstancedMesh) {
        unsafe {
            audionimbus_sys::iplInstancedMeshRemove(instanced_mesh.raw_ptr(), self.raw_ptr());
        }
    }

    /// Updates the local-to-world transform of an instanced mesh within its parent scene.
    ///
    /// This function allows the instanced mesh to be moved, rotated, and scaled dynamically.
    ///
    /// After calling this function, [`Self::commit`] must be called for the changes to take effect.
    pub fn update_instanced_mesh_transform(
        &mut self,
        instanced_mesh: &InstancedMesh,
        transform: &Matrix<f32, 4, 4>,
    ) {
        unsafe {
            audionimbus_sys::iplInstancedMeshUpdateTransform(
                instanced_mesh.raw_ptr(),
                self.raw_ptr(),
                transform.into(),
            );
        }
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
    pub fn commit(&mut self) {
        unsafe {
            audionimbus_sys::iplSceneCommit(self.raw_ptr());
        }
    }

    /// Saves a scene to a serialized object.
    ///
    /// Typically, the serialized object will then be saved to disk.
    ///
    /// This function can only be called on a scene created with [`SceneSettings::Default`].
    pub fn save(&self) -> SerializedObject {
        assert!(self.uses_default_ray_tracer);

        let serialized_object = SerializedObject(std::ptr::null_mut());

        unsafe {
            audionimbus_sys::iplSceneSave(self.raw_ptr(), serialized_object.raw_ptr());
        }

        serialized_object
    }

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

    pub fn raw_ptr(&self) -> audionimbus_sys::IPLScene {
        self.inner
    }

    pub fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLScene {
        &mut self.inner
    }
}

impl Clone for Scene {
    fn clone(&self) -> Self {
        unsafe {
            audionimbus_sys::iplSceneRetain(self.inner);
        }
        Self {
            inner: self.inner,
            uses_default_ray_tracer: self.uses_default_ray_tracer,
        }
    }
}

impl Drop for Scene {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplSceneRelease(&mut self.inner) }
    }
}

unsafe impl Send for Scene {}
unsafe impl Sync for Scene {}

/// Settings used to create a scene.
///
/// Each scene variant corresponds to a different ray tracing implementation.
#[derive(Debug)]
pub enum SceneSettings {
    /// Steam Audio’s built-in ray tracer.
    ///
    /// Supports multi-threading. Runs on all platforms that Steam Audio supports.
    Default,

    /// The Intel Embree ray tracer.
    ///
    /// Supports multi-threading.
    /// This is a highly optimized implementation, and is likely to be faster than the default ray tracer.
    /// However, Embree requires Windows, Linux, or macOS, and a 32-bit x86 or 64-bit x86_64 CPU.
    Embree {
        /// Handle to an Embree device.
        device: EmbreeDevice,
    },

    /// The AMD Radeon Rays ray tracer.
    ///
    /// This is an OpenCL implementation, and can use either the CPU or any GPU that supports OpenCL 1.2 or later.
    /// If using the GPU, it is likely to be significantly faster than the default ray tracer.
    /// However, with heavy real-time simulation workloads, it may impact your application’s frame rate.
    /// On supported AMD GPUs, you can use the Resource Reservation feature to mitigate this issue.
    RadeonRays {
        /// Handle to a Radeon Rays device.
        device: RadeonRaysDevice,
    },

    /// Allows you to specify callbacks to your own ray tracer.
    ///
    /// Useful if your application already uses a high-performance ray tracer.
    /// This option uses the least amount of memory at run-time, since it does not have to build any ray tracing data structures of its own.
    Custom {
        /// Callback for finding the closest hit along a ray.
        closest_hit_callback: unsafe extern "C" fn(
            ray: *const audionimbus_sys::IPLRay,
            min_distance: f32,
            max_distance: f32,
            hit: *mut audionimbus_sys::IPLHit,
            user_data: *mut std::ffi::c_void,
        ),

        /// Callback for finding whether a ray hits anything.
        any_hit_callback: unsafe extern "C" fn(
            ray: *const audionimbus_sys::IPLRay,
            min_distance: f32,
            max_distance: f32,
            occluded: *mut u8,
            user_data: *mut std::ffi::c_void,
        ),

        /// Callback for finding the closest hit along a batch of rays.
        batched_closest_hit_callback: unsafe extern "C" fn(
            num_rays: i32,
            rays: *const audionimbus_sys::IPLRay,
            min_distances: *const f32,
            max_distances: *const f32,
            hits: *mut audionimbus_sys::IPLHit,
            user_data: *mut std::ffi::c_void,
        ),

        /// Callback for finding whether a batch of rays hits anything.
        batched_any_hit_callback: unsafe extern "C" fn(
            num_rays: i32,
            rays: *const audionimbus_sys::IPLRay,
            min_distances: *const f32,
            max_distances: *const f32,
            occluded: *mut u8,
            user_data: *mut std::ffi::c_void,
        ),

        /// Arbitrary user-provided data for use by ray tracing callbacks.
        user_data: *mut std::ffi::c_void,
    },
}

impl Default for SceneSettings {
    fn default() -> Self {
        Self::Default
    }
}

impl From<&SceneSettings> for audionimbus_sys::IPLSceneSettings {
    fn from(scene_settings: &SceneSettings) -> Self {
        let (
            type_,
            closest_hit_callback,
            any_hit_callback,
            batched_closest_hit_callback,
            batched_any_hit_callback,
            user_data,
            embree_device,
            radeon_rays_device,
        ) = match scene_settings {
            SceneSettings::Default => (
                audionimbus_sys::IPLSceneType::IPL_SCENETYPE_DEFAULT,
                None,
                None,
                None,
                None,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            ),
            SceneSettings::Embree { device } => (
                audionimbus_sys::IPLSceneType::IPL_SCENETYPE_EMBREE,
                None,
                None,
                None,
                None,
                std::ptr::null_mut(),
                device.raw_ptr(),
                std::ptr::null_mut(),
            ),
            SceneSettings::RadeonRays { device } => (
                audionimbus_sys::IPLSceneType::IPL_SCENETYPE_RADEONRAYS,
                None,
                None,
                None,
                None,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                device.raw_ptr(),
            ),
            SceneSettings::Custom {
                closest_hit_callback,
                any_hit_callback,
                batched_closest_hit_callback,
                batched_any_hit_callback,
                user_data,
            } => (
                audionimbus_sys::IPLSceneType::IPL_SCENETYPE_CUSTOM,
                Some(*closest_hit_callback),
                Some(*any_hit_callback),
                Some(*batched_closest_hit_callback),
                Some(*batched_any_hit_callback),
                *user_data,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            ),
        };

        Self {
            type_,
            closestHitCallback: closest_hit_callback,
            anyHitCallback: any_hit_callback,
            batchedClosestHitCallback: batched_closest_hit_callback,
            batchedAnyHitCallback: batched_any_hit_callback,
            userData: user_data,
            embreeDevice: embree_device,
            radeonRaysDevice: radeon_rays_device,
        }
    }
}

/// The scene parameters.
#[derive(Copy, Clone, Debug)]
pub enum SceneParams<'a> {
    /// Steam Audio’s built-in ray tracer.
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
