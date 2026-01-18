use super::audio_effect_state::AudioEffectState;
use super::equalizer::Equalizer;
use crate::audio_buffer::{AudioBuffer, Sample};
use crate::audio_settings::AudioSettings;
use crate::callback::{CallbackInformation, ProgressCallback};
use crate::context::Context;
use crate::device::open_cl::OpenClDevice;
use crate::device::radeon_rays::RadeonRaysDevice;
use crate::device::true_audio_next::TrueAudioNextDevice;
use crate::error::{to_option_error, SteamAudioError};
use crate::ffi_wrapper::FFIWrapper;
use crate::geometry::{Scene, SceneParams};
use crate::probe::ProbeBatch;
use crate::simulation::BakedDataIdentifier;
use crate::ChannelPointers;

#[cfg(doc)]
use super::AmbisonicsDecodeEffect;
#[cfg(doc)]
use crate::simulation::{BakedDataVariation, SimulationOutputs, Simulator, Source};

/// Applies the result of physics-based reflections simulation to an audio buffer.
///
/// The result is encoded in ambisonics, and can be decoded using an ambisonics decode effect ([`AmbisonicsDecodeEffect`]).
///
/// # Examples
///
/// Applying reflections involves:
/// 1. Setting up a [`Simulator`] with a [`Scene`]
/// 2. Adding [`Source`]s to the scene (don't forget to commit the changes using [`Simulator::commit`]!)
/// 3. Running the simulation ([`Simulator::run_reflections`]) and retrieving the output for the source ([`Source::get_outputs`])
/// 4. Applying the reflection effect to the audio buffer using the simulation output ([`SimulationOutputs::reflections`]) as params
///
/// ```
/// use audionimbus::*;
///
/// let context = Context::default();
///
/// const SAMPLING_RATE: u32 = 48_000;
/// const FRAME_SIZE: u32 = 1024;
/// let audio_settings = AudioSettings {
///     sampling_rate: SAMPLING_RATE,
///     frame_size: FRAME_SIZE
/// };
///
/// // Create a simulator with reflections.
/// let mut simulator = Simulator::builder(SceneParams::Default, SAMPLING_RATE, FRAME_SIZE, 1)
///     .with_reflections(ReflectionsSimulationSettings::Convolution {
///         max_num_rays: 4096,
///         num_diffuse_samples: 32,
///         max_duration: 2.0,
///         max_num_sources: 8,
///         num_threads: 2,
///     })
///     .try_build(&context)?;
///
/// let scene = Scene::try_new(&context, &SceneSettings::default())?;
/// simulator.set_scene(&scene);
///
/// let mut source = Source::try_new(&simulator, &SourceSettings {
///     flags: SimulationFlags::REFLECTIONS,
/// })?;
///
/// source.set_inputs(SimulationFlags::REFLECTIONS, SimulationInputs {
///     source: CoordinateSystem::default(),
///     direct_simulation: None,
///     reflections_simulation: Some(ReflectionsSimulationParameters::Convolution {
///         baked_data_identifier: None,
///     }),
///     pathing_simulation: None,
/// });
///
/// simulator.add_source(&source);
/// simulator.set_shared_inputs(SimulationFlags::REFLECTIONS, &SimulationSharedInputs {
///     listener: CoordinateSystem::default(),
///     num_rays: 4096,
///     num_bounces: 16,
///     duration: 2.0,
///     order: 1,
///     irradiance_min_distance: 1.0,
///     pathing_visualization_callback: None,
/// });
/// simulator.commit();
///
/// simulator.run_reflections();
/// let outputs = source.get_outputs(SimulationFlags::REFLECTIONS);
///
/// const NUM_CHANNELS: u32 = num_ambisonics_channels(1); // 1st order ambisonics
/// let mut effect = ReflectionEffect::try_new(
///     &context,
///     &audio_settings,
///     &ReflectionEffectSettings::Convolution {
///         impulse_response_size: 2 * SAMPLING_RATE, // 2 seconds
///         num_channels: NUM_CHANNELS,
///     }
/// )?;
///
/// let input = vec![0.5; FRAME_SIZE as usize];
/// let input_buffer = AudioBuffer::try_with_data(&input)?;
/// let mut output = vec![0.0; (NUM_CHANNELS * FRAME_SIZE) as usize]; // 4 channels
/// let output_buffer = AudioBuffer::try_with_data_and_settings(
///     &mut output,
///     AudioBufferSettings::with_num_channels(NUM_CHANNELS)
/// )?;
///
/// let params = outputs.reflections();
/// let _ = effect.apply(&params, &input_buffer, &output_buffer);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Simulating Reverb
///
/// In addition to modeling reflections from sources, you can use this effect to simulate reverb
/// by placing a source at the listener's position:
///
/// ```
/// use audionimbus::*;
///
/// let context = Context::default();
/// const SAMPLING_RATE: u32 = 48_000;
/// const FRAME_SIZE: u32 = 1024;
/// let audio_settings = AudioSettings { sampling_rate: SAMPLING_RATE, frame_size: FRAME_SIZE };
///
/// // Create simulator with reflections
/// let mut simulator = Simulator::builder(SceneParams::Default, SAMPLING_RATE, FRAME_SIZE, 1)
///     .with_reflections(ReflectionsSimulationSettings::Convolution {
///         max_num_rays: 2048,
///         num_diffuse_samples: 32,
///         max_duration: 2.0,
///         max_num_sources: 8,
///         num_threads: 2,
///     })
///     .try_build(&context)?;
///
/// let scene = Scene::try_new(&context, &SceneSettings::default())?;
/// simulator.set_scene(&scene);
///
/// // Create a reverb source positioned at the listener.
/// let mut reverb_source = Source::try_new(&simulator, &SourceSettings {
///     flags: SimulationFlags::REFLECTIONS,
/// })?;
///
/// let listener_position = CoordinateSystem {
///     origin: Vector3::new(0.0, 1.5, 0.0), // Listener at head height
///     ..Default::default()
/// };
///
/// // Set source position to match listener position.
/// reverb_source.set_inputs(SimulationFlags::REFLECTIONS, SimulationInputs {
///     source: listener_position, // Source at listener = reverb
///     direct_simulation: None,
///     reflections_simulation: Some(ReflectionsSimulationParameters::Convolution {
///         baked_data_identifier: None,
///     }),
///     pathing_simulation: None,
/// });
///
/// simulator.add_source(&reverb_source);
/// simulator.set_shared_inputs(SimulationFlags::REFLECTIONS, &SimulationSharedInputs {
///     listener: listener_position,
///     num_rays: 2048,
///     num_bounces: 8,
///     duration: 2.0,
///     order: 1,
///     irradiance_min_distance: 1.0,
///     pathing_visualization_callback: None,
/// });
/// simulator.commit();
///
/// // Run simulation.
/// simulator.run_reflections();
/// let reverb_outputs = reverb_source.get_outputs(SimulationFlags::REFLECTIONS);
/// let reverb_params = reverb_outputs.reflections();
///
/// const NUM_CHANNELS: u32 = num_ambisonics_channels(1); // 1st order ambisonics
/// let mut reverb_effect = ReflectionEffect::try_new(
///     &context,
///     &audio_settings,
///     &ReflectionEffectSettings::Convolution {
///         impulse_response_size: 2 * SAMPLING_RATE,
///         num_channels: NUM_CHANNELS,
///     }
/// )?;
///
/// let input = vec![0.5; FRAME_SIZE as usize];
/// let input_buffer = AudioBuffer::try_with_data(&input)?;
/// let mut reverb_output = vec![0.0; (NUM_CHANNELS * FRAME_SIZE) as usize];
/// let output_buffer = AudioBuffer::try_with_data_and_settings(
///     &mut reverb_output,
///     AudioBufferSettings::with_num_channels(NUM_CHANNELS)
/// )?;
///
/// let _ = reverb_effect.apply(&reverb_params, &input_buffer, &output_buffer);
///
/// // Mix with dry signal (e.g., 70% dry, 30% reverb)
/// // Then decode the ambisonics output for final playback
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug)]
pub struct ReflectionEffect(audionimbus_sys::IPLReflectionEffect);

