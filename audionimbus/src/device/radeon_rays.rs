//! Radeon Rays ray tracing.

use super::open_cl::OpenClDevice;
use crate::error::{SteamAudioError, to_option_error};
use std::hash::{Hash, Hasher};

/// Application-wide state for the Radeon Rays ray tracer.
///
/// A Radeon Rays device must be created before using any of Steam Audio’s Radeon Rays ray tracing functionality.
///
/// `RadeonRaysDevice` is a reference-counted handle to an underlying Steam Audio object.
/// Cloning it is cheap; it produces a new handle pointing to the same underlying object, while
/// incrementing a reference count.
/// The underlying object is destroyed when all handles are dropped.
#[derive(Debug, PartialEq, Eq)]
pub struct RadeonRaysDevice(audionimbus_sys::IPLRadeonRaysDevice);

impl RadeonRaysDevice {
    /// Creates a new Radeon Rays device for GPU-accelerated ray tracing and returns a handle to it.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if device creation fails.
    pub fn try_new(open_cl_device: &OpenClDevice) -> Result<Self, SteamAudioError> {
        let mut radeon_rays_device = Self(std::ptr::null_mut());

        let radeon_rays_device_settings: *mut audionimbus_sys::IPLRadeonRaysDeviceSettings =
            std::ptr::null_mut();

        let status = unsafe {
            audionimbus_sys::iplRadeonRaysDeviceCreate(
                open_cl_device.raw_ptr(),
                radeon_rays_device_settings,
                radeon_rays_device.raw_ptr_mut(),
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        Ok(radeon_rays_device)
    }

    /// Returns the raw FFI pointer to the underlying Radeon Rays device.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub const fn raw_ptr(&self) -> audionimbus_sys::IPLRadeonRaysDevice {
        self.0
    }

    /// Returns a mutable reference to the raw FFI pointer.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub const fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLRadeonRaysDevice {
        &mut self.0
    }
}

impl Drop for RadeonRaysDevice {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplRadeonRaysDeviceRelease(&raw mut self.0) }
    }
}

unsafe impl Send for RadeonRaysDevice {}
unsafe impl Sync for RadeonRaysDevice {}

impl Clone for RadeonRaysDevice {
    /// Retains an additional reference to the Radeon Rays device.
    ///
    /// The returned [`RadeonRaysDevice`] shares the same underlying Steam Audio object.
    fn clone(&self) -> Self {
        // SAFETY: The device will not be destroyed until all references are released.
        Self(unsafe { audionimbus_sys::iplRadeonRaysDeviceRetain(self.0) })
    }
}

impl Hash for RadeonRaysDevice {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::ptr::hash(self.raw_ptr(), state);
    }
}

#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn test_clone() {
        let context = Context::default();
        let settings = OpenClDeviceSettings::default();
        let Ok(device_list) = OpenClDeviceList::try_new(&context, &settings) else {
            // OpenCL not available
            return;
        };
        let open_cl_device = OpenClDevice::try_new(&context, &device_list, 0).unwrap();
        let radeon_rays_device = RadeonRaysDevice::try_new(&open_cl_device).unwrap();
        let clone = radeon_rays_device.clone();
        assert_eq!(radeon_rays_device.raw_ptr(), clone.raw_ptr());
        drop(radeon_rays_device);
        assert!(!clone.raw_ptr().is_null());
    }
}
