use super::super::{AudioEffectState, EffectError, SpeakerLayout};
use crate::audio_buffer::{AudioBuffer, Sample};
use crate::audio_settings::AudioSettings;
use crate::context::Context;
use crate::error::{to_option_error, SteamAudioError};
use crate::ffi_wrapper::FFIWrapper;
use crate::num_ambisonics_channels;
use crate::{ChannelPointers, ChannelRequirement};

/// Renders Ambisonic audio by panning it to a standard speaker layout.
///
/// This involves calculating signals to emit from each speaker so as to approximate the Ambisonic sound field.
///
/// # Examples
///
/// ```
/// use audionimbus::*;
///
/// let context = Context::default();
/// let audio_settings = AudioSettings::default();
///
/// let mut effect = AmbisonicsPanningEffect::try_new(
///     &context,
///     &audio_settings,
///     &AmbisonicsPanningEffectSettings {
///         speaker_layout: SpeakerLayout::Surround5_1,
///         max_order: 1,
///     }
/// )?;
///
/// let params = AmbisonicsPanningEffectParams {
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
pub struct AmbisonicsPanningEffect {
    inner: audionimbus_sys::IPLAmbisonicsPanningEffect,

    /// Number of output channels needed for the speaker layout specified when creating the effect.
    num_output_channels: u32,
}

impl AmbisonicsPanningEffect {
    /// Creates a new ambisonics panning effect.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if effect creation fails.
    pub fn try_new(
        context: &Context,
        audio_settings: &AudioSettings,
        ambisonics_panning_effect_settings: &AmbisonicsPanningEffectSettings,
    ) -> Result<Self, SteamAudioError> {
        let mut inner = std::ptr::null_mut();

        let status = unsafe {
            audionimbus_sys::iplAmbisonicsPanningEffectCreate(
                context.raw_ptr(),
                &mut audionimbus_sys::IPLAudioSettings::from(audio_settings),
                &mut audionimbus_sys::IPLAmbisonicsPanningEffectSettings::from(
                    ambisonics_panning_effect_settings,
                ),
                &mut inner,
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        let num_output_channels = match &ambisonics_panning_effect_settings.speaker_layout {
            SpeakerLayout::Mono => 1,
            SpeakerLayout::Stereo => 2,
            SpeakerLayout::Quadraphonic => 4,
            SpeakerLayout::Surround5_1 => 6,
            SpeakerLayout::Surround7_1 => 8,
            SpeakerLayout::Custom { speaker_directions } => speaker_directions.len() as u32,
        };
        let ambisonics_panning_effect = Self {
            inner,
            num_output_channels,
        };

        Ok(ambisonics_panning_effect)
    }

    /// Applies an ambisonics panning effect to an audio buffer.
    ///
    /// This effect CANNOT be applied in-place.
    ///
    /// The input audio buffer must have as many channels as needed for the ambisonics order
    /// specified in the parameters.
    ///
    /// The output audio buffer must have as many channels as needed for the speaker layout
    /// specified when creating the effect.
    ///
    /// # Errors
    ///
    /// Returns [`EffectError`] if:
    /// - The input buffer does not have the correct number of channels for the Ambisonics order
    /// - The output buffer does not have the correct number of channels for the speaker layout
    pub fn apply<I, O, PI: ChannelPointers, PO: ChannelPointers>(
        &mut self,
        ambisonics_panning_effect_params: &AmbisonicsPanningEffectParams,
        input_buffer: &AudioBuffer<I, PI>,
        output_buffer: &AudioBuffer<O, PO>,
    ) -> Result<AudioEffectState, EffectError>
    where
        I: AsRef<[Sample]>,
        O: AsRef<[Sample]> + AsMut<[Sample]>,
    {
        let required_input_channels =
            num_ambisonics_channels(ambisonics_panning_effect_params.order);
        let num_input_channels = input_buffer.num_channels();
        if num_input_channels != required_input_channels {
            return Err(EffectError::InvalidInputChannels {
                expected: ChannelRequirement::Exactly(required_input_channels),
                actual: num_input_channels,
            });
        }

        let num_output_channels = output_buffer.num_channels();
        if num_output_channels != self.num_output_channels {
            return Err(EffectError::InvalidOutputChannels {
                expected: ChannelRequirement::Exactly(self.num_output_channels),
                actual: num_output_channels,
            });
        }

        let state = unsafe {
            audionimbus_sys::iplAmbisonicsPanningEffectApply(
                self.raw_ptr(),
                &mut *ambisonics_panning_effect_params.as_ffi(),
                &mut *input_buffer.as_ffi(),
                &mut *output_buffer.as_ffi(),
            )
        }
        .into();

        Ok(state)
    }

    /// Retrieves a single frame of tail samples from a Ambisonics panning effect’s internal buffers.
    ///
    /// After the input to the Ambisonics panning effect has stopped, this function must be called instead of [`Self::apply`] until the return value indicates that no more tail samples remain.
    ///
    /// The output audio buffer must have as many channels as needed for the speaker layout
    /// specified when creating the effect.
    ///
    /// # Errors
    ///
    /// Returns [`EffectError`] if the output buffer does not have the correct number of channels for the speaker layout.
    pub fn tail<O>(&self, output_buffer: &AudioBuffer<O>) -> Result<AudioEffectState, EffectError>
    where
        O: AsRef<[Sample]> + AsMut<[Sample]>,
    {
        let num_output_channels = output_buffer.num_channels();
        if num_output_channels != self.num_output_channels {
            return Err(EffectError::InvalidOutputChannels {
                expected: ChannelRequirement::Exactly(self.num_output_channels),
                actual: num_output_channels,
            });
        }

        let state = unsafe {
            audionimbus_sys::iplAmbisonicsPanningEffectGetTail(
                self.raw_ptr(),
                &mut *output_buffer.as_ffi(),
            )
        }
        .into();

        Ok(state)
    }

    /// Returns the number of tail samples remaining in an Ambisonics panning effect’s internal buffers.
    ///
    /// Tail samples are audio samples that should be played even after the input to the effect has stopped playing and no further input samples are available.
    pub fn tail_size(&self) -> usize {
        unsafe { audionimbus_sys::iplAmbisonicsPanningEffectGetTailSize(self.raw_ptr()) as usize }
    }

    /// Resets the internal processing state of an ambisonics panning effect.
    pub fn reset(&mut self) {
        unsafe { audionimbus_sys::iplAmbisonicsPanningEffectReset(self.raw_ptr()) };
    }

    /// Returns the raw FFI pointer to the underlying ambisonics panning effect.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub fn raw_ptr(&self) -> audionimbus_sys::IPLAmbisonicsPanningEffect {
        self.inner
    }

    /// Returns a mutable reference to the raw FFI pointer.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLAmbisonicsPanningEffect {
        &mut self.inner
    }
}