impl ReflectionEffect {
    /// Creates a new reflection effect.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if effect creation fails.
    pub fn try_new(
        context: &Context,
        audio_settings: &AudioSettings,
        reflection_effect_settings: &ReflectionEffectSettings,
    ) -> Result<Self, SteamAudioError> {
        let mut reflection_effect = Self(std::ptr::null_mut());

        let status = unsafe {
            audionimbus_sys::iplReflectionEffectCreate(
                context.raw_ptr(),
                &mut audionimbus_sys::IPLAudioSettings::from(audio_settings),
                &mut audionimbus_sys::IPLReflectionEffectSettings::from(reflection_effect_settings),
                reflection_effect.raw_ptr_mut(),
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        Ok(reflection_effect)
    }

    /// Applies a reflection effect to an audio buffer.
    ///
    /// This effect CANNOT be applied in-place.
    ///
    /// Cannot be used with [`ReflectionEffectSettings::TrueAudioNext`].
    pub fn apply<I, O, PI: ChannelPointers, PO: ChannelPointers>(
        &mut self,
        reflection_effect_params: &ReflectionEffectParams,
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

        unsafe {
            audionimbus_sys::iplReflectionEffectApply(
                self.raw_ptr(),
                &mut *reflection_effect_params.as_ffi(),
                &mut *input_buffer.as_ffi(),
                &mut *output_buffer.as_ffi(),
                std::ptr::null_mut(),
            )
        }
        .into()
    }

    /// Applies a reflection effect to an audio buffer.
    ///
    /// The output of this effect will be mixed into the given mixer.
    ///
    /// The mixed output can be retrieved elsewhere in the audio pipeline using [`ReflectionMixer::apply`].
    /// This can have a performance benefit if using convolution.
    pub fn apply_into_mixer<I, PI: ChannelPointers>(
        &mut self,
        reflection_effect_params: &mut ReflectionEffectParams,
        input_buffer: &AudioBuffer<I, PI>,
        mixer: &ReflectionMixer,
    ) -> AudioEffectState
    where
        I: AsRef<[Sample]>,
    {
        assert_eq!(
            input_buffer.num_channels(),
            1,
            "input buffer must have 1 channel",
        );

        unsafe {
            audionimbus_sys::iplReflectionEffectApply(
                self.raw_ptr(),
                &mut *reflection_effect_params.as_ffi(),
                &mut *input_buffer.as_ffi(),
                std::ptr::null_mut(),
                mixer.raw_ptr(),
            )
        }
        .into()
    }

    /// Retrieves a single frame of tail samples from a reflection effect’s internal buffers.
    ///
    /// After the input to the reflection effect has stopped, this function must be called instead of [`Self::apply`] until the return value indicates that no more tail samples remain.
    ///
    /// The output audio buffer must have as many channels as the impulse response specified when creating the effect (for convolution, hybrid, and TAN) or at least 1 channel (for parametric).
    pub fn tail<O>(&self, output_buffer: &AudioBuffer<O>) -> AudioEffectState
    where
        O: AsRef<[Sample]> + AsMut<[Sample]>,
    {
        unsafe {
            audionimbus_sys::iplReflectionEffectGetTail(
                self.raw_ptr(),
                &mut *output_buffer.as_ffi(),
                std::ptr::null_mut(),
            )
        }
        .into()
    }

    /// Retrieves a single frame of tail samples from a reflection effect’s internal buffers.
    ///
    /// After the input to the reflection effect has stopped, this function must be called instead of [`Self::apply`] until the return value indicates that no more tail samples remain.
    ///
    /// The tail samples will be mixed into the given mixer.
    /// The mixed output can be retrieved elsewhere in the audio pipeline using [`ReflectionMixer::apply`].
    /// This can have a performance benefit if using convolution.
    /// If using TAN, specifying a mixer is required.
    ///
    ///The output audio buffer must have as many channels as the impulse response specified when creating the effect (for convolution, hybrid, and TAN) or at least 1 channel (for parametric).
    pub fn tail_into_mixer<O>(
        &self,
        output_buffer: &AudioBuffer<O>,
        mixer: &ReflectionMixer,
    ) -> AudioEffectState
    where
        O: AsRef<[Sample]> + AsMut<[Sample]>,
    {
        unsafe {
            audionimbus_sys::iplReflectionEffectGetTail(
                self.raw_ptr(),
                &mut *output_buffer.as_ffi(),
                mixer.raw_ptr(),
            )
        }
        .into()
    }

    /// Returns the number of tail samples remaining in a reflection effect’s internal buffers.
    ///
    /// Tail samples are audio samples that should be played even after the input to the effect has stopped playing and no further input samples are available.
    pub fn tail_size(&self) -> usize {
        unsafe { audionimbus_sys::iplReflectionEffectGetTailSize(self.raw_ptr()) as usize }
    }

    /// Resets the internal processing state of a reflection effect.
    pub fn reset(&mut self) {
        unsafe { audionimbus_sys::iplReflectionEffectReset(self.raw_ptr()) };
    }

    /// Returns the raw FFI pointer to the underlying reflection effect.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub fn raw_ptr(&self) -> audionimbus_sys::IPLReflectionEffect {
        self.0
    }

    /// Returns a mutable reference to the raw FFI pointer.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLReflectionEffect {
        &mut self.0
    }
}

