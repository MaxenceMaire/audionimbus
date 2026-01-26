use super::super::AudioEffectState;
use super::super::EffectError;
use crate::audio_buffer::{AudioBuffer, Sample};
use crate::audio_settings::AudioSettings;
use crate::context::Context;
use crate::error::{to_option_error, SteamAudioError};
use crate::ffi_wrapper::FFIWrapper;
use crate::geometry::Direction;
use crate::num_ambisonics_channels;
use crate::{ChannelPointers, ChannelRequirement};

/// Encodes a point source into ambisonics.
///
/// Given a point source with some direction relative to the listener, this effect generates an Ambisonic audio buffer that approximates a point source in the given direction.
/// This allows multiple point sources and ambiences to mixed to a single ambisonics buffer before being spatialized.
///
/// # Examples
///
/// ```
/// use audionimbus::*;
///
/// let context = Context::default();
/// let audio_settings = AudioSettings::default();
///
/// let mut effect = AmbisonicsEncodeEffect::try_new(
///     &context,
///     &audio_settings,
///     &AmbisonicsEncodeEffectSettings { max_order: 1 }
/// )?;
///
/// let params = AmbisonicsEncodeEffectParams {
///     direction: Direction::new(1.0, 0.0, 0.0), // From the right
///     order: 1,
/// };
///
/// const FRAME_SIZE: usize = 1024;
/// let input = vec![0.5; FRAME_SIZE]; // Mono
/// let input_buffer = AudioBuffer::try_with_data(&input)?;
/// const NUM_CHANNELS: u32 = num_ambisonics_channels(1); // 4 channels (1st order)
/// let mut output = vec![0.0; NUM_CHANNELS as usize * FRAME_SIZE];
/// let output_buffer = AudioBuffer::try_with_data_and_settings(
///     &mut output,
///     AudioBufferSettings::with_num_channels(NUM_CHANNELS)
/// )?;
///
/// let _ = effect.apply(&params, &input_buffer, &output_buffer);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug)]
pub struct AmbisonicsEncodeEffect {
    inner: audionimbus_sys::IPLAmbisonicsEncodeEffect,

    /// Ambisonics order specified when creating the effect.
    max_order: u32,
}

impl AmbisonicsEncodeEffect {
    /// Creates a new ambisonics encode effect.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if effect creation fails.
    pub fn try_new(
        context: &Context,
        audio_settings: &AudioSettings,
        ambisonics_encode_effect_settings: &AmbisonicsEncodeEffectSettings,
    ) -> Result<Self, SteamAudioError> {
        let mut inner = std::ptr::null_mut();

        let status = unsafe {
            audionimbus_sys::iplAmbisonicsEncodeEffectCreate(
                context.raw_ptr(),
                &mut audionimbus_sys::IPLAudioSettings::from(audio_settings),
                &mut audionimbus_sys::IPLAmbisonicsEncodeEffectSettings::from(
                    ambisonics_encode_effect_settings,
                ),
                &mut inner,
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        let ambisonics_encode_effect = Self {
            inner,
            max_order: ambisonics_encode_effect_settings.max_order,
        };

        Ok(ambisonics_encode_effect)
    }

    /// Applies an ambisonics encode effect to an audio buffer.
    ///
    /// This effect CANNOT be applied in-place.
    ///
    /// The input audio buffer must have 1 channel, and the output audio buffer must have as many
    /// channels as needed for the Ambisonics order used (see [`crate::num_ambisonics_channels`]).
    ///
    /// # Errors
    ///
    /// Returns [`EffectError`] if:
    /// - The input buffer does not have exactly 1 channel
    /// - The output buffer does not have the correct number of channels for the Ambisonics order
    pub fn apply<I, O, PI: ChannelPointers, PO: ChannelPointers>(
        &mut self,
        ambisonics_encode_effect_params: &AmbisonicsEncodeEffectParams,
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

        let required_num_channels = num_ambisonics_channels(self.max_order);
        let num_output_channels = output_buffer.num_channels();
        if num_output_channels != required_num_channels {
            return Err(EffectError::InvalidAmbisonicOrder {
                order: self.max_order,
                buffer_channels: num_output_channels,
                required_channels: required_num_channels,
            });
        }

        let state = unsafe {
            audionimbus_sys::iplAmbisonicsEncodeEffectApply(
                self.raw_ptr(),
                &mut *ambisonics_encode_effect_params.as_ffi(),
                &mut *input_buffer.as_ffi(),
                &mut *output_buffer.as_ffi(),
            )
        }
        .into();

        Ok(state)
    }

    /// Retrieves a single frame of tail samples from an Ambisonics encode effect’s internal buffers.
    ///
    /// After the input to the Ambisonics encode effect has stopped, this function must be called instead of [`Self::apply`] until the return value indicates that no more tail samples remain.
    ///
    /// The output audio buffer must have as many channels as needed for the Ambisonics order specified when creating the effect.
    ///
    /// # Errors
    ///
    /// Returns [`EffectError`] if the output buffer does not have the correct number of channels for the Ambisonics order.
    pub fn tail<O>(&self, output_buffer: &AudioBuffer<O>) -> Result<AudioEffectState, EffectError>
    where
        O: AsRef<[Sample]> + AsMut<[Sample]>,
    {
        let required_num_channels = num_ambisonics_channels(self.max_order);
        let num_output_channels = output_buffer.num_channels();
        if num_output_channels != required_num_channels {
            return Err(EffectError::InvalidAmbisonicOrder {
                order: self.max_order,
                buffer_channels: num_output_channels,
                required_channels: required_num_channels,
            });
        }

        let state = unsafe {
            audionimbus_sys::iplAmbisonicsEncodeEffectGetTail(
                self.raw_ptr(),
                &mut *output_buffer.as_ffi(),
            )
        }
        .into();

        Ok(state)
    }

    /// Returns the number of tail samples remaining in an Ambisonics encode effect’s internal buffers.
    ///
    /// Tail samples are audio samples that should be played even after the input to the effect has stopped playing and no further input samples are available.
    pub fn tail_size(&self) -> usize {
        unsafe { audionimbus_sys::iplAmbisonicsEncodeEffectGetTailSize(self.raw_ptr()) as usize }
    }

    /// Resets the internal processing state of an ambisonics encode effect.
    pub fn reset(&mut self) {
        unsafe { audionimbus_sys::iplAmbisonicsEncodeEffectReset(self.raw_ptr()) };
    }

    /// Returns the raw FFI pointer to the underlying ambisonics encode effect.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub fn raw_ptr(&self) -> audionimbus_sys::IPLAmbisonicsEncodeEffect {
        self.inner
    }

    /// Returns a mutable reference to the raw FFI pointer.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLAmbisonicsEncodeEffect {
        &mut self.inner
    }
}

