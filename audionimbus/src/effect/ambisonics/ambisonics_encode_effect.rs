use super::super::AudioEffectState;
use crate::audio_buffer::{AudioBuffer, Sample};
use crate::audio_settings::AudioSettings;
use crate::context::Context;
use crate::error::{to_option_error, SteamAudioError};
use crate::ffi_wrapper::FFIWrapper;
use crate::geometry::Direction;

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
        let mut ambisonics_encode_effect = Self(std::ptr::null_mut());

        let status = unsafe {
            audionimbus_sys::iplAmbisonicsEncodeEffectCreate(
                context.raw_ptr(),
                &mut audionimbus_sys::IPLAudioSettings::from(audio_settings),
                &mut audionimbus_sys::IPLAmbisonicsEncodeEffectSettings::from(
                    ambisonics_encode_effect_settings,
                ),
                ambisonics_encode_effect.raw_ptr_mut(),
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        Ok(ambisonics_encode_effect)
    }

    /// Applies an Ambisonics encode effect to an audio buffer.
    ///
    /// This effect CANNOT be applied in-place.
    pub fn apply<I, O>(
        &self,
        ambisonics_encode_effect_params: &AmbisonicsEncodeEffectParams,
        input_buffer: &AudioBuffer<I>,
        output_buffer: &AudioBuffer<O>,
    ) -> AudioEffectState
    where
        I: AsRef<[Sample]>,
        O: AsRef<[Sample]> + AsMut<[Sample]>,
    {
        let required_num_channels = (ambisonics_encode_effect_params.order + 1).pow(2);
        assert_eq!(
            input_buffer.num_channels(),
            required_num_channels,
            "ambisonic order N = {} requires (N + 1)^2 = {} channels",
            ambisonics_encode_effect_params.order,
            required_num_channels
        );

        unsafe {
            audionimbus_sys::iplAmbisonicsEncodeEffectApply(
                self.raw_ptr(),
                &mut *ambisonics_encode_effect_params.as_ffi(),
                &mut *input_buffer.as_ffi(),
                &mut *output_buffer.as_ffi(),
            )
        }
        .into()
    }

    pub fn raw_ptr(&self) -> audionimbus_sys::IPLAmbisonicsEncodeEffect {
        self.0
    }

    pub fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLAmbisonicsEncodeEffect {
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
    /// The maximum Ambisonics order that will be used by input audio buffers.
    /// Maximum Ambisonics order to encode audio buffers to.
    pub max_order: usize,
}

impl From<&AmbisonicsEncodeEffectSettings> for audionimbus_sys::IPLAmbisonicsEncodeEffectSettings {
    fn from(settings: &AmbisonicsEncodeEffectSettings) -> Self {
        Self {
            maxOrder: settings.max_order as i32,
        }
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
    pub order: usize,
}

impl AmbisonicsEncodeEffectParams {
    pub(crate) fn as_ffi(
        &self,
    ) -> FFIWrapper<'_, audionimbus_sys::IPLAmbisonicsEncodeEffectParams, Self> {
        let ambisonics_encode_effect_params = audionimbus_sys::IPLAmbisonicsEncodeEffectParams {
            direction: self.direction.into(),
            order: self.order as i32,
        };

        FFIWrapper::new(ambisonics_encode_effect_params)
    }
}