impl Clone for ReflectionEffect {
    fn clone(&self) -> Self {
        unsafe {
            audionimbus_sys::iplReflectionEffectRetain(self.0);
        }
        Self(self.0)
    }
}

impl Drop for ReflectionEffect {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplReflectionEffectRelease(&mut self.0) }
    }
}

unsafe impl Send for ReflectionEffect {}
unsafe impl Sync for ReflectionEffect {}

/// Settings used to create a reflection effect.
#[derive(Copy, Clone, Debug)]
pub enum ReflectionEffectSettings {
    /// Multi-channel convolution reverb.
    /// Reflections reaching the listener are encoded in an Impulse Response (IR), which is a filter that records each reflection as it arrives.
    /// This algorithm renders reflections with the most detail, but may result in significant CPU usage.
    /// Using a reflection mixer with this algorithm provides a reduction in CPU usage.
    Convolution {
        /// Number of samples per channel in the IR.
        impulse_response_size: u32,

        /// Number of channels in the IR.
        num_channels: u32,
    },

    /// Parametric (or artificial) reverb, using feedback delay networks.
    /// The reflected sound field is reduced to a few numbers that describe how reflected energy decays over time.
    /// This is then used to drive an approximate model of reverberation in an indoor space.
    /// This algorithm results in lower CPU usage, but cannot render individual echoes, especially in outdoor spaces.
    /// A reflection mixer cannot be used with this algorithm.
    Parametric {
        /// Number of samples per channel in the IR.
        impulse_response_size: u32,

        /// Number of channels in the IR.
        num_channels: u32,
    },

