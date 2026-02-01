use super::audio_effect_state::AudioEffectState;
use super::{EffectError, Equalizer};
use crate::audio_buffer::{AudioBuffer, Sample};
use crate::audio_settings::AudioSettings;
use crate::context::Context;
use crate::error::{to_option_error, SteamAudioError};
use crate::ffi_wrapper::FFIWrapper;
use crate::{ChannelPointers, ChannelRequirement};

/// Filters and attenuates an audio signal based on various properties of the direct path between a point source and the listener.
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
/// let mut effect = DirectEffect::try_new(
///     &context,
///     &audio_settings,
///     &DirectEffectSettings { num_channels: 1 },
/// )?;
///
/// let params = DirectEffectParams {
///     distance_attenuation: Some(0.6),
///     air_absorption: Some(Equalizer([0.9, 0.7, 0.5])),
///     directivity: Some(0.7),
///     occlusion: Some(0.4),
///     transmission: None,
/// };
///
/// let input_buffer = AudioBuffer::try_with_data([1.0; 1024])?;
/// let mut output_container = vec![0.0; input_buffer.num_samples() as usize];
/// let mut output_buffer = AudioBuffer::try_with_data(&mut output_container)?;
///
/// let _ = effect.apply(&params, &input_buffer, &mut output_buffer);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug)]
pub struct DirectEffect {
    inner: audionimbus_sys::IPLDirectEffect,

    /// Number of channels used by input and output buffers.
    /// Used for buffer validation when applying the effect.
    num_channels: u32,
}

impl DirectEffect {
    /// Creates a new direct effect.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if effect creation fails.
    pub fn try_new(
        context: &Context,
        audio_settings: &AudioSettings,
        direct_effect_settings: &DirectEffectSettings,
    ) -> Result<Self, SteamAudioError> {
        let mut inner = std::ptr::null_mut();

        let status = unsafe {
            audionimbus_sys::iplDirectEffectCreate(
                context.raw_ptr(),
                &mut audionimbus_sys::IPLAudioSettings::from(audio_settings),
                &mut audionimbus_sys::IPLDirectEffectSettings::from(direct_effect_settings),
                &mut inner,
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        let direct_effect = Self {
            inner,
            num_channels: direct_effect_settings.num_channels,
        };

        Ok(direct_effect)
    }

    /// Applies a direct effect to an audio buffer.
    ///
    /// This effect CAN be applied in-place.
    ///
    /// The input and output audio buffers must have as many channels as specified when creating
    /// the effect.
    ///
    /// # Errors
    ///
    /// Returns [`EffectError`] if the input or output buffers have a number of channels different
    /// from that specified when creating the effect.
    pub fn apply<I, O, PI: ChannelPointers, PO: ChannelPointers>(
        &mut self,
        direct_effect_params: &DirectEffectParams,
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
            audionimbus_sys::iplDirectEffectApply(
                self.raw_ptr(),
                &mut *direct_effect_params.as_ffi(),
                &mut *input_buffer.as_ffi(),
                &mut *output_buffer.as_ffi(),
            )
        }
        .into();

        Ok(state)
    }

    /// Retrieves a single frame of tail samples from a direct effect’s internal buffers.
    ///
    /// After the input to the direct effect has stopped, this function must be called instead of [`Self::apply`] until the return value indicates that no more tail samples remain.
    ///
    /// The output audio buffer must have as many channels as specified when creating the effect.
    ///
    /// # Errors
    ///
    /// Returns [`EffectError`] if the output buffer has a number of channels different
    /// from that specified when creating the effect.
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
            audionimbus_sys::iplDirectEffectGetTail(self.raw_ptr(), &mut *output_buffer.as_ffi())
        }
        .into();

