use super::audio_effect_state::AudioEffectState;
use super::SpeakerLayout;
use crate::audio_buffer::{AudioBuffer, Sample};
use crate::audio_settings::AudioSettings;
use crate::context::Context;
use crate::error::{to_option_error, SteamAudioError};
use crate::ffi_wrapper::FFIWrapper;
use crate::{ChannelPointers, Hrtf};

/// Spatializes multi-channel speaker-based audio (e.g., stereo, quadraphonic, 5.1, or 7.1) using HRTF-based binaural rendering.
///
/// The audio signal for each speaker is spatialized from a point in space corresponding to the speaker’s location.
/// This allows users to experience a surround sound mix over regular stereo headphones.
///
/// Virtual surround is also a fast way to get approximate binaural rendering.
/// All sources can be panned to some surround format (say, 7.1).
/// After the sources are mixed, the mix can be rendered using virtual surround.
/// This can reduce CPU usage, at the cost of spatialization accuracy.
#[derive(Debug)]
pub struct VirtualSurroundEffect(audionimbus_sys::IPLVirtualSurroundEffect);

impl VirtualSurroundEffect {
    pub fn try_new(
        context: &Context,
        audio_settings: &AudioSettings,
        virtual_surround_effect_settings: &VirtualSurroundEffectSettings,
    ) -> Result<Self, SteamAudioError> {
        let mut virtual_surround_effect = Self(std::ptr::null_mut());

        let status = unsafe {
            audionimbus_sys::iplVirtualSurroundEffectCreate(
                context.raw_ptr(),
                &mut audionimbus_sys::IPLAudioSettings::from(audio_settings),
                &mut audionimbus_sys::IPLVirtualSurroundEffectSettings::from(
                    virtual_surround_effect_settings,
                ),
                virtual_surround_effect.raw_ptr_mut(),
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        Ok(virtual_surround_effect)
    }

    /// Applies a virtual surround effect to an audio buffer.
    ///
    /// This effect CANNOT be applied in-place.
    pub fn apply<I, O, PI: ChannelPointers, PO: ChannelPointers>(
        &mut self,
        virtual_surround_effect_params: &VirtualSurroundEffectParams,
        input_buffer: &AudioBuffer<I, PI>,
        output_buffer: &AudioBuffer<O, PO>,
    ) -> AudioEffectState
    where
        I: AsRef<[Sample]>,
        O: AsRef<[Sample]> + AsMut<[Sample]>,
    {
        unsafe {
            audionimbus_sys::iplVirtualSurroundEffectApply(
                self.raw_ptr(),
                &mut *virtual_surround_effect_params.as_ffi(),
                &mut *input_buffer.as_ffi(),
                &mut *output_buffer.as_ffi(),
            )
        }
        .into()
    }

    /// Retrieves a single frame of tail samples from a virtual surround effect’s internal buffers.
    ///
    /// After the input to the virtual surround effect has stopped, this function must be called instead of [`Self::apply`] until the return value indicates that no more tail samples remain.
    ///
    /// The output audio buffer must be 2-channel.
    pub fn tail<O>(&self, output_buffer: &AudioBuffer<O>) -> AudioEffectState
    where
        O: AsRef<[Sample]> + AsMut<[Sample]>,
    {
        assert_eq!(
            output_buffer.num_channels(),
            2,
            "input buffer must have 2 channels",
        );

        unsafe {
            audionimbus_sys::iplVirtualSurroundEffectGetTail(
                self.raw_ptr(),
                &mut *output_buffer.as_ffi(),
            )
        }
        .into()
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

    pub fn raw_ptr(&self) -> audionimbus_sys::IPLVirtualSurroundEffect {
        self.0
    }

    pub fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLVirtualSurroundEffect {
        &mut self.0
    }
}

impl Clone for VirtualSurroundEffect {
    fn clone(&self) -> Self {
        unsafe {
            audionimbus_sys::iplVirtualSurroundEffectRetain(self.0);
        }
        Self(self.0)
    }
}

impl Drop for VirtualSurroundEffect {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplVirtualSurroundEffectRelease(&mut self.0) }
    }
}

unsafe impl Send for VirtualSurroundEffect {}
unsafe impl Sync for VirtualSurroundEffect {}

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