    /// A hybrid of convolution and parametric reverb.
    /// The initial portion of the IR is rendered using convolution reverb, but the later part is used to estimate a parametric reverb.
    /// The point in the IR where this transition occurs can be controlled.
    /// This algorithm allows a trade-off between rendering quality and CPU usage.
    /// An reflection mixer cannot be used with this algorithm.
    Hybrid {
        /// Number of samples per channel in the IR.
        impulse_response_size: u32,

        /// Number of channels in the IR.
        num_channels: u32,
    },

    /// Multi-channel convolution reverb, using AMD TrueAudio Next for GPU acceleration.
    /// This algorithm is similar to [`Self::Convolution`], but uses the GPU instead of the CPU for processing, allowing significantly more sources to be processed.
    /// A reflection mixer must be used with this algorithm, because the GPU will process convolution reverb at a single point in your audio processing pipeline.
    TrueAudioNext {
        /// Number of samples per channel in the IR.
        impulse_response_size: u32,

        /// Number of channels in the IR.
        num_channels: u32,
    },
}

impl From<&ReflectionEffectSettings> for audionimbus_sys::IPLReflectionEffectSettings {
    fn from(settings: &ReflectionEffectSettings) -> Self {
        let (type_, impulse_response_size, num_channels) = match settings {
            ReflectionEffectSettings::Convolution {
                impulse_response_size,
                num_channels,
            } => (
                audionimbus_sys::IPLReflectionEffectType::IPL_REFLECTIONEFFECTTYPE_CONVOLUTION,
                impulse_response_size,
                num_channels,
            ),
            ReflectionEffectSettings::Parametric {
                impulse_response_size,
                num_channels,
            } => (
                audionimbus_sys::IPLReflectionEffectType::IPL_REFLECTIONEFFECTTYPE_PARAMETRIC,
                impulse_response_size,
                num_channels,
            ),
            ReflectionEffectSettings::Hybrid {
                impulse_response_size,
                num_channels,
            } => (
                audionimbus_sys::IPLReflectionEffectType::IPL_REFLECTIONEFFECTTYPE_HYBRID,
                impulse_response_size,
                num_channels,
            ),
            ReflectionEffectSettings::TrueAudioNext {
                impulse_response_size,
                num_channels,
            } => (
                audionimbus_sys::IPLReflectionEffectType::IPL_REFLECTIONEFFECTTYPE_TAN,
                impulse_response_size,
                num_channels,
            ),
        };

        Self {
            type_,
            irSize: *impulse_response_size as i32,
            numChannels: *num_channels as i32,
        }
    }
}

/// Parameters for applying a reflection effect to an audio buffer.
#[derive(Debug, PartialEq)]
pub struct ReflectionEffectParams {
    /// Type of reflection effect algorithm to use.
    pub reflection_effect_type: ReflectionEffectType,

    /// The impulse response.
    pub impulse_response: ReflectionEffectIR,

    /// 3-band reverb decay times (RT60).
    pub reverb_times: [f32; 3],

    /// 3-band EQ coefficients applied to the parametric part to ensure smooth transition.
    pub equalizer: Equalizer<3>,

    /// Samples after which parametric part starts.
    pub delay: u32,

    /// Number of IR channels to process.
    /// May be less than the number of channels specified when creating the effect, in which case CPU usage will be reduced.
    pub num_channels: u32,

    /// Number of IR samples per channel to process.
    /// May be less than the number of samples specified when creating the effect, in which case CPU usage will be reduced.
    pub impulse_response_size: u32,

    /// The TrueAudio Next device to use for convolution processing.
    pub true_audio_next_device: TrueAudioNextDevice,

    /// The TrueAudio Next slot index to use for convolution processing.
    /// The slot identifies the IR to use.
    pub true_audio_next_slot: u32,
}

unsafe impl Send for ReflectionEffectParams {}

