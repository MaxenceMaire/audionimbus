//! Frequency-dependent attenuation of sound as it bends along the path from the source to the listener.

use crate::callback::DeviationCallback;

/// A deviation model that can be used for modeling frequency-dependent attenuation of sound as it bends along the path from the source to the listener.
#[derive(Debug, Default)]
pub enum DeviationModel {
    /// The default deviation model.
    /// This is a physics-based model, based on the Uniform Theory of Diffraction, with various additional assumptions.
    #[default]
    Default,

    /// An arbitrary deviation model, defined by a callback function.
    Callback(DeviationCallback),
}

impl From<&DeviationModel> for audionimbus_sys::IPLDeviationModel {
    fn from(deviation_model: &DeviationModel) -> Self {
        let (type_, callback, user_data) = match deviation_model {
            DeviationModel::Default => (
                audionimbus_sys::IPLDeviationModelType::IPL_DEVIATIONTYPE_DEFAULT,
                None,
                std::ptr::null_mut(),
            ),
            DeviationModel::Callback(callback) => {
                let (callback_fn, user_data) = callback.as_raw_parts();
                (
                    audionimbus_sys::IPLDeviationModelType::IPL_DEVIATIONTYPE_CALLBACK,
                    Some(callback_fn),
                    user_data,
                )
            }
        };

        Self {
            type_,
            callback,
            userData: user_data,
        }
    }
}
