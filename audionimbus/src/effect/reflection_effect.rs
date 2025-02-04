use super::audio_effect_state::AudioEffectState;
use super::equalizer::Equalizer;
use crate::audio_buffer::AudioBuffer;
use crate::audio_settings::AudioSettings;
use crate::context::Context;
use crate::error::{to_option_error, SteamAudioError};
use crate::ffi_wrapper::FFIWrapper;
use crate::true_audio_next::TrueAudioNextDevice;

// TODO: description
#[derive(Debug)]
pub struct ReflectionEffect(audionimbus_sys::IPLReflectionEffect);

impl ReflectionEffect {
    pub fn try_new(
        context: &Context,
        audio_settings: &AudioSettings,
        reflection_effect_settings: &ReflectionEffectSettings,
    ) -> Result<Self, SteamAudioError> {
        let reflection_effect = unsafe {
            let reflection_effect: *mut audionimbus_sys::IPLReflectionEffect = std::ptr::null_mut();
            let status = audionimbus_sys::iplReflectionEffectCreate(
                context.as_raw_ptr(),
                &mut audionimbus_sys::IPLAudioSettings::from(audio_settings),
                &mut audionimbus_sys::IPLReflectionEffectSettings::from(reflection_effect_settings),
                reflection_effect,
            );

            if let Some(error) = to_option_error(status) {
                return Err(error);
            }

            *reflection_effect
        };

        Ok(Self(reflection_effect))
    }

    // TODO: fix link references in comment
    /// Applies a reflection effect to an audio buffer.
    ///
    /// This effect CANNOT be applied in-place.
    ///
    /// Cannot be used with TrueAudioNext.
    pub fn apply(
        &self,
        reflection_effect_params: &ReflectionEffectParams,
        input_buffer: &mut AudioBuffer,
        output_buffer: &mut AudioBuffer,
    ) -> AudioEffectState {
        unsafe {
            audionimbus_sys::iplReflectionEffectApply(
                self.as_raw_ptr(),
                &mut *reflection_effect_params.as_ffi(),
                &mut *input_buffer.as_ffi(),
                &mut *output_buffer.as_ffi(),
                std::ptr::null_mut(),
            )
        }
        .into()
    }

    // TODO:
    /// Applies a reflection effect to an audio buffer.
    ///
    /// The output of this effect will be mixed into the given mixer.
    ///
    /// The mixed output can be retrieved elsewhere in the audio pipeline using [`ReflectionMixer::apply`].
    /// This can have a performance benefit if using convolution.
    pub fn apply_into_mixer(
        &self,
        reflection_effect_params: &ReflectionEffectParams,
        input_buffer: &mut AudioBuffer,
        mixer: (), // TODO:
    ) -> AudioEffectState {
        unsafe {
            audionimbus_sys::iplReflectionEffectApply(
                self.as_raw_ptr(),
                &mut *reflection_effect_params.as_ffi(),
                &mut *input_buffer.as_ffi(),
                std::ptr::null_mut(),
                std::ptr::null_mut(), // TODO: mixer
            )
        }
        .into()
    }

    pub fn as_raw_ptr(&self) -> audionimbus_sys::IPLReflectionEffect {
        self.0
    }
}

impl Drop for ReflectionEffect {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplReflectionEffectRelease(&mut self.0) }
    }
}

/// Settings used to create a reflection effect.
#[derive(Debug)]
pub enum ReflectionEffectSettings {
    /// Multi-channel convolution reverb.
    /// Reflections reaching the listener are encoded in an Impulse Response (IR), which is a filter that records each reflection as it arrives.
    /// This algorithm renders reflections with the most detail, but may result in significant CPU usage.
    /// Using a reflection mixer with this algorithm provides a reduction in CPU usage.
    Convolution {
        /// Number of samples per channel in the IR.
        impulse_reponse_size: i32,

        /// Number of channels in the IR.
        num_channels: i32,
    },

    /// Parametric (or artificial) reverb, using feedback delay networks.
    /// The reflected sound field is reduced to a few numbers that describe how reflected energy decays over time.
    /// This is then used to drive an approximate model of reverberation in an indoor space.
    /// This algorithm results in lower CPU usage, but cannot render individual echoes, especially in outdoor spaces.
    /// A reflection mixer cannot be used with this algorithm.
    Parametric {
        /// Number of samples per channel in the IR.
        impulse_reponse_size: i32,

        /// Number of channels in the IR.
        num_channels: i32,
    },

    /// A hybrid of convolution and parametric reverb.
    /// The initial portion of the IR is rendered using convolution reverb, but the later part is used to estimate a parametric reverb.
    /// The point in the IR where this transition occurs can be controlled.
    /// This algorithm allows a trade-off between rendering quality and CPU usage.
    /// An reflection mixer cannot be used with this algorithm.
    Hybrid {
        /// Number of samples per channel in the IR.
        impulse_reponse_size: i32,

        /// Number of channels in the IR.
        num_channels: i32,
    },

