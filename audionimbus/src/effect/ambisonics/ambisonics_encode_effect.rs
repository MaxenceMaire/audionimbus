use super::super::AudioEffectState;
use super::SpeakerLayout;
use crate::audio_buffer::AudioBuffer;
use crate::audio_settings::AudioSettings;
use crate::context::Context;
use crate::error::{to_option_error, SteamAudioError};
use crate::ffi_wrapper::FFIWrapper;
use crate::geometry::Direction;
use crate::hrtf::Hrtf;

/// Encodes a point source into Ambisonics.
///
/// Given a point source with some direction relative to the listener, this effect generates an Ambisonic audio buffer that approximates a point source in the given direction.
/// This allows multiple point sources and ambiences to mixed to a single Ambisonics buffer before being spatialized.
#[derive(Debug)]
pub struct AmbisonicsEncodeEffect(pub audionimbus_sys::IPLAmbisonicsEncodeEffect);

impl AmbisonicsEncodeEffect {
    pub fn try_new(
        context: &Context,
        audio_settings: &AudioSettings,
        ambisonics_encode_effect_settings: &AmbisonicsEncodeEffectSettings,
    ) -> Result<Self, SteamAudioError> {
        let ambisonics_encode_effect = unsafe {
            let ambisonics_encode_effect: *mut audionimbus_sys::IPLAmbisonicsEncodeEffect =
                std::ptr::null_mut();
            let status = audionimbus_sys::iplAmbisonicsEncodeEffectCreate(
                context.as_raw_ptr(),
                &mut audionimbus_sys::IPLAudioSettings::from(audio_settings),
                &mut audionimbus_sys::IPLAmbisonicsEncodeEffectSettings::from(
                    ambisonics_encode_effect_settings,
                ),
                ambisonics_encode_effect,
            );

            if let Some(error) = to_option_error(status) {
                return Err(error);
            }

            *ambisonics_encode_effect
        };

        Ok(Self(ambisonics_encode_effect))
    }

    pub fn apply(
        &self,
        ambisonics_encode_effect_params: &AmbisonicsEncodeEffectParams,
        input_buffer: &mut AudioBuffer,
        output_buffer: &mut AudioBuffer,
    ) -> AudioEffectState {
        unsafe {
            audionimbus_sys::iplAmbisonicsEncodeEffectApply(
                **self,
                &mut *ambisonics_encode_effect_params.as_ffi(),
                &mut *input_buffer.as_ffi(),
                &mut *output_buffer.as_ffi(),
            )
        }
        .into()
    }
}

impl std::ops::Deref for AmbisonicsEncodeEffect {
    type Target = audionimbus_sys::IPLAmbisonicsEncodeEffect;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for AmbisonicsEncodeEffect {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Drop for AmbisonicsEncodeEffect {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplAmbisonicsEncodeEffectRelease(&mut self.0) }
    }
}

/// Settings used to create an Ambisonics decode effect.
#[derive(Debug)]
pub struct AmbisonicsEncodeEffectSettings {
    /// The speaker layout that will be used by output audio buffers.
    pub speaker_layout: SpeakerLayout,

    /// The HRTF to use.
    pub hrtf: Hrtf,

    /// The maximum Ambisonics order that will be used by input audio buffers.
    pub max_order: i32,
}

impl From<&AmbisonicsEncodeEffectSettings> for audionimbus_sys::IPLAmbisonicsEncodeEffectSettings {
    fn from(settings: &AmbisonicsEncodeEffectSettings) -> Self {
        todo!()
    }
}

/// Parameters for applying an Ambisonics encode effect to an audio buffer.
#[derive(Debug)]
pub struct AmbisonicsEncodeEffectParams {
    /// Vector pointing from the listener towards the source.
    ///
    /// Need not be normalized; Steam Audio will automatically normalize this vector.
    /// If a zero-length vector is passed, the output will be order 0 (omnidirectional).
    pub direction: Direction,

    /// Ambisonic order of the output buffer.
    ///
    /// May be less than the `max_order` specified when creating the effect, in which case the effect will generate fewer output channels, reducing CPU usage.
    pub order: i32,
}

impl AmbisonicsEncodeEffectParams {
    pub(crate) fn as_ffi(
        &self,
    ) -> FFIWrapper<'_, audionimbus_sys::IPLAmbisonicsEncodeEffectParams, Self> {
        let ambisonics_encode_effect_params = audionimbus_sys::IPLAmbisonicsEncodeEffectParams {
            direction: self.direction.into(),
            order: self.order,
        };

        FFIWrapper::new(ambisonics_encode_effect_params)
    }
}
