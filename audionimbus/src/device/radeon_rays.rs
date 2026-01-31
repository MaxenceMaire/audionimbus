use super::open_cl::OpenClDevice;
use crate::error::{to_option_error, SteamAudioError};

/// Application-wide state for the Radeon Rays ray tracer.
///
/// A Radeon Rays device must be created before using any of Steam Audio’s Radeon Rays ray tracing functionality.
#[derive(Debug)]
pub struct RadeonRaysDevice(audionimbus_sys::IPLRadeonRaysDevice);

impl RadeonRaysDevice {
    /// Creates a new Radeon Rays device for GPU-accelerated ray tracing.
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
    pub fn raw_ptr(&self) -> audionimbus_sys::IPLRadeonRaysDevice {
        self.0
    }

    /// Returns a mutable reference to the raw FFI pointer.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLRadeonRaysDevice {
        &mut self.0
    }
}

impl Clone for RadeonRaysDevice {
    fn clone(&self) -> Self {
        unsafe {
            audionimbus_sys::iplRadeonRaysDeviceRetain(self.0);
        }
        Self(self.0)
    }
}

impl Drop for RadeonRaysDevice {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplRadeonRaysDeviceRelease(&mut self.0) }
    }
}

unsafe impl Send for RadeonRaysDevice {}
unsafe impl Sync for RadeonRaysDevice {}
