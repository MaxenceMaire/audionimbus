use super::super::{AudioEffectState, EffectError};
use crate::audio_buffer::{AudioBuffer, Sample};
use crate::audio_settings::AudioSettings;
use crate::context::Context;
use crate::error::{to_option_error, SteamAudioError};
use crate::ffi_wrapper::FFIWrapper;
use crate::geometry::CoordinateSystem;
use crate::num_ambisonics_channels;
use crate::{ChannelPointers, ChannelRequirement};

/// Applies a rotation to an ambisonics audio buffer.
///
/// The input buffer is assumed to describe a sound field in "world space".
/// The output buffer is then the same sound field, but expressed relative to the listener’s orientation.
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
/// let mut effect = AmbisonicsRotationEffect::try_new(
///     &context,
///     &audio_settings,
///     &AmbisonicsRotationEffectSettings {
///         max_order: 1,
///     }
/// )?;
///
/// let params = AmbisonicsRotationEffectParams {
///     orientation: CoordinateSystem::default(), // Identity orientation
///     order: 1,
/// };
///
/// const FRAME_SIZE: usize = 1024;
/// let input = vec![0.5; 4 * FRAME_SIZE]; // 4 channels
/// let input_buffer = AudioBuffer::try_with_data_and_settings(
///     &input,
///     AudioBufferSettings::with_num_channels(4)
/// )?;
/// let mut output = vec![0.0; 4 * FRAME_SIZE];
/// let output_buffer = AudioBuffer::try_with_data_and_settings(
///     &mut output,
///     AudioBufferSettings::with_num_channels(4)
/// )?;
///
/// let _ = effect.apply(&params, &input_buffer, &output_buffer);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug)]
pub struct AmbisonicsRotationEffect {
    inner: audionimbus_sys::IPLAmbisonicsRotationEffect,

    /// The number of input and output channels needed for the ambisonics order used when creating
    /// the effect.
    num_channels: u32,
}

impl AmbisonicsRotationEffect {
    /// Creates a new ambisonics rotation effect.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if effect creation fails.
    pub fn try_new(
        context: &Context,
        audio_settings: &AudioSettings,
        ambisonics_rotation_effect_settings: &AmbisonicsRotationEffectSettings,
    ) -> Result<Self, SteamAudioError> {
        let mut inner = std::ptr::null_mut();

        let status = unsafe {
            audionimbus_sys::iplAmbisonicsRotationEffectCreate(
                context.raw_ptr(),
                &mut audionimbus_sys::IPLAudioSettings::from(audio_settings),
                &mut audionimbus_sys::IPLAmbisonicsRotationEffectSettings::from(
                    ambisonics_rotation_effect_settings,
                ),
                &mut inner,
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        let num_channels = num_ambisonics_channels(ambisonics_rotation_effect_settings.max_order);
        let ambisonics_rotation_effect = Self {
            inner,
            num_channels,
        };

        Ok(ambisonics_rotation_effect)
    }

    /// Applies an ambisonics rotation effect to an audio buffer.
    ///
    /// This effect CANNOT be applied in-place.
    ///
    /// Both input and output audio buffers must have as many channels as needed for the
    /// Ambisonics order used (see [`crate::num_ambisonics_channels`]).
    ///
    /// # Errors
    ///
    /// Returns [`EffectError`] if:
    /// - The input buffer does not have the correct number of channels for the Ambisonics order
    /// - The output buffer does not have the correct number of channels for the Ambisonics order
    pub fn apply<I, O, PI: ChannelPointers, PO: ChannelPointers>(
        &mut self,
        ambisonics_rotation_effect_params: &AmbisonicsRotationEffectParams,
        input_buffer: &AudioBuffer<I, PI>,
        output_buffer: &AudioBuffer<O, PO>,
    ) -> Result<AudioEffectState, EffectError>
    where
        I: AsRef<[Sample]>,
        O: AsRef<[Sample]> + AsMut<[Sample]>,
    {
        let num_input_channels = input_buffer.num_channels();
        if num_input_channels != self.num_channels {
            return Err(EffectError::InvalidInputChannels {
                expected: ChannelRequirement::Exactly(self.num_channels),
                actual: num_input_channels,
            });
        }

        let num_output_channels = output_buffer.num_channels();
        if num_output_channels != self.num_channels {
            return Err(EffectError::InvalidOutputChannels {
                expected: ChannelRequirement::Exactly(self.num_channels),
                actual: num_output_channels,
            });
        }

        let state = unsafe {
            audionimbus_sys::iplAmbisonicsRotationEffectApply(
                self.raw_ptr(),
                &mut *ambisonics_rotation_effect_params.as_ffi(),
                &mut *input_buffer.as_ffi(),
                &mut *output_buffer.as_ffi(),
            )
        }
        .into();

        Ok(state)
    }

    /// Retrieves a single frame of tail samples from an Ambisonics rotation effect’s internal buffers.
    ///
    /// After the input to the Ambisonics rotation effect has stopped, this function must be called instead of [`Self::apply`] until the return value indicates that no more tail samples remain.
    ///
    /// The output audio buffer must have as many channels as needed for the Ambisonics order specified when creating the effect.
    ///
    /// # Errors
    ///
    /// Returns [`EffectError`] if the output buffer does not have the correct number of channels
    /// for the Ambisonics order.
    pub fn tail<O>(&self, output_buffer: &AudioBuffer<O>) -> Result<AudioEffectState, EffectError>
    where
        O: AsRef<[Sample]> + AsMut<[Sample]>,
    {
        let num_output_channels = output_buffer.num_channels();
        if num_output_channels != self.num_channels {
            return Err(EffectError::InvalidOutputChannels {
                expected: ChannelRequirement::Exactly(self.num_channels),
                actual: num_output_channels,
            });
        }

        let state = unsafe {
            audionimbus_sys::iplAmbisonicsRotationEffectGetTail(
                self.raw_ptr(),
                &mut *output_buffer.as_ffi(),
            )
        }
        .into();

        Ok(state)
    }

    /// Returns the number of tail samples remaining in an Ambisonics rotation effect’s internal buffers.
    ///
    /// Tail samples are audio samples that should be played even after the input to the effect has stopped playing and no further input samples are available.
    pub fn tail_size(&self) -> usize {
        unsafe { audionimbus_sys::iplAmbisonicsRotationEffectGetTailSize(self.raw_ptr()) as usize }
    }

    /// Resets the internal processing state of an ambisonics rotation effect.
    pub fn reset(&mut self) {
        unsafe { audionimbus_sys::iplAmbisonicsRotationEffectReset(self.raw_ptr()) };
    }

    /// Returns the raw FFI pointer to the underlying ambisonics rotation effect.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub fn raw_ptr(&self) -> audionimbus_sys::IPLAmbisonicsRotationEffect {
        self.inner
    }

    /// Returns a mutable reference to the raw FFI pointer.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLAmbisonicsRotationEffect {
        &mut self.inner
    }
}

