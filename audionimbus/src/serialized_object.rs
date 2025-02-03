use crate::context::Context;
use crate::error::{to_option_error, SteamAudioError};

/// A serialized representation of an API object, like an IPLScene or IPLProbeBatch.
///
/// Create an empty serialized object if you want to serialize an existing object to a byte array, or create a serialized object that wraps an existing byte array if you want to deserialize it.
#[derive(Debug)]
pub struct SerializedObject(audionimbus_sys::IPLSerializedObject);

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
        let serialized_object = unsafe {
            let serialized_object: *mut audionimbus_sys::IPLSerializedObject = std::ptr::null_mut();
            let status = audionimbus_sys::iplSerializedObjectCreate(
                context.as_raw_ptr(),
                &mut serialized_object_settings,
                serialized_object,
            );

            if let Some(error) = to_option_error(status) {
                return Err(error);
            }

            *serialized_object
        };

        Ok(Self(serialized_object))
    }

    pub fn as_raw_ptr(&self) -> audionimbus_sys::IPLSerializedObject {
        self.0
    }

    pub fn to_vec(&self) -> Vec<u8> {
        let raw_ptr = self.as_raw_ptr();

        let data_ptr = unsafe { audionimbus_sys::iplSerializedObjectGetData(raw_ptr) };

        let size = unsafe { audionimbus_sys::iplSerializedObjectGetSize(raw_ptr) } as usize;

        let data_slice = unsafe { std::slice::from_raw_parts(data_ptr, size) };

        data_slice.to_vec()
    }
}

impl Drop for SerializedObject {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplSerializedObjectRelease(&mut self.0) }
    }
}
