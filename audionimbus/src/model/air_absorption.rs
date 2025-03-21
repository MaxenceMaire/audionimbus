use crate::context::Context;
use crate::geometry;

/// An air absorption model that can be used for modeling frequency-dependent attenuation of sound over distance.
#[derive(Debug, Copy, Clone)]
pub enum AirAbsorptionModel {
    /// The default air absorption model.
    /// This is an exponential falloff, with decay rates derived from physical properties of air.
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

impl Default for AirAbsorptionModel {
    fn default() -> Self {
        Self::Default
    }
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

/// Calculates the air absorption coefficients between a source and a listener.
pub fn air_absorption(
    context: &Context,
    source: &geometry::Point,
    listener: &geometry::Point,
    model: &AirAbsorptionModel,
) -> [f32; 3] {
    let mut air_absorption = [0.0; 3];

    unsafe {
        audionimbus_sys::iplAirAbsorptionCalculate(
            context.raw_ptr(),
            (*source).into(),
            (*listener).into(),
            &mut model.into(),
            air_absorption.as_mut_ptr(),
        );
    }

    air_absorption
}
