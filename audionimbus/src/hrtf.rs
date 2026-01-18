//! Head-Related Transfer Function (HRTF) for spatial audio rendering.

use crate::audio_settings::AudioSettings;
use crate::context::Context;
use crate::error::{to_option_error, SteamAudioError};

/// A Head-Related Transfer Function (HRTF).
///
/// HRTFs describe how sound from different directions is perceived by a each of a listener’s ears, and are a crucial component of spatial audio.
/// Steam Audio includes a built-in HRTF, while also allowing developers and users to import their own custom HRTFs.
#[derive(Debug, PartialEq)]
pub struct Hrtf(pub(crate) audionimbus_sys::IPLHRTF);

impl Hrtf {
    /// Creates a new Head-Related Transfer Function (HRTF).
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if creation fails.
    pub fn try_new(
        context: &Context,
        audio_settings: &AudioSettings,
        hrtf_settings: &HrtfSettings,
    ) -> Result<Self, SteamAudioError> {
        let mut hrtf = Self(std::ptr::null_mut());

        let (mut settings_ffi, _filename_keeper) = hrtf_settings.to_ffi();

        let status = unsafe {
            audionimbus_sys::iplHRTFCreate(
                context.raw_ptr(),
                &mut audionimbus_sys::IPLAudioSettings::from(audio_settings),
                &mut settings_ffi,
                hrtf.raw_ptr_mut(),
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        Ok(hrtf)
    }

    /// Returns the raw FFI pointer to the underlying HRTF.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub fn raw_ptr(&self) -> audionimbus_sys::IPLHRTF {
        self.0
    }

    /// Returns a mutable reference to the raw FFI pointer.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLHRTF {
        &mut self.0
    }
}

impl From<audionimbus_sys::IPLHRTF> for Hrtf {
    fn from(ptr: audionimbus_sys::IPLHRTF) -> Self {
        Self(ptr)
    }
}

impl Clone for Hrtf {
    fn clone(&self) -> Self {
        unsafe {
            audionimbus_sys::iplHRTFRetain(self.0);
        }
        Self(self.0)
    }
}

impl Drop for Hrtf {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplHRTFRelease(&mut self.0) }
    }
}

unsafe impl Send for Hrtf {}
unsafe impl Sync for Hrtf {}

/// Settings used to create an [`Hrtf`].
#[derive(Debug)]
pub struct HrtfSettings {
    /// Volume correction factor to apply to the loaded HRTF data.
    ///
    /// A value of 1.0 means the HRTF data will be used without any change.
    pub volume: f32,

    /// Optional SOFA information to be used to load HRTF data.
    pub sofa_information: Option<Sofa>,

    /// Volume normalization setting.
    pub volume_normalization: VolumeNormalization,
}

impl HrtfSettings {
    /// Converts the settings to the FFI representation.
    ///
    /// Returns a tuple of the FFI settings struct and an optional `CString` that must be kept
    /// alive for the duration of any FFI calls using the returned settings. The `CString` contains
    /// the SOFA filename path and is returned separately because Rust's ownership rules require
    /// it to live as long as the C pointer in the FFI struct remains valid.
    pub fn to_ffi(&self) -> (audionimbus_sys::IPLHRTFSettings, Option<std::ffi::CString>) {
        let (type_, sofa_data, sofa_data_size, filename_cstring) =
            if let Some(information) = &self.sofa_information {
                match information {
                    Sofa::Filename(filename) => {
                        let cstring = std::ffi::CString::new(filename.clone()).unwrap();
                        (
                            audionimbus_sys::IPLHRTFType::IPL_HRTFTYPE_SOFA,
                            std::ptr::null(),
                            0,
                            Some(cstring),
                        )
                    }
                    Sofa::Buffer(buffer) => (
                        audionimbus_sys::IPLHRTFType::IPL_HRTFTYPE_SOFA,
                        buffer.as_ptr(),
                        buffer.len() as i32,
                        None,
                    ),
                }
            } else {
                (
                    audionimbus_sys::IPLHRTFType::IPL_HRTFTYPE_DEFAULT,
                    std::ptr::null(),
                    0,
                    None,
                )
            };

        let sofa_filename = filename_cstring
            .as_ref()
            .map(|c| c.as_ptr())
            .unwrap_or(std::ptr::null());

        let settings = audionimbus_sys::IPLHRTFSettings {
            type_,
            sofaFileName: sofa_filename,
            sofaData: sofa_data,
            sofaDataSize: sofa_data_size,
            volume: self.volume,
            normType: self.volume_normalization.into(),
        };

        (settings, filename_cstring)
    }
}

impl Default for HrtfSettings {
    fn default() -> Self {
        Self {
            volume: 1.0,
            sofa_information: None,
            volume_normalization: VolumeNormalization::None,
        }
    }
}

/// Whether to load SOFA data from a filename or a buffer.
#[derive(Debug)]
pub enum Sofa {
    /// SOFA file from which to load HRTF data.
    Filename(String),

    /// Buffer containing SOFA file data from which to load HRTF data.
    Buffer(Vec<u8>),
}

/// HRTF volume normalization setting.
#[derive(Debug, Copy, Clone)]
pub enum VolumeNormalization {
    /// No normalization.
    None,

    /// Root-mean squared normalization.
    ///
    /// Normalize HRTF volume to ensure similar volume from all directions based on root-mean-square value of each HRTF.
    RootMeanSquared,
}

impl From<VolumeNormalization> for audionimbus_sys::IPLHRTFNormType {
    fn from(volume_normalization: VolumeNormalization) -> Self {
        match volume_normalization {
            VolumeNormalization::None => audionimbus_sys::IPLHRTFNormType::IPL_HRTFNORMTYPE_NONE,
            VolumeNormalization::RootMeanSquared => {
                audionimbus_sys::IPLHRTFNormType::IPL_HRTFNORMTYPE_RMS
            }
        }
    }
}

/// Techniques for interpolating HRTF data.
///
/// This is used when rendering a point source whose position relative to the listener is not contained in the measured HRTF data.
#[derive(Copy, Clone, Debug)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_try_new_hrtf_default() {
        let context = Context::default();
        let audio_settings = AudioSettings::default();
        let hrtf_settings = HrtfSettings::default();
        let hrtf_result = Hrtf::try_new(&context, &audio_settings, &hrtf_settings);
        assert!(hrtf_result.is_ok());
    }
}
