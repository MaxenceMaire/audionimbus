/// A callback function along with associated user data.
#[derive(Debug)]
pub struct CallbackInformation<T> {
    /// The callback function.
    pub callback: T,

    /// Pointer to arbitrary data that will be provided to the callback function whenever it is called. May be `NULL`.
    pub user_data: *mut std::ffi::c_void,
}

/// Callback for updating the application on the progress of a function.
///
/// You can use this to provide the user with visual feedback, like a progress bar.
///
/// # Arguments
///
/// - `progress`: fraction of the function work that has been completed, between 0.0 and 1.0.
/// - `user_data`: pointer to arbitrary user-specified data provided when calling the function that will call this callback.
pub type ProgressCallback = unsafe extern "C" fn(progress: f32, user_data: *mut std::ffi::c_void);
