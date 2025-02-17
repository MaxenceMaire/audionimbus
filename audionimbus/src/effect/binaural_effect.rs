use super::audio_effect_state::AudioEffectState;
use crate::audio_buffer::{AudioBuffer, Sample};
use crate::audio_settings::AudioSettings;
use crate::context::Context;
use crate::error::{to_option_error, SteamAudioError};
use crate::ffi_wrapper::FFIWrapper;
use crate::geometry::Direction;
use crate::hrtf::{Hrtf, HrtfInterpolation};

/// Spatializes a point source using an HRTF, based on the 3D position of the source relative to the listener.
///
/// The source audio can be 1- or 2-channel; in either case all input channels are spatialized from the same position.
#[derive(Debug)]
pub struct BinauralEffect(audionimbus_sys::IPLBinauralEffect);

impl BinauralEffect {
    pub fn try_new(
        context: &Context,
        audio_settings: &AudioSettings,
        binaural_effect_settings: &BinauralEffectSettings,
    ) -> Result<Self, SteamAudioError> {
        let mut binaural_effect = Self(std::ptr::null_mut());

        let status = unsafe {
            audionimbus_sys::iplBinauralEffectCreate(
                context.raw_ptr(),
                &mut audionimbus_sys::IPLAudioSettings::from(audio_settings),
                &mut audionimbus_sys::IPLBinauralEffectSettings::from(binaural_effect_settings),
                binaural_effect.raw_ptr_mut(),
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        Ok(binaural_effect)
    }

    /// Applies a binaural effect to an audio buffer.
    ///
    /// This effect CANNOT be applied in-place.
    pub fn apply(
        &self,
        binaural_effect_params: &BinauralEffectParams,
        input_buffer: &AudioBuffer<&'_ [Sample]>,
        output_buffer: &AudioBuffer<&'_ mut [Sample]>,
    ) -> AudioEffectState {
        unsafe {
            audionimbus_sys::iplBinauralEffectApply(
                self.raw_ptr(),
                &mut *binaural_effect_params.as_ffi(),
                &mut *input_buffer.as_ffi(),
                &mut *output_buffer.as_ffi(),
            )
        }
        .into()
    }

    pub fn raw_ptr(&self) -> audionimbus_sys::IPLBinauralEffect {
        self.0
    }

    pub fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLBinauralEffect {
        &mut self.0
    }
}

impl Drop for BinauralEffect {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplBinauralEffectRelease(&mut self.0) }
    }
}

/// Settings used to create a binaural effect.
#[derive(Debug)]
pub struct BinauralEffectSettings<'a> {
    /// The HRTF to use.
    pub hrtf: &'a Hrtf,
}

impl From<&BinauralEffectSettings<'_>> for audionimbus_sys::IPLBinauralEffectSettings {
    fn from(settings: &BinauralEffectSettings) -> Self {
        Self {
            hrtf: settings.hrtf.raw_ptr(),
        }
    }
}

/// Parameters for applying an Ambisonics binaural effect to an audio buffer.
#[derive(Debug)]
pub struct BinauralEffectParams<'a> {
    /// Unit vector pointing from the listener towards the source.
    pub direction: Direction,

    /// The interpolation technique to use.
    pub interpolation: HrtfInterpolation,

    /// Amount to blend input audio with spatialized audio.
    ///
    /// When set to 0.0, output audio is not spatialized at all and is close to input audio.
    /// If set to 1.0, output audio is fully spatialized.
    pub spatial_blend: f32,

    /// The HRTF to use.
    pub hrtf: &'a Hrtf,

    /// Optional left- and right-ear peak delays for the HRTF used to spatialize the input audio.
    /// Can be None, in which case peak delays will not be written.
    pub peak_delays: Option<[f32; 2]>,
}

impl BinauralEffectParams<'_> {
    pub(crate) fn as_ffi(&self) -> FFIWrapper<'_, audionimbus_sys::IPLBinauralEffectParams, Self> {
        let peak_delays_ptr = self
            .peak_delays
            .as_ref()
            .map(|peak_delays| peak_delays.as_ptr() as *mut f32)
            .unwrap_or(std::ptr::null_mut());

        let binaural_effect_params = audionimbus_sys::IPLBinauralEffectParams {
            direction: self.direction.into(),
            interpolation: self.interpolation.into(),
            spatialBlend: self.spatial_blend,
            hrtf: self.hrtf.raw_ptr(),
            peakDelays: peak_delays_ptr,
        };

        FFIWrapper::new(binaural_effect_params)
    }
}
