//! Callback definitions.

/// Internal macro to generate callback wrapper types.
macro_rules! callback {
    (
        $(#[$meta:meta])*
        $vis:vis $name:ident($($arg:ident: $arg_ty:ty),*) $(-> $ret:ty)?
    ) => {
        $(#[$meta])*
        $vis struct $name {
            callback: Box<dyn FnMut($($arg_ty),*) $(-> $ret)? + Send>,
        }

        impl $name {
            /// Creates a new callback from a closure.
            pub fn new<F>(f: F) -> Self
            where
                F: FnMut($($arg_ty),*) $(-> $ret)? + Send + 'static,
            {
                Self {
                    callback: Box::new(f),
                }
            }

            unsafe extern "C" fn trampoline(
                $($arg: $arg_ty,)*
                user_data: *mut std::ffi::c_void,
            ) $(-> $ret)? {
                let callback = &mut *(user_data as *mut Box<dyn FnMut($($arg_ty),*) $(-> $ret)? + Send>);
                callback($($arg),*)
            }

            #[allow(dead_code)]
            pub(crate) fn as_raw_parts(&self) -> (
                unsafe extern "C" fn($($arg_ty,)* *mut std::ffi::c_void) $(-> $ret)?,
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

callback! {
    /// A progress callback for long-running operations.
    ///
    /// # Callback arguments
    ///
    /// - `progress`: Fraction of the function work that has been completed, between 0.0 and 1.0.
    pub ProgressCallback(progress: f32)
}
