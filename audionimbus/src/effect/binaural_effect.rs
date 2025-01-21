use super::audio_effect_state::AudioEffectState;
use crate::audio_buffer::AudioBuffer;
use crate::audio_settings::AudioSettings;
use crate::context::Context;
use crate::error::{to_option_error, SteamAudioError};
use crate::ffi_wrapper::FFIWrapper;
use crate::geometry::Vector3;
use crate::hrtf::{Hrtf, HrtfInterpolation};

/// Spatializes a point source using an HRTF, based on the 3D position of the source relative to the listener.
///
/// The source audio can be 1- or 2-channel; in either case all input channels are spatialized from the same position.
pub struct BinauralEffect(pub audionimbus_sys::IPLBinauralEffect);

impl BinauralEffect {
    pub fn try_new(
        context: Context,
        audio_settings: &AudioSettings,
        binaural_effect_settings: &BinauralEffectSettings,
    ) -> Result<Self, SteamAudioError> {
        let binaural_effect = unsafe {
            let binaural_effect: *mut audionimbus_sys::IPLBinauralEffect = std::ptr::null_mut();
            let status = audionimbus_sys::iplBinauralEffectCreate(
                *context,
                &mut audionimbus_sys::IPLAudioSettings::from(audio_settings),
                &mut audionimbus_sys::IPLBinauralEffectSettings::from(binaural_effect_settings),
                binaural_effect,
            );

            if let Some(error) = to_option_error(status) {
                return Err(error);
            }

            *binaural_effect
        };

        Ok(Self(binaural_effect))
    }

    pub fn apply(
        &self,
        binaural_effect_params: &BinauralEffectParams,
        input_buffer: &mut AudioBuffer,
        output_buffer: &mut AudioBuffer,
    ) -> AudioEffectState {
        unsafe {
            audionimbus_sys::iplBinauralEffectApply(
                **self,
                &mut *binaural_effect_params.as_ffi(),
                &mut *input_buffer.as_ffi(),
                &mut *output_buffer.as_ffi(),
            )
        }
        .into()
    }
}

impl std::ops::Deref for BinauralEffect {
    type Target = audionimbus_sys::IPLBinauralEffect;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for BinauralEffect {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Drop for BinauralEffect {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplBinauralEffectRelease(&mut self.0) }
    }
}

/// Settings used to create a binaural effect.
pub struct BinauralEffectSettings {
    /// The HRTF to use.
    pub hrtf: Hrtf,
}

impl From<&BinauralEffectSettings> for audionimbus_sys::IPLBinauralEffectSettings {
    fn from(settings: &BinauralEffectSettings) -> Self {
        todo!()
    }
}

/// Parameters for applying an Ambisonics binaural effect to an audio buffer.
pub struct BinauralEffectParams {
    /// Unit vector pointing from the listener towards the source.
    pub direction: Vector3,

    /// The interpolation technique to use.
    pub interpolation: HrtfInterpolation,

    /// Amount to blend input audio with spatialized audio.
    ///
    /// When set to 0.0, output audio is not spatialized at all and is close to input audio.
    /// If set to 1.0, output audio is fully spatialized.
    pub spatial_blend: f32,

    /// The HRTF to use.
    pub hrtf: Hrtf,

    /// Optional left- and right-ear peak delays for the HRTF used to spatialize the input audio.
    /// Can be None, in which case peak delays will not be written.
    pub peak_delays: Option<[f32; 2]>,
}

impl BinauralEffectParams {
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
            hrtf: *self.hrtf,
            peakDelays: peak_delays_ptr,
        };

        FFIWrapper::new(binaural_effect_params)
    }
}
