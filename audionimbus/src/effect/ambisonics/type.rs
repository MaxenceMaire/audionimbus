/// Supported channel ordering and normalization schemes for Ambisonic audio.
#[derive(Copy, Clone, Debug)]
pub enum AmbisonicsType {
    /// ACN channel ordering, orthonormal spherical harmonics.
    N3D,

    /// ACN channel ordering, semi-normalized spherical harmonics.
    /// AmbiX format.
    SN3D,

    /// Furse-Malham (B-format).
    FuMa,
}

impl From<AmbisonicsType> for audionimbus_sys::IPLAmbisonicsType {
    fn from(ambisonics_type: AmbisonicsType) -> Self {
        match ambisonics_type {
            AmbisonicsType::N3D => Self::IPL_AMBISONICSTYPE_N3D,
            AmbisonicsType::SN3D => Self::IPL_AMBISONICSTYPE_SN3D,
            AmbisonicsType::FuMa => Self::IPL_AMBISONICSTYPE_FUMA,
        }
    }
}
