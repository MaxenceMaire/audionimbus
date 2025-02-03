use super::super::AudioEffectState;
use super::SpeakerLayout;
use crate::audio_buffer::AudioBuffer;
use crate::audio_settings::AudioSettings;
use crate::context::Context;
use crate::error::{to_option_error, SteamAudioError};
use crate::ffi_wrapper::FFIWrapper;
use crate::geometry::CoordinateSystem;
use crate::hrtf::Hrtf;

/// Applies a rotation to an Ambisonics audio buffer, then decodes it using panning or binaural rendering.
///
/// This is essentially an Ambisonics rotate effect followed by either an Ambisonics panning effect or an Ambisonics binaural effect.
#[derive(Debug)]
pub struct AmbisonicsDecodeEffect(pub audionimbus_sys::IPLAmbisonicsDecodeEffect);

impl AmbisonicsDecodeEffect {
    pub fn try_new(
        context: &Context,
        audio_settings: &AudioSettings,
        ambisonics_decode_effect_settings: &AmbisonicsDecodeEffectSettings,
    ) -> Result<Self, SteamAudioError> {
        let ambisonics_decode_effect = unsafe {
            let ambisonics_decode_effect: *mut audionimbus_sys::IPLAmbisonicsDecodeEffect =
                std::ptr::null_mut();
            let status = audionimbus_sys::iplAmbisonicsDecodeEffectCreate(
                context.as_raw_ptr(),
                &mut audionimbus_sys::IPLAudioSettings::from(audio_settings),
                &mut audionimbus_sys::IPLAmbisonicsDecodeEffectSettings::from(
                    ambisonics_decode_effect_settings,
                ),
                ambisonics_decode_effect,
            );

            if let Some(error) = to_option_error(status) {
                return Err(error);
            }

            *ambisonics_decode_effect
        };

        Ok(Self(ambisonics_decode_effect))
    }

    pub fn apply(
        &self,
        ambisonics_decode_effect_params: &AmbisonicsDecodeEffectParams,
        input_buffer: &mut AudioBuffer,
        output_buffer: &mut AudioBuffer,
    ) -> AudioEffectState {
        unsafe {
            audionimbus_sys::iplAmbisonicsDecodeEffectApply(
                **self,
                &mut *ambisonics_decode_effect_params.as_ffi(),
                &mut *input_buffer.as_ffi(),
                &mut *output_buffer.as_ffi(),
            )
        }
        .into()
    }
}

impl std::ops::Deref for AmbisonicsDecodeEffect {
    type Target = audionimbus_sys::IPLAmbisonicsDecodeEffect;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for AmbisonicsDecodeEffect {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Drop for AmbisonicsDecodeEffect {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplAmbisonicsDecodeEffectRelease(&mut self.0) }
    }
}

/// Settings used to create an Ambisonics decode effect.
#[derive(Debug)]
pub struct AmbisonicsDecodeEffectSettings {
    /// The speaker layout that will be used by output audio buffers.
    pub speaker_layout: SpeakerLayout,

    /// The HRTF to use.
    pub hrtf: Hrtf,

    /// The maximum Ambisonics order that will be used by input audio buffers.
    pub max_order: i32,
}

impl From<&AmbisonicsDecodeEffectSettings> for audionimbus_sys::IPLAmbisonicsDecodeEffectSettings {
    fn from(settings: &AmbisonicsDecodeEffectSettings) -> Self {
        todo!()
    }
}

/// Parameters for applying an Ambisonics decode effect to an audio buffer.
#[derive(Debug)]
pub struct AmbisonicsDecodeEffectParams {
    /// Ambisonic order of the input buffer.
    ///
    /// May be less than the `max_order` specified when creating the effect, in which case the effect will process fewer input channels, reducing CPU usage.
    order: i32,

    /// The HRTF to use.
    hrtf: Hrtf,

    /// The orientation of the listener.
    orientation: CoordinateSystem,

    /// Whether to use binaural rendering or panning.
    binaural: bool,
}

impl AmbisonicsDecodeEffectParams {
    pub(crate) fn as_ffi(
        &self,
    ) -> FFIWrapper<'_, audionimbus_sys::IPLAmbisonicsDecodeEffectParams, Self> {
        let ambisonics_decode_effect_params = audionimbus_sys::IPLAmbisonicsDecodeEffectParams {
            order: self.order,
            hrtf: *self.hrtf,
            orientation: self.orientation.into(),
            binaural: if self.binaural {
                audionimbus_sys::IPLbool::IPL_TRUE
            } else {
                audionimbus_sys::IPLbool::IPL_FALSE
            },
        };

        FFIWrapper::new(ambisonics_decode_effect_params)
    }
}
