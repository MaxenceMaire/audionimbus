use super::super::AudioEffectState;
use super::SpeakerLayout;
use crate::audio_buffer::{AudioBuffer, Sample};
use crate::audio_settings::AudioSettings;
use crate::context::Context;
use crate::error::{to_option_error, SteamAudioError};
use crate::ffi_wrapper::FFIWrapper;
use crate::geometry::CoordinateSystem;
use crate::hrtf::Hrtf;
use crate::ChannelPointers;

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
///     }
/// )?;
///
/// let params = AmbisonicsDecodeEffectParams {
///     order: 1,
///     hrtf: &hrtf,
///     orientation: CoordinateSystem::default(),
///     binaural: true,
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
pub struct AmbisonicsDecodeEffect(audionimbus_sys::IPLAmbisonicsDecodeEffect);

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
        let mut ambisonics_decode_effect = Self(std::ptr::null_mut());

        let status = unsafe {
            audionimbus_sys::iplAmbisonicsDecodeEffectCreate(
                context.raw_ptr(),
                &mut audionimbus_sys::IPLAudioSettings::from(audio_settings),
                &mut audionimbus_sys::IPLAmbisonicsDecodeEffectSettings::from(
                    ambisonics_decode_effect_settings,
                ),
                ambisonics_decode_effect.raw_ptr_mut(),
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        Ok(ambisonics_decode_effect)
    }

    /// Applies an ambisonics decode effect to an audio buffer.
    ///
    /// This effect CANNOT be applied in-place.
    pub fn apply<I, O, PI: ChannelPointers, PO: ChannelPointers>(
        &mut self,
        ambisonics_decode_effect_params: &AmbisonicsDecodeEffectParams,
        input_buffer: &AudioBuffer<I, PI>,
        output_buffer: &AudioBuffer<O, PO>,
    ) -> AudioEffectState
    where
        I: AsRef<[Sample]>,
        O: AsRef<[Sample]> + AsMut<[Sample]>,
    {
        let required_num_channels = (ambisonics_decode_effect_params.order + 1).pow(2);
        assert_eq!(
            input_buffer.num_channels(),
            required_num_channels,
            "ambisonic order N = {} requires (N + 1)^2 = {} input channels",
            ambisonics_decode_effect_params.order,
            required_num_channels
        );

        unsafe {
            audionimbus_sys::iplAmbisonicsDecodeEffectApply(
                self.raw_ptr(),
                &mut *ambisonics_decode_effect_params.as_ffi(),
                &mut *input_buffer.as_ffi(),
                &mut *output_buffer.as_ffi(),
            )
        }
        .into()
    }

    /// Retrieves a single frame of tail samples from an Ambisonics decode effect’s internal buffers.
    ///
    /// After the input to the Ambisonics decode effect has stopped, this function must be called instead of [`Self::apply`] until the return value indicates that no more tail samples remain.
    ///
    /// The output audio buffer must have as many channels as needed for the speaker layout specified when creating the effect (if using panning) or 2 channels (if using binaural rendering).
    pub fn tail<O>(&self, output_buffer: &AudioBuffer<O>) -> AudioEffectState
    where
        O: AsRef<[Sample]> + AsMut<[Sample]>,
    {
        unsafe {
            audionimbus_sys::iplAmbisonicsDecodeEffectGetTail(
                self.raw_ptr(),
                &mut *output_buffer.as_ffi(),
            )
        }
        .into()
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
        self.0
    }

    /// Returns a mutable reference to the raw FFI pointer.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLAmbisonicsDecodeEffect {
        &mut self.0
    }
}

impl From<audionimbus_sys::IPLAmbisonicsDecodeEffect> for AmbisonicsDecodeEffect {
    fn from(ptr: audionimbus_sys::IPLAmbisonicsDecodeEffect) -> Self {
        Self(ptr)
    }
}

impl Clone for AmbisonicsDecodeEffect {
    fn clone(&self) -> Self {
        unsafe {
            audionimbus_sys::iplAmbisonicsDecodeEffectRetain(self.0);
        }
        Self(self.0)
    }
}

impl Drop for AmbisonicsDecodeEffect {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplAmbisonicsDecodeEffectRelease(&mut self.0) }
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

    /// Whether to use binaural rendering or panning.
    pub binaural: bool,
}

impl AmbisonicsDecodeEffectParams<'_> {
    pub(crate) fn as_ffi(
        &self,
    ) -> FFIWrapper<'_, audionimbus_sys::IPLAmbisonicsDecodeEffectParams, Self> {
        let ambisonics_decode_effect_params = audionimbus_sys::IPLAmbisonicsDecodeEffectParams {
            order: self.order as i32,
            hrtf: self.hrtf.raw_ptr(),
            orientation: self.orientation.into(),
            binaural: if self.binaural {
                audionimbus_sys::IPLbool::IPL_TRUE
            } else {
                audionimbus_sys::IPLbool::IPL_FALSE
            },
        };

        FFIWrapper::new(ambisonics_decode_effect_params)
    }
}
