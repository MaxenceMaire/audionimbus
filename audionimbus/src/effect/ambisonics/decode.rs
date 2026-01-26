use super::super::AudioEffectState;
use super::super::EffectError;
use super::SpeakerLayout;
use crate::audio_buffer::{AudioBuffer, Sample};
use crate::audio_settings::AudioSettings;
use crate::context::Context;
use crate::error::{to_option_error, SteamAudioError};
use crate::geometry::CoordinateSystem;
use crate::hrtf::Hrtf;
use crate::num_ambisonics_channels;
use crate::{ChannelPointers, ChannelRequirement};

/// Applies a rotation to an ambisonics audio buffer, then decodes it using panning or binaural rendering.
///
/// This is essentially an ambisonics rotate effect followed by either an ambisonics panning effect or an ambisonics binaural effect.
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
/// let mut effect = AmbisonicsDecodeEffect::try_new(
///     &context,
///     &audio_settings,
///     &AmbisonicsDecodeEffectSettings {
///         speaker_layout: SpeakerLayout::Stereo,
///         hrtf: &hrtf,
///         max_order: 1,
///         rendering: Rendering::Binaural,
///     }
/// )?;
///
/// let params = AmbisonicsDecodeEffectParams {
///     order: 1,
///     hrtf: &hrtf,
///     orientation: CoordinateSystem::default(),
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
pub struct AmbisonicsDecodeEffect {
    inner: audionimbus_sys::IPLAmbisonicsDecodeEffect,

    /// Ambisonics order specified when creating the effect.
    max_order: u32,

    /// Number of output channels required.
    num_output_channels: u32,

    /// Whether the effect uses binaural rendering or panning.
    rendering: Rendering,
}

