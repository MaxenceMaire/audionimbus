use super::super::{AudioEffectState, EffectError};
use crate::audio_buffer::{AudioBuffer, Sample};
use crate::audio_settings::AudioSettings;
use crate::context::Context;
use crate::error::{to_option_error, SteamAudioError};
use crate::ffi_wrapper::FFIWrapper;
use crate::hrtf::Hrtf;
use crate::num_ambisonics_channels;
use crate::{ChannelPointers, ChannelRequirement};

/// Renders ambisonic audio using HRTF-based binaural rendering.
///
/// This results in more immersive spatialization of the ambisonic audio as compared to using an ambisonics binaural effect, at the cost of slightly increased CPU usage.
///
/// # Examples
///
/// ```
/// use audionimbus::*;
///
/// let context = Context::default();
/// let audio_settings = AudioSettings::default();
/// let hrtf = Hrtf::try_new(&context, &audio_settings, &HrtfSettings::default())?;
///
/// let mut effect = AmbisonicsBinauralEffect::try_new(
///     &context,
///     &audio_settings,
///     &AmbisonicsBinauralEffectSettings {
///         hrtf: &hrtf,
///         max_order: 1,
///     }
/// )?;
///
/// let params = AmbisonicsBinauralEffectParams {
///     hrtf: &hrtf,
///     order: 1,
/// };
///
/// const FRAME_SIZE: usize = 1024;
/// let input = vec![0.5; 4 * FRAME_SIZE]; // 4 channels
/// let input_buffer = AudioBuffer::try_with_data_and_settings(
///     &input,
///     AudioBufferSettings::with_num_channels(4)
/// )?;
/// let mut output = vec![0.0; 2 * FRAME_SIZE]; // Stereo
/// let output_buffer = AudioBuffer::try_with_data_and_settings(
///     &mut output,
///     AudioBufferSettings::with_num_channels(2)
/// )?;
///
/// let _ = effect.apply(&params, &input_buffer, &output_buffer);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug)]
pub struct AmbisonicsBinauralEffect(audionimbus_sys::IPLAmbisonicsBinauralEffect);

impl AmbisonicsBinauralEffect {
    /// Creates a new ambisonics binaural effect.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if effect creation fails.
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
    ///
    /// The input audio buffer must have as many channels as needed for the Ambisonics order
    /// specified in the parameters.
    ///
    /// The output audio buffer must have two channels.
    ///
    /// # Errors
    ///
    /// Returns [`EffectError`] if:
    /// - The input buffer does not have the correct number of channels for the Ambisonics order
    /// - The output buffer does not have two channels
    pub fn apply<I, O, PI: ChannelPointers, PO: ChannelPointers>(
        &mut self,
        ambisonics_binaural_effect_params: &AmbisonicsBinauralEffectParams,
        input_buffer: &AudioBuffer<I, PI>,
        output_buffer: &AudioBuffer<O, PO>,
    ) -> Result<AudioEffectState, EffectError>
    where
        I: AsRef<[Sample]>,
        O: AsRef<[Sample]> + AsMut<[Sample]>,
    {
        let required_input_channels =
            num_ambisonics_channels(ambisonics_binaural_effect_params.order);
        let num_input_channels = input_buffer.num_channels();
        if num_input_channels != required_input_channels {
            return Err(EffectError::InvalidInputChannels {
                expected: ChannelRequirement::Exactly(required_input_channels),
                actual: num_input_channels,
            });
        }

        let num_output_channels = output_buffer.num_channels();
        if num_output_channels != 2 {
            return Err(EffectError::InvalidOutputChannels {
                expected: ChannelRequirement::Exactly(2),
                actual: num_output_channels,
            });
        }

        let state = unsafe {
            audionimbus_sys::iplAmbisonicsBinauralEffectApply(
                self.raw_ptr(),
                &mut *ambisonics_binaural_effect_params.as_ffi(),
                &mut *input_buffer.as_ffi(),
                &mut *output_buffer.as_ffi(),
            )
        }
        .into();

        Ok(state)
    }

