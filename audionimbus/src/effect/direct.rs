use super::audio_effect_state::AudioEffectState;
use super::Equalizer;
use crate::audio_buffer::{AudioBuffer, Sample};
use crate::audio_settings::AudioSettings;
use crate::context::Context;
use crate::error::{to_option_error, SteamAudioError};
use crate::ffi_wrapper::FFIWrapper;
use crate::ChannelPointers;

#[cfg(feature = "firewheel")]
use firewheel::diff::{Diff, Patch, RealtimeClone};

/// Filters and attenuates an audio signal based on various properties of the direct path between a point source and the listener.
#[derive(Debug)]
pub struct DirectEffect(audionimbus_sys::IPLDirectEffect);

impl DirectEffect {
    pub fn try_new(
        context: &Context,
        audio_settings: &AudioSettings,
        direct_effect_settings: &DirectEffectSettings,
    ) -> Result<Self, SteamAudioError> {
        let mut direct_effect = Self(std::ptr::null_mut());

        let status = unsafe {
            audionimbus_sys::iplDirectEffectCreate(
                context.raw_ptr(),
                &mut audionimbus_sys::IPLAudioSettings::from(audio_settings),
                &mut audionimbus_sys::IPLDirectEffectSettings::from(direct_effect_settings),
                direct_effect.raw_ptr_mut(),
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        Ok(direct_effect)
    }

    /// Applies a direct effect to an audio buffer.
    ///
    /// This effect CAN be applied in-place.
    pub fn apply<I, O, PI: ChannelPointers, PO: ChannelPointers>(
        &self,
        direct_effect_params: &DirectEffectParams,
        input_buffer: &AudioBuffer<I, PI>,
        output_buffer: &AudioBuffer<O, PO>,
    ) -> AudioEffectState
    where
        I: AsRef<[Sample]>,
        O: AsRef<[Sample]> + AsMut<[Sample]>,
    {
        unsafe {
            audionimbus_sys::iplDirectEffectApply(
                self.raw_ptr(),
                &mut *direct_effect_params.as_ffi(),
                &mut *input_buffer.as_ffi(),
                &mut *output_buffer.as_ffi(),
            )
        }
        .into()
    }

    /// Retrieves a single frame of tail samples from a direct effect’s internal buffers.
    ///
    /// After the input to the direct effect has stopped, this function must be called instead of [`Self::apply`] until the return value indicates that no more tail samples remain.
    ///
    /// The output audio buffer must have as many channels as specified when creating the effect.
    pub fn tail<O>(&self, output_buffer: &AudioBuffer<O>) -> AudioEffectState
    where
        O: AsRef<[Sample]> + AsMut<[Sample]>,
    {
        unsafe {
            audionimbus_sys::iplDirectEffectGetTail(self.raw_ptr(), &mut *output_buffer.as_ffi())
        }
        .into()
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

    pub fn raw_ptr(&self) -> audionimbus_sys::IPLDirectEffect {
        self.0
    }

    pub fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLDirectEffect {
        &mut self.0
    }
}

impl Clone for DirectEffect {
    fn clone(&self) -> Self {
        unsafe {
            audionimbus_sys::iplDirectEffectRetain(self.0);
        }
        Self(self.0)
    }
}

impl Drop for DirectEffect {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplDirectEffectRelease(&mut self.0) }
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
#[derive(Default, Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "firewheel", derive(Diff, Patch, RealtimeClone))]
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
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "firewheel", derive(Diff, Patch, RealtimeClone))]
pub enum Transmission {
    /// Frequency-independent transmission.
    FrequencyIndependent(Equalizer<3>),

    /// Frequency-dependent transmission.
    FrequencyDependent(Equalizer<3>),
}
