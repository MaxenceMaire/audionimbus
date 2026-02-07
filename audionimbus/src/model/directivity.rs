//! Directivity patterns for modeling sound intensity as a function of the source's orientation.

use crate::callback::DirectivityCallback;
use crate::context::Context;
use crate::geometry;

/// A directivity pattern that can be used to model changes in sound intensity as a function of the source’s orientation.
/// Can be used with both direct and indirect sound propagation.
#[derive(Debug)]
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
    Callback(DirectivityCallback),
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
            Directivity::Callback(callback) => {
                let (callback_fn, user_data) = callback.as_raw_parts();
                (f32::default(), f32::default(), Some(callback_fn), user_data)
            }
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

        let listener = Point::new(0.0, 0.0, 1.0);
        let attenuation =
            directivity_attenuation(&context, source, listener, &Directivity::default());
        assert_eq!(attenuation, 1.0); // No attenuation when listener is in front

        let listener = Point::new(0.0, 1.0, 0.0);
        let attenuation =
            directivity_attenuation(&context, source, listener, &Directivity::default());
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

        // Test multiple directions.
        let directions = [
            Point::new(1.0, 0.0, 0.0),
            Point::new(0.0, 1.0, 0.0),
            Point::new(0.0, 0.0, 1.0),
            Point::new(-1.0, 0.0, 0.0),
        ];

        let directivity = Directivity::WeightedDipole {
            weight: 0.0, // Omnidirectional
            power: 1.0,
        };

        let mut attenuations = Vec::new();
        for &listener in &directions {
            let attn = directivity_attenuation(&context, source, listener, &directivity);
            attenuations.push(attn);
        }

        // All directions should have similar attenuation.
        let first = attenuations[0];
        for &attenuation in &attenuations {
            assert!((attenuation - first).abs() < 0.01);
        }
    }

    #[test]
    fn test_callback_model() {
        let context = Context::default();

        // Source at origin, pointing along +z axis (default ahead direction)
        let source = CoordinateSystem::default();

        let listener_front = Point::new(0.0, 0.0, 1.0); // In front (along -z in world, which is +z in source local)

        let directivity = Directivity::Callback(DirectivityCallback::new(|_direction| 0.5));
        let attenuation = directivity_attenuation(&context, source, listener_front, &directivity);
        assert_eq!(attenuation, 0.5);
    }
}
