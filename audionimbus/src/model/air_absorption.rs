//! Frequency-dependent attenuation of sound over distance.

use crate::context::Context;
use crate::{geometry, Equalizer};

/// An air absorption model that can be used for modeling frequency-dependent attenuation of sound over distance.
#[derive(Debug, Copy, Clone, Default)]
pub enum AirAbsorptionModel {
    /// The default air absorption model.
    /// This is an exponential falloff, with decay rates derived from physical properties of air.
    #[default]
    Default,

    /// An exponential falloff.
    /// You can configure the decay rates for each frequency band.
    Exponential {
        /// The exponential falloff coefficients to use.
        coefficients: [f32; 3],
    },

    /// An arbitrary air absorption model, defined by a callback function.
    Callback {
        /// Callback for calculating how much air absorption should be applied to a sound based on its distance from the listener.
        ///
        /// # Arguments
        ///
        /// - `distance`: the distance (in meters) between the source and the listener.
        /// - `band`: index of the frequency band for which to calculate air absorption. 0.0 = low frequencies, 1.0 = middle frequencies, 2.0 = high frequencies.
        /// - `user_data`: pointer to the arbitrary data specified.
        ///
        /// # Returns
        ///
        /// The air absorption to apply, between 0.0 and 1.0.
        /// 0.0 = sound in the frequency band `band` is not audible, 1.0 = sound in the frequency band `band` is not attenuated.
        callback:
            unsafe extern "C" fn(distance: f32, band: i32, user_data: *mut std::ffi::c_void) -> f32,

        /// Pointer to arbitrary data that will be provided to the callback function whenever it is called. May be `NULL`.
        user_data: *mut std::ffi::c_void,

        /// Set to `true` to indicate that the air absorption model defined by the callback function has changed since the last time simulation was run.
        /// For example, the callback may be evaluating a set of curves defined in a GUI.
        /// If the user is editing the curves in real-time, set this to `true` whenever the curves change, so Steam Audio can update simulation results to match.
        dirty: bool,
    },
}

impl From<&AirAbsorptionModel> for audionimbus_sys::IPLAirAbsorptionModel {
    fn from(air_absorption_model: &AirAbsorptionModel) -> Self {
        let (type_, coefficients, callback, user_data, dirty) = match air_absorption_model {
            AirAbsorptionModel::Default => (
                audionimbus_sys::IPLAirAbsorptionModelType::IPL_AIRABSORPTIONTYPE_DEFAULT,
                <[f32; 3]>::default(),
                None,
                std::ptr::null_mut(),
                bool::default(),
            ),
            AirAbsorptionModel::Exponential { coefficients } => (
                audionimbus_sys::IPLAirAbsorptionModelType::IPL_AIRABSORPTIONTYPE_EXPONENTIAL,
                *coefficients,
                None,
                std::ptr::null_mut(),
                bool::default(),
            ),
            AirAbsorptionModel::Callback {
                callback,
                user_data,
                dirty,
            } => (
                audionimbus_sys::IPLAirAbsorptionModelType::IPL_AIRABSORPTIONTYPE_CALLBACK,
                <[f32; 3]>::default(),
                Some(*callback),
                *user_data,
                *dirty,
            ),
        };

        Self {
            type_,
            coefficients,
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

/// # Safety
/// This function segfaults when using the callback air absorption model.
///
/// Calculates the air absorption coefficients between a source and a listener.
pub unsafe fn air_absorption(
    context: &Context,
    source: geometry::Point,
    listener: geometry::Point,
    model: &AirAbsorptionModel,
) -> Equalizer<3> {
    let mut air_absorption = Equalizer([0.0; 3]);

    unsafe {
        audionimbus_sys::iplAirAbsorptionCalculate(
            context.raw_ptr(),
            source.into(),
            listener.into(),
            &mut model.into(),
            air_absorption.0.as_mut_ptr(),
        );
    }

    air_absorption
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
        let model = AirAbsorptionModel::default();

        // TODO: remove `unsafe` once Steam Audio's `iplAirAbsorptionCalculate` segfault is fixed.
        // See issue: https://github.com/ValveSoftware/steam-audio/issues/519
        let absorption = unsafe { air_absorption(&context, source, listener, &model) };

        // All bands should have some absorption (< 1.0) at 10m.
        for &band in &absorption.0 {
            assert!(band > 0.0 && band < 1.0);
        }
    }

    #[test]
    fn test_exponential_model() {
        let context = Context::default();
        let source = Point::new(5.0, 0.0, 0.0);
        let listener = Point::new(0.0, 0.0, 0.0);
        let model = AirAbsorptionModel::Exponential {
            coefficients: [0.01, 0.02, 0.03],
        };

        // TODO: remove `unsafe` once Steam Audio's `iplAirAbsorptionCalculate` segfault is fixed.
        // See issue: https://github.com/ValveSoftware/steam-audio/issues/519
        let absorption = unsafe { air_absorption(&context, source, listener, &model) };

        // Higher frequencies should have more absorption.
        assert!(absorption.0[2] <= absorption.0[1]);
        assert!(absorption.0[1] <= absorption.0[0]);
    }

    #[test]
    fn test_zero_distance() {
        let context = Context::default();
        let source = Point::new(0.0, 0.0, 0.0);
        let listener = Point::new(0.0, 0.0, 0.0);
        let model = AirAbsorptionModel::default();

        // TODO: remove `unsafe` once Steam Audio's `iplAirAbsorptionCalculate` segfault is fixed.
        // See issue: https://github.com/ValveSoftware/steam-audio/issues/519
        let absorption = unsafe { air_absorption(&context, source, listener, &model) };

        // At zero distance, no absorption
        for &band in &absorption.0 {
            assert_eq!(band, 1.0);
        }
    }

    // BUG: Steam Audio's `iplAirAbsorptionCalculate` segfaults when using a callback.
    // TODO: uncomment test once the issue is fixed.
    // See issue: https://github.com/ValveSoftware/steam-audio/issues/519
    /*
    #[test]
    fn test_callback_model() {

        let context = Context::default();
        let source = Point::new(10.0, 0.0, 0.0);
        let listener = Point::new(0.0, 0.0, 0.0);

        unsafe extern "C" fn custom_absorption(
            distance: f32,
            band: i32,
            _user_data: *mut std::ffi::c_void,
        ) -> f32 {
            // More absorption in higher bands (band is 0, 1, or 2)
            // At 10m distance, return different values for each band.
            let absorption_per_meter = match band {
                0 => 0.005, // Low frequency - less absorption
                1 => 0.010, // Mid frequency
                2 => 0.015, // High frequency - more absorption
                _ => 0.01,
            };
            // Exponential decay
            (-absorption_per_meter * distance).exp()
        }

        let model = AirAbsorptionModel::Callback {
            callback: custom_absorption,
            user_data: std::ptr::null_mut(),
            dirty: false,
        };

        let absorption = air_absorption(&context, source, listener, &model);

        // All values should be between 0 and 1.
        for &band in &absorption.0 {
            assert!(band > 0.0 && band <= 1.0);
        }

        // Higher bands should have more absorption (lower values).
        assert!(absorption.0[2] < absorption.0[1]);
        assert!(absorption.0[1] < absorption.0[0]);
    }
    */
}
