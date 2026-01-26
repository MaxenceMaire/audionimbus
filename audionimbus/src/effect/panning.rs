use super::audio_effect_state::AudioEffectState;
use super::EffectError;
use super::SpeakerLayout;
use crate::audio_buffer::{AudioBuffer, Sample};
use crate::audio_settings::AudioSettings;
use crate::context::Context;
use crate::error::{to_option_error, SteamAudioError};
use crate::ffi_wrapper::FFIWrapper;
use crate::geometry::Direction;
use crate::{ChannelPointers, ChannelRequirement};

/// Pans a single-channel point source to a multi-channel speaker layout based on the 3D position of the source relative to the listener.
///
/// # Examples
///
/// ```
/// use audionimbus::*;
///
/// let context = Context::default();
/// let audio_settings = AudioSettings::default();
///
/// let mut effect = PanningEffect::try_new(
///     &context,
///     &audio_settings,
///     &PanningEffectSettings {
///         speaker_layout: SpeakerLayout::Stereo,
///     }
/// )?;
///
/// let params = PanningEffectParams {
///     direction: Direction::new(1.0, 0.0, 0.0), // Sound from the right
/// };
///
/// let input = vec![0.5; 1024];
/// let input_buffer = AudioBuffer::try_with_data(&input)?;
/// let mut output = vec![0.0; 2 * input_buffer.num_samples() as usize];
/// let output_buffer = AudioBuffer::try_with_data_and_settings(
///     &mut output,
///     AudioBufferSettings::with_num_channels(2),
/// )?;
///
/// let _ = effect.apply(&params, &input_buffer, &output_buffer);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug)]
pub struct PanningEffect {
    inner: audionimbus_sys::IPLPanningEffect,

    /// Number of output channels needed for the speaker layout specified when creating the effect.
    num_output_channels: u32,
}

impl PanningEffect {
    /// Creates a new panning effect.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if effect creation fails.
    pub fn try_new(
        context: &Context,
        audio_settings: &AudioSettings,
        panning_effect_settings: &PanningEffectSettings,
    ) -> Result<Self, SteamAudioError> {
        let mut inner = std::ptr::null_mut();

        let status = unsafe {
            audionimbus_sys::iplPanningEffectCreate(
                context.raw_ptr(),
                &mut audionimbus_sys::IPLAudioSettings::from(audio_settings),
                &mut audionimbus_sys::IPLPanningEffectSettings::from(panning_effect_settings),
                &mut inner,
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        let num_output_channels = match &panning_effect_settings.speaker_layout {
            SpeakerLayout::Mono => 1,
            SpeakerLayout::Stereo => 2,
            SpeakerLayout::Quadraphonic => 4,
            SpeakerLayout::Surround5_1 => 6,
            SpeakerLayout::Surround7_1 => 8,
            SpeakerLayout::Custom { speaker_directions } => speaker_directions.len() as u32,
        };

        Ok(Self {
            inner,
            num_output_channels,
        })
    }

    /// Applies a panning effect to an audio buffer.
    ///
    /// This effect CANNOT be applied in-place.
    ///
    /// The input audio buffer must have one channel, and the output audio buffer must have as many
    /// channels as needed for the speaker layout specified when creating the effect.
    ///
    /// # Errors
    ///
    /// Returns [`EffectError`] if:
    /// - The input buffer has more than one channel
    /// - The output buffer has a number of channels different from that needed for the speaker
    ///   layout specified when creating the effect
    pub fn apply<I, O, PI: ChannelPointers, PO: ChannelPointers>(
        &mut self,
        panning_effect_params: &PanningEffectParams,
        input_buffer: &AudioBuffer<I, PI>,
        output_buffer: &AudioBuffer<O, PO>,
    ) -> Result<AudioEffectState, EffectError>
    where
        I: AsRef<[Sample]>,
        O: AsRef<[Sample]> + AsMut<[Sample]>,
    {
        let num_input_channels = input_buffer.num_channels();
        if num_input_channels != 1 {
            return Err(EffectError::InvalidInputChannels {
                expected: ChannelRequirement::Exactly(1),
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
            audionimbus_sys::iplPanningEffectApply(
                self.raw_ptr(),
                &mut *panning_effect_params.as_ffi(),
                &mut *input_buffer.as_ffi(),
                &mut *output_buffer.as_ffi(),
            )
        }
        .into();

        Ok(state)
    }

    /// Retrieves a single frame of tail samples from a panning effect’s internal buffers.
    ///
    /// After the input to the panning effect has stopped, this function must be called instead of [`Self::apply`] until the return value indicates that no more tail samples remain.
    ///
    /// The output audio buffer must have as many channels as needed for the speaker layout specified when creating the panning effect.
    ///
    /// # Errors
    ///
    /// Returns [`EffectError`] if the output buffer has a number of channels different from that
    /// needed for the speaker layout specified when creating the effect.
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
            audionimbus_sys::iplPanningEffectGetTail(self.raw_ptr(), &mut *output_buffer.as_ffi())
        }
        .into();

        Ok(state)
    }

    /// Returns the number of tail samples remaining in a panning effect’s internal buffers.
    ///
    /// Tail samples are audio samples that should be played even after the input to the effect has stopped playing and no further input samples are available.
    pub fn tail_size(&self) -> usize {
        unsafe { audionimbus_sys::iplPanningEffectGetTailSize(self.raw_ptr()) as usize }
    }

    /// Resets the internal processing state of a panning effect.
    pub fn reset(&mut self) {
        unsafe { audionimbus_sys::iplPanningEffectReset(self.raw_ptr()) };
    }

    /// Returns the raw FFI pointer to the underlying panning effect.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub fn raw_ptr(&self) -> audionimbus_sys::IPLPanningEffect {
        self.inner
    }

    /// Returns a mutable reference to the raw FFI pointer.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLPanningEffect {
        &mut self.inner
    }
}

