use crate::context::Context;
use crate::error::{to_option_error, SteamAudioError};
use crate::open_cl_device::OpenClDevice;

/// A 3D scene, which can contain geometry objects that can interact with acoustic rays.
///
/// The scene object itself doesn’t contain any geometry, but is a container for [`StaticMesh`] and [`InstancedMesh`] objects, which do contain geometry.
#[derive(Debug)]
pub struct Scene(pub audionimbus_sys::IPLScene);

impl Scene {
    pub fn try_new(
        context: Context,
        scene_settings: &SceneSettings<()>,
    ) -> Result<Self, SteamAudioError> {
        let scene = unsafe {
            let scene: *mut audionimbus_sys::IPLScene = std::ptr::null_mut();
            let status = audionimbus_sys::iplSceneCreate(
                *context,
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

#[derive(Debug)]
pub struct EmbreeDevice(pub audionimbus_sys::IPLEmbreeDevice);

impl EmbreeDevice {
    pub fn new(context: Context) -> Result<Self, SteamAudioError> {
        let embree_device = unsafe {
            let embree_device: *mut audionimbus_sys::IPLEmbreeDevice = std::ptr::null_mut();
            let embree_device_settings: *mut audionimbus_sys::IPLEmbreeDeviceSettings =
                std::ptr::null_mut();
            let status = audionimbus_sys::iplEmbreeDeviceCreate(
                *context,
                embree_device_settings,
                embree_device,
            );

            if let Some(error) = to_option_error(status) {
                return Err(error);
            }

            *embree_device
        };

        Ok(Self(embree_device))
    }
}

impl std::ops::Deref for EmbreeDevice {
    type Target = audionimbus_sys::IPLEmbreeDevice;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for EmbreeDevice {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Drop for EmbreeDevice {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplEmbreeDeviceRelease(&mut self.0) }
    }
}

#[derive(Debug)]
pub struct RadeonRaysDevice(pub audionimbus_sys::IPLRadeonRaysDevice);

impl RadeonRaysDevice {
    pub fn new(open_cl_device: OpenClDevice) -> Result<Self, SteamAudioError> {
        let radeon_rays_device = unsafe {
            let radeon_rays_device: *mut audionimbus_sys::IPLRadeonRaysDevice =
                std::ptr::null_mut();
            let radeon_rays_device_settings: *mut audionimbus_sys::IPLEmbreeDeviceSettings =
                std::ptr::null_mut();
            let status = audionimbus_sys::iplRadeonRaysDeviceCreate(
                *open_cl_device,
                radeon_rays_device_settings,
                radeon_rays_device,
            );

            if let Some(error) = to_option_error(status) {
                return Err(error);
            }

            *radeon_rays_device
        };

        Ok(Self(radeon_rays_device))
    }
}

impl std::ops::Deref for RadeonRaysDevice {
    type Target = audionimbus_sys::IPLRadeonRaysDevice;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for RadeonRaysDevice {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Drop for RadeonRaysDevice {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplRadeonRaysDeviceRelease(&mut self.0) }
    }
}
