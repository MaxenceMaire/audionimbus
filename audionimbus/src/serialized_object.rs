use crate::context::Context;
use crate::error::{SteamAudioError, to_option_error};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

#[cfg(doc)]
use crate::geometry::Scene;
#[cfg(doc)]
use crate::probe::ProbeBatch;

/// A serialized representation of an API object, like a [`Scene`] or [`ProbeBatch`].
///
/// Create one from owned bytes to deserialize an API object.
/// Use `Scene::try_save`, `StaticMesh::try_save`, or `ProbeBatch::try_save` to serialize those
/// objects.
///
/// `SerializedObject` is a reference-counted handle to an underlying Steam Audio object.
/// Cloning it is cheap; it produces a new handle pointing to the same underlying object, while
/// incrementing a reference count.
/// The underlying object is destroyed when all handles are dropped.
#[derive(Debug)]
pub struct SerializedObject {
    raw: audionimbus_sys::IPLSerializedObject,
    input_bytes: Option<Arc<Vec<u8>>>,
}

impl SerializedObject {
    /// Creates an empty serialized object for an internal save operation.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying Steam Audio library fails to create the serialized object.
    ///
    pub(crate) fn try_new(context: &Context) -> Result<Self, SteamAudioError> {
        Self::try_with_input(context, None)
    }

    /// Creates a serialized object that takes ownership of a byte buffer for deserialization.
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
    /// let buffer = vec![0u8; 1024]; // Load your serialized data here.
    /// let serialized_object = SerializedObject::try_with_buffer(&context, buffer)?;
    /// # Ok::<(), audionimbus::SteamAudioError>(())
    /// ```
    pub fn try_with_buffer(context: &Context, buffer: Vec<u8>) -> Result<Self, SteamAudioError> {
        Self::try_with_input(context, Some(Arc::new(buffer)))
    }

    /// Creates a serialized object with optional owned input bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying Steam Audio library fails to create the object.
    fn try_with_input(
        context: &Context,
        input_bytes: Option<Arc<Vec<u8>>>,
    ) -> Result<Self, SteamAudioError> {
        let (data, size) = input_bytes.as_ref().map_or_else(
            || (std::ptr::null_mut(), 0),
            |bytes| {
                (
                    bytes.as_ptr().cast_mut().cast::<audionimbus_sys::IPLbyte>(),
                    bytes.len(),
                )
            },
        );
        let mut serialized_object_settings =
            audionimbus_sys::IPLSerializedObjectSettings { data, size };
        let mut serialized_object = Self {
            raw: std::ptr::null_mut(),
            input_bytes,
        };

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
        self.raw
    }

    /// Returns a mutable reference to the raw FFI pointer.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub const fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLSerializedObject {
        &mut self.raw
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
    /// # use audionimbus::{Context, Scene};
    /// # let context = Context::default();
    /// # let scene = Scene::try_new(&context)?;
    /// let serialized_object = scene.try_save(&context)?;
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
        unsafe { audionimbus_sys::iplSerializedObjectRelease(&raw mut self.raw) }
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
        Self {
            raw: unsafe { audionimbus_sys::iplSerializedObjectRetain(self.raw) },
            input_bytes: self.input_bytes.clone(),
        }
    }
}

impl PartialEq for SerializedObject {
    fn eq(&self, other: &Self) -> bool {
        self.raw_ptr() == other.raw_ptr()
    }
}

impl Eq for SerializedObject {}

impl Hash for SerializedObject {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::ptr::hash(self.raw_ptr(), state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_new() {
        let context = Context::default();
        let serialized_object = SerializedObject::try_new(&context);
        assert!(serialized_object.is_ok());
    }

    #[test]
    fn try_with_buffer() {
        let context = Context::default();
        let buffer = vec![1, 2, 3, 4];
        let data = buffer.as_ptr();
        let serialized_object = SerializedObject::try_with_buffer(&context, buffer).unwrap();

        assert_eq!(
            serialized_object.input_bytes.as_ref().unwrap().as_ptr(),
            data
        );
        assert_eq!(serialized_object.to_vec(), vec![1, 2, 3, 4]);
    }

    #[test]
    fn clone() {
        let context = Context::default();
        let buffer = vec![1, 2, 3, 4];
        let serialized_object = SerializedObject::try_with_buffer(&context, buffer).unwrap();
        let clone = serialized_object.clone();

        assert_eq!(serialized_object.raw_ptr(), clone.raw_ptr());
        assert!(Arc::ptr_eq(
            serialized_object.input_bytes.as_ref().unwrap(),
            clone.input_bytes.as_ref().unwrap()
        ));
        drop(serialized_object);
        assert_eq!(clone.to_vec(), vec![1, 2, 3, 4]);
    }
}
