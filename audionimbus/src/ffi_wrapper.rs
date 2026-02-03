/// A generic wrapper that ties the lifetime of an FFI type (`T`) to the lifetime of a struct (`Owner`).
#[derive(Debug)]
pub struct FFIWrapper<'a, T, Owner> {
    pub ffi_object: T,
    _marker: std::marker::PhantomData<&'a Owner>,
}

impl<T, Owner> FFIWrapper<'_, T, Owner> {
    /// Creates a new FFI wrapper around the given object.
    pub const fn new(ffi_object: T) -> Self {
        FFIWrapper {
            ffi_object,
            _marker: std::marker::PhantomData,
        }
    }

    /// Consumes the wrapper and returns the inner FFI object.
    pub fn into_inner(self) -> T {
        self.ffi_object
    }
}

impl<T, Owner> std::ops::Deref for FFIWrapper<'_, T, Owner> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.ffi_object
    }
}

impl<T, Owner> std::ops::DerefMut for FFIWrapper<'_, T, Owner> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ffi_object
    }
}
