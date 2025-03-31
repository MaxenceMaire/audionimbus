/// Supported channel ordering and normalization schemes for Ambisonic audio.
#[derive(Copy, Clone, Debug)]
pub enum AmbisonicsType {
    /// ACN channel ordering, orthonormal spherical harmonics.
    N3D,

    /// ACN channel ordering, semi-normalized spherical harmonics.
    /// AmbiX format.
    SN3D,

    /// Furse-Malham (B-format).
    FUMA,
}

impl From<AmbisonicsType> for audionimbus_sys::IPLAmbisonicsType {
    fn from(ambisonics_type: AmbisonicsType) -> Self {
        match ambisonics_type {
            AmbisonicsType::N3D => audionimbus_sys::IPLAmbisonicsType::IPL_AMBISONICSTYPE_N3D,
            AmbisonicsType::SN3D => audionimbus_sys::IPLAmbisonicsType::IPL_AMBISONICSTYPE_SN3D,
            AmbisonicsType::FUMA => audionimbus_sys::IPLAmbisonicsType::IPL_AMBISONICSTYPE_FUMA,
        }
    }
}
