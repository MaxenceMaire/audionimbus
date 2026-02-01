//! Directivity patterns for modeling sound intensity as a function of the source's orientation.

use crate::context::Context;
use crate::geometry;

/// A directivity pattern that can be used to model changes in sound intensity as a function of the source’s orientation.
/// Can be used with both direct and indirect sound propagation.
#[derive(Debug, Copy, Clone)]
pub enum Directivity {
    /// The default directivity model is a weighted dipole.
    /// This is a linear blend between an omnidirectional source (which emits sound with equal intensity in all directions), and a dipole oriented along the z-axis in the source’s coordinate system (which focuses sound along the +z and -z axes).
    WeightedDipole {
        /// How much of the dipole to blend into the directivity pattern.
        /// 0.0 = pure omnidirectional, 1.0 = pure dipole.
        /// 0.5 results in a cardioid directivity pattern.
        weight: f32,

        /// How “sharp” the dipole is.
        /// Higher values result in sound being focused within a narrower range of directions.
        power: f32,
    },

    /// A callback function to implement any other arbitrary directivity pattern.
    Callback {
        /// Callback for calculating how much to attenuate a sound based on its directivity pattern and orientation in world space.
        ///
        /// # Arguments
        ///
        /// - `direction`: unit vector (in world space) pointing forwards from the source. This is the direction that the source is “pointing towards”.
        /// - `user_data`: pointer to the arbitrary data specified.
        ///
        /// # Returns
        ///
        /// The directivity value to apply, between 0.0 and 1.0.
        /// 0.0 = the sound is not audible, 1.0 = the sound is as loud as it would be if it had a uniform (omnidirectional) directivity pattern.
        callback: unsafe extern "C" fn(
            direction: audionimbus_sys::IPLVector3,
            user_data: *mut std::ffi::c_void,
        ) -> f32,

        /// Pointer to arbitrary data that will be provided to the callback function whenever it is called. May be `NULL`.
        user_data: *mut std::ffi::c_void,
    },
}

impl Default for Directivity {
    fn default() -> Self {
        Self::WeightedDipole {
            weight: 0.5,
            power: 0.5,
        }
    }
}

impl From<&Directivity> for audionimbus_sys::IPLDirectivity {
    fn from(directivity: &Directivity) -> Self {
        let (dipole_weight, dipole_power, callback, user_data) = match directivity {
            Directivity::WeightedDipole { weight, power } => {
                (*weight, *power, None, std::ptr::null_mut())
            }
            Directivity::Callback {
                callback,
                user_data,
            } => (f32::default(), f32::default(), Some(*callback), *user_data),
        };

        Self {
            dipoleWeight: dipole_weight,
            dipolePower: dipole_power,
            callback,
            userData: user_data,
        }
    }
}

/// Calculates the attenuation of a source due to its directivity pattern and orientation relative to a listener.
pub fn directivity_attenuation(
    context: &Context,
    source: geometry::CoordinateSystem,
    listener: geometry::Point,
    directivity: &Directivity,
) -> f32 {
    unsafe {
        audionimbus_sys::iplDirectivityCalculate(
            context.raw_ptr(),
            source.into(),
            listener.into(),
            &mut directivity.into(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CoordinateSystem, Point};

    #[test]
    fn test_default_model() {
        let context = Context::default();
        let source = CoordinateSystem::default();
        let directivity = Directivity::default();

        let listener = Point::new(0.0, 0.0, 1.0);
        let attenuation = directivity_attenuation(&context, source, listener, &directivity);
        assert_eq!(attenuation, 1.0); // No attenuation when listener is in front

        let listener = Point::new(0.0, 1.0, 0.0);
        let attenuation = directivity_attenuation(&context, source, listener, &directivity);
        assert_eq!(attenuation, 0.70710677);
    }

    #[test]
    fn test_weighted_dipole() {
        let context = Context::default();
        let source = CoordinateSystem::default();

        // Test listener in front (along +z).
        let listener_front = Point::new(0.0, 0.0, 1.0);
        // Test listener behind (along -z).
        let listener_behind = Point::new(0.0, 0.0, -1.0);
        // Test listener to the side (along +x).
        let listener_side = Point::new(1.0, 0.0, 0.0);

        let directivity = Directivity::WeightedDipole {
            weight: 0.5, // Cardioid pattern
            power: 1.0,
        };

        let attenuation_front =
            directivity_attenuation(&context, source, listener_front, &directivity);
        let attenuation_behind =
            directivity_attenuation(&context, source, listener_behind, &directivity);
        let attenuation_side =
            directivity_attenuation(&context, source, listener_side, &directivity);

        // Side should have less intensity.
        assert!(attenuation_side < attenuation_front);
        assert_eq!(attenuation_behind, 0.0);
    }

    #[test]
    fn test_omnidirectional() {
        let context = Context::default();
        let source = CoordinateSystem::default();

        let directivity = Directivity::WeightedDipole {
            weight: 0.0, // Omnidirectional
            power: 1.0,
        };

        // Test multiple directions.
        let directions = [
            Point::new(1.0, 0.0, 0.0),
            Point::new(0.0, 1.0, 0.0),
            Point::new(0.0, 0.0, 1.0),
            Point::new(-1.0, 0.0, 0.0),
        ];

        let mut attenuations = Vec::new();
        for &listener in &directions {
            let attn = directivity_attenuation(&context, source, listener, &directivity);
            attenuations.push(attn);
        }

        // All directions should have similar attenuation.
        let first = attenuations[0];
        for &attn in &attenuations {
            assert!((attn - first).abs() < 0.01);
        }
    }

    #[test]
    fn test_callback_model() {
        let context = Context::default();

        // Source at origin, pointing along +z axis (default ahead direction)
        let source = CoordinateSystem::default();

        // Listener at various positions
        let listener_front = Point::new(0.0, 0.0, -1.0); // In front (along -z in world, which is +z in source local)
        let listener_side = Point::new(1.0, 0.0, 0.0); // To the side

        unsafe extern "C" fn custom_directivity(
            _direction: audionimbus_sys::IPLVector3,
            _user_data: *mut std::ffi::c_void,
        ) -> f32 {
            0.5
        }

        let directivity = Directivity::Callback {
            callback: custom_directivity,
            user_data: std::ptr::null_mut(),
        };

        let attenuation_front =
            directivity_attenuation(&context, source, listener_front, &directivity);
        let attenuation_side =
            directivity_attenuation(&context, source, listener_side, &directivity);

        // Both should be valid attenuation values
        assert_eq!(attenuation_front, 0.5);
        assert_eq!(attenuation_side, 0.5);
    }
}
