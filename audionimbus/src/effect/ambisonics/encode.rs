use super::super::AudioEffectState;
use crate::audio_buffer::{AudioBuffer, Sample};
use crate::audio_settings::AudioSettings;
use crate::context::Context;
use crate::error::{to_option_error, SteamAudioError};
use crate::ffi_wrapper::FFIWrapper;
use crate::geometry::Direction;
use crate::ChannelPointers;

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
pub struct AmbisonicsEncodeEffect(audionimbus_sys::IPLAmbisonicsEncodeEffect);

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

    /// Applies an ambisonics encode effect to an audio buffer.
    ///
    /// This effect CANNOT be applied in-place.
    pub fn apply<I, O, PI: ChannelPointers, PO: ChannelPointers>(
        &mut self,
        ambisonics_encode_effect_params: &AmbisonicsEncodeEffectParams,
        input_buffer: &AudioBuffer<I, PI>,
        output_buffer: &AudioBuffer<O, PO>,
    ) -> AudioEffectState
    where
        I: AsRef<[Sample]>,
        O: AsRef<[Sample]> + AsMut<[Sample]>,
    {
        assert_eq!(
            input_buffer.num_channels(),
            1,
            "input buffer must have 1 channel",
        );

        let required_num_channels = (ambisonics_encode_effect_params.order + 1).pow(2);
        assert_eq!(
            output_buffer.num_channels(),
            required_num_channels,
            "ambisonic order N = {} requires (N + 1)^2 = {} output channels",
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

    /// Retrieves a single frame of tail samples from an Ambisonics encode effect’s internal buffers.
    ///
    /// After the input to the Ambisonics encode effect has stopped, this function must be called instead of [`Self::apply`] until the return value indicates that no more tail samples remain.
    ///
    /// The output audio buffer must have as many channels as needed for the Ambisonics order specified when creating the effect.
    pub fn tail<O>(&self, output_buffer: &AudioBuffer<O>) -> AudioEffectState
    where
        O: AsRef<[Sample]> + AsMut<[Sample]>,
    {
        unsafe {
            audionimbus_sys::iplAmbisonicsEncodeEffectGetTail(
                self.raw_ptr(),
                &mut *output_buffer.as_ffi(),
            )
        }
        .into()
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

    pub fn raw_ptr(&self) -> audionimbus_sys::IPLAmbisonicsEncodeEffect {
        self.0
    }

    pub fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLAmbisonicsEncodeEffect {
        &mut self.0
    }
}

impl Clone for AmbisonicsEncodeEffect {
    fn clone(&self) -> Self {
        unsafe {
            audionimbus_sys::iplAmbisonicsEncodeEffectRetain(self.0);
        }
        Self(self.0)
    }
}

impl Drop for AmbisonicsEncodeEffect {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplAmbisonicsEncodeEffectRelease(&mut self.0) }
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
