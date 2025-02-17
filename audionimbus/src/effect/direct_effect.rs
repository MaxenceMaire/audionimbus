use super::audio_effect_state::AudioEffectState;
use super::Equalizer;
use crate::audio_buffer::{AudioBuffer, Sample};
use crate::audio_settings::AudioSettings;
use crate::context::Context;
use crate::error::{to_option_error, SteamAudioError};
use crate::ffi_wrapper::FFIWrapper;

/// Filters and attenuates an audio signal based on various properties of the direct path between a point source and the listener.
#[derive(Debug)]
pub struct DirectEffect(pub audionimbus_sys::IPLDirectEffect);

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
    pub fn apply(
        &self,
        direct_effect_params: &DirectEffectParams,
        input_buffer: &AudioBuffer<&'_ [Sample]>,
        output_buffer: &AudioBuffer<&'_ mut [Sample]>,
    ) -> AudioEffectState {
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

    pub fn raw_ptr(&self) -> audionimbus_sys::IPLDirectEffect {
        self.0
    }

    pub fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLDirectEffect {
        &mut self.0
    }
}

impl Drop for DirectEffect {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplDirectEffectRelease(&mut self.0) }
    }
}

/// Settings used to create a direct effect.
#[derive(Debug)]
pub struct DirectEffectSettings {
    /// Number of channels that will be used by input and output buffers.
    pub num_channels: usize,
}

impl From<&DirectEffectSettings> for audionimbus_sys::IPLDirectEffectSettings {
    fn from(settings: &DirectEffectSettings) -> Self {
        Self {
            numChannels: settings.num_channels as i32,
        }
    }
}

/// Parameters for applying a direct effect to an audio buffer.
#[derive(Default, Debug)]
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
        let distance_attenuation = if params.flags
            & audionimbus_sys::IPLDirectEffectFlags::IPL_DIRECTEFFECTFLAGS_APPLYDISTANCEATTENUATION
            == audionimbus_sys::IPLDirectEffectFlags::IPL_DIRECTEFFECTFLAGS_APPLYDISTANCEATTENUATION
        {
            Some(params.distanceAttenuation)
        } else {
            None
        };

        let air_absorption = if params.flags
            & audionimbus_sys::IPLDirectEffectFlags::IPL_DIRECTEFFECTFLAGS_APPLYAIRABSORPTION
            == audionimbus_sys::IPLDirectEffectFlags::IPL_DIRECTEFFECTFLAGS_APPLYAIRABSORPTION
        {
            Some(Equalizer(params.airAbsorption))
        } else {
            None
        };

        let directivity = if params.flags
            & audionimbus_sys::IPLDirectEffectFlags::IPL_DIRECTEFFECTFLAGS_APPLYDIRECTIVITY
            == audionimbus_sys::IPLDirectEffectFlags::IPL_DIRECTEFFECTFLAGS_APPLYDIRECTIVITY
        {
            Some(params.directivity)
        } else {
            None
        };

        let occlusion = if params.flags
            & audionimbus_sys::IPLDirectEffectFlags::IPL_DIRECTEFFECTFLAGS_APPLYOCCLUSION
            == audionimbus_sys::IPLDirectEffectFlags::IPL_DIRECTEFFECTFLAGS_APPLYOCCLUSION
        {
            Some(params.occlusion)
        } else {
            None
        };

        let transmission = if params.flags
            & audionimbus_sys::IPLDirectEffectFlags::IPL_DIRECTEFFECTFLAGS_APPLYTRANSMISSION
            == audionimbus_sys::IPLDirectEffectFlags::IPL_DIRECTEFFECTFLAGS_APPLYTRANSMISSION
        {
            Some(match params.transmissionType {
                audionimbus_sys::IPLTransmissionType::IPL_TRANSMISSIONTYPE_FREQINDEPENDENT => {
                    Transmission::FrequencyIndependent(Equalizer(params.transmission))
                }
                audionimbus_sys::IPLTransmissionType::IPL_TRANSMISSIONTYPE_FREQDEPENDENT => {
                    Transmission::FrequencyDependent(Equalizer(params.transmission))
                }
            })
        } else {
            None
        };

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
        let mut flags = audionimbus_sys::IPLDirectEffectFlags(u32::default());

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
#[derive(Debug)]
pub enum Transmission {
    /// Frequency-independent transmission.
    FrequencyIndependent(Equalizer<3>),

    /// Frequency-dependent transmission.
    FrequencyDependent(Equalizer<3>),
}
