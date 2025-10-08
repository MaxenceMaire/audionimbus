/// A deviation model that can be used for modeling frequency-dependent attenuation of sound as it bends along the path from the source to the listener.
#[derive(Debug, Copy, Clone, Default)]
pub enum DeviationModel {
    /// The default deviation model.
    /// This is a physics-based model, based on the Uniform Theory of Diffraction, with various additional assumptions.
    #[default]
    Default,

    /// An arbitrary deviation model, defined by a callback function.
    Callback {
        /// Callback for calculating how much to attenuate sound in a given frequency band based on the angle of deviation when the sound path bends around a corner as it propagated from the source to the listener.
        ///
        /// # Arguments
        ///
        /// - `angle`: angle (in radians) that the sound path deviates (bends) by.
        /// - `band`: index of the frequency band for which to calculate air absorption.
        /// - `user_data`: pointer to the arbitrary data specified.
        ///
        /// # Returns
        ///
        /// The frequency-dependent attenuation to apply, between 0.0 and 1.0.
        /// 0.0 = sound in the frequency band is not audible; 1.0 = sound in the frequency band is not attenuated.
        callback: unsafe extern "C" fn(
            angle: f32,
            band: std::ffi::c_int,
            user_data: *mut std::ffi::c_void,
        ) -> f32,

        /// Pointer to arbitrary data that will be provided to the callback function whenever it is called. May be `NULL`.
        user_data: *mut std::ffi::c_void,
    },
}

impl From<&DeviationModel> for audionimbus_sys::IPLDeviationModel {
    fn from(deviation_model: &DeviationModel) -> Self {
        let (type_, callback, user_data) = match deviation_model {
            DeviationModel::Default => (
                audionimbus_sys::IPLDeviationModelType::IPL_DEVIATIONTYPE_DEFAULT,
                None,
                std::ptr::null_mut(),
            ),
            DeviationModel::Callback {
                callback,
                user_data,
            } => (
                audionimbus_sys::IPLDeviationModelType::IPL_DEVIATIONTYPE_CALLBACK,
                Some(*callback),
                *user_data,
            ),
        };

        Self {
            type_,
            callback,
            userData: user_data,
        }
    }
}