impl AmbisonicsDecodeEffect {
    /// Creates a new ambisonics decode effect.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if effect creation fails.
    pub fn try_new(
        context: &Context,
        audio_settings: &AudioSettings,
        ambisonics_decode_effect_settings: &AmbisonicsDecodeEffectSettings,
    ) -> Result<Self, SteamAudioError> {
        let mut inner = std::ptr::null_mut();

        let status = unsafe {
            audionimbus_sys::iplAmbisonicsDecodeEffectCreate(
                context.raw_ptr(),
                &mut audionimbus_sys::IPLAudioSettings::from(audio_settings),
                &mut audionimbus_sys::IPLAmbisonicsDecodeEffectSettings::from(
                    ambisonics_decode_effect_settings,
                ),
                &mut inner,
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        let num_output_channels = match ambisonics_decode_effect_settings.rendering {
            Rendering::Binaural => 2,
            Rendering::Panning => match &ambisonics_decode_effect_settings.speaker_layout {
                SpeakerLayout::Mono => 1,
                SpeakerLayout::Stereo => 2,
                SpeakerLayout::Quadraphonic => 4,
                SpeakerLayout::Surround5_1 => 6,
                SpeakerLayout::Surround7_1 => 8,
                SpeakerLayout::Custom { speaker_directions } => speaker_directions.len() as u32,
            },
        };

        let ambisonics_decode_effect = Self {
            inner,
            max_order: ambisonics_decode_effect_settings.max_order,
            num_output_channels,
            rendering: ambisonics_decode_effect_settings.rendering,
        };

        Ok(ambisonics_decode_effect)
    }

    /// Applies an ambisonics decode effect to an audio buffer.
    ///
    /// This effect CANNOT be applied in-place.
    ///
    /// The input audio buffer must have as many channels as needed for the Ambisonics order used
    /// (see [`num_ambisonics_channels`]).
    /// The output audio buffer must have:
    /// - 2 channels if using binaural rendering
    /// - As many channels as needed for the speaker layout if using panning
    ///
    /// # Errors
    ///
    /// Returns [`EffectError`] if:
    /// - The input buffer does not have the correct number of channels for the Ambisonics order
    /// - The output buffer does not have the correct number of channels (i.e., two channels if
    ///   using binaural rendering, or as many channels as needed for the speaker layout if using
    ///   panning).
    pub fn apply<I, O, PI: ChannelPointers, PO: ChannelPointers>(
        &mut self,
        ambisonics_decode_effect_params: &AmbisonicsDecodeEffectParams,
        input_buffer: &AudioBuffer<I, PI>,
        output_buffer: &AudioBuffer<O, PO>,
    ) -> Result<AudioEffectState, EffectError>
    where
        I: AsRef<[Sample]>,
        O: AsRef<[Sample]> + AsMut<[Sample]>,
    {
        let required_input_channels = num_ambisonics_channels(self.max_order);
        let num_input_channels = input_buffer.num_channels();
        if num_input_channels != required_input_channels {
            return Err(EffectError::InvalidAmbisonicOrder {
                order: ambisonics_decode_effect_params.order,
                buffer_channels: num_input_channels,
                required_channels: required_input_channels,
            });
        }

        let num_output_channels = output_buffer.num_channels();
        if num_output_channels != self.num_output_channels {
            return Err(EffectError::InvalidOutputChannels {
                expected: ChannelRequirement::Exactly(self.num_output_channels),
                actual: num_output_channels,
            });
        }

        let mut ambisonics_decode_effect_params_ffi =
            audionimbus_sys::IPLAmbisonicsDecodeEffectParams {
                order: ambisonics_decode_effect_params.order as i32,
                hrtf: ambisonics_decode_effect_params.hrtf.raw_ptr(),
                orientation: ambisonics_decode_effect_params.orientation.into(),
                binaural: match self.rendering {
                    Rendering::Binaural => audionimbus_sys::IPLbool::IPL_TRUE,
                    Rendering::Panning => audionimbus_sys::IPLbool::IPL_FALSE,
                },
            };

        let state = unsafe {
            audionimbus_sys::iplAmbisonicsDecodeEffectApply(
                self.raw_ptr(),
                &mut ambisonics_decode_effect_params_ffi,
                &mut *input_buffer.as_ffi(),
                &mut *output_buffer.as_ffi(),
            )
        }
        .into();

        Ok(state)
    }

    /// Retrieves a single frame of tail samples from an Ambisonics decode effect’s internal buffers.
    ///
    /// After the input to the Ambisonics decode effect has stopped, this function must be called instead of [`Self::apply`] until the return value indicates that no more tail samples remain.
    ///
    /// The output audio buffer must have as many channels as needed for the speaker layout specified when creating the effect (if using panning) or 2 channels (if using binaural rendering).
    ///
    /// # Errors
    ///
    /// Returns [`EffectError`] if the output buffer does not have the correct number of channels (i.e., two channels if
    /// using binaural rendering, or as many channels as needed for the speaker layout if using panning).
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
            audionimbus_sys::iplAmbisonicsDecodeEffectGetTail(
                self.raw_ptr(),
                &mut *output_buffer.as_ffi(),
            )
        }
        .into();

        Ok(state)
    }

    /// Returns the number of tail samples remaining in an Ambisonics decode effect’s internal buffers.
    ///
    /// Tail samples are audio samples that should be played even after the input to the effect has stopped playing and no further input samples are available.
    pub fn tail_size(&self) -> usize {
        unsafe { audionimbus_sys::iplAmbisonicsDecodeEffectGetTailSize(self.raw_ptr()) as usize }
    }

    /// Resets the internal processing state of an ambisonics decode effect.
    pub fn reset(&mut self) {
        unsafe { audionimbus_sys::iplAmbisonicsDecodeEffectReset(self.raw_ptr()) };
    }

    /// Returns the raw FFI pointer to the underlying ambisonics decode effect.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub fn raw_ptr(&self) -> audionimbus_sys::IPLAmbisonicsDecodeEffect {
        self.inner
    }

    /// Returns a mutable reference to the raw FFI pointer.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLAmbisonicsDecodeEffect {
        &mut self.inner
    }
}

