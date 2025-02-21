use super::super::AudioEffectState;
use crate::audio_buffer::{AudioBuffer, Sample};
use crate::audio_settings::AudioSettings;
use crate::context::Context;
use crate::error::{to_option_error, SteamAudioError};
use crate::ffi_wrapper::FFIWrapper;
use crate::hrtf::Hrtf;

/// Renders ambisonic audio using HRTF-based binaural rendering.
///
/// This results in more immersive spatialization of the ambisonic audio as compared to using an ambisonics binaural effect, at the cost of slightly increased CPU usage.
#[derive(Debug)]
pub struct AmbisonicsBinauralEffect(pub audionimbus_sys::IPLAmbisonicsBinauralEffect);

impl AmbisonicsBinauralEffect {
    pub fn try_new(
        context: &Context,
        audio_settings: &AudioSettings,
        ambisonics_binaural_effect_settings: &AmbisonicsBinauralEffectSettings,
    ) -> Result<Self, SteamAudioError> {
        let mut ambisonics_binaural_effect = Self(std::ptr::null_mut());

        let status = unsafe {
            audionimbus_sys::iplAmbisonicsBinauralEffectCreate(
                context.raw_ptr(),
                &mut audionimbus_sys::IPLAudioSettings::from(audio_settings),
                &mut audionimbus_sys::IPLAmbisonicsBinauralEffectSettings::from(
                    ambisonics_binaural_effect_settings,
                ),
                ambisonics_binaural_effect.raw_ptr_mut(),
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        Ok(ambisonics_binaural_effect)
    }

    /// Applies an ambisonics binaural effect to an audio buffer.
    ///
    /// This effect CANNOT be applied in-place.
    pub fn apply<I, O>(
        &self,
        ambisonics_binaural_effect_params: &AmbisonicsBinauralEffectParams,
        input_buffer: &AudioBuffer<I>,
        output_buffer: &AudioBuffer<O>,
    ) -> AudioEffectState
    where
        I: AsRef<[Sample]>,
        O: AsRef<[Sample]> + AsMut<[Sample]>,
    {
        let required_num_channels = (ambisonics_binaural_effect_params.order + 1).pow(2);
        assert_eq!(
            input_buffer.num_channels(),
            required_num_channels,
            "ambisonic order N = {} requires (N + 1)^2 = {} channels",
            ambisonics_binaural_effect_params.order,
            required_num_channels
        );

        unsafe {
            audionimbus_sys::iplAmbisonicsBinauralEffectApply(
                self.raw_ptr(),
                &mut *ambisonics_binaural_effect_params.as_ffi(),
                &mut *input_buffer.as_ffi(),
                &mut *output_buffer.as_ffi(),
            )
        }
        .into()
    }

    /// Resets the internal processing state of an ambisonics binaural effect.
    pub fn reset(&mut self) {
        unsafe { audionimbus_sys::iplAmbisonicsBinauralEffectReset(self.raw_ptr()) };
    }

    pub fn raw_ptr(&self) -> audionimbus_sys::IPLAmbisonicsBinauralEffect {
        self.0
    }

    pub fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLAmbisonicsBinauralEffect {
        &mut self.0
    }
}

impl Clone for AmbisonicsBinauralEffect {
    fn clone(&self) -> Self {
        unsafe {
            audionimbus_sys::iplAmbisonicsBinauralEffectRetain(self.0);
        }
        Self(self.0)
    }
}

impl Drop for AmbisonicsBinauralEffect {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplAmbisonicsBinauralEffectRelease(&mut self.0) }
    }
}

unsafe impl Send for AmbisonicsBinauralEffect {}
unsafe impl Sync for AmbisonicsBinauralEffect {}

/// Settings used to create an ambisonics binaural effect.
#[derive(Debug)]
pub struct AmbisonicsBinauralEffectSettings<'a> {
    /// The HRTF to use.
    pub hrtf: &'a Hrtf,

    /// The maximum ambisonics order that will be used by input audio buffers.
    pub max_order: usize,
}

impl From<&AmbisonicsBinauralEffectSettings<'_>>
    for audionimbus_sys::IPLAmbisonicsBinauralEffectSettings
{
    fn from(settings: &AmbisonicsBinauralEffectSettings) -> Self {
        Self {
            hrtf: settings.hrtf.raw_ptr(),
            maxOrder: settings.max_order as i32,
        }
    }
}

/// Parameters for applying an ambisonics binaural effect to an audio buffer.
#[derive(Debug)]
pub struct AmbisonicsBinauralEffectParams<'a> {
    /// The HRTF to use.
    pub hrtf: &'a Hrtf,

    /// Ambisonic order of the input buffer.
    ///
    /// May be less than the `max_order` specified when creating the effect, in which case the effect will process fewer input channels, reducing CPU usage.
    pub order: usize,
}

impl AmbisonicsBinauralEffectParams<'_> {
    pub(crate) fn as_ffi(
        &self,
    ) -> FFIWrapper<'_, audionimbus_sys::IPLAmbisonicsBinauralEffectParams, Self> {
        let ambisonics_binaural_effect_params =
            audionimbus_sys::IPLAmbisonicsBinauralEffectParams {
                hrtf: self.hrtf.raw_ptr(),
                order: self.order as i32,
            };

        FFIWrapper::new(ambisonics_binaural_effect_params)
    }
}
