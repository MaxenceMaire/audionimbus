use super::audio_effect_state::AudioEffectState;
use super::SpeakerLayout;
use crate::audio_buffer::{AudioBuffer, Sample};
use crate::audio_settings::AudioSettings;
use crate::context::Context;
use crate::error::{to_option_error, SteamAudioError};
use crate::ffi_wrapper::FFIWrapper;
use crate::geometry::Direction;
use crate::ChannelPointers;

/// Pans a single-channel point source to a multi-channel speaker layout based on the 3D position of the source relative to the listener.
#[derive(Debug)]
pub struct PanningEffect(audionimbus_sys::IPLPanningEffect);

impl PanningEffect {
    pub fn try_new(
        context: &Context,
        audio_settings: &AudioSettings,
        panning_effect_settings: &PanningEffectSettings,
    ) -> Result<Self, SteamAudioError> {
        let mut panning_effect = Self(std::ptr::null_mut());

        let status = unsafe {
            audionimbus_sys::iplPanningEffectCreate(
                context.raw_ptr(),
                &mut audionimbus_sys::IPLAudioSettings::from(audio_settings),
                &mut audionimbus_sys::IPLPanningEffectSettings::from(panning_effect_settings),
                panning_effect.raw_ptr_mut(),
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        Ok(panning_effect)
    }

    /// Applies a panning effect to an audio buffer.
    ///
    /// This effect CANNOT be applied in-place.
    pub fn apply<I, O, PI: ChannelPointers, PO: ChannelPointers>(
        &self,
        panning_effect_params: &PanningEffectParams,
        input_buffer: &AudioBuffer<I, PI>,
        output_buffer: &AudioBuffer<O, PO>,
    ) -> AudioEffectState
    where
        I: AsRef<[Sample]>,
        O: AsRef<[Sample]> + AsMut<[Sample]>,
    {
        unsafe {
            audionimbus_sys::iplPanningEffectApply(
                self.raw_ptr(),
                &mut *panning_effect_params.as_ffi(),
                &mut *input_buffer.as_ffi(),
                &mut *output_buffer.as_ffi(),
            )
        }
        .into()
    }

    /// Retrieves a single frame of tail samples from a panning effect’s internal buffers.
    ///
    /// After the input to the panning effect has stopped, this function must be called instead of [`Self::apply`] until the return value indicates that no more tail samples remain.
    ///
    /// The output audio buffer must have as many channels as needed for the speaker layout specified when creating the panning effect.
    pub fn tail<O>(&self, output_buffer: &AudioBuffer<O>) -> AudioEffectState
    where
        O: AsRef<[Sample]> + AsMut<[Sample]>,
    {
        unsafe {
            audionimbus_sys::iplPanningEffectGetTail(self.raw_ptr(), &mut *output_buffer.as_ffi())
        }
        .into()
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

    pub fn raw_ptr(&self) -> audionimbus_sys::IPLPanningEffect {
        self.0
    }

    pub fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLPanningEffect {
        &mut self.0
    }
}

impl Clone for PanningEffect {
    fn clone(&self) -> Self {
        unsafe {
            audionimbus_sys::iplPanningEffectRetain(self.0);
        }
        Self(self.0)
    }
}

impl Drop for PanningEffect {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplPanningEffectRelease(&mut self.0) }
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