impl Clone for AmbisonicsDecodeEffect {
    fn clone(&self) -> Self {
        unsafe {
            audionimbus_sys::iplAmbisonicsDecodeEffectRetain(self.inner);
        }

        Self {
            inner: self.inner,
            max_order: self.max_order,
            num_output_channels: self.num_output_channels,
            rendering: self.rendering,
        }
    }
}

impl Drop for AmbisonicsDecodeEffect {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplAmbisonicsDecodeEffectRelease(&mut self.inner) }
    }
}

unsafe impl Send for AmbisonicsDecodeEffect {}
unsafe impl Sync for AmbisonicsDecodeEffect {}

/// Settings used to create an ambisonics decode effect.
#[derive(Debug)]
pub struct AmbisonicsDecodeEffectSettings<'a> {
    /// The speaker layout that will be used by output audio buffers.
    pub speaker_layout: SpeakerLayout,

    /// The HRTF to use.
    pub hrtf: &'a Hrtf,

    /// The maximum ambisonics order that will be used by input audio buffers.
    pub max_order: u32,

    /// Whether to use binaural rendering or panning.
    pub rendering: Rendering,
}

impl From<&AmbisonicsDecodeEffectSettings<'_>>
    for audionimbus_sys::IPLAmbisonicsDecodeEffectSettings
{
    fn from(settings: &AmbisonicsDecodeEffectSettings) -> Self {
        Self {
            speakerLayout: audionimbus_sys::IPLSpeakerLayout::from(&settings.speaker_layout),
            hrtf: settings.hrtf.raw_ptr(),
            maxOrder: settings.max_order as i32,
        }
    }
}

/// Parameters for applying an ambisonics decode effect to an audio buffer.
#[derive(Debug)]
pub struct AmbisonicsDecodeEffectParams<'a> {
    /// Ambisonic order of the input buffer.
    ///
    /// May be less than the `max_order` specified when creating the effect, in which case the effect will process fewer input channels, reducing CPU usage.
    pub order: u32,

    /// The HRTF to use.
    pub hrtf: &'a Hrtf,

    /// The orientation of the listener.
    pub orientation: CoordinateSystem,
}