/// The impulse response of [`ReflectionEffectParams`].
#[derive(Debug, Eq, PartialEq)]
pub struct ReflectionEffectIR(pub audionimbus_sys::IPLReflectionEffectIR);

unsafe impl Send for ReflectionEffectIR {}

impl ReflectionEffectParams {
    /// Multi-channel convolution reverb.
    /// Reflections reaching the listener are encoded in an Impulse Response (IR), which is a filter that records each reflection as it arrives.
    /// This algorithm renders reflections with the most detail, but may result in significant CPU usage.
    /// Using a reflection mixer with this algorithm provides a reduction in CPU usage.
    ///
    /// # Arguments
    ///
    /// - `impulse_response`: the impulse response.
    /// - `num_channels`: number of IR channels to process. May be less than the number of channels specified when creating the effect, in which case CPU usage will be reduced.
    /// - `impulse_response_size`: number of IR samples per channel to process. May be less than the number of samples specified when creating the effect, in which case CPU usage will be reduced.
    pub fn convolution(
        impulse_response: audionimbus_sys::IPLReflectionEffectIR,
        num_channels: u32,
        impulse_response_size: u32,
    ) -> Self {
        Self {
            reflection_effect_type: ReflectionEffectType::Convolution,
            impulse_response: ReflectionEffectIR(impulse_response),
            reverb_times: <[f32; 3]>::default(),
            equalizer: Equalizer::default(),
            delay: 0,
            num_channels,
            impulse_response_size,
            true_audio_next_device: TrueAudioNextDevice(std::ptr::null_mut()),
            true_audio_next_slot: 0,
        }
    }

    /// Parametric (or artificial) reverb, using feedback delay networks.
    /// The reflected sound field is reduced to a few numbers that describe how reflected energy decays over time.
    /// This is then used to drive an approximate model of reverberation in an indoor space.
    /// This algorithm results in lower CPU usage, but cannot render individual echoes, especially in outdoor spaces.
    /// A reflection mixer cannot be used with this algorithm.
    ///
    /// # Arguments
    ///
    /// - `reverb_times`: 3-band reverb decay times (RT60).
    /// - `num_channels`: number of IR channels to process. May be less than the number of channels specified when creating the effect, in which case CPU usage will be reduced.
    /// - `impulse_response_size`: number of IR samples per channel to process. May be less than the number of samples specified when creating the effect, in which case CPU usage will be reduced.
    pub fn parametric(
        reverb_times: [f32; 3],
        num_channels: u32,
        impulse_response_size: u32,
    ) -> Self {
        Self {
            reflection_effect_type: ReflectionEffectType::Parametric,
            impulse_response: ReflectionEffectIR(std::ptr::null_mut()),
            reverb_times,
            equalizer: Equalizer::default(),
            delay: 0,
            num_channels,
            impulse_response_size,
            true_audio_next_device: TrueAudioNextDevice(std::ptr::null_mut()),
            true_audio_next_slot: 0,
        }
    }

    /// A hybrid of convolution and parametric reverb.
    /// The initial portion of the IR is rendered using convolution reverb, but the later part is used to estimate a parametric reverb.
    /// The point in the IR where this transition occurs can be controlled.
    /// This algorithm allows a trade-off between rendering quality and CPU usage.
    /// A reflection mixer cannot be used with this algorithm.
    ///
    /// # Arguments
    ///
    /// - `impulse_response`: the impulse response.
    /// - `reverb_times`: 3-band reverb decay times (RT60).
    /// - `equalizer`: 3-band EQ coefficients applied to the parametric part to ensure smooth transition.
    /// - `delay`: samples after which parametric part starts.
    /// - `num_channels`: number of IR channels to process. May be less than the number of channels specified when creating the effect, in which case CPU usage will be reduced.
    /// - `impulse_response_size`: number of IR samples per channel to process. May be less than the number of samples specified when creating the effect, in which case CPU usage will be reduced.
    pub fn hybrid(
        impulse_response: audionimbus_sys::IPLReflectionEffectIR,
        reverb_times: [f32; 3],
        equalizer: Equalizer<3>,
        delay: u32,
        num_channels: u32,
        impulse_response_size: u32,
    ) -> Self {
        Self {
            reflection_effect_type: ReflectionEffectType::Hybrid,
            impulse_response: ReflectionEffectIR(impulse_response),
            reverb_times,
            equalizer,
            delay,
            num_channels,
            impulse_response_size,
            true_audio_next_device: TrueAudioNextDevice(std::ptr::null_mut()),
            true_audio_next_slot: 0,
        }
    }