impl Clone for AmbisonicsEncodeEffect {
    fn clone(&self) -> Self {
        unsafe {
            audionimbus_sys::iplAmbisonicsEncodeEffectRetain(self.inner);
        }

        Self {
            inner: self.inner,
            max_order: self.max_order,
        }
    }
}

impl Drop for AmbisonicsEncodeEffect {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplAmbisonicsEncodeEffectRelease(&mut self.inner) }
    }
}

unsafe impl Send for AmbisonicsEncodeEffect {}
unsafe impl Sync for AmbisonicsEncodeEffect {}

/// Settings used to create an ambisonics decode effect.
#[derive(Debug)]
pub struct AmbisonicsEncodeEffectSettings {
    /// The maximum ambisonics order that will be used by input audio buffers.
    /// Maximum ambisonics order to encode audio buffers to.
    pub max_order: u32,
}

impl From<&AmbisonicsEncodeEffectSettings> for audionimbus_sys::IPLAmbisonicsEncodeEffectSettings {
    fn from(settings: &AmbisonicsEncodeEffectSettings) -> Self {
        Self {
            maxOrder: settings.max_order as i32,
        }
    }
}

/// Parameters for applying an ambisonics encode effect to an audio buffer.
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
    pub order: u32,
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

            let mut effect = AmbisonicsEncodeEffect::try_new(
                &context,
                &audio_settings,
                &AmbisonicsEncodeEffectSettings { max_order: 1 },
            )
            .unwrap();

            let params = AmbisonicsEncodeEffectParams {
                direction: Direction::new(1.0, 0.0, 0.0),
                order: 1,
            };

            let input = vec![0.5; 1024];
            let input_buffer = AudioBuffer::try_with_data(&input).unwrap();

            let mut output = vec![0.0; 4 * 1024];
            let output_buffer = AudioBuffer::try_with_data_and_settings(
                &mut output,
                AudioBufferSettings::with_num_channels(4),
            )
            .unwrap();

            assert!(effect.apply(&params, &input_buffer, &output_buffer).is_ok());
        }

        #[test]
        fn test_invalid_input_channels() {
            let context = Context::default();
            let audio_settings = AudioSettings::default();

            let mut effect = AmbisonicsEncodeEffect::try_new(
                &context,
                &audio_settings,
                &AmbisonicsEncodeEffectSettings { max_order: 1 },
            )
            .unwrap();

            let params = AmbisonicsEncodeEffectParams {
                direction: Direction::new(1.0, 0.0, 0.0),
                order: 1,
            };

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

            let mut effect = AmbisonicsEncodeEffect::try_new(
                &context,
                &audio_settings,
                &AmbisonicsEncodeEffectSettings { max_order: 1 },
            )
            .unwrap();

            let params = AmbisonicsEncodeEffectParams {
                direction: Direction::new(1.0, 0.0, 0.0),
                order: 1,
            };

            let input = vec![0.5; 1024];
            let input_buffer = AudioBuffer::try_with_data(&input).unwrap();

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
                    required_channels: 4
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

            let effect = AmbisonicsEncodeEffect::try_new(
                &context,
                &audio_settings,
                &AmbisonicsEncodeEffectSettings { max_order: 1 },
            )
            .unwrap();

            let mut output = vec![0.0; 4 * 1024];
            let output_buffer = AudioBuffer::try_with_data_and_settings(
                &mut output,
                AudioBufferSettings::with_num_channels(4),
            )
            .unwrap();

            assert!(effect.tail(&output_buffer).is_ok());
        }

        #[test]
        fn test_invalid_output_channels() {
            let context = Context::default();
            let audio_settings = AudioSettings::default();

            let effect = AmbisonicsEncodeEffect::try_new(
                &context,
                &audio_settings,
                &AmbisonicsEncodeEffectSettings { max_order: 1 },
            )
            .unwrap();

            let mut output = vec![0.0; 2 * 1024];
            let output_buffer = AudioBuffer::try_with_data_and_settings(
                &mut output,
                AudioBufferSettings::with_num_channels(2),
            )
            .unwrap();

            assert_eq!(
                effect.tail(&output_buffer),
                Err(EffectError::InvalidAmbisonicOrder {
                    order: 1,
                    buffer_channels: 2,
                    required_channels: 4
                })
            );
        }
    }
}
