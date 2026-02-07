//! Callback definitions.

use crate::geometry::Vector3;
use std::cell::Cell;
use std::ffi::c_void;

#[cfg(doc)]
use crate::simulation::Simulator;

/// Internal macro to generate callback wrapper types.
macro_rules! callback {
    (
        $(#[$meta:meta])*
        $vis:vis $name:ident($($arg:ident: $arg_ty:ty),* $(,)?) $(-> $ret:ty)?
    ) => {
        $(#[$meta])*
        $vis struct $name {
            callback: Box<dyn FnMut($($arg_ty),*) $(-> $ret)? + Send>,
        }

        impl $name {
            pub fn new<F>(f: F) -> Self
            where
                F: FnMut($($arg_ty),*) $(-> $ret)? + Send + 'static,
            {
                Self {
                    callback: Box::new(f),
                }
            }

            unsafe extern "C" fn trampoline(
                $($arg: <$arg_ty as $crate::callback::FfiConvert>::FfiType,)*
                user_data: *mut c_void,
            ) $(-> <$ret as $crate::callback::FfiConvert>::FfiType)? {
                let callback = &mut *(user_data as *mut Box<dyn FnMut($($arg_ty),*) $(-> $ret)? + Send>);

                $(let $arg = <$arg_ty as $crate::callback::FfiConvert>::from_ffi($arg);)*

                #[allow(unused_variables)]
                let result = callback($($arg),*);

                $(return <$ret as $crate::callback::FfiConvert>::to_ffi(result);)?
            }

            #[allow(dead_code)]
            pub(crate) fn as_raw_parts(&self) -> (
                unsafe extern "C" fn($(<$arg_ty as $crate::callback::FfiConvert>::FfiType,)* *mut c_void) $(-> <$ret as $crate::callback::FfiConvert>::FfiType)?,
                *mut c_void,
            ) {
                (
                    Self::trampoline,
                    &self.callback as *const _ as *mut c_void,
                )
            }
        }

        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct(stringify!($name))
                    .field("callback", &"<closure>")
                    .finish()
            }
        }
    };
}

pub(crate) use callback;

/// Trait for types that can be converted to/from FFI representations.
pub(crate) trait FfiConvert {
    type FfiType;

    #[allow(dead_code)]
    fn to_ffi(self) -> Self::FfiType;

    #[allow(dead_code)]
    fn from_ffi(ffi: Self::FfiType) -> Self;
}

/// Marker trait for types that pass through to FFI unchanged.
pub(crate) trait FfiPassthrough: Copy + 'static {}

impl<T: FfiPassthrough> FfiConvert for T {
    type FfiType = T;

    fn to_ffi(self) -> Self::FfiType {
        self
    }

    fn from_ffi(ffi: Self::FfiType) -> Self {
        ffi
    }
}

impl FfiPassthrough for f32 {}
impl FfiPassthrough for usize {}
impl FfiPassthrough for std::ffi::c_int {}

impl FfiConvert for Vector3 {
    type FfiType = audionimbus_sys::IPLVector3;

    fn to_ffi(self) -> Self::FfiType {
        audionimbus_sys::IPLVector3 {
            x: self.x,
            y: self.y,
            z: self.z,
        }
    }

    fn from_ffi(ffi: Self::FfiType) -> Self {
        Vector3 {
            x: ffi.x,
            y: ffi.y,
            z: ffi.z,
        }
    }
}

impl FfiConvert for bool {
    type FfiType = audionimbus_sys::IPLbool;

    fn to_ffi(self) -> Self::FfiType {
        if self {
            audionimbus_sys::IPLbool::IPL_TRUE
        } else {
            audionimbus_sys::IPLbool::IPL_FALSE
        }
    }

    fn from_ffi(ffi: Self::FfiType) -> Self {
        match ffi {
            audionimbus_sys::IPLbool::IPL_TRUE => true,
            audionimbus_sys::IPLbool::IPL_FALSE => false,
        }
    }
}

callback! {
    /// A progress callback for long-running operations.
    ///
    /// # Callback arguments
    ///
    /// - `progress`: Fraction of the function work that has been completed, between 0.0 and 1.0.
    pub ProgressCallback(progress: f32)
}

