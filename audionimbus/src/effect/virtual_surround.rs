//! Virtual surround sound rendering for headphones using HRTF.

use super::audio_effect_state::AudioEffectState;
use super::{EffectError, SpeakerLayout};
use crate::audio_buffer::{AudioBuffer, Sample};
use crate::audio_settings::AudioSettings;
use crate::context::Context;
use crate::error::{to_option_error, SteamAudioError};
use crate::ffi_wrapper::FFIWrapper;
use crate::{ChannelPointers, ChannelRequirement, Hrtf};

/// Spatializes multi-channel speaker-based audio (e.g., stereo, quadraphonic, 5.1, or 7.1) using HRTF-based binaural rendering.
///
/// The audio signal for each speaker is spatialized from a point in space corresponding to the speaker’s location.
/// This allows users to experience a surround sound mix over regular stereo headphones.
///
/// Virtual surround is also a fast way to get approximate binaural rendering.
/// All sources can be panned to some surround format (say, 7.1).
/// After the sources are mixed, the mix can be rendered using virtual surround.
/// This can reduce CPU usage, at the cost of spatialization accuracy.
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
/// let mut effect = VirtualSurroundEffect::try_new(
///     &context,
///     &audio_settings,
///     &VirtualSurroundEffectSettings {
///         speaker_layout: SpeakerLayout::Surround7_1,
///         hrtf: &hrtf,
///     },
/// )?;
///
/// let params = VirtualSurroundEffectParams { hrtf: &hrtf };
///
/// const FRAME_SIZE: usize = 1024;
/// let input = vec![0.5; 8 * FRAME_SIZE]; // 8 channels
/// let mut output = vec![0.0; 2 * FRAME_SIZE]; // Stereo output
/// let input_buffer =
///     AudioBuffer::try_with_data_and_settings(&input, AudioBufferSettings::with_num_channels(8))?;
/// let output_buffer = AudioBuffer::try_with_data_and_settings(
///     &mut output,
///     AudioBufferSettings::with_num_channels(2),
/// )?;
///
/// let _ = effect.apply(&params, &input_buffer, &output_buffer);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug)]
pub struct VirtualSurroundEffect {
    inner: audionimbus_sys::IPLVirtualSurroundEffect,

    /// Number of input channels needed for the speaker layout specified when creating the effect.
    num_input_channels: u32,
}