    /// Retrieves a single frame of tail samples from an Ambisonics binaural effect’s internal buffers.
    ///
    /// After the input to the Ambisonics binaural effect has stopped, this function must be called instead of [`Self::apply`] until the return value indicates that no more tail samples remain.
    ///
    /// The output audio buffer must have two channels.
    ///
    /// # Errors
    ///
    /// Returns [`EffectError`] if the output buffer does not have two channels.
    pub fn tail<O>(&self, output_buffer: &AudioBuffer<O>) -> Result<AudioEffectState, EffectError>
    where
        O: AsRef<[Sample]> + AsMut<[Sample]>,
    {
        let num_output_channels = output_buffer.num_channels();
        if num_output_channels != 2 {
            return Err(EffectError::InvalidOutputChannels {
                expected: ChannelRequirement::Exactly(2),
                actual: num_output_channels,
            });
        }

        let state = unsafe {
            audionimbus_sys::iplAmbisonicsBinauralEffectGetTail(
                self.raw_ptr(),
                &mut *output_buffer.as_ffi(),
            )
        }
        .into();

        Ok(state)
    }

    /// Returns the number of tail samples remaining in an Ambisonics binaural effect’s internal buffers.
    ///
    /// Tail samples are audio samples that should be played even after the input to the effect has stopped playing and no further input samples are available.
    pub fn tail_size(&self) -> usize {
        unsafe { audionimbus_sys::iplAmbisonicsBinauralEffectGetTailSize(self.raw_ptr()) as usize }
    }

    /// Resets the internal processing state of an ambisonics binaural effect.
    pub fn reset(&mut self) {
        unsafe { audionimbus_sys::iplAmbisonicsBinauralEffectReset(self.raw_ptr()) };
    }

