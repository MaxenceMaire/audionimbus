use crate::context::Context;
use crate::error::{to_option_error, SteamAudioError};

/// Application-wide state for the Embree ray tracer.
///
/// An Embree device must be created before using any of Steam Audio’s Embree ray tracing functionality.
#[derive(Debug)]
pub struct EmbreeDevice(pub audionimbus_sys::IPLEmbreeDevice);

impl EmbreeDevice {
    pub fn new(context: &Context) -> Result<Self, SteamAudioError> {
        let embree_device = unsafe {
            let embree_device: *mut audionimbus_sys::IPLEmbreeDevice = std::ptr::null_mut();
            let embree_device_settings: *mut audionimbus_sys::IPLEmbreeDeviceSettings =
                std::ptr::null_mut();
            let status = audionimbus_sys::iplEmbreeDeviceCreate(
                context.raw_ptr(),
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

impl Drop for EmbreeDevice {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplEmbreeDeviceRelease(&mut self.0) }
    }
}
