use super::audio_effect_state::AudioEffectState;
use super::Equalizer;
use crate::audio_buffer::AudioBuffer;
use crate::audio_settings::AudioSettings;
use crate::context::Context;
use crate::error::{to_option_error, SteamAudioError};
use crate::ffi_wrapper::FFIWrapper;

#[derive(Debug)]
pub struct DirectEffect(pub audionimbus_sys::IPLDirectEffect);

impl DirectEffect {
    pub fn try_new(
        context: &Context,
        audio_settings: &AudioSettings,
        direct_effect_settings: &DirectEffectSettings,
    ) -> Result<Self, SteamAudioError> {
        let direct_effect = unsafe {
            let direct_effect: *mut audionimbus_sys::IPLDirectEffect = std::ptr::null_mut();
            let status = audionimbus_sys::iplDirectEffectCreate(
                context.as_raw_ptr(),
                &mut audionimbus_sys::IPLAudioSettings::from(audio_settings),
                &mut audionimbus_sys::IPLDirectEffectSettings::from(direct_effect_settings),
                direct_effect,
            );

            if let Some(error) = to_option_error(status) {
                return Err(error);
            }

            *direct_effect
        };

        Ok(Self(direct_effect))
    }

    // TODO: rustdoc comment
    pub fn apply(
        &self,
        direct_effect_params: &DirectEffectParams,
        input_buffer: &mut AudioBuffer,
        output_buffer: &mut AudioBuffer,
    ) -> AudioEffectState {
        unsafe {
            audionimbus_sys::iplDirectEffectApply(
                self.as_raw_ptr(),
                &mut *direct_effect_params.as_ffi(),
                &mut *input_buffer.as_ffi(),
                &mut *output_buffer.as_ffi(),
            )
        }
        .into()
    }

    pub fn as_raw_ptr(&self) -> audionimbus_sys::IPLDirectEffect {
        self.0
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
    pub num_channels: i32,
}

impl From<&DirectEffectSettings> for audionimbus_sys::IPLDirectEffectSettings {
    fn from(settings: &DirectEffectSettings) -> Self {
        todo!()
    }
}

/// Parameters for applying a direct effect to an audio buffer.
#[derive(Debug)]
pub struct DirectEffectParams {
    /// Optional distance attenuation, with a value between 0.0 and 1.0.
    distance_attenuation: Option<f32>,

    /// Optional air absorption.
    air_absorption: Option<Equalizer<3>>,

    /// Optional directivity term, with a value between 0.0 and 1.0.
    directivity: Option<f32>,

    /// Optional occlusion factor, with a value between 0.0 and 1.0.
    occlusion: Option<f32>,

    /// Optional transmission.
    transmission: Option<Transmission>,
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

#[derive(Debug)]
pub enum Transmission {
    /// Frequency-independent transmission.
    FrequencyIndependent(Equalizer<3>),

    /// Frequency-dependent transmission.
    FrequencyDependent(Equalizer<3>),
}