    /// Returns the raw FFI pointer to the underlying ambisonics binaural effect.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub fn raw_ptr(&self) -> audionimbus_sys::IPLAmbisonicsBinauralEffect {
        self.0
    }

    /// Returns a mutable reference to the raw FFI pointer.
    ///
    /// This is intended for internal use and advanced scenarios.
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
    pub max_order: u32,
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
    pub order: u32,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;

    mod apply {
        use super::*;

        #[test]
        fn test_valid_first_order() {
            let context = Context::default();
            let audio_settings = AudioSettings::default();
            let hrtf = Hrtf::try_new(&context, &audio_settings, &HrtfSettings::default()).unwrap();

            let mut effect = AmbisonicsBinauralEffect::try_new(
                &context,
                &audio_settings,
                &AmbisonicsBinauralEffectSettings {
                    hrtf: &hrtf,
                    max_order: 1,
                },
            )
            .unwrap();

            let params = AmbisonicsBinauralEffectParams {
                hrtf: &hrtf,
                order: 1,
            };

            let mut input = vec![0.5; 4 * 1024];
            let input_buffer = AudioBuffer::try_with_data_and_settings(
                &mut input,
                AudioBufferSettings::with_num_channels(4),
            )
            .unwrap();

            let mut output = vec![0.0; 2 * 1024];
            let output_buffer = AudioBuffer::try_with_data_and_settings(
                &mut output,
                AudioBufferSettings::with_num_channels(2),
            )
            .unwrap();

            assert!(effect.apply(&params, &input_buffer, &output_buffer).is_ok());
        }

        #[test]
        fn test_invalid_input_channels() {
            let context = Context::default();
            let audio_settings = AudioSettings::default();
            let hrtf = Hrtf::try_new(&context, &audio_settings, &HrtfSettings::default()).unwrap();

            let mut effect = AmbisonicsBinauralEffect::try_new(
                &context,
                &audio_settings,
                &AmbisonicsBinauralEffectSettings {
                    hrtf: &hrtf,
                    max_order: 1,
                },
            )
            .unwrap();

            let params = AmbisonicsBinauralEffectParams {
                hrtf: &hrtf,
                order: 1,
            };

            let mut input = vec![0.5; 3 * 1024];
            let input_buffer = AudioBuffer::try_with_data_and_settings(
                &mut input,
                AudioBufferSettings::with_num_channels(3),
            )
            .unwrap();

            let mut output = vec![0.0; 2 * 1024];
            let output_buffer = AudioBuffer::try_with_data_and_settings(
                &mut output,
                AudioBufferSettings::with_num_channels(2),
            )
            .unwrap();

            assert_eq!(
                effect.apply(&params, &input_buffer, &output_buffer),
                Err(EffectError::InvalidInputChannels {
                    expected: ChannelRequirement::Exactly(4),
                    actual: 3
                })
            );
        }

        #[test]
        fn test_invalid_output_channels() {
            let context = Context::default();
            let audio_settings = AudioSettings::default();
            let hrtf = Hrtf::try_new(&context, &audio_settings, &HrtfSettings::default()).unwrap();

            let mut effect = AmbisonicsBinauralEffect::try_new(
                &context,
                &audio_settings,
                &AmbisonicsBinauralEffectSettings {
                    hrtf: &hrtf,
                    max_order: 1,
                },
            )
            .unwrap();

            let params = AmbisonicsBinauralEffectParams {
                hrtf: &hrtf,
                order: 1,
            };

            let mut input = vec![0.5; 4 * 1024];
            let input_buffer = AudioBuffer::try_with_data_and_settings(
                &mut input,
                AudioBufferSettings::with_num_channels(4),
            )
            .unwrap();

            let mut output = vec![0.0; 3 * 1024];
            let output_buffer = AudioBuffer::try_with_data_and_settings(
                &mut output,
                AudioBufferSettings::with_num_channels(3),
            )
            .unwrap();

            assert_eq!(
                effect.apply(&params, &input_buffer, &output_buffer),
                Err(EffectError::InvalidOutputChannels {
                    expected: ChannelRequirement::Exactly(2),
                    actual: 3
                })
            );
        }
    }

    mod tail {
        use super::*;

        #[test]
        fn test_valid() {
            let context = Context::default();
            let audio_settings = AudioSettings::default();
            let hrtf = Hrtf::try_new(&context, &audio_settings, &HrtfSettings::default()).unwrap();

            let effect = AmbisonicsBinauralEffect::try_new(
                &context,
                &audio_settings,
                &AmbisonicsBinauralEffectSettings {
                    hrtf: &hrtf,
                    max_order: 1,
                },
            )
            .unwrap();

            let mut output = vec![0.0; 2 * 1024];
            let output_buffer = AudioBuffer::try_with_data_and_settings(
                &mut output,
                AudioBufferSettings::with_num_channels(2),
            )
            .unwrap();

            assert!(effect.tail(&output_buffer).is_ok());
        }

        #[test]
        fn test_invalid_output_channels() {
            let context = Context::default();
            let audio_settings = AudioSettings::default();
            let hrtf = Hrtf::try_new(&context, &audio_settings, &HrtfSettings::default()).unwrap();

            let effect = AmbisonicsBinauralEffect::try_new(
                &context,
                &audio_settings,
                &AmbisonicsBinauralEffectSettings {
                    hrtf: &hrtf,
                    max_order: 1,
                },
            )
            .unwrap();

            let mut output = vec![0.0; 3 * 1024];
            let output_buffer = AudioBuffer::try_with_data_and_settings(
                &mut output,
                AudioBufferSettings::with_num_channels(3),
            )
            .unwrap();

            assert_eq!(
                effect.tail(&output_buffer),
                Err(EffectError::InvalidOutputChannels {
                    expected: ChannelRequirement::Exactly(2),
                    actual: 3
                })
            );
        }
    }
}
