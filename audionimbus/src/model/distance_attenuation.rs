use crate::context::Context;
use crate::geometry;

/// A distance attenuation model that can be used for modeling attenuation of sound over distance.
/// Can be used with both direct and indirect sound propagation.
#[derive(Debug, Copy, Clone, Default)]
pub enum DistanceAttenuationModel {
    /// The default distance attenuation model.
    /// This is an inverse distance falloff, with all sounds within 1 meter of the listener rendered without distance attenuation.
    #[default]
    Default,

    /// An inverse distance falloff.
    /// You can configure the minimum distance, within which distance attenuation is not applied.
    InverseDistance {
        /// No distance attenuation is applied to any sound whose distance from the listener is less than this value.
        min_distance: f32,
    },

    /// An arbitrary distance falloff function, defined by a callback function.
    Callback {
        /// Callback for calculating how much attenuation should be applied to a sound based on its distance from the listener.
        ///
        /// # Arguments
        ///
        /// - `distance`: the distance (in meters) between the source and the listener.
        /// - `user_data`: pointer to the arbitrary data specified.
        ///
        /// # Returns
        ///
        /// The distance attenuation to apply, between 0.0 and 1.0.
        /// 0.0 = the sound is not audible, 1.0 = the sound is as loud as it would be if it were emitted from the listener’s position.
        callback: unsafe extern "C" fn(distance: f32, user_data: *mut std::ffi::c_void) -> f32,

        /// Pointer to arbitrary data that will be provided to the callback function whenever it is called. May be `NULL`.
        user_data: *mut std::ffi::c_void,

        /// Set to `true` to indicate that the distance attenuation model defined by the callback function has changed since the last time simulation was run.
        /// For example, the callback may be evaluating a curve defined in a GUI.
        /// If the user is editing the curve in real-time, set this to `true` whenever the curve changes, so Steam Audio can update simulation results to match.
        dirty: bool,
    },
}

impl From<&DistanceAttenuationModel> for audionimbus_sys::IPLDistanceAttenuationModel {
    fn from(distance_attenuation_model: &DistanceAttenuationModel) -> Self {
        let (type_, min_distance, callback, user_data, dirty) = match distance_attenuation_model {
            DistanceAttenuationModel::Default => (
                audionimbus_sys::IPLDistanceAttenuationModelType::IPL_DISTANCEATTENUATIONTYPE_DEFAULT,
                f32::default(),
                None,
                std::ptr::null_mut(),
                bool::default()
            ),
            DistanceAttenuationModel::InverseDistance { min_distance } => (
                audionimbus_sys::IPLDistanceAttenuationModelType::IPL_DISTANCEATTENUATIONTYPE_INVERSEDISTANCE,
                *min_distance,
                None,
                std::ptr::null_mut(),
                bool::default()
            ),
            DistanceAttenuationModel::Callback { callback, user_data, dirty } => (
                audionimbus_sys::IPLDistanceAttenuationModelType::IPL_DISTANCEATTENUATIONTYPE_CALLBACK,
                f32::default(),
                Some(*callback),
                *user_data,
                *dirty
            )
        };

        Self {
            type_,
            minDistance: min_distance,
            callback,
            userData: user_data,
            dirty: if dirty {
                audionimbus_sys::IPLbool::IPL_TRUE
            } else {
                audionimbus_sys::IPLbool::IPL_FALSE
            },
        }
    }
}

/// Calculates the distance attenuation between a source and a listener.
pub fn distance_attenuation(
    context: &Context,
    source: &geometry::Point,
    listener: &geometry::Point,
    model: &DistanceAttenuationModel,
) -> f32 {
    unsafe {
        audionimbus_sys::iplDistanceAttenuationCalculate(
            context.raw_ptr(),
            (*source).into(),
            (*listener).into(),
            &mut model.into(),
        )
    }
}