    /// Multi-channel convolution reverb, using AMD TrueAudio Next for GPU acceleration.
    /// This algorithm is similar to [`Self::Convolution`], but uses the GPU instead of the CPU for processing, allowing significantly more sources to be processed.
    /// A reflection mixer must be used with this algorithm, because the GPU will process convolution reverb at a single point in your audio processing pipeline.
    TrueAudioNext {
        /// Number of samples per channel in the IR.
        impulse_reponse_size: i32,

        /// Number of channels in the IR.
        num_channels: i32,
    },
}

impl From<&ReflectionEffectSettings> for audionimbus_sys::IPLReflectionEffectSettings {
    fn from(settings: &ReflectionEffectSettings) -> Self {
        todo!()
    }
}

/// Parameters for applying a reflection effect to an audio buffer.
#[derive(Debug)]
pub enum ReflectionEffectParams {
    /// Multi-channel convolution reverb.
    /// Reflections reaching the listener are encoded in an Impulse Response (IR), which is a filter that records each reflection as it arrives.
    /// This algorithm renders reflections with the most detail, but may result in significant CPU usage.
    /// Using a reflection mixer with this algorithm provides a reduction in CPU usage.
    Convolution {
        /// The impulse response.
        impulse_response: (), // TODO:

        /// Number of IR channels to process.
        /// May be less than the number of channels specified when creating the effect, in which case CPU usage will be reduced.
        num_channels: i32,

        /// Number of IR samples per channel to process.
        /// May be less than the number of samples specified when creating the effect, in which case CPU usage will be reduced.
        ir_size: i32,
    },

    /// Parametric (or artificial) reverb, using feedback delay networks.
    /// The reflected sound field is reduced to a few numbers that describe how reflected energy decays over time.
    /// This is then used to drive an approximate model of reverberation in an indoor space.
    /// This algorithm results in lower CPU usage, but cannot render individual echoes, especially in outdoor spaces.
    /// A reflection mixer cannot be used with this algorithm.
    Parametric {
        /// 3-band reverb decay times (RT60).
        reverb_times: [f32; 3],

        /// Number of IR channels to process.
        /// May be less than the number of channels specified when creating the effect, in which case CPU usage will be reduced.
        num_channels: i32,

        /// Number of IR samples per channel to process.
        /// May be less than the number of samples specified when creating the effect, in which case CPU usage will be reduced.
        ir_size: i32,
    },

    /// A hybrid of convolution and parametric reverb.
    /// The initial portion of the IR is rendered using convolution reverb, but the later part is used to estimate a parametric reverb.
    /// The point in the IR where this transition occurs can be controlled.
    /// This algorithm allows a trade-off between rendering quality and CPU usage.
    /// A reflection mixer cannot be used with this algorithm.
    Hybrid {
        /// The impulse response.
        impulse_response: (), // TODO:

        /// 3-band reverb decay times (RT60).
        reverb_times: [f32; 3],

        /// 3-band EQ coefficients applied to the parametric part to ensure smooth transition.
        equalizer: Equalizer<3>,

        /// Samples after which parametric part starts.
        delay: i32,

        /// Number of IR channels to process.
        /// May be less than the number of channels specified when creating the effect, in which case CPU usage will be reduced.
        num_channels: i32,

        /// Number of IR samples per channel to process.
        /// May be less than the number of samples specified when creating the effect, in which case CPU usage will be reduced.
        ir_size: i32,
    },

    /// Multi-channel convolution reverb, using AMD TrueAudio Next for GPU acceleration.
    /// This algorithm is similar to [`Self::Convolution`], but uses the GPU instead of the CPU for processing, allowing significantly more sources to be processed.
    /// A reflection mixer must be used with this algorithm, because the GPU will process convolution reverb at a single point in your audio processing pipeline.
    TrueAudioNext {
        /// Number of IR channels to process.
        /// May be less than the number of channels specified when creating the effect, in which case CPU usage will be reduced.
        num_channels: i32,

        /// Number of IR samples per channel to process.
        /// May be less than the number of samples specified when creating the effect, in which case CPU usage will be reduced.
        ir_size: i32,

        /// The TrueAudio Next device to use for convolution processing.
        device: TrueAudioNextDevice,

        /// The TrueAudio Next slot index to use for convolution processing.
        /// The slot identifies the IR to use.
        slot: i32,
    },
}

impl ReflectionEffectParams {
    pub(crate) fn as_ffi(
        &self,
    ) -> FFIWrapper<'_, audionimbus_sys::IPLReflectionEffectParams, Self> {
        todo!()
    }
}