        Ok(state)
    }

    /// Returns the number of tail samples remaining in a direct effect’s internal buffers.
    ///
    /// Tail samples are audio samples that should be played even after the input to the effect has stopped playing and no further input samples are available.
    pub fn tail_size(&self) -> usize {
        unsafe { audionimbus_sys::iplDirectEffectGetTailSize(self.raw_ptr()) as usize }
    }

    /// Resets the internal processing state of a direct effect.
    pub fn reset(&mut self) {
        unsafe { audionimbus_sys::iplDirectEffectReset(self.raw_ptr()) };
    }

    /// Returns the raw FFI pointer to the underlying direct effect.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub fn raw_ptr(&self) -> audionimbus_sys::IPLDirectEffect {
        self.inner
    }

    /// Returns a mutable reference to the raw FFI pointer.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLDirectEffect {
        &mut self.inner
    }
}

impl Clone for DirectEffect {
    fn clone(&self) -> Self {
        unsafe {
            audionimbus_sys::iplDirectEffectRetain(self.inner);
        }

        Self {
            inner: self.inner,
            num_channels: self.num_channels,
        }
    }
}

impl Drop for DirectEffect {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplDirectEffectRelease(&mut self.inner) }
    }
}

unsafe impl Send for DirectEffect {}
unsafe impl Sync for DirectEffect {}

/// Settings used to create a direct effect.
#[derive(Debug)]
pub struct DirectEffectSettings {
    /// Number of channels that will be used by input and output buffers.
    pub num_channels: u32,
}

impl From<&DirectEffectSettings> for audionimbus_sys::IPLDirectEffectSettings {
    fn from(settings: &DirectEffectSettings) -> Self {
        Self {
            numChannels: settings.num_channels as i32,
        }
    }
}

/// Parameters for applying a direct effect to an audio buffer.
#[derive(Default, Clone, Debug, PartialEq)]
pub struct DirectEffectParams {
    /// Optional distance attenuation, with a value between 0.0 and 1.0.
    pub distance_attenuation: Option<f32>,

    /// Optional air absorption.
    pub air_absorption: Option<Equalizer<3>>,

    /// Optional directivity term, with a value between 0.0 and 1.0.
    pub directivity: Option<f32>,

    /// Optional occlusion factor, with a value between 0.0 and 1.0.
    pub occlusion: Option<f32>,

    /// Optional transmission.
    pub transmission: Option<Transmission>,
}

impl From<audionimbus_sys::IPLDirectEffectParams> for DirectEffectParams {
    fn from(params: audionimbus_sys::IPLDirectEffectParams) -> Self {
        let distance_attenuation = Some(params.distanceAttenuation);
        let air_absorption = Some(Equalizer(params.airAbsorption));
        let directivity = Some(params.directivity);
        let occlusion = Some(params.occlusion);
        let transmission = Some(match params.transmissionType {
            audionimbus_sys::IPLTransmissionType::IPL_TRANSMISSIONTYPE_FREQINDEPENDENT => {
                Transmission::FrequencyIndependent(Equalizer(params.transmission))
            }
            audionimbus_sys::IPLTransmissionType::IPL_TRANSMISSIONTYPE_FREQDEPENDENT => {
                Transmission::FrequencyDependent(Equalizer(params.transmission))
            }
        });

        Self {
            distance_attenuation,
            air_absorption,
            directivity,
            occlusion,
            transmission,
        }
    }
}

impl DirectEffectParams {
    pub(crate) fn as_ffi(&self) -> FFIWrapper<'_, audionimbus_sys::IPLDirectEffectParams, Self> {
        let mut flags = audionimbus_sys::IPLDirectEffectFlags(<_>::default());

        let distance_attenuation = self.distance_attenuation.unwrap_or_default();
        if self.distance_attenuation.is_some() {
            flags |= audionimbus_sys::IPLDirectEffectFlags::IPL_DIRECTEFFECTFLAGS_APPLYDISTANCEATTENUATION;
        }

        let default_air_absorption = Default::default();
        let air_absorption = self
            .air_absorption
            .as_ref()
            .unwrap_or(&default_air_absorption);
        if self.air_absorption.is_some() {
            flags |=
                audionimbus_sys::IPLDirectEffectFlags::IPL_DIRECTEFFECTFLAGS_APPLYAIRABSORPTION;
        }

