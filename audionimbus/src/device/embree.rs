use crate::context::Context;
use crate::error::{to_option_error, SteamAudioError};

/// Application-wide state for the Embree ray tracer.
///
/// An Embree device must be created before using any of Steam Audio’s Embree ray tracing functionality.
#[derive(Debug)]
pub struct EmbreeDevice(audionimbus_sys::IPLEmbreeDevice);

impl EmbreeDevice {
    pub fn new(context: &Context) -> Result<Self, SteamAudioError> {
        let mut embree_device = Self(std::ptr::null_mut());

        let embree_device_settings: *mut audionimbus_sys::IPLEmbreeDeviceSettings =
            std::ptr::null_mut();

        let status = unsafe {
            audionimbus_sys::iplEmbreeDeviceCreate(
                context.raw_ptr(),
                embree_device_settings,
                embree_device.raw_ptr_mut(),
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        Ok(embree_device)
    }

    pub fn null() -> Self {
        Self(std::ptr::null_mut())
    }

    pub fn raw_ptr(&self) -> audionimbus_sys::IPLEmbreeDevice {
        self.0
    }

    pub fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLEmbreeDevice {
        &mut self.0
    }
}

impl Clone for EmbreeDevice {
    fn clone(&self) -> Self {
        unsafe {
            audionimbus_sys::iplEmbreeDeviceRetain(self.0);
        }
        Self(self.0)
    }
}

impl Drop for EmbreeDevice {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplEmbreeDeviceRelease(&mut self.0) }
    }
}

unsafe impl Send for EmbreeDevice {}
unsafe impl Sync for EmbreeDevice {}
