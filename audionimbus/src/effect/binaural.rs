use super::audio_effect_state::AudioEffectState;
use super::EffectError;
use crate::audio_buffer::{AudioBuffer, Sample};
use crate::audio_settings::AudioSettings;
use crate::context::Context;
use crate::error::{to_option_error, SteamAudioError};
use crate::ffi_wrapper::FFIWrapper;
use crate::geometry::Direction;
use crate::hrtf::{Hrtf, HrtfInterpolation};
use crate::{ChannelPointers, ChannelRequirement};

/// Spatializes a point source using an HRTF, based on the 3D position of the source relative to the listener.
///
/// The source audio can be 1- or 2-channel; in either case all input channels are spatialized from the same position.
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
/// let mut effect = BinauralEffect::try_new(
///     &context,
///     &audio_settings,
///     &BinauralEffectSettings { hrtf: &hrtf },
/// )?;
///
/// let params = BinauralEffectParams {
///     direction: Direction::new(1.0, 0.0, 0.0), // Sound from the right
///     interpolation: HrtfInterpolation::Nearest,
///     spatial_blend: 1.0,
///     hrtf: &hrtf,
///     peak_delays: None,
/// };
///
/// let input_buffer = AudioBuffer::try_with_data([1.0; 1024])?;
/// let mut output_container = vec![0.0; 2 * input_buffer.num_samples() as usize];
/// let mut output_buffer = AudioBuffer::try_with_data_and_settings(
///     &mut output_container,
///     AudioBufferSettings::with_num_channels(2),
/// )?;
///
/// let _ = effect.apply(&params, &input_buffer, &mut output_buffer);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug)]
pub struct BinauralEffect(audionimbus_sys::IPLBinauralEffect);

impl BinauralEffect {
    /// Creates a new binaural effect.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if effect creation fails.
    pub fn try_new(
        context: &Context,
        audio_settings: &AudioSettings,
        binaural_effect_settings: &BinauralEffectSettings,
    ) -> Result<Self, SteamAudioError> {
        let mut binaural_effect = Self(std::ptr::null_mut());

        let status = unsafe {
            audionimbus_sys::iplBinauralEffectCreate(
                context.raw_ptr(),
                &mut audionimbus_sys::IPLAudioSettings::from(audio_settings),
                &mut audionimbus_sys::IPLBinauralEffectSettings::from(binaural_effect_settings),
                binaural_effect.raw_ptr_mut(),
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        Ok(binaural_effect)
    }

    /// Applies a binaural effect to an audio buffer.
    ///
    /// This effect CANNOT be applied in-place.
    ///
    /// The input audio buffer must have 1 or 2 channels, and the output audio buffer must have 2 channels.
    ///
    /// # Errors
    ///
    /// Returns [`EffectError`] if:
    /// - The input buffer has more than 2 channels
    /// - The output buffer does not have exactly 2 channels
    pub fn apply<I, O, PI: ChannelPointers, PO: ChannelPointers>(
        &mut self,
        binaural_effect_params: &BinauralEffectParams,
        input_buffer: &AudioBuffer<I, PI>,
        output_buffer: &AudioBuffer<O, PO>,
    ) -> Result<AudioEffectState, EffectError>
    where
        I: AsRef<[Sample]>,
        O: AsRef<[Sample]> + AsMut<[Sample]>,
    {
        let num_input_channels = input_buffer.num_channels();
        if !(1..=2).contains(&num_input_channels) {
            return Err(EffectError::InvalidInputChannels {
                expected: ChannelRequirement::Range { min: 1, max: 2 },
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
            audionimbus_sys::iplBinauralEffectApply(
                self.raw_ptr(),
                &mut *binaural_effect_params.as_ffi(),
                &mut *input_buffer.as_ffi(),
                &mut *output_buffer.as_ffi(),
            )
        }
        .into();

        Ok(state)
    }

    /// Retrieves a single frame of tail samples from a binaural effect’s internal buffers.
    ///
    /// After the input to the binaural effect has stopped, this function must be called instead of [`Self::apply`] until the return value indicates that no more tail samples remain.
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
            audionimbus_sys::iplBinauralEffectGetTail(self.raw_ptr(), &mut *output_buffer.as_ffi())
        }
        .into();

        Ok(state)
    }

    /// Returns the number of tail samples remaining in a binaural effect’s internal buffers.
    ///
    /// Tail samples are audio samples that should be played even after the input to the effect has stopped playing and no further input samples are available.
    pub fn tail_size(&self) -> usize {
        unsafe { audionimbus_sys::iplBinauralEffectGetTailSize(self.raw_ptr()) as usize }
    }

    /// Resets the internal processing state of a binaural effect.
    pub fn reset(&mut self) {
        unsafe { audionimbus_sys::iplBinauralEffectReset(self.raw_ptr()) };
    }

    /// Returns the raw FFI pointer to the underlying binaural effect.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub fn raw_ptr(&self) -> audionimbus_sys::IPLBinauralEffect {
        self.0
    }

    /// Returns a mutable reference to the raw FFI pointer.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLBinauralEffect {
        &mut self.0
    }
}