        let directivity = self.directivity.unwrap_or_default();
        if self.directivity.is_some() {
            flags |= audionimbus_sys::IPLDirectEffectFlags::IPL_DIRECTEFFECTFLAGS_APPLYDIRECTIVITY;
        }

        let occlusion = self.occlusion.unwrap_or_default();
        if self.occlusion.is_some() {
            flags |= audionimbus_sys::IPLDirectEffectFlags::IPL_DIRECTEFFECTFLAGS_APPLYOCCLUSION;
        }

        let (transmission_type, transmission) = if let Some(transmission) = &self.transmission {
            flags |= audionimbus_sys::IPLDirectEffectFlags::IPL_DIRECTEFFECTFLAGS_APPLYTRANSMISSION;

            match transmission {
                Transmission::FrequencyIndependent(equalizer) => (
                    audionimbus_sys::IPLTransmissionType::IPL_TRANSMISSIONTYPE_FREQINDEPENDENT,
                    equalizer,
                ),
                Transmission::FrequencyDependent(equalizer) => (
                    audionimbus_sys::IPLTransmissionType::IPL_TRANSMISSIONTYPE_FREQDEPENDENT,
                    equalizer,
                ),
            }
        } else {
            (
                audionimbus_sys::IPLTransmissionType::IPL_TRANSMISSIONTYPE_FREQINDEPENDENT,
                &Default::default(),
            )
        };

        let direct_effect_params = audionimbus_sys::IPLDirectEffectParams {
            flags,
            transmissionType: transmission_type,
            distanceAttenuation: distance_attenuation,
            airAbsorption: **air_absorption,
            directivity,
            occlusion,
            transmission: **transmission,
        };

        FFIWrapper::new(direct_effect_params)
    }
}

