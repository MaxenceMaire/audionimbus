use crate::error::{to_option_error, SteamAudioError};
use crate::open_cl::OpenClDevice;

/// Application-wide state for the Radeon Rays ray tracer.
///
/// A Radeon Rays device must be created before using any of Steam Audio’s Radeon Rays ray tracing functionality.
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
                open_cl_device.raw_ptr(),
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

    pub fn raw_ptr(&self) -> audionimbus_sys::IPLRadeonRaysDevice {
        self.0
    }
}

impl Drop for RadeonRaysDevice {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplRadeonRaysDeviceRelease(&mut self.0) }
    }
}
