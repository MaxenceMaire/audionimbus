//! Callback definitions.

use crate::geometry::Vector3;

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

                callback($($arg),*);

                $(return <$ret as $crate::callback::FfiConvert>::to_ffi(result);)?
            }

            #[allow(dead_code)]
            pub(crate) fn as_raw_parts(&self) -> (
                unsafe extern "C" fn($(<$arg_ty as $crate::callback::FfiConvert>::FfiType,)* *mut std::ffi::c_void) $(-> <$ret as $crate::callback::FfiConvert>::FfiType)?,
                *mut std::ffi::c_void,
            ) {
                (
                    Self::trampoline,
                    &*self.callback as *const _ as *mut std::ffi::c_void,
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
pub trait FfiConvert {
    type FfiType;

    fn to_ffi(self) -> Self::FfiType;
    fn from_ffi(ffi: Self::FfiType) -> Self;
}

/// Marker trait for types that pass through to FFI unchanged.
pub trait FfiPassthrough: Copy + 'static {}

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