/// Rendering for the ambisonics decode effect.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Rendering {
    /// Binaural rendering
    Binaural,

    /// Panning
    Panning,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;

    mod apply {
        use super::*;

        #[test]
        fn test_valid_first_order_binaural() {
            let context = Context::default();
            let audio_settings = AudioSettings::default();
            let hrtf_settings = HrtfSettings::default();
            let hrtf = Hrtf::try_new(&context, &audio_settings, &hrtf_settings).unwrap();

            let mut effect = AmbisonicsDecodeEffect::try_new(
                &context,
                &audio_settings,
                &AmbisonicsDecodeEffectSettings {
                    speaker_layout: SpeakerLayout::Stereo,
                    hrtf: &hrtf,
                    max_order: 1,
                    rendering: Rendering::Binaural,
                },
            )
            .unwrap();

            let params = AmbisonicsDecodeEffectParams {
                order: 1,
                hrtf: &hrtf,
                orientation: CoordinateSystem::default(),
            };

            let input = vec![0.5; 4 * 1024];
            let input_buffer = AudioBuffer::try_with_data_and_settings(
                &input,
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
        fn test_valid_first_order_panning() {
            let context = Context::default();
            let audio_settings = AudioSettings::default();
            let hrtf_settings = HrtfSettings::default();
            let hrtf = Hrtf::try_new(&context, &audio_settings, &hrtf_settings).unwrap();

            let mut effect = AmbisonicsDecodeEffect::try_new(
                &context,
                &audio_settings,
                &AmbisonicsDecodeEffectSettings {
                    speaker_layout: SpeakerLayout::Surround7_1,
                    hrtf: &hrtf,
                    max_order: 1,
                    rendering: Rendering::Panning,
                },
            )
            .unwrap();

            let params = AmbisonicsDecodeEffectParams {
                order: 1,
                hrtf: &hrtf,
                orientation: CoordinateSystem::default(),
            };

            let input = vec![0.5; 4 * 1024];
            let input_buffer = AudioBuffer::try_with_data_and_settings(
                &input,
                AudioBufferSettings::with_num_channels(4),
            )
            .unwrap();

            let mut output = vec![0.0; 8 * 1024];
            let output_buffer = AudioBuffer::try_with_data_and_settings(
                &mut output,
                AudioBufferSettings::with_num_channels(8),
            )
            .unwrap();

            assert!(effect.apply(&params, &input_buffer, &output_buffer).is_ok());
        }

        #[test]
        fn test_valid_invalid_input_channels() {
            let context = Context::default();
            let audio_settings = AudioSettings::default();
            let hrtf_settings = HrtfSettings::default();
            let hrtf = Hrtf::try_new(&context, &audio_settings, &hrtf_settings).unwrap();

            let mut effect = AmbisonicsDecodeEffect::try_new(
                &context,
                &audio_settings,
                &AmbisonicsDecodeEffectSettings {
                    speaker_layout: SpeakerLayout::Stereo,
                    hrtf: &hrtf,
                    max_order: 1,
                    rendering: Rendering::Binaural,
                },
            )
            .unwrap();

            let params = AmbisonicsDecodeEffectParams {
                order: 1,
                hrtf: &hrtf,
                orientation: CoordinateSystem::default(),
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
                effect.apply(&params, &input_buffer, &output_buffer),
                Err(EffectError::InvalidAmbisonicOrder {
                    order: 1,
                    buffer_channels: 2,
                    required_channels: 4,
                })
            );
        }

        #[test]
        fn test_valid_invalid_output_channels() {
            let context = Context::default();
            let audio_settings = AudioSettings::default();
            let hrtf_settings = HrtfSettings::default();
            let hrtf = Hrtf::try_new(&context, &audio_settings, &hrtf_settings).unwrap();

            let mut effect = AmbisonicsDecodeEffect::try_new(
                &context,
                &audio_settings,
                &AmbisonicsDecodeEffectSettings {
                    speaker_layout: SpeakerLayout::Stereo,
                    hrtf: &hrtf,
                    max_order: 1,
                    rendering: Rendering::Binaural,
                },
            )
            .unwrap();

            let params = AmbisonicsDecodeEffectParams {
                order: 1,
                hrtf: &hrtf,
                orientation: CoordinateSystem::default(),
            };

            let input = vec![0.5; 4 * 1024];
            let input_buffer = AudioBuffer::try_with_data_and_settings(
                &input,
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
            let hrtf_settings = HrtfSettings::default();
            let hrtf = Hrtf::try_new(&context, &audio_settings, &hrtf_settings).unwrap();

            let effect = AmbisonicsDecodeEffect::try_new(
                &context,
                &audio_settings,
                &AmbisonicsDecodeEffectSettings {
                    speaker_layout: SpeakerLayout::Stereo,
                    hrtf: &hrtf,
                    max_order: 1,
                    rendering: Rendering::Binaural,
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
            let hrtf_settings = HrtfSettings::default();
            let hrtf = Hrtf::try_new(&context, &audio_settings, &hrtf_settings).unwrap();

            let effect = AmbisonicsDecodeEffect::try_new(
                &context,
                &audio_settings,
                &AmbisonicsDecodeEffectSettings {
                    speaker_layout: SpeakerLayout::Stereo,
                    hrtf: &hrtf,
                    max_order: 1,
                    rendering: Rendering::Binaural,
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
                    actual: 4,
                })
            );
        }
    }
}
