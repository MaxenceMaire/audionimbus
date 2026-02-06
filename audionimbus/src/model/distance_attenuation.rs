//! Attenuation of sound over distance.

pub use crate::callback::DistanceAttenuationCallback;
use crate::context::Context;
use crate::geometry;

/// A distance attenuation model that can be used for modeling attenuation of sound over distance.
/// Can be used with both direct and indirect sound propagation.
#[derive(Debug, Default)]
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
        callback: DistanceAttenuationCallback,

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
            DistanceAttenuationModel::Callback { callback, dirty } => {
                let (callback_fn, user_data) = callback.as_raw_parts();

                (
                    audionimbus_sys::IPLDistanceAttenuationModelType::IPL_DISTANCEATTENUATIONTYPE_CALLBACK,
                    f32::default(),
                    Some(callback_fn),
                    user_data,
                    *dirty
                )
            }
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
    source: geometry::Point,
    listener: geometry::Point,
    model: &DistanceAttenuationModel,
) -> f32 {
    unsafe {
        audionimbus_sys::iplDistanceAttenuationCalculate(
            context.raw_ptr(),
            source.into(),
            listener.into(),
            &mut model.into(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Point;

    #[test]
    fn test_default_model() {
        let context = Context::default();
        let source = Point::new(10.0, 0.0, 0.0);
        let listener = Point::new(0.0, 0.0, 0.0);
        let model = DistanceAttenuationModel::default();

        let attenuation = distance_attenuation(&context, source, listener, &model);

        // The default model is an inverse distance falloff.
        assert_eq!(attenuation, 0.1);
    }

    #[test]
    fn test_inverse_distance_model() {
        let context = Context::default();
        let source = Point::new(5.0, 0.0, 0.0);
        let listener = Point::new(0.0, 0.0, 0.0);
        let model = DistanceAttenuationModel::InverseDistance { min_distance: 1.0 };

        let attenuation = distance_attenuation(&context, source, listener, &model);

        assert_eq!(attenuation, 0.2);
    }

    #[test]
    fn test_zero_distance() {
        let context = Context::default();
        let source = Point::new(0.0, 0.0, 0.0);
        let listener = Point::new(0.0, 0.0, 0.0);
        let model = DistanceAttenuationModel::default();

        let attenuation = distance_attenuation(&context, source, listener, &model);

        // At zero distance, attenuation should be 1.0 (no attenuation).
        assert_eq!(attenuation, 1.0);
    }

    #[test]
    fn test_various_distances() {
        let context = Context::default();
        let listener = Point::new(0.0, 0.0, 0.0);
        let model = DistanceAttenuationModel::default();

        let distances = [1.0, 5.0, 10.0, 50.0];
        let mut prev_attenuation = 1.1; // Start higher than max possible.

        for &dist in &distances {
            let source = Point::new(dist, 0.0, 0.0);
            let attenuation = distance_attenuation(&context, source, listener, &model);

            // Attenuation should decrease with distance.
            assert!(attenuation < prev_attenuation);
            prev_attenuation = attenuation;
        }
    }

    #[test]
    fn test_callback_model() {
        let context = Context::default();
        let source = Point::new(10.0, 0.0, 0.0);
        let listener = Point::new(0.0, 0.0, 0.0);

        let model = DistanceAttenuationModel::Callback {
            callback: DistanceAttenuationCallback::new(|distance: f32| {
                // Custom: linear falloff to 0 at 100m.
                (1.0 - distance / 100.0).max(0.0)
            }),
            dirty: false,
        };

        let attenuation = distance_attenuation(&context, source, listener, &model);

        // At 10m, should be 0.9.
        assert_eq!(attenuation, 0.9);
    }
}