impl Clone for PanningEffect {
    fn clone(&self) -> Self {
        unsafe {
            audionimbus_sys::iplPanningEffectRetain(self.inner);
        }

        Self {
            inner: self.inner,
            num_output_channels: self.num_output_channels,
        }
    }
}

impl Drop for PanningEffect {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplPanningEffectRelease(&mut self.inner) }
    }
}

unsafe impl Send for PanningEffect {}
unsafe impl Sync for PanningEffect {}

/// Settings used to create a panning effect.
#[derive(Debug)]
pub struct PanningEffectSettings {
    /// The speaker layout to pan input audio to.
    pub speaker_layout: SpeakerLayout,
}

impl From<&PanningEffectSettings> for audionimbus_sys::IPLPanningEffectSettings {
    fn from(settings: &PanningEffectSettings) -> Self {
        Self {
            speakerLayout: (&settings.speaker_layout).into(),
        }
    }
}

/// Parameters for applying a panning effect to an audio buffer.
#[derive(Debug)]
pub struct PanningEffectParams {
    /// Unit vector pointing from the listener towards the source.
    pub direction: Direction,
}

impl From<audionimbus_sys::IPLPanningEffectParams> for PanningEffectParams {
    fn from(params: audionimbus_sys::IPLPanningEffectParams) -> Self {
        Self {
            direction: params.direction.into(),
        }
    }
}

impl PanningEffectParams {
    pub(crate) fn as_ffi(&self) -> FFIWrapper<'_, audionimbus_sys::IPLPanningEffectParams, Self> {
        let panning_effect_params = audionimbus_sys::IPLPanningEffectParams {
            direction: self.direction.into(),
        };

        FFIWrapper::new(panning_effect_params)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;

    mod apply {
        use super::*;

        #[test]
        fn test_valid_stereo() {
            let context = Context::default();
            let audio_settings = AudioSettings::default();

            let mut effect = PanningEffect::try_new(
                &context,
                &audio_settings,
                &PanningEffectSettings {
                    speaker_layout: SpeakerLayout::Stereo,
                },
            )
            .unwrap();

            let panning_effect_params = PanningEffectParams {
                direction: Direction::new(1.0, 0.0, 0.0),
            };

            let input = vec![0.5; 1024];
            let input_buffer = AudioBuffer::try_with_data(&input).unwrap();

            let mut output = vec![0.0; 2 * 1024];
            let output_buffer = AudioBuffer::try_with_data_and_settings(
                &mut output,
                AudioBufferSettings::with_num_channels(2),
            )
            .unwrap();

            assert!(effect
                .apply(&panning_effect_params, &input_buffer, &output_buffer)
                .is_ok());
        }

