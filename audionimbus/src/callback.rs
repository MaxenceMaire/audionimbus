//! Callback definitions.

use crate::geometry::Vector3;

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
                user_data: *mut std::ffi::c_void,
            ) $(-> <$ret as $crate::callback::FfiConvert>::FfiType)? {
                let callback = &mut *(user_data as *mut Box<dyn FnMut($($arg_ty),*) $(-> $ret)? + Send>);

                $(let $arg = <$arg_ty as $crate::callback::FfiConvert>::from_ffi($arg);)*

                #[allow(unused_variables)]
                let result = callback($($arg),*);

                $(return <$ret as $crate::callback::FfiConvert>::to_ffi(result);)?
            }

            #[allow(dead_code)]
            pub(crate) fn as_raw_parts(&self) -> (
                unsafe extern "C" fn($(<$arg_ty as $crate::callback::FfiConvert>::FfiType,)* *mut std::ffi::c_void) $(-> <$ret as $crate::callback::FfiConvert>::FfiType)?,
                *mut std::ffi::c_void,
            ) {
                (
                    Self::trampoline,
                    &self.callback as *const _ as *mut std::ffi::c_void,
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

callback! {
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
    pub DirectivityCallback(direction: Vector3) -> f32
}
