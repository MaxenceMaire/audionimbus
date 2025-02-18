use crate::context::Context;
use crate::error::{to_option_error, SteamAudioError};

#[cfg(doc)]
use crate::geometry::Scene;
#[cfg(doc)]
use crate::probe::ProbeBatch;

/// A serialized representation of an API object, like a [`Scene`] or [`ProbeBatch`].
///
/// Create an empty serialized object if you want to serialize an existing object to a byte array, or create a serialized object that wraps an existing byte array if you want to deserialize it.
#[derive(Debug)]
pub struct SerializedObject(pub(crate) audionimbus_sys::IPLSerializedObject);

impl SerializedObject {
    pub fn try_new(context: &Context) -> Result<Self, SteamAudioError> {
        let serialized_object_settings = audionimbus_sys::IPLSerializedObjectSettings {
            data: std::ptr::null_mut(),
            size: 0,
        };

        Self::try_with_settings(context, serialized_object_settings)
    }

    pub fn try_with_buffer(
        context: &Context,
        buffer: &mut Vec<u8>,
    ) -> Result<Self, SteamAudioError> {
        let serialized_object_settings = audionimbus_sys::IPLSerializedObjectSettings {
            data: buffer.as_mut_ptr() as *mut audionimbus_sys::IPLbyte,
            size: buffer.len(),
        };

        Self::try_with_settings(context, serialized_object_settings)
    }

    fn try_with_settings(
        context: &Context,
        mut serialized_object_settings: audionimbus_sys::IPLSerializedObjectSettings,
    ) -> Result<Self, SteamAudioError> {
        let mut serialized_object = Self(std::ptr::null_mut());

        let status = unsafe {
            audionimbus_sys::iplSerializedObjectCreate(
                context.raw_ptr(),
                &mut serialized_object_settings,
                serialized_object.raw_ptr_mut(),
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        Ok(serialized_object)
    }

    pub fn raw_ptr(&self) -> audionimbus_sys::IPLSerializedObject {
        self.0
    }

    pub fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLSerializedObject {
        &mut self.0
    }

    pub fn to_vec(&self) -> Vec<u8> {
        let raw_ptr = self.raw_ptr();

        let data_ptr = unsafe { audionimbus_sys::iplSerializedObjectGetData(raw_ptr) };

        let size = unsafe { audionimbus_sys::iplSerializedObjectGetSize(raw_ptr) } as usize;

        let data_slice = unsafe { std::slice::from_raw_parts(data_ptr, size) };

        data_slice.to_vec()
    }
}

impl Clone for SerializedObject {
    fn clone(&self) -> Self {
        unsafe {
            audionimbus_sys::iplSerializedObjectRetain(self.0);
        }
        Self(self.0)
    }
}

impl Drop for SerializedObject {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplSerializedObjectRelease(&mut self.0) }
    }
}

unsafe impl Send for SerializedObject {}
unsafe impl Sync for SerializedObject {}