impl Clone for BinauralEffect {
    fn clone(&self) -> Self {
        unsafe {
            audionimbus_sys::iplBinauralEffectRetain(self.0);
        }
        Self(self.0)
    }
}

impl Drop for BinauralEffect {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplBinauralEffectRelease(&mut self.0) }
    }
}

unsafe impl Send for BinauralEffect {}
unsafe impl Sync for BinauralEffect {}

/// Settings used to create a binaural effect.
#[derive(Debug)]
pub struct BinauralEffectSettings<'a> {
    /// The HRTF to use.
    pub hrtf: &'a Hrtf,
}

impl From<&BinauralEffectSettings<'_>> for audionimbus_sys::IPLBinauralEffectSettings {
    fn from(settings: &BinauralEffectSettings) -> Self {
        Self {
            hrtf: settings.hrtf.raw_ptr(),
        }
    }
}

/// Parameters for applying an ambisonics binaural effect to an audio buffer.
#[derive(Debug)]
pub struct BinauralEffectParams<'a> {
    /// Unit vector pointing from the listener towards the source.
    pub direction: Direction,

    /// The interpolation technique to use.
    pub interpolation: HrtfInterpolation,

    /// Amount to blend input audio with spatialized audio.
    ///
    /// When set to 0.0, output audio is not spatialized at all and is close to input audio.
    /// If set to 1.0, output audio is fully spatialized.
    pub spatial_blend: f32,

    /// The HRTF to use.
    pub hrtf: &'a Hrtf,

    /// Optional left- and right-ear peak delays for the HRTF used to spatialize the input audio.
    /// Can be None, in which case peak delays will not be written.
    pub peak_delays: Option<[f32; 2]>,
}

impl BinauralEffectParams<'_> {
    pub(crate) fn as_ffi(&self) -> FFIWrapper<'_, audionimbus_sys::IPLBinauralEffectParams, Self> {
        let peak_delays_ptr = self
            .peak_delays
            .as_ref()
            .map(|peak_delays| peak_delays.as_ptr() as *mut f32)
            .unwrap_or(std::ptr::null_mut());

        let binaural_effect_params = audionimbus_sys::IPLBinauralEffectParams {
            direction: self.direction.into(),
            interpolation: self.interpolation.into(),
            spatialBlend: self.spatial_blend,
            hrtf: self.hrtf.raw_ptr(),
            peakDelays: peak_delays_ptr,
        };

        FFIWrapper::new(binaural_effect_params)
    }
}

#[cfg(test)]
mod tests {
    use crate::*;

    mod apply {
        use super::*;