impl VirtualSurroundEffect {
    /// Creates a new virtual surround effect.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if effect creation fails.
    pub fn try_new(
        context: &Context,
        audio_settings: &AudioSettings,
        virtual_surround_effect_settings: &VirtualSurroundEffectSettings,
    ) -> Result<Self, SteamAudioError> {
        let mut inner = std::ptr::null_mut();

        let status = unsafe {
            audionimbus_sys::iplVirtualSurroundEffectCreate(
                context.raw_ptr(),
                &mut audionimbus_sys::IPLAudioSettings::from(audio_settings),
                &mut audionimbus_sys::IPLVirtualSurroundEffectSettings::from(
                    virtual_surround_effect_settings,
                ),
                &raw mut inner,
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        let num_input_channels = match &virtual_surround_effect_settings.speaker_layout {
            SpeakerLayout::Mono => 1,
            SpeakerLayout::Stereo => 2,
            SpeakerLayout::Quadraphonic => 4,
            SpeakerLayout::Surround5_1 => 6,
            SpeakerLayout::Surround7_1 => 8,
            SpeakerLayout::Custom { speaker_directions } => speaker_directions.len() as u32,
        };

        Ok(Self {
            inner,
            num_input_channels,
        })
    }

    /// Applies a virtual surround effect to an audio buffer.
    ///
    /// This effect CANNOT be applied in-place.
    ///
    /// The input audio buffer must have as many channels as needed for the speaker layout specified
    /// when creating the effect, and the output audio buffer must have 2 channels.
    ///
    /// # Errors
    ///
    /// Returns [`EffectError`] if:
    /// - The input buffer does not have the correct number of channels for the speaker layout
    /// - The output buffer does not have exactly two channels
    pub fn apply<I, O, PI: ChannelPointers, PO: ChannelPointers>(
        &mut self,
        virtual_surround_effect_params: &VirtualSurroundEffectParams,
        input_buffer: &AudioBuffer<I, PI>,
        output_buffer: &AudioBuffer<O, PO>,
    ) -> Result<AudioEffectState, EffectError>
    where
        I: AsRef<[Sample]>,
        O: AsRef<[Sample]> + AsMut<[Sample]>,
    {
        let num_input_channels = input_buffer.num_channels();
        if num_input_channels != self.num_input_channels {
            return Err(EffectError::InvalidInputChannels {
                expected: ChannelRequirement::Exactly(self.num_input_channels),
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
            audionimbus_sys::iplVirtualSurroundEffectApply(
                self.raw_ptr(),
                &raw mut *virtual_surround_effect_params.as_ffi(),
                &raw mut *input_buffer.as_ffi(),
                &raw mut *output_buffer.as_ffi(),
            )
        }
        .into();

        Ok(state)
    }

    /// Retrieves a single frame of tail samples from a virtual surround effect’s internal buffers.
    ///
    /// After the input to the virtual surround effect has stopped, this function must be called instead of [`Self::apply`] until the return value indicates that no more tail samples remain.
    ///
    /// The output audio buffer must be 2-channel.
    ///
    /// # Errors
    ///
    /// Returns [`EffectError`] if the output buffer does not have exactly 2 channels.
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
            audionimbus_sys::iplVirtualSurroundEffectGetTail(
                self.raw_ptr(),
                &raw mut *output_buffer.as_ffi(),
            )
        }
        .into();

        Ok(state)
    }

    /// Returns the number of tail samples remaining in a virtual surround effect’s internal buffers.
    ///
    /// Tail samples are audio samples that should be played even after the input to the effect has stopped playing and no further input samples are available.
    pub fn tail_size(&self) -> usize {
        unsafe { audionimbus_sys::iplVirtualSurroundEffectGetTailSize(self.raw_ptr()) as usize }
    }

    /// Resets the internal processing state of a virtual surround effect.
    pub fn reset(&mut self) {
        unsafe { audionimbus_sys::iplVirtualSurroundEffectReset(self.raw_ptr()) };
    }

    /// Returns the raw FFI pointer to the underlying virtual surround effect.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub const fn raw_ptr(&self) -> audionimbus_sys::IPLVirtualSurroundEffect {
        self.inner
    }

    /// Returns a mutable reference to the raw FFI pointer.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub const fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLVirtualSurroundEffect {
        &mut self.inner
    }
}

impl Drop for VirtualSurroundEffect {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplVirtualSurroundEffectRelease(&raw mut self.inner) }
    }
}

unsafe impl Send for VirtualSurroundEffect {}

/// Settings used to create a virtual surround effect.
#[derive(Debug)]
pub struct VirtualSurroundEffectSettings<'a> {
    /// The speaker layout that will be used by input audio buffers.
    pub speaker_layout: SpeakerLayout,

    /// The HRTF to use.
    pub hrtf: &'a Hrtf,
}

impl From<&VirtualSurroundEffectSettings<'_>>
    for audionimbus_sys::IPLVirtualSurroundEffectSettings
{
    fn from(settings: &VirtualSurroundEffectSettings) -> Self {
        Self {
            speakerLayout: (&settings.speaker_layout).into(),
            hrtf: settings.hrtf.raw_ptr(),
        }
    }
}

/// Parameters for applying a virtual surround effect to an audio buffer.
#[derive(Debug)]
pub struct VirtualSurroundEffectParams<'a> {
    /// The HRTF to use.
    pub hrtf: &'a Hrtf,
}

