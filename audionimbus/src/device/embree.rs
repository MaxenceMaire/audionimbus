//! Embree ray tracing.

use crate::context::Context;
use crate::error::{SteamAudioError, to_option_error};
use std::hash::{Hash, Hasher};

/// Application-wide state for the Embree ray tracer.
///
/// An Embree device must be created before using any of Steam Audio’s Embree ray tracing functionality.
///
/// `EmbreeDevice` is a reference-counted handle to an underlying Steam Audio object.
/// Cloning it is cheap; it produces a new handle pointing to the same underlying object, while
/// incrementing a reference count.
/// The underlying object is destroyed when all handles are dropped.
#[derive(Debug, PartialEq, Eq)]
pub struct EmbreeDevice(audionimbus_sys::IPLEmbreeDevice);

impl EmbreeDevice {
    /// Creates a new Embree device for ray tracing and returns a handle to it.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if device creation fails.
    pub fn try_new(context: &Context) -> Result<Self, SteamAudioError> {
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

    /// Returns the raw FFI pointer to the underlying Embree device.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub const fn raw_ptr(&self) -> audionimbus_sys::IPLEmbreeDevice {
        self.0
    }

    /// Returns a mutable reference to the raw FFI pointer.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub const fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLEmbreeDevice {
        &mut self.0
    }
}

impl Drop for EmbreeDevice {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplEmbreeDeviceRelease(&raw mut self.0) }
    }
}

unsafe impl Send for EmbreeDevice {}
unsafe impl Sync for EmbreeDevice {}

impl Clone for EmbreeDevice {
    /// Retains an additional reference to the Embree device.
    ///
    /// The returned [`EmbreeDevice`] shares the same underlying Steam Audio object.
    fn clone(&self) -> Self {
        // SAFETY: The device will not be destroyed until all references are released.
        Self(unsafe { audionimbus_sys::iplEmbreeDeviceRetain(self.0) })
    }
}

impl Hash for EmbreeDevice {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::ptr::hash(self.raw_ptr(), state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clone() {
        let context = Context::default();
        let Ok(device) = EmbreeDevice::try_new(&context) else {
            // Device not available
            return;
        };
        let clone = device.clone();
        assert_eq!(device.raw_ptr(), clone.raw_ptr());
        drop(device);
        assert!(!clone.raw_ptr().is_null());
    }
}
