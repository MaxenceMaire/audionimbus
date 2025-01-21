use crate::audio_settings::AudioSettings;
use crate::context::Context;
use crate::error::{to_option_error, SteamAudioError};

/// A Head-Related Transfer Function (HRTF).
///
/// HRTFs describe how sound from different directions is perceived by a each of a listener’s ears, and are a crucial component of spatial audio.
/// Steam Audio includes a built-in HRTF, while also allowing developers and users to import their own custom HRTFs.
pub struct Hrtf(pub audionimbus_sys::IPLHRTF);

impl Hrtf {
    pub fn try_new(
        context: Context,
        audio_settings: &AudioSettings,
        hrtf_settings: &HrtfSettings,
    ) -> Result<Self, SteamAudioError> {
        let hrtf = unsafe {
            let hrtf: *mut audionimbus_sys::IPLHRTF = std::ptr::null_mut();
            let status = audionimbus_sys::iplHRTFCreate(
                *context,
                &mut audionimbus_sys::IPLAudioSettings::from(audio_settings),
                &mut audionimbus_sys::IPLHRTFSettings::from(hrtf_settings),
                hrtf,
            );

            if let Some(error) = to_option_error(status) {
                return Err(error);
            }

            *hrtf
        };

        Ok(Self(hrtf))
    }
}

impl std::ops::Deref for Hrtf {
    type Target = audionimbus_sys::IPLHRTF;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for Hrtf {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Drop for Hrtf {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplHRTFRelease(&mut self.0) }
    }
}

/// Settings used to create an [`Hrtf`].
pub struct HrtfSettings {
    /// Volume correction factor to apply to the loaded HRTF data.
    ///
    /// A value of 1.0 means the HRTF data will be used without any change.
    pub volume: f32,

    /// An optional buffer containing SOFA file data from which to load HRTF data.
    pub sofa_data: Option<Vec<u8>>,

    /// Volume normalization setting.
    pub volume_normalization: VolumeNormalization,
}

impl Default for HrtfSettings {
    fn default() -> Self {
        Self {
            volume: 1.0,
            sofa_data: None,
            volume_normalization: VolumeNormalization::None,
        }
    }
}

impl From<&HrtfSettings> for audionimbus_sys::IPLHRTFSettings {
    fn from(settings: &HrtfSettings) -> Self {
        todo!()
    }
}

/// HRTF volume normalization setting.
pub enum VolumeNormalization {
    /// No normalization.
    None,

    /// Root-mean squared normalization.
    ///
    /// Normalize HRTF volume to ensure similar volume from all directions based on root-mean-square value of each HRTF.
    RootMeanSquared,
}

/// Techniques for interpolating HRTF data.
///
/// This is used when rendering a point source whose position relative to the listener is not contained in the measured HRTF data.
#[derive(Copy, Clone)]
pub enum HrtfInterpolation {
    /// Nearest-neighbor filtering, i.e., no interpolation.
    ///
    /// Selects the measurement location that is closest to the source’s actual location.
    Nearest,

    /// Bilinear filtering.
    ///
    /// Incurs a relatively high CPU overhead as compared to nearest-neighbor filtering, so use this for sounds where it has a significant benefit.
    /// Typically, bilinear filtering is most useful for wide-band noise-like sounds, such as radio static, mechanical noise, fire, etc.
    Bilinear,
}

impl From<HrtfInterpolation> for audionimbus_sys::IPLHRTFInterpolation {
    fn from(hrtf_interpolation: HrtfInterpolation) -> Self {
        match hrtf_interpolation {
            HrtfInterpolation::Nearest => {
                audionimbus_sys::IPLHRTFInterpolation::IPL_HRTFINTERPOLATION_NEAREST
            }
            HrtfInterpolation::Bilinear => {
                audionimbus_sys::IPLHRTFInterpolation::IPL_HRTFINTERPOLATION_BILINEAR
            }
        }
    }
}