/// Transmission parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Transmission {
    /// Frequency-independent transmission.
    FrequencyIndependent(Equalizer<3>),

    /// Frequency-dependent transmission.
    FrequencyDependent(Equalizer<3>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AudioBufferSettings, ContextSettings};

    mod apply {
        use super::*;

        #[test]
        fn test_valid() {
            let input_container = vec![0.5; 1024];
            let input_buffer = AudioBuffer::try_with_data(&input_container).unwrap();

            let mut output_container = vec![0.0; input_buffer.num_samples() as usize];
            let output_buffer = AudioBuffer::try_with_data(&mut output_container).unwrap();

            let context_settings = ContextSettings::default();
            let context = Context::try_new(&context_settings).unwrap();

            let audio_settings = AudioSettings {
                frame_size: input_buffer.num_samples(),
                ..Default::default()
            };

            let direct_effect_settings = DirectEffectSettings { num_channels: 1 };

            let mut direct_effect =
                DirectEffect::try_new(&context, &audio_settings, &direct_effect_settings).unwrap();

            let direct_effect_params = DirectEffectParams {
                distance_attenuation: Some(0.6),
                air_absorption: Some(Equalizer([0.9, 0.7, 0.5])),
                directivity: Some(0.7),
                occlusion: Some(0.4),
                transmission: Some(Transmission::FrequencyIndependent(Equalizer([
                    0.3, 0.2, 0.1,
                ]))),
            };

            assert!(direct_effect
                .apply(&direct_effect_params, &input_buffer, &output_buffer)
                .is_ok());
        }

        #[test]
        fn test_invalid_input_num_channels() {
            const FRAME_SIZE: usize = 1024;

            let mut input_container = vec![0.5; 4 * FRAME_SIZE];
            let input_buffer = AudioBuffer::try_with_data_and_settings(
                &mut input_container,
                AudioBufferSettings::with_num_channels(4),
            )
            .unwrap();

            let mut output_container = vec![0.0; FRAME_SIZE];
            let output_buffer = AudioBuffer::try_with_data(&mut output_container).unwrap();

            let context_settings = ContextSettings::default();
            let context = Context::try_new(&context_settings).unwrap();

            let audio_settings = AudioSettings {
                frame_size: FRAME_SIZE as u32,
                ..Default::default()
            };

            let direct_effect_settings = DirectEffectSettings { num_channels: 1 };

            let mut direct_effect =
                DirectEffect::try_new(&context, &audio_settings, &direct_effect_settings).unwrap();

            let direct_effect_params = DirectEffectParams {
                distance_attenuation: Some(0.6),
                air_absorption: Some(Equalizer([0.9, 0.7, 0.5])),
                directivity: Some(0.7),
                occlusion: Some(0.4),
                transmission: Some(Transmission::FrequencyIndependent(Equalizer([
                    0.3, 0.2, 0.1,
                ]))),
            };

            assert_eq!(
                direct_effect.apply(&direct_effect_params, &input_buffer, &output_buffer),
                Err(EffectError::InvalidInputChannels {
                    expected: ChannelRequirement::Exactly(1),
                    actual: 4
                })
            );
        }

        #[test]
        fn test_invalid_output_num_channels() {
            const FRAME_SIZE: usize = 1024;

            let mut input_container = vec![0.0; FRAME_SIZE];
            let input_buffer = AudioBuffer::try_with_data(&mut input_container).unwrap();

            let mut output_container = vec![0.5; 4 * FRAME_SIZE];
            let output_buffer = AudioBuffer::try_with_data_and_settings(
                &mut output_container,
                AudioBufferSettings::with_num_channels(4),
            )
            .unwrap();

            let context_settings = ContextSettings::default();
            let context = Context::try_new(&context_settings).unwrap();

            let audio_settings = AudioSettings {
                frame_size: FRAME_SIZE as u32,
                ..Default::default()
            };

            let direct_effect_settings = DirectEffectSettings { num_channels: 1 };

            let mut direct_effect =
                DirectEffect::try_new(&context, &audio_settings, &direct_effect_settings).unwrap();

            let direct_effect_params = DirectEffectParams {
                distance_attenuation: Some(0.6),
                air_absorption: Some(Equalizer([0.9, 0.7, 0.5])),
                directivity: Some(0.7),
                occlusion: Some(0.4),
                transmission: Some(Transmission::FrequencyIndependent(Equalizer([
                    0.3, 0.2, 0.1,
                ]))),
            };

            assert_eq!(
                direct_effect.apply(&direct_effect_params, &input_buffer, &output_buffer),
                Err(EffectError::InvalidOutputChannels {
                    expected: ChannelRequirement::Exactly(1),
                    actual: 4
                })
            );
        }
    }

    mod tail {
        use super::*;

        #[test]
        fn test_valid() {
            const FRAME_SIZE: usize = 1024;

            let mut output_container = vec![0.0; FRAME_SIZE];
            let output_buffer = AudioBuffer::try_with_data(&mut output_container).unwrap();

            let context_settings = ContextSettings::default();
            let context = Context::try_new(&context_settings).unwrap();

            let audio_settings = AudioSettings::default();
            let direct_effect_settings = DirectEffectSettings { num_channels: 1 };

            let direct_effect =
                DirectEffect::try_new(&context, &audio_settings, &direct_effect_settings).unwrap();

            assert!(direct_effect.tail(&output_buffer).is_ok());
        }

        #[test]
        fn test_invalid_output_num_channels() {
            const FRAME_SIZE: usize = 1024;

            let mut output_container = vec![0.0; 2 * FRAME_SIZE];
            let output_buffer = AudioBuffer::try_with_data_and_settings(
                &mut output_container,
                AudioBufferSettings::with_num_channels(2),
            )
            .unwrap();

            let context_settings = ContextSettings::default();
            let context = Context::try_new(&context_settings).unwrap();

            let audio_settings = AudioSettings::default();
            let direct_effect_settings = DirectEffectSettings { num_channels: 1 };

            let direct_effect =
                DirectEffect::try_new(&context, &audio_settings, &direct_effect_settings).unwrap();

            assert_eq!(
                direct_effect.tail(&output_buffer),
                Err(EffectError::InvalidOutputChannels {
                    expected: ChannelRequirement::Exactly(1),
                    actual: 2,
                })
            );
        }
    }
}