impl VirtualSurroundEffectParams<'_> {
    pub(crate) fn as_ffi(
        &self,
    ) -> FFIWrapper<'_, audionimbus_sys::IPLVirtualSurroundEffectParams, Self> {
        let virtual_surround_effect_params = audionimbus_sys::IPLVirtualSurroundEffectParams {
            hrtf: self.hrtf.raw_ptr(),
        };

        FFIWrapper::new(virtual_surround_effect_params)
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
            let hrtf = Hrtf::try_new(&context, &audio_settings, &HrtfSettings::default()).unwrap();

            let mut effect = VirtualSurroundEffect::try_new(
                &context,
                &audio_settings,
                &VirtualSurroundEffectSettings {
                    speaker_layout: SpeakerLayout::Stereo,
                    hrtf: &hrtf,
                },
            )
            .unwrap();

            let params = VirtualSurroundEffectParams { hrtf: &hrtf };

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

            assert!(effect.apply(&params, &input_buffer, &output_buffer).is_ok());
        }

        #[test]
        fn test_valid_surround_7_1() {
            let context = Context::default();
            let audio_settings = AudioSettings::default();
            let hrtf = Hrtf::try_new(&context, &audio_settings, &HrtfSettings::default()).unwrap();

            let mut effect = VirtualSurroundEffect::try_new(
                &context,
                &audio_settings,
                &VirtualSurroundEffectSettings {
                    speaker_layout: SpeakerLayout::Surround7_1,
                    hrtf: &hrtf,
                },
            )
            .unwrap();

            let params = VirtualSurroundEffectParams { hrtf: &hrtf };

            let input = vec![0.5; 8 * 1024];
            let input_buffer = AudioBuffer::try_with_data_and_settings(
                &input,
                AudioBufferSettings::with_num_channels(8),
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

            let mut effect = VirtualSurroundEffect::try_new(
                &context,
                &audio_settings,
                &VirtualSurroundEffectSettings {
                    speaker_layout: SpeakerLayout::Surround5_1,
                    hrtf: &hrtf,
                },
            )
            .unwrap();

            let params = VirtualSurroundEffectParams { hrtf: &hrtf };

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
                Err(EffectError::InvalidInputChannels {
                    expected: ChannelRequirement::Exactly(6),
                    actual: 2
                })
            );
        }

        #[test]
        fn test_invalid_output_channels() {
            let context = Context::default();
            let audio_settings = AudioSettings::default();
            let hrtf = Hrtf::try_new(&context, &audio_settings, &HrtfSettings::default()).unwrap();

            let mut effect = VirtualSurroundEffect::try_new(
                &context,
                &audio_settings,
                &VirtualSurroundEffectSettings {
                    speaker_layout: SpeakerLayout::Stereo,
                    hrtf: &hrtf,
                },
            )
            .unwrap();

            let params = VirtualSurroundEffectParams { hrtf: &hrtf };

            let input = vec![0.5; 2 * 1024];
            let input_buffer = AudioBuffer::try_with_data_and_settings(
                &input,
                AudioBufferSettings::with_num_channels(2),
            )
            .unwrap();

            let mut output = vec![0.0; 4 * 1024];
            let output_buffer = AudioBuffer::try_with_data_and_settings(
                &mut output,
                AudioBufferSettings::with_num_channels(4),
            )
            .unwrap();

            assert_eq!(
                effect.apply(&params, &input_buffer, &output_buffer),
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
            let hrtf = Hrtf::try_new(&context, &audio_settings, &HrtfSettings::default()).unwrap();

            let effect = VirtualSurroundEffect::try_new(
                &context,
                &audio_settings,
                &VirtualSurroundEffectSettings {
                    speaker_layout: SpeakerLayout::Stereo,
                    hrtf: &hrtf,
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

            let effect = VirtualSurroundEffect::try_new(
                &context,
                &audio_settings,
                &VirtualSurroundEffectSettings {
                    speaker_layout: SpeakerLayout::Stereo,
                    hrtf: &hrtf,
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
