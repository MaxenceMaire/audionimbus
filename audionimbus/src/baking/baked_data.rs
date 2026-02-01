//! Baked data types.

use crate::geometry::Sphere;

/// Identifies a “layer” of data stored in a probe batch.
/// Each probe batch may store multiple layers of data, such as reverb, static source reflections, or pathing.
/// Each layer can be accessed using an identifier.
#[derive(Copy, Clone, Debug)]
pub enum BakedDataIdentifier {
    /// Reflections.
    /// The source and listener positions used to compute the reflections data stored at each probe depends on the \c IPLBakedDataVariation selected.
    Reflections {
        /// The way in which source and listener positions depend on probe position.
        variation: BakedDataVariation,
    },

    /// Pathing.
    /// The probe batch stores data about the shortest paths between any pair of probes in the batch.
    Pathing {
        /// The way in which source and listener positions depend on probe position.
        variation: BakedDataVariation,
    },
}

impl From<BakedDataIdentifier> for audionimbus_sys::IPLBakedDataIdentifier {
    fn from(baked_data_identifier: BakedDataIdentifier) -> Self {
        let (type_, variation) = match baked_data_identifier {
            BakedDataIdentifier::Reflections { variation } => (
                audionimbus_sys::IPLBakedDataType::IPL_BAKEDDATATYPE_REFLECTIONS,
                variation,
            ),
            BakedDataIdentifier::Pathing { variation } => (
                audionimbus_sys::IPLBakedDataType::IPL_BAKEDDATATYPE_PATHING,
                variation,
            ),
        };

        let (variation, endpoint_influence) = match variation {
            BakedDataVariation::Reverb => (
                audionimbus_sys::IPLBakedDataVariation::IPL_BAKEDDATAVARIATION_REVERB,
                Sphere::default().into(),
            ),
            BakedDataVariation::StaticSource { endpoint_influence } => (
                audionimbus_sys::IPLBakedDataVariation::IPL_BAKEDDATAVARIATION_STATICSOURCE,
                endpoint_influence.into(),
            ),
            BakedDataVariation::StaticListener { endpoint_influence } => (
                audionimbus_sys::IPLBakedDataVariation::IPL_BAKEDDATAVARIATION_STATICLISTENER,
                endpoint_influence.into(),
            ),
            BakedDataVariation::Dynamic => (
                audionimbus_sys::IPLBakedDataVariation::IPL_BAKEDDATAVARIATION_DYNAMIC,
                Sphere::default().into(),
            ),
        };

        Self {
            type_,
            variation,
            endpointInfluence: endpoint_influence,
        }
    }
}

/// The different ways in which the source and listener positions used to generate baked data can vary as a function of probe position.
#[derive(Copy, Clone, Debug)]
pub enum BakedDataVariation {
    /// At each probe, baked data is calculated with both the source and the listener at the probe position.
    /// This is useful for modeling traditional reverbs, which depend only on the listener’s position (or only on the source’s position).
    Reverb,

    /// At each probe, baked data is calculated with the source at some fixed position (specified separately), and the listener at the probe position.
    /// This is used for modeling reflections from a static source to any point within the probe batch.
    StaticSource {
        /// The static source used to generate baked data.
        /// Baked data is only stored for probes that lie within the radius of this sphere.
        endpoint_influence: Sphere,
    },

    /// At each probe, baked data is calculated with the source at the probe position, and the listener at some fixed position (specified separately).
    /// This is used for modeling reflections from a moving source to a static listener.
    StaticListener {
        /// The static listener used to generate baked data.
        /// Baked data is only stored for probes that lie within the radius of this sphere.
        endpoint_influence: Sphere,
    },

    /// Baked data is calculated for each pair of probes.
    /// For example, this is used for calculating paths between every pair of probes in a batch.
    Dynamic,
}
