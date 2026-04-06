use crate::context::Context;
use crate::error::{SteamAudioError, to_option_error};
use std::hash::{Hash, Hasher};

#[cfg(doc)]
use crate::geometry::Scene;
#[cfg(doc)]
use crate::probe::ProbeBatch;

/// A serialized representation of an API object, like a [`Scene`] or [`ProbeBatch`].
///
/// Create an empty serialized object if you want to serialize an existing object to a byte array, or create a serialized object that wraps an existing byte array if you want to deserialize it.
///
/// `SerializedObject` is a reference-counted handle to an underlying Steam Audio object.
/// Cloning it is cheap; it produces a new handle pointing to the same underlying object, while
/// incrementing a reference count.
/// The underlying object is destroyed when all handles are dropped.
#[derive(Debug, PartialEq, Eq)]
pub struct SerializedObject(pub(crate) audionimbus_sys::IPLSerializedObject);

impl SerializedObject {
    /// Creates a new empty serialized object for serialization purposes and returns a handle to it.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying Steam Audio library fails to create the serialized object.
    ///
    /// # Examples
    ///
    /// ```
    /// # use audionimbus::{Context, SerializedObject, SteamAudioError};
    /// let context = Context::default();
    /// let serialized_object = SerializedObject::try_new(&context)?;
    /// # Ok::<(), audionimbus::SteamAudioError>(())
    /// ```
    pub fn try_new(context: &Context) -> Result<Self, SteamAudioError> {
        let serialized_object_settings = audionimbus_sys::IPLSerializedObjectSettings {
            data: std::ptr::null_mut(),
            size: 0,
        };

        Self::try_with_settings(context, serialized_object_settings)
    }

    /// Creates a serialized object that wraps an existing byte buffer for deserialization and
    /// returns a handle to it.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying Steam Audio library fails to create the serialized
    /// object or if the buffer contains invalid data.
    ///
    /// # Examples
    ///
    /// ```
    /// # use audionimbus::{Context, SerializedObject, SteamAudioError};
    /// let context = Context::default();
    /// let mut buffer = vec![0u8; 1024]; // Load your serialized data here.
    /// let serialized_object = SerializedObject::try_with_buffer(&context, &mut buffer)?;
    /// # Ok::<(), audionimbus::SteamAudioError>(())
    /// ```
    pub fn try_with_buffer(
        context: &Context,
        buffer: &mut Vec<u8>,
    ) -> Result<Self, SteamAudioError> {
        let serialized_object_settings = audionimbus_sys::IPLSerializedObjectSettings {
            data: buffer.as_mut_ptr().cast::<audionimbus_sys::IPLbyte>(),
            size: buffer.len(),
        };

        Self::try_with_settings(context, serialized_object_settings)
    }

    /// Creates a serialized object with the given settings and returns a handle to it.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying Steam Audio library fails to create the object.
    fn try_with_settings(
        context: &Context,
        mut serialized_object_settings: audionimbus_sys::IPLSerializedObjectSettings,
    ) -> Result<Self, SteamAudioError> {
        let mut serialized_object = Self(std::ptr::null_mut());

        let status = unsafe {
            audionimbus_sys::iplSerializedObjectCreate(
                context.raw_ptr(),
                &raw mut serialized_object_settings,
                serialized_object.raw_ptr_mut(),
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        Ok(serialized_object)
    }

    /// Returns the raw FFI pointer to the underlying object.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub const fn raw_ptr(&self) -> audionimbus_sys::IPLSerializedObject {
        self.0
    }

    /// Returns a mutable reference to the raw FFI pointer.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub const fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLSerializedObject {
        &mut self.0
    }

    /// Extracts the serialized data as a byte vector.
    ///
    /// This method retrieves the underlying serialized data and copies it into a new
    /// `Vec<u8>`. Use this to extract serialized data that can be saved to a file or
    /// transmitted over the network.
    ///
    /// # Returns
    ///
    /// A `Vec<u8>` containing a copy of the serialized data.
    ///
    /// # Examples
    ///
    /// ```
    /// # use audionimbus::{Context, SerializedObject, Scene};
    /// # let context = Context::default();
    /// # let serialized_object = SerializedObject::try_new(&context)?;
    /// // After serializing some object into serialized_object...
    /// let bytes = serialized_object.to_vec();
    /// # Ok::<(), audionimbus::SteamAudioError>(())
    /// ```
    pub fn to_vec(&self) -> Vec<u8> {
        let raw_ptr = self.raw_ptr();

        let data_ptr = unsafe { audionimbus_sys::iplSerializedObjectGetData(raw_ptr) };

        let size = unsafe { audionimbus_sys::iplSerializedObjectGetSize(raw_ptr) } as usize;

        if data_ptr.is_null() || size == 0 {
            return Vec::new();
        }

        let data_slice = unsafe { std::slice::from_raw_parts(data_ptr, size) };

        data_slice.to_vec()
    }
}

impl Drop for SerializedObject {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplSerializedObjectRelease(&raw mut self.0) }
    }
}

unsafe impl Send for SerializedObject {}
unsafe impl Sync for SerializedObject {}

impl Clone for SerializedObject {
    /// Retains an additional reference to the serialized object.
    ///
    /// The returned [`SerializedObject`] shares the same underlying Steam Audio object.
    fn clone(&self) -> Self {
        // SAFETY: The serialized object will not be destroyed until all references are released.
        Self(unsafe { audionimbus_sys::iplSerializedObjectRetain(self.0) })
    }
}

impl Hash for SerializedObject {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::ptr::hash(self.raw_ptr(), state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_try_new() {
        let context = Context::default();
        let serialized_object = SerializedObject::try_new(&context);
        assert!(serialized_object.is_ok());
    }

    #[test]
    fn test_try_with_buffer() {
        let context = Context::default();
        let mut buffer = vec![0u8; 1024];
        let serialized_object = SerializedObject::try_with_buffer(&context, &mut buffer);
        assert!(serialized_object.is_ok());
    }

    #[test]
    fn test_clone() {
        let context = Context::default();
        let serialized_object = SerializedObject::try_new(&context).unwrap();
        let clone = serialized_object.clone();
        assert_eq!(serialized_object.raw_ptr(), clone.raw_ptr());
        drop(serialized_object);
        assert!(!clone.raw_ptr().is_null());
    }
}