    /// Multi-channel convolution reverb, using AMD TrueAudio Next for GPU acceleration.
    /// This algorithm is similar to [`ReflectionEffectType::Convolution`], but uses the GPU instead of the CPU for processing, allowing significantly more sources to be processed.
    /// A reflection mixer must be used with this algorithm, because the GPU will process convolution reverb at a single point in your audio processing pipeline.
    ///
    /// # Arguments
    ///
    /// - `num_channels`: number of IR channels to process. May be less than the number of channels specified when creating the effect, in which case CPU usage will be reduced.
    /// - `impulse_response_size`: number of IR samples per channel to process. May be less than the number of samples specified when creating the effect, in which case CPU usage will be reduced.
    /// - `device`: the TrueAudio Next device to use for convolution processing.
    /// - `slot`: the TrueAudio Next slot index to use for convolution processing. The slot identifies the IR to use.
    pub fn true_audio_next(
        num_channels: u32,
        impulse_response_size: u32,
        device: TrueAudioNextDevice,
        slot: u32,
    ) -> Self {
        Self {
            reflection_effect_type: ReflectionEffectType::TrueAudioNext,
            impulse_response: ReflectionEffectIR(std::ptr::null_mut()),
            reverb_times: <[f32; 3]>::default(),
            equalizer: Equalizer::default(),
            delay: 0,
            num_channels,
            impulse_response_size,
            true_audio_next_device: device,
            true_audio_next_slot: slot,
        }
    }
}

impl From<audionimbus_sys::IPLReflectionEffectParams> for ReflectionEffectParams {
    fn from(params: audionimbus_sys::IPLReflectionEffectParams) -> Self {
        Self {
            reflection_effect_type: params.type_.into(),
            impulse_response: ReflectionEffectIR(params.ir),
            reverb_times: params.reverbTimes,
            equalizer: Equalizer(params.eq),
            delay: params.delay as u32,
            num_channels: params.numChannels as u32,
            impulse_response_size: params.irSize as u32,
            true_audio_next_device: TrueAudioNextDevice(params.tanDevice),
            true_audio_next_slot: params.tanSlot as u32,
        }
    }
}

impl ReflectionEffectParams {
    pub(crate) fn as_ffi(
        &self,
    ) -> FFIWrapper<'_, audionimbus_sys::IPLReflectionEffectParams, Self> {
        let reflection_effect_params = audionimbus_sys::IPLReflectionEffectParams {
            type_: self.reflection_effect_type.into(),
            ir: self.impulse_response.0,
            reverbTimes: self.reverb_times,
            eq: *self.equalizer,
            delay: self.delay as i32,
            numChannels: self.num_channels as i32,
            irSize: self.impulse_response_size as i32,
            tanDevice: self.true_audio_next_device.raw_ptr(),
            tanSlot: self.true_audio_next_slot as i32,
        };

        FFIWrapper::new(reflection_effect_params)
    }
}

/// Bakes a single layer of reflections data in a probe batch.
///
/// Only one bake can be in progress at any point in time.
pub fn bake_reflections(
    context: &Context,
    reflections_bake_params: ReflectionsBakeParams,
    progress_callback: Option<CallbackInformation<ProgressCallback>>,
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
            context.raw_ptr(),
            &mut audionimbus_sys::IPLReflectionsBakeParams::from(reflections_bake_params),
            callback,
            user_data,
        );
    }
}

/// Cancels any running bakes of pathing data.
pub fn cancel_bake_reflections(context: &Context) {
    unsafe { audionimbus_sys::iplReflectionsBakerCancelBake(context.raw_ptr()) }
}

/// Parameters used to control how reflections data is baked.
#[derive(Debug, Copy, Clone)]
pub struct ReflectionsBakeParams<'a> {
    /// The scene in which the probes exist.
    pub scene: &'a Scene,

    /// A probe batch containing the probes at which reflections data should be baked.
    pub probe_batch: &'a ProbeBatch,

    /// The scene parameters.
    pub scene_params: SceneParams<'a>,

    /// An identifier for the data layer that should be baked.
    /// The identifier determines what data is simulated and stored at each probe.
    /// If the probe batch already contains data with this identifier, it will be overwritten.
    pub identifier: &'a BakedDataIdentifier,

    /// The types of data to save for each probe.
    pub bake_flags: ReflectionsBakeFlags,

    /// The number of rays to trace from each listener position when baking.
    /// Increasing this number results in improved accuracy, at the cost of increased bake times.
    pub num_rays: u32,

    /// The number of directions to consider when generating diffusely-reflected rays when baking.
    /// Increasing this number results in slightly improved accuracy of diffuse reflections.
    pub num_diffuse_samples: u32,

    /// The number of times each ray is reflected off of solid geometry.
    /// Increasing this number results in longer reverb tails and improved accuracy, at the cost of increased bake times.
    pub num_bounces: u32,

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
    pub order: u32,

    /// Number of threads to use for baking.
    pub num_threads: u32,

    /// When calculating how much sound energy reaches a surface directly from a source, any source that is closer than [`Self::irradiance_min_distance`] to the surface is assumed to be at a distance of [`Self::irradiance_min_distance`], for the purposes of energy calculations.
    pub irradiance_min_distance: f32,

    /// If using Radeon Rays or if [`Self::identifier`] uses [`BakedDataVariation::StaticListener`], this is the number of probes for which data is baked simultaneously.
    pub bake_batch_size: u32,
}

