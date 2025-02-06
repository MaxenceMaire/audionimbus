use super::{InstancedMesh, Matrix, StaticMesh};
use crate::context::Context;
use crate::embree::EmbreeDevice;
use crate::error::{to_option_error, SteamAudioError};
use crate::radeon_rays::RadeonRaysDevice;

/// A 3D scene, which can contain geometry objects that can interact with acoustic rays.
///
/// The scene object itself doesn’t contain any geometry, but is a container for [`StaticMesh`] and [`InstancedMesh`] objects, which do contain geometry.
#[derive(Debug)]
pub struct Scene(audionimbus_sys::IPLScene);

impl Scene {
    pub fn try_new(
        context: &Context,
        scene_settings: &SceneSettings<()>,
    ) -> Result<Self, SteamAudioError> {
        let scene = unsafe {
            let scene: *mut audionimbus_sys::IPLScene = std::ptr::null_mut();
            let status = audionimbus_sys::iplSceneCreate(
                context.raw_ptr(),
                &mut audionimbus_sys::IPLSceneSettings::from(scene_settings),
                scene,
            );

            if let Some(error) = to_option_error(status) {
                return Err(error);
            }

            *scene
        };

        Ok(Self(scene))
    }

    /// Adds a static mesh to a scene.
    ///
    /// After calling this function, [`Self::commit`] must be called for the changes to take effect.
    pub fn add_static_mesh(&self, static_mesh: &StaticMesh) {
        unsafe {
            audionimbus_sys::iplStaticMeshAdd(static_mesh.raw_ptr(), self.raw_ptr());
        }
    }

    /// Removes a static mesh from a scene.
    ///
    /// After calling this function, [`Self::commit`] must be called for the changes to take effect.
    pub fn remove_static_mesh(&self, static_mesh: &StaticMesh) {
        unsafe {
            audionimbus_sys::iplStaticMeshRemove(static_mesh.raw_ptr(), self.raw_ptr());
        }
    }

    /// Adds an instanced mesh to a scene.
    ///
    /// After calling this function, [`Self::commit`] must be called for the changes to take effect.
    pub fn add_instanced_mesh(&self, instanced_mesh: &InstancedMesh) {
        unsafe {
            audionimbus_sys::iplInstancedMeshAdd(instanced_mesh.raw_ptr(), self.raw_ptr());
        }
    }

    /// Removes an instanced mesh from a scene.
    ///
    /// After calling this function, [`Self::commit`] must be called for the changes to take effect.
    pub fn remove_instanced_mesh(&self, instanced_mesh: &InstancedMesh) {
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
        &self,
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
    pub fn commit(&self) {
        unsafe {
            audionimbus_sys::iplSceneCommit(self.raw_ptr());
        }
    }

    pub fn raw_ptr(&self) -> audionimbus_sys::IPLScene {
        self.0
    }
}

/// Settings used to create a scene.
///
/// Each scene variant corresponds to a different ray tracing implementation.
#[derive(Debug)]
pub enum SceneSettings<T> {
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
        closest_hit_callback: Option<()>, // TODO: should take a function.

        /// Callback for finding whether a ray hits anything.
        any_hit_callback: Option<()>, // TODO: should take a function.

        /// Callback for finding the closest hit along a batch of rays.
        batched_closest_hit_callback: Option<()>, // TODO: should take a function.

        /// Callback for finding whether a batch of rays hits anything.
        batched_any_hit_callback: Option<()>, // TODO: should take a function.

        /// Arbitrary user-provided data for use by ray tracing callbacks.
        user_data: T, // TODO: specify generic.
    },
}

impl<T> Default for SceneSettings<T> {
    fn default() -> Self {
        Self::Default
    }
}

impl From<&SceneSettings<()>> for audionimbus_sys::IPLSceneSettings {
    fn from(settings: &SceneSettings<()>) -> Self {
        todo!()
    }
}

/// The types of scenes that can be created. Each scene type corresponds to a different ray tracing implementation.
#[derive(Debug)]
pub enum SceneType {
    /// Steam Audio’s built-in ray tracer.
    /// Supports multi-threading.
    /// Runs on all platforms that Steam Audio supports.
    Default,

    /// The Intel Embree ray tracer.
    /// Supports multi-threading.
    /// This is a highly optimized implementation, and is likely to be faster than the default ray tracer.
    /// However, Embree requires Windows, Linux, or macOS, and a 32-bit x86 or 64-bit x86_64 CPU.
    Embree,

    /// The AMD Radeon Rays ray tracer.
    /// This is an OpenCL implementation, and can use either the CPU or any GPU that supports OpenCL 1.2 or later.
    /// If using the GPU, it is likely to be significantly faster than the default ray tracer.
    /// However, with heavy real-time simulation workloads, it may impact your application’s frame rate.
    /// On supported AMD GPUs, you can use the Resource Reservation feature to mitigate this issue.
    RadeonRays,

    /// Allows you to specify callbacks to your own ray tracer.
    /// Useful if your application already uses a high-performance ray tracer.
    /// This option uses the least amount of memory at run-time, since it does not have to build any ray tracing data structures of its own.
    Custom,
}
