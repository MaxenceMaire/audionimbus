use super::audio_effect_state::AudioEffectState;
use super::equalizer::Equalizer;
use crate::audio_buffer::AudioBuffer;
use crate::audio_settings::AudioSettings;
use crate::context::Context;
use crate::error::{to_option_error, SteamAudioError};
use crate::ffi_wrapper::FFIWrapper;
use crate::geometry::{Scene, SceneType};
use crate::open_cl::OpenClDevice;
use crate::probe::ProbeBatch;
use crate::progress_callback::ProgressCallbackInformation;
use crate::radeon_rays::RadeonRaysDevice;
use crate::simulator::BakedDataIdentifier;
use crate::simulator::BakedDataVariation;
use crate::true_audio_next::TrueAudioNextDevice;

/// Applies the result of physics-based reflections simulation to an audio buffer.
///
/// The result is encoded in Ambisonics, and can be decoded using an Ambisonics decode effect.
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

    /// Applies a reflection effect to an audio buffer.
    ///
    /// This effect CANNOT be applied in-place.
    ///
    /// Cannot be used with [`ReflectionEffectSettings::TrueAudioNext`].
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

/// Bakes a single layer of reflections data in a probe batch.
///
/// Only one bake can be in progress at any point in time.
pub fn bake_reflections(
    context: &Context,
    reflections_bake_params: &ReflectionsBakeParams,
    progress_callback: Option<ProgressCallbackInformation>,
) {
    let (callback, user_data) = if let Some(callback_information) = progress_callback {
        (
            Some(callback_information.callback),
            callback_information.user_data,
        )
    } else {
        (None, std::ptr::null_mut())
    };

    unsafe {
        audionimbus_sys::iplReflectionsBakerBake(
            context.as_raw_ptr(),
            &mut audionimbus_sys::IPLReflectionsBakeParams::from(reflections_bake_params),
            callback,
            user_data,
        );
    }
}

/// Parameters used to control how reflections data is baked.
#[derive(Debug)]
pub struct ReflectionsBakeParams {
    /// The scene in which the probes exist.
    pub scene: Scene,

    /// A probe batch containing the probes at which reflections data should be baked.
    pub probe_batch: ProbeBatch,

    /// The type of scene being used.
    pub scene_type: SceneType,

    /// An identifier for the data layer that should be baked.
    /// The identifier determines what data is simulated and stored at each probe.
    /// If the probe batch already contains data with this identifier, it will be overwritten.
    pub identifier: BakedDataIdentifier,

    /// The types of data to save for each probe.
    pub bake_flags: ReflectionsBakeFlags,

    /// The number of rays to trace from each listener position when baking.
    /// Increasing this number results in improved accuracy, at the cost of increased bake times.
    pub num_rays: i32,

    /// The number of directions to consider when generating diffusely-reflected rays when baking.
    /// Increasing this number results in slightly improved accuracy of diffuse reflections.
    pub num_diffuse_samples: i32,

    /// The number of times each ray is reflected off of solid geometry.
    /// Increasing this number results in longer reverb tails and improved accuracy, at the cost of increased bake times.
    pub num_bounces: i32,

    /// The length (in seconds) of the impulse responses to simulate.
    /// Increasing this number allows the baked data to represent longer reverb tails (and hence larger spaces), at the cost of increased memory usage while baking.
    pub simulated_duration: f32,

    /// The length (in seconds) of the impulse responses to save at each probe.
    /// Increasing this number allows the baked data to represent longer reverb tails (and hence larger spaces), at the cost of increased disk space usage and memory usage at run-time.
    ///
    /// It may be useful to set [`Self::saved_duration`] to be less than [`Self::simulated_duration`], especially if you plan to use hybrid reverb for rendering baked reflections.
    /// This way, the parametric reverb data is estimated using a longer IR, resulting in more accurate estimation, but only the early part of the IR can be saved for subsequent rendering.
    pub saved_duration: f32,

    /// Ambisonic order of the baked IRs.
    pub order: i32,

    /// Number of threads to use for baking.
    pub num_threads: i32,

    /// If using custom ray tracer callbacks, this the number of rays that will be passed to the callbacks every time rays need to be traced.
    pub ray_batch_size: i32,

    /// When calculating how much sound energy reaches a surface directly from a source, any source that is closer than [`Self::irradiance_min_distance`] to the surface is assumed to be at a distance of [`Self::irradiance_min_distance`], for the purposes of energy calculations.
    pub irradiance_min_distance: f32,

    /// If using Radeon Rays or if [`Self::identifier`] uses [`BakedDataVariation::StaticListener`], this is the number of probes for which data is baked simultaneously.
    pub bake_batch_size: i32,

    /// The OpenCL device, if using Radeon Rays.
    pub open_cl_device: OpenClDevice,

    /// The Radeon Rays device, if using Radeon Rays.
    pub radeon_rays_device: RadeonRaysDevice,
}

impl From<&ReflectionsBakeParams> for audionimbus_sys::IPLReflectionsBakeParams {
    fn from(params: &ReflectionsBakeParams) -> Self {
        todo!()
    }
}

bitflags::bitflags! {
    /// Flags for specifying what types of reflections data to bake.
    #[derive(Debug)]
    pub struct ReflectionsBakeFlags: u32 {
        /// Bake impulse responses for [`ReflectionEffectSettings::Convolution`], [`ReflectionEffectSettings::Hybrid`], or [`ReflectionEffectSettings::TrueAudioNext`].
        const BAKE_CONVOLUTION = 1 << 0;

        /// Bake parametric reverb for [`ReflectionEffectSettings::Parametric`] or [`ReflectionEffectSettings::Hybrid`].
        const BAKE_PARAMETRIC = 1 << 1;
    }
}