        #[test]
        fn test_valid_surround_5_1() {
            let context = Context::default();
            let audio_settings = AudioSettings::default();

            let mut effect = PanningEffect::try_new(
                &context,
                &audio_settings,
                &PanningEffectSettings {
                    speaker_layout: SpeakerLayout::Surround5_1,
                },
            )
            .unwrap();

            let panning_effect_params = PanningEffectParams {
                direction: Direction::new(1.0, 0.0, 0.0),
            };

            let input = vec![0.5; 1024];
            let input_buffer = AudioBuffer::try_with_data(&input).unwrap();

            let mut output = vec![0.0; 6 * 1024];
            let output_buffer = AudioBuffer::try_with_data_and_settings(
                &mut output,
                AudioBufferSettings::with_num_channels(6),
            )
            .unwrap();

            assert!(effect
                .apply(&panning_effect_params, &input_buffer, &output_buffer)
                .is_ok());
        }

        #[test]
        fn test_invalid_input_channels() {
            let context = Context::default();
            let audio_settings = AudioSettings::default();

            let mut effect = PanningEffect::try_new(
                &context,
                &audio_settings,
                &PanningEffectSettings {
                    speaker_layout: SpeakerLayout::Stereo,
                },
            )
            .unwrap();

            let panning_effect_params = PanningEffectParams {
                direction: Direction::new(1.0, 0.0, 0.0),
            };

            let input = vec![0.5; 2 * 1024];
            let input_buffer = AudioBuffer::try_with_data_and_settings(
                &input,
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
                effect.apply(&panning_effect_params, &input_buffer, &output_buffer),
                Err(EffectError::InvalidInputChannels {
                    expected: ChannelRequirement::Exactly(1),
                    actual: 2
                })
            );
        }

        #[test]
        fn test_invalid_output_channels() {
            let context = Context::default();
            let audio_settings = AudioSettings::default();

            let mut effect = PanningEffect::try_new(
                &context,
                &audio_settings,
                &PanningEffectSettings {
                    speaker_layout: SpeakerLayout::Stereo,
                },
            )
            .unwrap();

            let panning_effect_params = PanningEffectParams {
                direction: Direction::new(1.0, 0.0, 0.0),
            };

            let input = vec![0.5; 1024];
            let input_buffer = AudioBuffer::try_with_data(&input).unwrap();

            let mut output = vec![0.0; 4 * 1024];
            let output_buffer = AudioBuffer::try_with_data_and_settings(
                &mut output,
                AudioBufferSettings::with_num_channels(4),
            )
            .unwrap();

            assert_eq!(
                effect.apply(&panning_effect_params, &input_buffer, &output_buffer),
                Err(EffectError::InvalidOutputChannels {
                    expected: ChannelRequirement::Exactly(2),
                    actual: 4
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

            let effect = PanningEffect::try_new(
                &context,
                &audio_settings,
                &PanningEffectSettings {
                    speaker_layout: SpeakerLayout::Stereo,
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

            let effect = PanningEffect::try_new(
                &context,
                &audio_settings,
                &PanningEffectSettings {
                    speaker_layout: SpeakerLayout::Stereo,
                },
            )
            .unwrap();

            let mut output = vec![0.0; 4 * 1024];
            let output_buffer = AudioBuffer::try_with_data_and_settings(
                &mut output,
                AudioBufferSettings::with_num_channels(4),
            )
            .unwrap();

            assert_eq!(
                effect.tail(&output_buffer),
                Err(EffectError::InvalidOutputChannels {
                    expected: ChannelRequirement::Exactly(2),
                    actual: 4
                })
            );
        }
    }
}
