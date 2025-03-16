use super::super::AudioEffectState;
use super::super::SpeakerLayout;
use crate::audio_buffer::{AudioBuffer, Sample};
use crate::audio_settings::AudioSettings;
use crate::context::Context;
use crate::error::{to_option_error, SteamAudioError};
use crate::ffi_wrapper::FFIWrapper;

/// Renders Ambisonic audio by panning it to a standard speaker layout.
///
/// This involves calculating signals to emit from each speaker so as to approximate the Ambisonic sound field.
#[derive(Debug)]
pub struct AmbisonicsPanningEffect(audionimbus_sys::IPLAmbisonicsPanningEffect);

impl AmbisonicsPanningEffect {
    pub fn try_new(
        context: &Context,
        audio_settings: &AudioSettings,
        ambisonics_panning_effect_settings: &AmbisonicsPanningEffectSettings,
    ) -> Result<Self, SteamAudioError> {
        let mut ambisonics_panning_effect = Self(std::ptr::null_mut());

        let status = unsafe {
            audionimbus_sys::iplAmbisonicsPanningEffectCreate(
                context.raw_ptr(),
                &mut audionimbus_sys::IPLAudioSettings::from(audio_settings),
                &mut audionimbus_sys::IPLAmbisonicsPanningEffectSettings::from(
                    ambisonics_panning_effect_settings,
                ),
                ambisonics_panning_effect.raw_ptr_mut(),
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        Ok(ambisonics_panning_effect)
    }

    /// Applies an ambisonics panning effect to an audio buffer.
    ///
    /// This effect CANNOT be applied in-place.
    pub fn apply<I, O>(
        &self,
        ambisonics_panning_effect_params: &AmbisonicsPanningEffectParams,
        input_buffer: &AudioBuffer<I>,
        output_buffer: &AudioBuffer<O>,
    ) -> AudioEffectState
    where
        I: AsRef<[Sample]>,
        O: AsRef<[Sample]> + AsMut<[Sample]>,
    {
        let required_num_channels = (ambisonics_panning_effect_params.order + 1).pow(2);
        assert_eq!(
            input_buffer.num_channels(),
            required_num_channels,
            "ambisonic order N = {} requires (N + 1)^2 = {} input channels",
            ambisonics_panning_effect_params.order,
            required_num_channels
        );

        unsafe {
            audionimbus_sys::iplAmbisonicsPanningEffectApply(
                self.raw_ptr(),
                &mut *ambisonics_panning_effect_params.as_ffi(),
                &mut *input_buffer.as_ffi(),
                &mut *output_buffer.as_ffi(),
            )
        }
        .into()
    }

    /// Resets the internal processing state of an ambisonics panning effect.
    pub fn reset(&mut self) {
        unsafe { audionimbus_sys::iplAmbisonicsPanningEffectReset(self.raw_ptr()) };
    }

    pub fn raw_ptr(&self) -> audionimbus_sys::IPLAmbisonicsPanningEffect {
        self.0
    }

    pub fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLAmbisonicsPanningEffect {
        &mut self.0
    }
}

impl Clone for AmbisonicsPanningEffect {
    fn clone(&self) -> Self {
        unsafe {
            audionimbus_sys::iplAmbisonicsPanningEffectRetain(self.0);
        }
        Self(self.0)
    }
}

impl Drop for AmbisonicsPanningEffect {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplAmbisonicsPanningEffectRelease(&mut self.0) }
    }
}

unsafe impl Send for AmbisonicsPanningEffect {}
unsafe impl Sync for AmbisonicsPanningEffect {}

/// Settings used to create an ambisonics panning effect.
#[derive(Debug)]
pub struct AmbisonicsPanningEffectSettings {
    /// The speaker layout that will be used by output audio buffers.
    pub speaker_layout: SpeakerLayout,

    /// The maximum ambisonics order that will be used by input audio buffers.
    pub max_order: usize,
}

impl From<&AmbisonicsPanningEffectSettings>
    for audionimbus_sys::IPLAmbisonicsPanningEffectSettings
{
    fn from(settings: &AmbisonicsPanningEffectSettings) -> Self {
        Self {
            speakerLayout: (&settings.speaker_layout).into(),
            maxOrder: settings.max_order as i32,
        }
    }
}

/// Parameters for applying an ambisonics panning effect to an audio buffer.
#[derive(Debug)]
pub struct AmbisonicsPanningEffectParams {
    /// Ambisonic order of the input buffer.
    ///
    /// May be less than the `max_order` specified when creating the effect, in which case the effect will process fewer input channels, reducing CPU usage.
    pub order: usize,
}

impl AmbisonicsPanningEffectParams {
    pub(crate) fn as_ffi(
        &self,
    ) -> FFIWrapper<'_, audionimbus_sys::IPLAmbisonicsPanningEffectParams, Self> {
        let ambisonics_panning_effect_params = audionimbus_sys::IPLAmbisonicsPanningEffectParams {
            order: self.order as i32,
        };

        FFIWrapper::new(ambisonics_panning_effect_params)
    }
}