        #[test]
        fn test_valid_mono_input() {
            let context = Context::default();
            let audio_settings = AudioSettings::default();
            let hrtf = Hrtf::try_new(&context, &audio_settings, &HrtfSettings::default()).unwrap();

            let mut effect = BinauralEffect::try_new(
                &context,
                &audio_settings,
                &BinauralEffectSettings { hrtf: &hrtf },
            )
            .unwrap();

            let params = BinauralEffectParams {
                direction: Direction::new(1.0, 0.0, 0.0),
                interpolation: HrtfInterpolation::Nearest,
                spatial_blend: 1.0,
                hrtf: &hrtf,
                peak_delays: None,
            };

            let input = vec![0.5; 1024];
            let input_buffer = AudioBuffer::try_with_data(&input).unwrap();
            let mut output = vec![0.0; 2 * 1024];
            let output_buffer = AudioBuffer::try_with_data_and_settings(
                &mut output,
                AudioBufferSettings::with_num_channels(2),
            )
            .unwrap();

            assert!(effect.apply(&params, &input_buffer, &output_buffer).is_ok());
        }

        #[test]
        fn test_valid_stereo_input() {
            let context = Context::default();
            let audio_settings = AudioSettings::default();
            let hrtf = Hrtf::try_new(&context, &audio_settings, &HrtfSettings::default()).unwrap();

            let mut effect = BinauralEffect::try_new(
                &context,
                &audio_settings,
                &BinauralEffectSettings { hrtf: &hrtf },
            )
            .unwrap();

            let params = BinauralEffectParams {
                direction: Direction::new(1.0, 0.0, 0.0),
                interpolation: HrtfInterpolation::Nearest,
                spatial_blend: 1.0,
                hrtf: &hrtf,
                peak_delays: None,
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

            assert!(effect.apply(&params, &input_buffer, &output_buffer).is_ok());
        }

        #[test]
        fn test_invalid_input_num_channels() {
            let context = Context::default();
            let audio_settings = AudioSettings::default();
            let hrtf = Hrtf::try_new(&context, &audio_settings, &HrtfSettings::default()).unwrap();

            let mut effect = BinauralEffect::try_new(
                &context,
                &audio_settings,
                &BinauralEffectSettings { hrtf: &hrtf },
            )
            .unwrap();

            let params = BinauralEffectParams {
                direction: Direction::new(1.0, 0.0, 0.0),
                interpolation: HrtfInterpolation::Nearest,
                spatial_blend: 1.0,
                hrtf: &hrtf,
                peak_delays: None,
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

            assert_eq!(
                effect.apply(&params, &input_buffer, &output_buffer),
                Err(EffectError::InvalidInputChannels {
                    expected: ChannelRequirement::Range { min: 1, max: 2 },
                    actual: 4
                })
            );
        }

        #[test]
        fn test_invalid_output_num_channels() {
            let context = Context::default();
            let audio_settings = AudioSettings::default();
            let hrtf = Hrtf::try_new(&context, &audio_settings, &HrtfSettings::default()).unwrap();

            let mut effect = BinauralEffect::try_new(
                &context,
                &audio_settings,
                &BinauralEffectSettings { hrtf: &hrtf },
            )
            .unwrap();

            let params = BinauralEffectParams {
                direction: Direction::new(1.0, 0.0, 0.0),
                interpolation: HrtfInterpolation::Nearest,
                spatial_blend: 1.0,
                hrtf: &hrtf,
                peak_delays: None,
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

            let effect = BinauralEffect::try_new(
                &context,
                &audio_settings,
                &BinauralEffectSettings { hrtf: &hrtf },
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
        fn test_invalid_output_num_channels() {
            let context = Context::default();
            let audio_settings = AudioSettings::default();
            let hrtf = Hrtf::try_new(&context, &audio_settings, &HrtfSettings::default()).unwrap();

            let effect = BinauralEffect::try_new(
                &context,
                &audio_settings,
                &BinauralEffectSettings { hrtf: &hrtf },
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