impl Clone for AmbisonicsPanningEffect {
    fn clone(&self) -> Self {
        unsafe {
            audionimbus_sys::iplAmbisonicsPanningEffectRetain(self.inner);
        }

        Self {
            inner: self.inner,
            num_output_channels: self.num_output_channels,
        }
    }
}

impl Drop for AmbisonicsPanningEffect {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplAmbisonicsPanningEffectRelease(&mut self.inner) }
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
    pub max_order: u32,
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
    pub order: u32,
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

            let mut effect = AmbisonicsPanningEffect::try_new(
                &context,
                &audio_settings,
                &AmbisonicsPanningEffectSettings {
                    speaker_layout: SpeakerLayout::Stereo,
                    max_order: 1,
                },
            )
            .unwrap();

            let params = AmbisonicsPanningEffectParams { order: 1 };

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

            let mut effect = AmbisonicsPanningEffect::try_new(
                &context,
                &audio_settings,
                &AmbisonicsPanningEffectSettings {
                    speaker_layout: SpeakerLayout::Stereo,
                    max_order: 1,
                },
            )
            .unwrap();

            let params = AmbisonicsPanningEffectParams { order: 1 };

            let mut input = vec![0.5; 2 * 1024];
            let input_buffer = AudioBuffer::try_with_data_and_settings(
                &mut input,
                AudioBufferSettings::with_num_channels(2),
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
                    actual: 2,
                })
            );
        }

        #[test]
        fn test_invalid_output_channels() {
            let context = Context::default();
            let audio_settings = AudioSettings::default();

            let mut effect = AmbisonicsPanningEffect::try_new(
                &context,
                &audio_settings,
                &AmbisonicsPanningEffectSettings {
                    speaker_layout: SpeakerLayout::Stereo,
                    max_order: 1,
                },
            )
            .unwrap();

            let params = AmbisonicsPanningEffectParams { order: 1 };

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
                    actual: 3,
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

            let effect = AmbisonicsPanningEffect::try_new(
                &context,
                &audio_settings,
                &AmbisonicsPanningEffectSettings {
                    speaker_layout: SpeakerLayout::Stereo,
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

            let effect = AmbisonicsPanningEffect::try_new(
                &context,
                &audio_settings,
                &AmbisonicsPanningEffectSettings {
                    speaker_layout: SpeakerLayout::Stereo,
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
                    actual: 3,
                })
            );
        }
    }
}