impl From<ReflectionsBakeParams<'_>> for audionimbus_sys::IPLReflectionsBakeParams {
    fn from(params: ReflectionsBakeParams) -> Self {
        let mut ray_batch_size = 0;
        let mut open_cl_device = &OpenClDevice::null();
        let mut radeon_rays_device = &RadeonRaysDevice::null();
        let scene_type = match params.scene_params {
            SceneParams::Default => audionimbus_sys::IPLSceneType::IPL_SCENETYPE_DEFAULT,
            SceneParams::Embree => audionimbus_sys::IPLSceneType::IPL_SCENETYPE_EMBREE,
            SceneParams::RadeonRays {
                open_cl_device: ocl_device,
                radeon_rays_device: rr_device,
            } => {
                open_cl_device = ocl_device;
                radeon_rays_device = rr_device;
                audionimbus_sys::IPLSceneType::IPL_SCENETYPE_RADEONRAYS
            }
            SceneParams::Custom {
                ray_batch_size: rb_size,
            } => {
                ray_batch_size = rb_size;
                audionimbus_sys::IPLSceneType::IPL_SCENETYPE_CUSTOM
            }
        };

        Self {
            scene: params.scene.raw_ptr(),
            probeBatch: params.probe_batch.raw_ptr(),
            sceneType: scene_type,
            identifier: (*params.identifier).into(),
            bakeFlags: params.bake_flags.into(),
            numRays: params.num_rays as i32,
            numDiffuseSamples: params.num_diffuse_samples as i32,
            numBounces: params.num_bounces as i32,
            simulatedDuration: params.simulated_duration,
            savedDuration: params.saved_duration,
            order: params.order as i32,
            numThreads: params.num_threads as i32,
            rayBatchSize: ray_batch_size as i32,
            irradianceMinDistance: params.irradiance_min_distance,
            bakeBatchSize: params.bake_batch_size as i32,
            openCLDevice: open_cl_device.raw_ptr(),
            radeonRaysDevice: radeon_rays_device.raw_ptr(),
        }
    }
}

bitflags::bitflags! {
    /// Flags for specifying what types of reflections data to bake.
    #[derive(Copy, Clone, Debug)]
    pub struct ReflectionsBakeFlags: u32 {
        /// Bake impulse responses for [`ReflectionEffectSettings::Convolution`], [`ReflectionEffectSettings::Hybrid`], or [`ReflectionEffectSettings::TrueAudioNext`].
        const BAKE_CONVOLUTION = 1 << 0;

        /// Bake parametric reverb for [`ReflectionEffectSettings::Parametric`] or [`ReflectionEffectSettings::Hybrid`].
        const BAKE_PARAMETRIC = 1 << 1;
    }
}

impl From<ReflectionsBakeFlags> for audionimbus_sys::IPLReflectionsBakeFlags {
    fn from(reflections_bake_flags: ReflectionsBakeFlags) -> Self {
        Self(reflections_bake_flags.bits() as _)
    }
}

/// Type of reflection effect algorithm to use.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ReflectionEffectType {
    /// Multi-channel convolution reverb.
    /// Reflections reaching the listener are encoded in an Impulse Response (IR), which is a filter that records each reflection as it arrives.
    /// This algorithm renders reflections with the most detail, but may result in significant CPU usage.
    /// Using a reflection mixer with this algorithm provides a reduction in CPU usage.
    Convolution,

    /// Parametric (or artificial) reverb, using feedback delay networks.
    /// The reflected sound field is reduced to a few numbers that describe how reflected energy decays over time.
    /// This is then used to drive an approximate model of reverberation in an indoor space.
    /// This algorithm results in lower CPU usage, but cannot render individual echoes, especially in outdoor spaces.
    /// A reflection mixer cannot be used with this algorithm.
    Parametric,

    /// A hybrid of convolution and parametric reverb.
    /// The initial portion of the IR is rendered using convolution reverb, but the later part is used to estimate a parametric reverb.
    /// The point in the IR where this transition occurs can be controlled.
    /// This algorithm allows a trade-off between rendering quality and CPU usage.
    /// An reflection mixer cannot be used with this algorithm.
    Hybrid,

    /// Multi-channel convolution reverb, using AMD TrueAudio Next for GPU acceleration.
    /// This algorithm is similar to [`Self::Convolution`], but uses the GPU instead of the CPU for processing, allowing significantly more sources to be processed.
    /// A reflection mixer must be used with this algorithm, because the GPU will process convolution reverb at a single point in your audio processing pipeline.
    TrueAudioNext,
}