callback! {
    /// Callback for visualizing valid path segments during the call to [`Simulator::run_pathing`].
    ///
    /// You can use this to provide the user with visual feedback, like drawing each segment of a path.
    ///
    /// # Callback arguments
    ///
    /// - `from`: position of the starting probe.
    /// - `to`: position of the ending probe.
    /// - `occluded`: occlusion status of the ray segment between `from` to `to`.
    pub PathingVisualizationCallback(
        from: Vector3,
        to: Vector3,
        occluded: bool,
    )
}

callback! {
    /// Callback for calculating how much to attenuate sound in a given frequency band based on the angle of deviation when the sound path bends around a corner as it propagated from the source to the listener.
    ///
    /// # Callback arguments
    ///
    /// - `angle`: angle (in radians) that the sound path deviates (bends) by.
    /// - `band`: index of the frequency band for which to calculate air absorption.
    ///
    /// # Returns
    ///
    /// The frequency-dependent attenuation to apply, between 0.0 and 1.0.
    /// 0.0 = sound in the frequency band is not audible; 1.0 = sound in the frequency band is not attenuated.
    pub DeviationCallback(
        angle: f32,
        band: std::ffi::c_int,
    ) -> f32
}

callback! {
    /// Callback for calculating how much attenuation should be applied to a sound based on its distance from the listener.
    ///
    /// # Callback arguments
    ///
    /// - `distance`: the distance (in meters) between the source and the listener.
    ///
    /// # Returns
    ///
    /// The distance attenuation to apply, between 0.0 and 1.0.
    /// 0.0 = the sound is not audible, 1.0 = the sound is as loud as it would be if it were emitted from the listener’s position.
    pub DistanceAttenuationCallback(distance: f32) -> f32
}

// BUG: Steam Audio has a bug where the `userData` pointer passed to directivity callbacks
// is corrupted and does not match the originally provided pointer. This causes segfaults
// when attempting to dereference the pointer to access our closure.
//
// See: https://github.com/ValveSoftware/steam-audio/issues/526
//
// This absolutely unholy hack consists in storing the callback pointer in a static cell local to
// the thread instead of relying on Steam Audio to pass it back to us. It is safe because only one
// directivity calculation can execute at a time per thread.
//
// TODO: the this workaround and use `callback!` macro once the Steam Audio bug is fixed.
thread_local! {
    static DIRECTIVITY_CALLBACK_PTR: Cell<*mut c_void> = const { Cell::new(std::ptr::null_mut()) };
}

/// Callback for calculating how much to attenuate a sound based on its directivity pattern and orientation in world space.
///
/// # Callback arguments
///
/// - `direction`: unit vector (in world space) pointing forwards from the source. This is the direction that the source is “pointing towards”.
///
/// # Returns
///
/// The directivity value to apply, between 0.0 and 1.0.
/// 0.0 = the sound is not audible, 1.0 = the sound is as loud as it would be if it had a uniform (omnidirectional) directivity pattern.
pub struct DirectivityCallback {
    callback: Box<dyn FnMut(Vector3) -> f32 + Send>,
}

impl DirectivityCallback {
    pub fn new<F>(f: F) -> Self
    where
        F: FnMut(Vector3) -> f32 + Send + 'static,
    {
        Self {
            callback: Box::new(f),
        }
    }

    // WORKAROUND: This function ignores the `_user_data` parameter (which Steam Audio
    // corrupts) and instead retrieves the callback pointer from thread-local storage.
    unsafe extern "C" fn trampoline(
        direction: audionimbus_sys::IPLVector3,
        _user_data: *mut c_void,
    ) -> f32 {
        let callback_ptr = DIRECTIVITY_CALLBACK_PTR.get();
        let callback = &mut *(callback_ptr as *mut Box<dyn FnMut(Vector3) -> f32 + Send>);
        let direction = Vector3::from_ffi(direction);
        callback(direction)
    }

    // WORKAROUND: This stores the callback pointer in thread-local storage rather than
    // passing it through Steam Audio's userData parameter (which is corrupted by a bug).
    pub(crate) fn as_raw_parts(
        &self,
    ) -> (
        unsafe extern "C" fn(audionimbus_sys::IPLVector3, *mut c_void) -> f32,
        *mut c_void,
    ) {
        let callback_ptr = &self.callback as *const _ as *mut c_void;
        DIRECTIVITY_CALLBACK_PTR.set(callback_ptr);
        (Self::trampoline, callback_ptr)
    }
}

impl std::fmt::Debug for DirectivityCallback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DirectivityCallback")
            .field("callback", &"<closure>")
            .finish()
    }
}
