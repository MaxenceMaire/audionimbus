use crate::error::{to_option_error, SteamAudioError};
use crate::open_cl::OpenClDevice;

/// Application-wide state for the Radeon Rays ray tracer.
///
/// A Radeon Rays device must be created before using any of Steam Audio’s Radeon Rays ray tracing functionality.
#[derive(Debug)]
pub struct RadeonRaysDevice(audionimbus_sys::IPLRadeonRaysDevice);

impl RadeonRaysDevice {
    pub fn new(open_cl_device: &OpenClDevice) -> Result<Self, SteamAudioError> {
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

    pub fn null() -> Self {
        Self(std::ptr::null_mut())
    }

    pub fn raw_ptr(&self) -> audionimbus_sys::IPLRadeonRaysDevice {
        self.0
    }

    pub fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLRadeonRaysDevice {
        &mut self.0
    }
}

impl Drop for RadeonRaysDevice {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplRadeonRaysDeviceRelease(&mut self.0) }
    }
}