impl From<ReflectionEffectType> for audionimbus_sys::IPLReflectionEffectType {
    fn from(reflection_effect_type: ReflectionEffectType) -> Self {
        match reflection_effect_type {
            ReflectionEffectType::Convolution => {
                audionimbus_sys::IPLReflectionEffectType::IPL_REFLECTIONEFFECTTYPE_CONVOLUTION
            }
            ReflectionEffectType::Parametric => {
                audionimbus_sys::IPLReflectionEffectType::IPL_REFLECTIONEFFECTTYPE_PARAMETRIC
            }
            ReflectionEffectType::Hybrid => {
                audionimbus_sys::IPLReflectionEffectType::IPL_REFLECTIONEFFECTTYPE_HYBRID
            }
            ReflectionEffectType::TrueAudioNext => {
                audionimbus_sys::IPLReflectionEffectType::IPL_REFLECTIONEFFECTTYPE_TAN
            }
        }
    }
}

impl From<audionimbus_sys::IPLReflectionEffectType> for ReflectionEffectType {
    fn from(reflection_effect_type: audionimbus_sys::IPLReflectionEffectType) -> Self {
        match reflection_effect_type {
            audionimbus_sys::IPLReflectionEffectType::IPL_REFLECTIONEFFECTTYPE_CONVOLUTION => {
                ReflectionEffectType::Convolution
            }
            audionimbus_sys::IPLReflectionEffectType::IPL_REFLECTIONEFFECTTYPE_PARAMETRIC => {
                ReflectionEffectType::Parametric
            }
            audionimbus_sys::IPLReflectionEffectType::IPL_REFLECTIONEFFECTTYPE_HYBRID => {
                ReflectionEffectType::Hybrid
            }
            audionimbus_sys::IPLReflectionEffectType::IPL_REFLECTIONEFFECTTYPE_TAN => {
                ReflectionEffectType::TrueAudioNext
            }
        }
    }
}

/// Mixes the outputs of multiple reflection effects, and generates a single sound field containing all the reflected sound reaching the listener.
///
/// Using this is optional. Depending on the reflection effect algorithm used, a reflection mixer may provide a reduction in CPU usage.
#[derive(Debug)]
pub struct ReflectionMixer(audionimbus_sys::IPLReflectionMixer);

impl ReflectionMixer {
    /// Creates a new reflection mixer.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if mixer creation fails.
    pub fn try_new(
        context: &Context,
        audio_settings: &AudioSettings,
        reflection_effect_settings: &ReflectionEffectSettings,
    ) -> Result<Self, SteamAudioError> {
        let mut reflection_mixer = Self(std::ptr::null_mut());

        let status = unsafe {
            audionimbus_sys::iplReflectionMixerCreate(
                context.raw_ptr(),
                &mut audionimbus_sys::IPLAudioSettings::from(audio_settings),
                &mut audionimbus_sys::IPLReflectionEffectSettings::from(reflection_effect_settings),
                reflection_mixer.raw_ptr_mut(),
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        Ok(reflection_mixer)
    }

    /// Retrieves the contents of the reflection mixer and places it into the audio buffer.
    pub fn apply<O, PO: ChannelPointers>(
        &mut self,
        reflection_effect_params: &mut ReflectionEffectParams,
        output_buffer: &AudioBuffer<O, PO>,
    ) -> AudioEffectState
    where
        O: AsRef<[Sample]> + AsMut<[Sample]>,
    {
        let audio_effect_state = unsafe {
            audionimbus_sys::iplReflectionMixerApply(
                self.raw_ptr(),
                &mut *reflection_effect_params.as_ffi(),
                &mut *output_buffer.as_ffi(),
            )
        };
        audio_effect_state.into()
    }

    /// Resets the internal processing state of a reflection mixer.
    pub fn reset(&mut self) {
        unsafe { audionimbus_sys::iplReflectionMixerReset(self.raw_ptr()) };
    }

    /// Returns the raw FFI pointer to the underlying reflection mixer.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub fn raw_ptr(&self) -> audionimbus_sys::IPLReflectionMixer {
        self.0
    }

    /// Returns a mutable reference to the raw FFI pointer.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLReflectionMixer {
        &mut self.0
    }
}

impl Clone for ReflectionMixer {
    fn clone(&self) -> Self {
        unsafe {
            audionimbus_sys::iplReflectionMixerRetain(self.0);
        }
        Self(self.0)
    }
}

impl Drop for ReflectionMixer {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplReflectionMixerRelease(&mut self.0) }
    }
}

unsafe impl Send for ReflectionMixer {}
unsafe impl Sync for ReflectionMixer {}