impl Clone for AmbisonicsRotationEffect {
    fn clone(&self) -> Self {
        unsafe {
            audionimbus_sys::iplAmbisonicsRotationEffectRetain(self.inner);
        }

        Self {
            inner: self.inner,
            num_channels: self.num_channels,
        }
    }
}

impl Drop for AmbisonicsRotationEffect {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplAmbisonicsRotationEffectRelease(&mut self.inner) }
    }
}

unsafe impl Send for AmbisonicsRotationEffect {}
unsafe impl Sync for AmbisonicsRotationEffect {}

/// Settings used to create an ambisonics rotation effect.
#[derive(Debug)]
pub struct AmbisonicsRotationEffectSettings {
    /// The maximum ambisonics order that will be used by input audio buffers.
    pub max_order: u32,
}

impl From<&AmbisonicsRotationEffectSettings>
    for audionimbus_sys::IPLAmbisonicsRotationEffectSettings
{
    fn from(settings: &AmbisonicsRotationEffectSettings) -> Self {
        Self {
            maxOrder: settings.max_order as i32,
        }
    }
}

/// Parameters for applying an ambisonics rotation effect to an audio buffer.
#[derive(Debug)]
pub struct AmbisonicsRotationEffectParams {
    /// The orientation of the listener.
    pub orientation: CoordinateSystem,

    /// Ambisonic order of the input and output buffers.
    ///
    /// May be less than the `max_order` specified when creating the effect, in which case the effect will process fewer channels, reducing CPU usage.
    pub order: u32,
}

impl AmbisonicsRotationEffectParams {
    pub(crate) fn as_ffi(
        &self,
    ) -> FFIWrapper<'_, audionimbus_sys::IPLAmbisonicsRotationEffectParams, Self> {
        let ambisonics_rotation_effect_params =
            audionimbus_sys::IPLAmbisonicsRotationEffectParams {
                orientation: self.orientation.into(),
                order: self.order as i32,
            };

        FFIWrapper::new(ambisonics_rotation_effect_params)
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

            let mut effect = AmbisonicsRotationEffect::try_new(
                &context,
                &audio_settings,
                &AmbisonicsRotationEffectSettings { max_order: 1 },
            )
            .unwrap();

            let params = AmbisonicsRotationEffectParams {
                orientation: CoordinateSystem::default(),
                order: 1,
            };

            let input = vec![0.5; 4 * 1024];
            let input_buffer = AudioBuffer::try_with_data_and_settings(
                &input,
                AudioBufferSettings::with_num_channels(4),
            )
            .unwrap();

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

            let mut effect = AmbisonicsRotationEffect::try_new(
                &context,
                &audio_settings,
                &AmbisonicsRotationEffectSettings { max_order: 1 },
            )
            .unwrap();

            let params = AmbisonicsRotationEffectParams {
                orientation: CoordinateSystem::default(),
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
                    expected: ChannelRequirement::Exactly(4),
                    actual: 2
                })
            );
        }

        #[test]
        fn test_invalid_output_channels() {
            let context = Context::default();
            let audio_settings = AudioSettings::default();

            let mut effect = AmbisonicsRotationEffect::try_new(
                &context,
                &audio_settings,
                &AmbisonicsRotationEffectSettings { max_order: 1 },
            )
            .unwrap();

            let params = AmbisonicsRotationEffectParams {
                orientation: CoordinateSystem::default(),
                order: 1,
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
                    expected: ChannelRequirement::Exactly(4),
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

            let effect = AmbisonicsRotationEffect::try_new(
                &context,
                &audio_settings,
                &AmbisonicsRotationEffectSettings { max_order: 1 },
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

            let effect = AmbisonicsRotationEffect::try_new(
                &context,
                &audio_settings,
                &AmbisonicsRotationEffectSettings { max_order: 1 },
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
                Err(EffectError::InvalidOutputChannels {
                    expected: ChannelRequirement::Exactly(4),
                    actual: 2,
                })
            );
        }
    }
}
