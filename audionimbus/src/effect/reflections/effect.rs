//! Reflection effects and their retained parameters.

use crate::ChannelRequirement;
use crate::audio_buffer::{AudioBuffer, AudioBufferMut, read_as_ffi};
use crate::audio_settings::AudioSettings;
use crate::context::Context;
use crate::device::true_audio_next::TrueAudioNextDevice;
use crate::effect::EffectError;
use crate::effect::audio_effect_state::AudioEffectState;
use crate::effect::equalizer::Equalizer;
use crate::effect::error::{ImpulseResponseSizeExceedsMaxError, NumChannelsExceedsMaxError};
use crate::error::{SteamAudioError, to_option_error};
use crate::ffi_wrapper::FFIWrapper;
#[cfg(feature = "dynamic")]
use std::fmt;
use std::hash::{Hash, Hasher};

#[cfg(feature = "dynamic")]
use super::binding::ReflectionAlgorithm;
use super::binding::ReflectionBinding;
use super::mixer::{ReflectionMixer, StaticReflectionMixer};
use super::{
    CanApplyDirectly, CanUseReflectionMixer, Convolution, Hybrid, Parametric, ReflectionEffectType,
    TrueAudioNext,
};
#[cfg(feature = "dynamic")]
use crate::binding::Dynamic;
use crate::binding::Static;

#[cfg(doc)]
use crate::effect::AmbisonicsDecodeEffect;
#[cfg(doc)]
use crate::geometry::Scene;
#[cfg(doc)]
use crate::simulation::{SimulationOutputs, Simulator, Source};

/// Applies the result of physics-based reflections simulation to an audio buffer.
///
/// The result is encoded in ambisonics, and can be decoded using an ambisonics decode effect ([`AmbisonicsDecodeEffect`]).
///
/// `ReflectionEffect` is a reference-counted handle to an underlying Steam Audio object.
/// Cloning it is cheap; it produces a new handle pointing to the same underlying object, while
/// incrementing a reference count.
/// The underlying object is destroyed when all handles are dropped.
///
/// # Examples
///
/// Applying reflections involves:
/// 1. Setting up a [`Simulator`] with a [`Scene`]
/// 2. Adding [`Source`]s to the scene (don't forget to commit the changes using `Simulator::commit`!)
/// 3. Running the simulation (`Simulator::run_reflections`) and retrieving the output for the source (`Source::get_outputs`)
/// 4. Applying the reflection effect to the audio buffer (or to a [`ReflectionMixer`](crate::ReflectionMixer) for better performance with supported reflection algorithms) using the simulation output (`SimulationOutputs::reflections`) as params
///
/// ```
/// # use audionimbus::*;
/// let context = Context::default();
/// let audio_settings = AudioSettings::default();
///
/// // Create a simulator with reflections.
/// let simulation_settings = SimulationSettings::new(&audio_settings)
///     .with_reflections(ConvolutionSettings {
///         max_num_rays: 4096,
///         num_diffuse_samples: 32,
///         max_duration: 2.0,
///         max_num_sources: 8,
///         num_threads: 2,
///         max_order: 1,
///     });
/// let mut simulator = Simulator::try_new(&context, &simulation_settings)?;
///
/// let scene = Scene::try_new(&context)?;
/// simulator.set_scene(&scene);
///
/// let mut source = Source::try_new(&simulator)?;
///
/// source.set_reflections_inputs(
///     &SimulationInputs {
///         source: CoordinateSystem::default(),
///         parameters: SimulationParameters::new()
///             .with_reflections(
///                 ConvolutionParameters {
///                     baked_data_identifier: None,
///                 },
///             ),
///     }
/// );
///
/// simulator.add_source(&source);
/// simulator.set_shared_reflections_inputs(
///     &SimulationSharedInputs::new(CoordinateSystem::default()).with_reflections(
///         ReflectionsSharedInputs {
///             num_rays: 4096,
///             num_bounces: 16,
///             duration: 2.0,
///             order: 1,
///             irradiance_min_distance: 1.0,
///         },
///     ),
/// );
/// simulator.commit();
///
/// simulator.run_reflections();
/// let params = source.get_reflections_outputs()?;
///
/// const NUM_CHANNELS: u32 = num_ambisonics_channels(1); // 1st order ambisonics
/// let mut effect = ReflectionEffect::<Convolution>::try_new(
///     &context,
///     &audio_settings,
///     &ReflectionEffectSettings {
///         impulse_response_size: 2 * audio_settings.sampling_rate, // 2 seconds
///         num_channels: NUM_CHANNELS,
///     },
/// )?;
///
/// let input = vec![0.5; audio_settings.frame_size as usize];
/// let input_buffer = AudioBufferRef::try_from(input.as_slice())?;
/// let mut output = vec![0.0; (NUM_CHANNELS * audio_settings.frame_size) as usize]; // 4 channels
/// let mut output_buffer = AudioBufferMut::try_new(&mut output, NUM_CHANNELS as usize)?;
///
/// let _ = effect.apply(&params, &input_buffer, &mut output_buffer);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Simulating Reverb
///
/// In addition to modeling reflections from sources, you can use this effect to simulate reverb
/// by placing a source at the listener's position:
///
/// ```
/// # use audionimbus::*;
/// let context = Context::default();
/// let audio_settings = AudioSettings::default();
///
/// // Create simulator with reflections
/// let simulation_settings = SimulationSettings::new(&audio_settings)
///     .with_reflections(ConvolutionSettings {
///         max_num_rays: 2048,
///         num_diffuse_samples: 32,
///         max_duration: 2.0,
///         max_num_sources: 8,
///         num_threads: 2,
///         max_order: 1,
///     });
/// let mut simulator = Simulator::try_new(&context, &simulation_settings)?;
///
/// let scene = Scene::try_new(&context)?;
/// simulator.set_scene(&scene);
///
/// // Create a reverb source positioned at the listener.
/// let mut reverb_source = Source::try_new(&simulator)?;
///
/// let listener_position = CoordinateSystem {
///     origin: Vector3::new(0.0, 1.5, 0.0), // Listener at head height
///     ..Default::default()
/// };
///
/// // Set source position to match listener position.
/// reverb_source.set_reflections_inputs(
///     &SimulationInputs {
///         source: listener_position, // Source at listener = reverb
///         parameters: SimulationParameters::new()
///             .with_reflections(ConvolutionParameters {
///                 baked_data_identifier: None,
///             }),
///     }
/// );
///
/// simulator.add_source(&reverb_source);
/// simulator.set_shared_reflections_inputs(
///     &SimulationSharedInputs::new(CoordinateSystem::default()).with_reflections(
///         ReflectionsSharedInputs {
///             num_rays: 4096,
///             num_bounces: 16,
///             duration: 2.0,
///             order: 1,
///             irradiance_min_distance: 1.0,
///         },
///     ),
/// );
/// simulator.commit();
///
/// // Run simulation.
/// simulator.run_reflections();
/// let reverb_params = reverb_source.get_reflections_outputs()?;
///
/// const NUM_CHANNELS: u32 = num_ambisonics_channels(1); // 1st order ambisonics
/// let mut reverb_effect = ReflectionEffect::<Convolution>::try_new(
///     &context,
///     &audio_settings,
///     &ReflectionEffectSettings {
///         impulse_response_size: 2 * audio_settings.sampling_rate,
///         num_channels: NUM_CHANNELS,
///     },
/// )?;
///
/// let input = vec![0.5; audio_settings.frame_size as usize];
/// let input_buffer = AudioBufferRef::try_from(input.as_slice())?;
/// let mut reverb_output = vec![0.0; (NUM_CHANNELS * audio_settings.frame_size) as usize];
/// let mut output_buffer =
///     AudioBufferMut::try_new(&mut reverb_output, NUM_CHANNELS as usize)?;
///
/// let _ = reverb_effect.apply(&reverb_params, &input_buffer, &mut output_buffer);
///
/// // Mix with dry signal (e.g., 70% dry, 30% reverb)
/// // Then decode the ambisonics output for final playback
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub type StaticReflectionEffect<T> = ReflectionEffect<Static<T>>;

/// A reflection effect with a runtime-selected algorithm.
///
/// Static and runtime reflection handles cannot be mixed.
///
/// ```
/// use audionimbus::{AudioBufferMut, AudioSettings, Context, ReflectionEffectSettings};
/// use audionimbus::dynamic::{ReflectionAlgorithm, ReflectionEffect, ReflectionMixer};
/// let context = Context::default();
/// let audio_settings = AudioSettings::default();
/// let settings = ReflectionEffectSettings {
///     impulse_response_size: audio_settings.sampling_rate,
///     num_channels: 4,
/// };
/// let mut effect = ReflectionEffect::try_new(
///     &context, &audio_settings, &settings, ReflectionAlgorithm::Convolution,
/// )?;
/// let mut mixer = ReflectionMixer::try_new(
///     &context, &audio_settings, &settings, effect.algorithm(),
/// )?;
/// let mut samples = vec![0.0; 4 * audio_settings.frame_size as usize];
/// let mut output = AudioBufferMut::try_new(&mut samples, 4)?;
/// effect.tail_into_mixer(&mut output, &mixer)?;
/// effect.reset();
/// mixer.reset();
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[cfg(feature = "dynamic")]
pub type DynamicReflectionEffect = ReflectionEffect<Dynamic<ReflectionAlgorithm>>;

/// Reflection effect storage for either API binding.
#[derive(Debug)]
pub struct ReflectionEffect<B: ReflectionBinding> {
    /// The native reflection handle.
    inner: audionimbus_sys::IPLReflectionEffect,

    /// Number of output channels needed, i.e. as many channels as the impulse response specified
    /// when creating the effect.
    num_output_channels: ChannelRequirement,

    /// The compile-time or runtime reflection binding.
    binding: B,
}

#[cfg(feature = "dynamic")]
impl DynamicReflectionEffect {
    /// Creates a reflection effect with the selected algorithm.
    ///
    /// # Errors
    ///
    /// - [`SteamAudioError::Unspecified`] if the native operation fails.
    /// - [`SteamAudioError::OutOfMemory`] if native memory allocation fails.
    /// - [`SteamAudioError::Initialization`] if an external dependency cannot initialize.
    pub fn try_new(
        context: &Context,
        audio_settings: &AudioSettings,
        reflection_effect_settings: &ReflectionEffectSettings,
        algorithm: ReflectionAlgorithm,
    ) -> Result<Self, SteamAudioError> {
        Self::try_new_shared(
            context,
            audio_settings,
            reflection_effect_settings,
            Dynamic(algorithm),
        )
    }

    /// Returns the selected algorithm.
    pub const fn algorithm(&self) -> ReflectionAlgorithm {
        self.binding.0
    }

    /// Applies reflections directly to an audio buffer.
    ///
    /// # Errors
    ///
    /// - [`ReflectionError::DirectApplyUnsupported`] when TAN requires a mixer.
    /// - [`ReflectionError::AlgorithmMismatch`] when the parameters use another algorithm.
    /// - [`ReflectionError::Effect`] when an audio buffer has an invalid channel count.
    pub fn apply(
        &mut self,
        reflection_effect_params: &crate::dynamic::ReflectionEffectParams,
        input_buffer: &impl AudioBuffer,
        output_buffer: &mut AudioBufferMut<'_>,
    ) -> Result<AudioEffectState, ReflectionError> {
        if self.algorithm() == ReflectionAlgorithm::TrueAudioNext {
            return Err(ReflectionError::DirectApplyUnsupported);
        }
        if self.algorithm() != reflection_effect_params.algorithm() {
            return Err(ReflectionError::AlgorithmMismatch {
                expected: self.algorithm(),
                actual: reflection_effect_params.algorithm(),
            });
        }

        self.apply_shared(reflection_effect_params, input_buffer, output_buffer, None)
            .map_err(ReflectionError::Effect)
    }

    /// Applies reflections into a mixer with the same algorithm.
    ///
    /// # Errors
    ///
    /// - [`ReflectionError::MixerUnsupported`] when parametric or hybrid reverb cannot use a mixer.
    /// - [`ReflectionError::AlgorithmMismatch`] when the parameters or mixer use another algorithm.
    /// - [`ReflectionError::Effect`] when an audio buffer has an invalid channel count.
    pub fn apply_into_mixer(
        &mut self,
        reflection_effect_params: &crate::dynamic::ReflectionEffectParams,
        input_buffer: &impl AudioBuffer,
        output_buffer: &mut AudioBufferMut<'_>,
        mixer: &crate::dynamic::ReflectionMixer,
    ) -> Result<AudioEffectState, ReflectionError> {
        if !matches!(
            self.algorithm(),
            ReflectionAlgorithm::Convolution | ReflectionAlgorithm::TrueAudioNext
        ) {
            return Err(ReflectionError::MixerUnsupported);
        }
        if self.algorithm() != reflection_effect_params.algorithm() {
            return Err(ReflectionError::AlgorithmMismatch {
                expected: self.algorithm(),
                actual: reflection_effect_params.algorithm(),
            });
        }
        if self.algorithm() != mixer.algorithm() {
            return Err(ReflectionError::AlgorithmMismatch {
                expected: self.algorithm(),
                actual: mixer.algorithm(),
            });
        }

        self.apply_shared(
            reflection_effect_params,
            input_buffer,
            output_buffer,
            Some(mixer),
        )
        .map_err(ReflectionError::Effect)
    }

    /// Retrieves tail samples directly into an audio buffer.
    ///
    /// # Errors
    ///
    /// - [`ReflectionError::DirectApplyUnsupported`] when TAN requires a mixer.
    /// - [`ReflectionError::Effect`] when the output buffer has an invalid channel count.
    pub fn tail(
        &self,
        output_buffer: &mut AudioBufferMut<'_>,
    ) -> Result<AudioEffectState, ReflectionError> {
        if self.algorithm() == ReflectionAlgorithm::TrueAudioNext {
            return Err(ReflectionError::DirectApplyUnsupported);
        }

        self.tail_shared(output_buffer, None)
            .map_err(ReflectionError::Effect)
    }

    /// Retrieves tail samples into a mixer with the same algorithm.
    ///
    /// # Errors
    ///
    /// - [`ReflectionError::MixerUnsupported`] when parametric or hybrid reverb cannot use a mixer.
    /// - [`ReflectionError::AlgorithmMismatch`] when the mixer uses another algorithm.
    /// - [`ReflectionError::Effect`] when the output buffer has an invalid channel count.
    pub fn tail_into_mixer(
        &self,
        output_buffer: &mut AudioBufferMut<'_>,
        mixer: &crate::dynamic::ReflectionMixer,
    ) -> Result<AudioEffectState, ReflectionError> {
        if !matches!(
            self.algorithm(),
            ReflectionAlgorithm::Convolution | ReflectionAlgorithm::TrueAudioNext
        ) {
            return Err(ReflectionError::MixerUnsupported);
        }
        if self.algorithm() != mixer.algorithm() {
            return Err(ReflectionError::AlgorithmMismatch {
                expected: self.algorithm(),
                actual: mixer.algorithm(),
            });
        }

        self.tail_shared(output_buffer, Some(mixer))
            .map_err(ReflectionError::Effect)
    }
}

impl<T: ReflectionEffectType> StaticReflectionEffect<T> {
    /// Creates a new reflection effect and returns a handle to it.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if effect creation fails.
    pub fn try_new(
        context: &Context,
        audio_settings: &AudioSettings,
        reflection_effect_settings: &ReflectionEffectSettings,
    ) -> Result<Self, SteamAudioError> {
        Self::try_new_shared(
            context,
            audio_settings,
            reflection_effect_settings,
            Static::new(),
        )
    }

    /// Returns a mutable reference to the raw FFI pointer.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub const fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLReflectionEffect {
        &mut self.inner
    }
}

impl<B: ReflectionBinding> ReflectionEffect<B> {
    /// Creates a reflection handle for the selected binding.
    ///
    /// # Errors
    ///
    /// - [`SteamAudioError::Unspecified`] when native creation fails.
    /// - [`SteamAudioError::OutOfMemory`] when allocation fails.
    /// - [`SteamAudioError::Initialization`] when a native dependency fails.
    fn try_new_shared(
        context: &Context,
        audio_settings: &AudioSettings,
        reflection_effect_settings: &ReflectionEffectSettings,
        binding: B,
    ) -> Result<Self, SteamAudioError> {
        let mut inner = std::ptr::null_mut();

        let status = unsafe {
            audionimbus_sys::iplReflectionEffectCreate(
                context.raw_ptr(),
                &mut audionimbus_sys::IPLAudioSettings::from(audio_settings),
                &mut binding.ffi_settings(reflection_effect_settings),
                &raw mut inner,
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        let num_output_channels = binding.num_output_channels(reflection_effect_settings);

        let reflection_effect = Self {
            inner,
            num_output_channels,
            binding,
        };

        Ok(reflection_effect)
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
    pub const fn raw_ptr(&self) -> audionimbus_sys::IPLReflectionEffect {
        self.inner
    }
}

impl<T: ReflectionEffectType + CanApplyDirectly> StaticReflectionEffect<T> {
    /// Applies a reflection effect to an audio buffer.
    ///
    /// This effect CANNOT be applied in-place.
    ///
    /// The input audio buffer must have one channel, and the output audio buffer must have as many
    /// channels as the impulse response specified when creating the effect (for convolution,
    /// hybrid, and TrueAudioNext) or at least one channel (for parametric).
    ///
    /// # Errors
    ///
    /// Returns [`EffectError`] if:
    /// - The input buffer has more than one channel
    /// - The output audio buffer does not have as many channels as the impulse response specified
    ///   when creating the effect (for convolution, hybrid, and TrueAudioNext) or at least one channel
    ///   (for parametric)
    pub fn apply(
        &mut self,
        reflection_effect_params: &StaticReflectionEffectParams<T>,
        input_buffer: &impl AudioBuffer,
        output_buffer: &mut AudioBufferMut<'_>,
    ) -> Result<AudioEffectState, EffectError> {
        self.apply_shared(reflection_effect_params, input_buffer, output_buffer, None)
    }

    /// Retrieves a single frame of tail samples from a reflection effect’s internal buffers.
    ///
    /// After the input to the reflection effect has stopped, this function must be called instead of [`Self::apply`] until the return value indicates that no more tail samples remain.
    ///
    /// The output audio buffer must have as many channels as the impulse response specified when
    /// creating the effect (for convolution, hybrid, and TrueAudioNext) or at least one
    /// channel (for parametric).
    ///
    /// # Errors
    ///
    /// Returns [`EffectError`] if the output audio buffer does not have as many channels as the impulse response specified
    /// when creating the effect (for convolution, hybrid, and TrueAudioNext) or at lea one channel (for parametric).
    pub fn tail(
        &self,
        output_buffer: &mut AudioBufferMut<'_>,
    ) -> Result<AudioEffectState, EffectError> {
        self.tail_shared(output_buffer, None)
    }
}

impl<T: ReflectionEffectType + CanUseReflectionMixer> StaticReflectionEffect<T> {
    /// Applies a reflection effect to an audio buffer.
    ///
    /// The output of this effect will be mixed into the given mixer.
    ///
    /// The mixed output can be retrieved elsewhere in the audio pipeline using [`ReflectionMixer::apply`](crate::ReflectionMixer::apply).
    /// This can have a performance benefit if using convolution.
    ///
    /// The input audio buffer must have one channel, and the output audio buffer must have as many
    /// channels as the impulse response specified when creating the effect (for convolution,
    /// hybrid, and TrueAudioNext) or at least one channel (for parametric).
    ///
    /// # Errors
    ///
    /// Returns [`EffectError`] if:
    /// - The input buffer has more than one channel
    /// - The output audio buffer does not have as many channels as the impulse response specified
    ///   when creating the effect (for convolution, hybrid, and TrueAudioNext) or at lea one channel
    ///   (for parametric)
    pub fn apply_into_mixer(
        &mut self,
        reflection_effect_params: &StaticReflectionEffectParams<T>,
        input_buffer: &impl AudioBuffer,
        output_buffer: &mut AudioBufferMut<'_>,
        mixer: &StaticReflectionMixer<T>,
    ) -> Result<AudioEffectState, EffectError> {
        self.apply_shared(
            reflection_effect_params,
            input_buffer,
            output_buffer,
            Some(mixer),
        )
    }

    /// Retrieves a single frame of tail samples from a reflection effect’s internal buffers.
    ///
    /// After the input to the reflection effect has stopped, this function must be called instead of [`Self::apply`] until the return value indicates that no more tail samples remain.
    ///
    /// The tail samples will be mixed into the given mixer.
    /// The mixed output can be retrieved elsewhere in the audio pipeline using [`ReflectionMixer::apply`](crate::ReflectionMixer::apply).
    /// This can have a performance benefit if using convolution.
    /// If using TAN, specifying a mixer is required.
    ///
    /// The output audio buffer must have as many channels as the impulse response specified when
    /// creating the effect (for convolution, hybrid, and TrueAudioNext) or at least one
    /// channel (for parametric).
    ///
    /// # Errors
    ///
    /// Returns [`EffectError`] if the output audio buffer does not have as many channels as the impulse response specified
    /// when creating the effect (for convolution, hybrid, and TrueAudioNext) or at lea one channel (for parametric).
    pub fn tail_into_mixer(
        &self,
        output_buffer: &mut AudioBufferMut<'_>,
        mixer: &StaticReflectionMixer<T>,
    ) -> Result<AudioEffectState, EffectError> {
        self.tail_shared(output_buffer, Some(mixer))
    }
}

impl<B: ReflectionBinding> ReflectionEffect<B> {
    /// Applies reflections through the selected output route.
    ///
    /// # Errors
    ///
    /// - [`EffectError::InvalidOutputChannels`] when the output channel count is invalid.
    /// - [`EffectError::InvalidInputChannels`] when the input is not mono.
    #[inline(always)]
    fn apply_shared(
        &mut self,
        reflection_effect_params: &ReflectionEffectParams<B>,
        input_buffer: &impl AudioBuffer,
        output_buffer: &mut AudioBufferMut<'_>,
        mixer: Option<&ReflectionMixer<B>>,
    ) -> Result<AudioEffectState, EffectError> {
        let num_input_channels = input_buffer.num_channels() as u32;
        if num_input_channels != 1 {
            return Err(EffectError::InvalidInputChannels {
                expected: ChannelRequirement::Exactly(1),
                actual: num_input_channels,
            });
        }

        let num_output_channels = output_buffer.num_channels() as u32;
        if !self
            .num_output_channels
            .is_satisfied_by(num_output_channels)
        {
            return Err(EffectError::InvalidOutputChannels {
                expected: self.num_output_channels,
                actual: num_output_channels,
            });
        }

        let mut input_buffer = read_as_ffi(input_buffer);
        let mut output_buffer = output_buffer.as_ffi_mut();
        let state = unsafe {
            audionimbus_sys::iplReflectionEffectApply(
                self.raw_ptr(),
                &raw mut *reflection_effect_params.as_ffi(),
                &raw mut *input_buffer,
                &raw mut *output_buffer,
                mixer.map_or(std::ptr::null_mut(), ReflectionMixer::raw_ptr),
            )
        }
        .into();

        Ok(state)
    }

    /// Retrieves tail samples through the selected output route.
    ///
    /// # Errors
    ///
    /// - [`EffectError::InvalidOutputChannels`] when the output channel count is invalid.
    fn tail_shared(
        &self,
        output_buffer: &mut AudioBufferMut<'_>,
        mixer: Option<&ReflectionMixer<B>>,
    ) -> Result<AudioEffectState, EffectError> {
        let num_output_channels = output_buffer.num_channels() as u32;
        if !self
            .num_output_channels
            .is_satisfied_by(num_output_channels)
        {
            return Err(EffectError::InvalidOutputChannels {
                expected: self.num_output_channels,
                actual: num_output_channels,
            });
        }

        let mut output_buffer = output_buffer.as_ffi_mut();
        let state = unsafe {
            audionimbus_sys::iplReflectionEffectGetTail(
                self.raw_ptr(),
                &raw mut *output_buffer,
                mixer.map_or(std::ptr::null_mut(), ReflectionMixer::raw_ptr),
            )
        }
        .into();

        Ok(state)
    }
}

impl<B: ReflectionBinding> Drop for ReflectionEffect<B> {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplReflectionEffectRelease(&raw mut self.inner) }
    }
}

unsafe impl<B: ReflectionBinding> Send for ReflectionEffect<B> {}
unsafe impl<B: ReflectionBinding> Sync for ReflectionEffect<B> {}

impl<B: ReflectionBinding> Clone for ReflectionEffect<B> {
    /// Retains an additional reference to the reflection effect.
    ///
    /// The returned [`ReflectionEffect`](crate::ReflectionEffect) shares the same underlying Steam Audio object.
    fn clone(&self) -> Self {
        // SAFETY: The reflection effect will not be destroyed until all references are released.
        Self {
            inner: unsafe { audionimbus_sys::iplReflectionEffectRetain(self.inner) },
            num_output_channels: self.num_output_channels,
            binding: self.binding,
        }
    }
}

impl<B: ReflectionBinding> PartialEq for ReflectionEffect<B> {
    fn eq(&self, other: &Self) -> bool {
        self.raw_ptr() == other.raw_ptr()
    }
}

impl<B: ReflectionBinding> Eq for ReflectionEffect<B> {}

impl<B: ReflectionBinding> Hash for ReflectionEffect<B> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::ptr::hash(self.raw_ptr(), state);
    }
}

/// Settings used to create a reflection effect.
#[derive(Copy, Clone, Debug)]
pub struct ReflectionEffectSettings {
    /// Number of samples per channel in the IR.
    pub impulse_response_size: u32,

    /// Number of channels in the IR.
    pub num_channels: u32,
}

/// The impulse response of [`ReflectionEffectParams`](crate::ReflectionEffectParams).
#[derive(Debug, Eq, PartialEq)]
pub struct ReflectionEffectIR(pub audionimbus_sys::IPLReflectionEffectIR);

unsafe impl Send for ReflectionEffectIR {}

/// Parameters for applying a reflection effect to an audio buffer.
pub type StaticReflectionEffectParams<T> = ReflectionEffectParams<Static<T>>;

/// Reflection outputs from a runtime-selected source.
#[cfg(feature = "dynamic")]
pub type DynamicReflectionEffectParams = ReflectionEffectParams<Dynamic<ReflectionAlgorithm>>;

/// Reflection parameters for either API binding.
#[derive(Debug, PartialEq)]
pub struct ReflectionEffectParams<B: ReflectionBinding> {
    /// The impulse response.
    impulse_response: ReflectionEffectIR,

    /// 3-band reverb decay times (RT60).
    reverb_times: [f32; 3],

    /// 3-band EQ coefficients applied to the parametric part to ensure smooth transition.
    equalizer: Equalizer<3>,

    /// Samples after which parametric part starts.
    delay: u32,

    /// Number of IR channels to process.
    /// May be less than the number of channels specified when creating the effect, in which case CPU usage will be reduced.
    num_channels: u32,

    /// Number of IR samples per channel to process.
    /// May be less than the number of samples specified when creating the effect, in which case CPU usage will be reduced.
    impulse_response_size: u32,

    /// Maximum number of IR channels, as specified during creation.
    max_num_channels: u32,

    /// Maximum number of IR samples per channel, as specified during creation.
    max_impulse_response_size: u32,

    /// The TrueAudio Next device to use for convolution processing.
    true_audio_next_device: Option<TrueAudioNextDevice>,

    /// The TrueAudio Next slot index to use for convolution processing.
    /// The slot identifies the IR to use.
    true_audio_next_slot: u32,

    /// Retained source pointer, keeping the IR pointer valid.
    _source: audionimbus_sys::IPLSource,

    /// The compile-time or runtime reflection binding.
    binding: B,
}

impl StaticReflectionEffectParams<Convolution> {
    /// Constructs multi-channel convolution reverb params.
    ///
    /// # Arguments
    ///
    /// - `impulse_response`: the impulse response.
    /// - `num_channels`: number of IR channels to process. May be less than the number of channels specified when creating the effect, in which case CPU usage will be reduced.
    /// - `impulse_response_size`: number of IR samples per channel to process. May be less than the number of samples specified when creating the effect, in which case CPU usage will be reduced.
    ///
    /// # Safety
    ///
    /// `impulse_response` is a pointer managed internally by Steam Audio.
    /// The caller must ensure the type that owns this IR remains valid for the lifetime of these
    /// params.
    pub unsafe fn new(
        impulse_response: audionimbus_sys::IPLReflectionEffectIR,
        num_channels: u32,
        impulse_response_size: u32,
    ) -> Self {
        Self {
            impulse_response: ReflectionEffectIR(impulse_response),
            reverb_times: <[f32; 3]>::default(),
            equalizer: Equalizer::default(),
            delay: 0,
            num_channels,
            impulse_response_size,
            max_num_channels: num_channels,
            max_impulse_response_size: impulse_response_size,
            true_audio_next_device: None,
            true_audio_next_slot: 0,
            _source: std::ptr::null_mut(),
            binding: Static::new(),
        }
    }
}

impl StaticReflectionEffectParams<Parametric> {
    /// Constructs parametric (or artificial) reverb params.
    ///
    /// # Arguments
    ///
    /// - `reverb_times`: 3-band reverb decay times (RT60).
    /// - `num_channels`: number of IR channels to process. May be less than the number of channels specified when creating the effect, in which case CPU usage will be reduced.
    /// - `impulse_response_size`: number of IR samples per channel to process. May be less than the number of samples specified when creating the effect, in which case CPU usage will be reduced.
    pub fn new(reverb_times: [f32; 3], num_channels: u32, impulse_response_size: u32) -> Self {
        Self {
            impulse_response: ReflectionEffectIR(std::ptr::null_mut()),
            reverb_times,
            equalizer: Equalizer::default(),
            delay: 0,
            num_channels,
            impulse_response_size,
            max_num_channels: num_channels,
            max_impulse_response_size: impulse_response_size,
            true_audio_next_device: None,
            true_audio_next_slot: 0,
            _source: std::ptr::null_mut(),
            binding: Static::new(),
        }
    }
}

impl StaticReflectionEffectParams<Hybrid> {
    /// Constructs params for a hybrid of convolution and parametric reverb.
    ///
    /// # Arguments
    ///
    /// - `impulse_response`: the impulse response.
    /// - `reverb_times`: 3-band reverb decay times (RT60).
    /// - `equalizer`: 3-band EQ coefficients applied to the parametric part to ensure smooth transition.
    /// - `delay`: samples after which parametric part starts.
    /// - `num_channels`: number of IR channels to process. May be less than the number of channels specified when creating the effect, in which case CPU usage will be reduced.
    /// - `impulse_response_size`: number of IR samples per channel to process. May be less than the number of samples specified when creating the effect, in which case CPU usage will be reduced.
    ///
    /// # Safety
    ///
    /// `impulse_response` is a pointer managed internally by Steam Audio.
    /// The caller must ensure the type that owns this IR remains valid for the lifetime of these
    /// params.
    pub const unsafe fn new(
        impulse_response: audionimbus_sys::IPLReflectionEffectIR,
        reverb_times: [f32; 3],
        equalizer: Equalizer<3>,
        delay: u32,
        num_channels: u32,
        impulse_response_size: u32,
    ) -> Self {
        Self {
            impulse_response: ReflectionEffectIR(impulse_response),
            reverb_times,
            equalizer,
            delay,
            num_channels,
            impulse_response_size,
            max_num_channels: num_channels,
            max_impulse_response_size: impulse_response_size,
            true_audio_next_device: None,
            true_audio_next_slot: 0,
            _source: std::ptr::null_mut(),
            binding: Static::new(),
        }
    }
}

impl StaticReflectionEffectParams<TrueAudioNext> {
    /// Constructs multi-channel convolution reverb (using AMD TrueAudio Next for GPU acceleration)
    /// params.
    ///
    /// # Arguments
    ///
    /// - `num_channels`: number of IR channels to process. May be less than the number of channels specified when creating the effect, in which case CPU usage will be reduced.
    /// - `impulse_response_size`: number of IR samples per channel to process. May be less than the number of samples specified when creating the effect, in which case CPU usage will be reduced.
    /// - `device`: the TrueAudio Next device to use for convolution processing.
    /// - `slot`: the TrueAudio Next slot index to use for convolution processing. The slot identifies the IR to use.
    pub fn new(
        num_channels: u32,
        impulse_response_size: u32,
        device: TrueAudioNextDevice,
        slot: u32,
    ) -> Self {
        Self {
            impulse_response: ReflectionEffectIR(std::ptr::null_mut()),
            reverb_times: <[f32; 3]>::default(),
            equalizer: Equalizer::default(),
            delay: 0,
            num_channels,
            impulse_response_size,
            max_num_channels: num_channels,
            max_impulse_response_size: impulse_response_size,
            true_audio_next_device: Some(device),
            true_audio_next_slot: slot,
            _source: std::ptr::null_mut(),
            binding: Static::new(),
        }
    }
}

impl<B: ReflectionBinding> ReflectionEffectParams<B> {
    /// Sets the number of impulse response channels to process.
    ///
    /// May be less than the number of channels specified when creating the effect.
    ///
    /// # Errors
    ///
    /// Returns [`NumChannelsExceedsMaxError`] if `num_channels` exceeds the value specified during
    /// creation.
    pub fn set_num_channels(
        &mut self,
        num_channels: u32,
    ) -> Result<(), NumChannelsExceedsMaxError> {
        if num_channels > self.max_num_channels {
            return Err(NumChannelsExceedsMaxError {
                requested: num_channels,
                max: self.max_num_channels,
            });
        }

        self.num_channels = num_channels;

        Ok(())
    }

    /// Sets the number of impulse response samples per channel to process.
    ///
    /// May be less than the number of samples specified when creating the effect.
    ///
    /// # Errors
    ///
    /// Returns [`ImpulseResponseSizeExceedsMaxError`] if `impulse_response_size` exceeds the value
    /// specified during creation.
    pub fn set_impulse_response_size(
        &mut self,
        impulse_response_size: u32,
    ) -> Result<(), ImpulseResponseSizeExceedsMaxError> {
        if impulse_response_size > self.max_impulse_response_size {
            return Err(ImpulseResponseSizeExceedsMaxError {
                requested: impulse_response_size,
                max: self.max_impulse_response_size,
            });
        }

        self.impulse_response_size = impulse_response_size;

        Ok(())
    }

    /// Constructs params from FFI representation.
    ///
    /// Only simulator outputs can establish the retained impulse response contract.
    /// Retains native parameter resources for the selected binding.
    ///
    /// # Safety
    ///
    /// The impulse response and device must match the binding and remain valid until retained.
    /// The `ir` pointer in `params` must remain valid for the lifetime of the params.
    pub(crate) unsafe fn from_ffi_unchecked(
        params: audionimbus_sys::IPLReflectionEffectParams,
        source: audionimbus_sys::IPLSource,
        binding: B,
    ) -> Self {
        let device = if params.tanDevice.is_null() {
            None
        } else {
            Some(TrueAudioNextDevice(
                // SAFETY: Safety invariants upheld by the caller.
                unsafe { audionimbus_sys::iplTrueAudioNextDeviceRetain(params.tanDevice) },
            ))
        };

        // SAFETY: Safety invariants upheld by the caller.
        let source = unsafe { audionimbus_sys::iplSourceRetain(source) };

        let num_channels = params.numChannels as u32;
        let impulse_response_size = params.irSize as u32;

        Self {
            impulse_response: ReflectionEffectIR(params.ir),
            reverb_times: params.reverbTimes,
            equalizer: Equalizer(params.eq),
            delay: params.delay as u32,
            num_channels,
            impulse_response_size,
            max_num_channels: num_channels,
            max_impulse_response_size: impulse_response_size,
            true_audio_next_device: device,
            true_audio_next_slot: params.tanSlot as u32,
            _source: source,
            binding,
        }
    }
}

#[cfg(feature = "dynamic")]
impl DynamicReflectionEffectParams {
    /// Returns the selected algorithm.
    pub const fn algorithm(&self) -> ReflectionAlgorithm {
        self.binding.0
    }
}

unsafe impl<B: ReflectionBinding> Send for ReflectionEffectParams<B> {}

impl<B: ReflectionBinding> Drop for ReflectionEffectParams<B> {
    fn drop(&mut self) {
        if !self._source.is_null() {
            unsafe { audionimbus_sys::iplSourceRelease(&mut self._source) };
        }
    }
}

impl<B: ReflectionBinding> ReflectionEffectParams<B> {
    /// Returns native parameters while borrowing their retained resources.
    ///
    /// Runtime routing checks must precede native use of these parameters.
    pub(crate) fn as_ffi(
        &self,
    ) -> FFIWrapper<'_, audionimbus_sys::IPLReflectionEffectParams, Self> {
        let device_ptr = self
            .true_audio_next_device
            .as_ref()
            .map(|d| d.raw_ptr())
            .unwrap_or(std::ptr::null_mut());

        let reflection_effect_params = audionimbus_sys::IPLReflectionEffectParams {
            type_: self.binding.ffi_type(),
            ir: self.impulse_response.0,
            reverbTimes: self.reverb_times,
            eq: *self.equalizer,
            delay: self.delay as i32,
            numChannels: self.num_channels as i32,
            irSize: self.impulse_response_size as i32,
            tanDevice: device_ptr,
            tanSlot: self.true_audio_next_slot as i32,
        };

        FFIWrapper::new(reflection_effect_params)
    }
}

/// Errors from runtime reflection creation and processing.
#[cfg(feature = "dynamic")]
#[derive(Debug, PartialEq, Eq)]
pub enum ReflectionError {
    /// Native creation failed.
    SteamAudio(SteamAudioError),
    /// An audio buffer has an invalid channel count.
    Effect(EffectError),
    /// Related reflection handles use different algorithms.
    AlgorithmMismatch {
        /// The algorithm required by the receiving handle.
        expected: ReflectionAlgorithm,
        /// The algorithm supplied by the related handle.
        actual: ReflectionAlgorithm,
    },
    /// The selected algorithm requires a mixer.
    DirectApplyUnsupported,
    /// The selected algorithm does not support a mixer.
    MixerUnsupported,
}

#[cfg(feature = "dynamic")]
impl fmt::Display for ReflectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SteamAudio(error) => error.fmt(f),
            Self::Effect(error) => error.fmt(f),
            Self::AlgorithmMismatch { expected, actual } => {
                write!(
                    f,
                    "reflection algorithm mismatch: expected {expected:?}, got {actual:?}"
                )
            }
            Self::DirectApplyUnsupported => write!(f, "reflection algorithm requires a mixer"),
            Self::MixerUnsupported => write!(f, "reflection algorithm does not support a mixer"),
        }
    }
}

#[cfg(feature = "dynamic")]
impl std::error::Error for ReflectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SteamAudio(error) => Some(error),
            Self::Effect(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(all(doctest, feature = "dynamic"))]
/// Reflection binding checks.
///
/// Static and runtime parameters remain separate:
///
/// ```compile_fail
/// use audionimbus::{AudioBufferRef, AudioBufferMut, Convolution, ReflectionEffectParams};
/// use audionimbus::dynamic::ReflectionEffect;
/// fn apply(effect: &mut ReflectionEffect, params: &ReflectionEffectParams<Convolution>,
///          input: &AudioBufferRef<'_>, output: &mut AudioBufferMut<'_>) {
///     effect.apply(params, input, output);
/// }
/// ```
///
/// ```compile_fail
/// use audionimbus::{AudioBufferRef, AudioBufferMut, Convolution, ReflectionEffect};
/// use audionimbus::dynamic::ReflectionEffectParams;
/// fn apply(effect: &mut ReflectionEffect<Convolution>, params: &ReflectionEffectParams,
///          input: &AudioBufferRef<'_>, output: &mut AudioBufferMut<'_>) {
///     effect.apply(params, input, output);
/// }
/// ```
///
/// Runtime algorithms do not satisfy static capability bounds:
///
/// ```compile_fail
/// use audionimbus::ReflectionEffect;
/// use audionimbus::dynamic::ReflectionAlgorithm;
/// let _: Option<ReflectionEffect<ReflectionAlgorithm>> = None;
/// ```
///
/// Static TAN effects retain their mixer requirement:
///
/// ```compile_fail
/// use audionimbus::{AudioBufferRef, AudioBufferMut, ReflectionEffect, ReflectionEffectParams, TrueAudioNext};
/// fn apply(effect: &mut ReflectionEffect<TrueAudioNext>, params: &ReflectionEffectParams<TrueAudioNext>,
///          input: &AudioBufferRef<'_>, output: &mut AudioBufferMut<'_>) {
///     effect.apply(params, input, output);
/// }
/// ```
///
/// Runtime effects do not expose raw-pointer mutation:
///
/// ```compile_fail
/// use audionimbus::dynamic::ReflectionEffect;
/// fn replace(effect: &mut ReflectionEffect) {
///     effect.raw_ptr_mut();
/// }
/// ```
/// ```compile_fail
/// use audionimbus::{AudioBufferMut, ReflectionEffect, TrueAudioNext};
/// fn tail(effect: &ReflectionEffect<TrueAudioNext>, output: &mut AudioBufferMut<'_>) {
///     effect.tail(output);
/// }
/// ```
///
/// Static output parameters cannot be converted into runtime outputs:
///
/// ```compile_fail
/// use audionimbus::{Convolution, ReflectionEffectParams};
/// fn convert(params: ReflectionEffectParams<Convolution>) -> audionimbus::dynamic::ReflectionEffectParams {
///     params.into()
/// }
/// ```
///
/// Runtime output parameters cannot be converted into static outputs:
///
/// ```compile_fail
/// use audionimbus::{Convolution, ReflectionEffectParams};
/// fn convert(params: audionimbus::dynamic::ReflectionEffectParams) -> ReflectionEffectParams<Convolution> {
///     params.into()
/// }
/// ```
///
/// Static and runtime mixers remain separate:
///
/// ```compile_fail
/// use audionimbus::{AudioBufferRef, AudioBufferMut, Convolution, ReflectionMixer};
/// use audionimbus::dynamic::{ReflectionEffect, ReflectionEffectParams};
/// fn apply(effect: &mut ReflectionEffect, params: &ReflectionEffectParams, mixer: &ReflectionMixer<Convolution>,
///          input: &AudioBufferRef<'_>, output: &mut AudioBufferMut<'_>) {
///     effect.apply_into_mixer(params, input, output, mixer);
/// }
/// ```
///
/// ```compile_fail
/// use audionimbus::{AudioBufferRef, AudioBufferMut, Convolution, ReflectionEffect, ReflectionEffectParams};
/// use audionimbus::dynamic::ReflectionMixer;
/// fn apply(effect: &mut ReflectionEffect<Convolution>, params: &ReflectionEffectParams<Convolution>, mixer: &ReflectionMixer,
///          input: &AudioBufferRef<'_>, output: &mut AudioBufferMut<'_>) {
///     effect.apply_into_mixer(params, input, output, mixer);
/// }
/// ```
struct ReflectionBindings;

#[cfg(test)]
mod tests {
    use crate::*;

    mod reflection_effect {
        use super::*;

        mod apply {
            use super::*;

            #[test]
            fn test_valid() {
                let context = Context::default();

                let audio_settings = AudioSettings::default();
                let impulse_response_size = 2 * audio_settings.sampling_rate;

                let simulation_settings = SimulationSettings::new(&audio_settings)
                    .with_reflections(ConvolutionSettings {
                        max_num_rays: 4096,
                        num_diffuse_samples: 32,
                        max_duration: 2.0,
                        max_num_sources: 8,
                        num_threads: 1,
                        max_order: 1,
                    });
                let mut simulator = Simulator::try_new(&context, &simulation_settings).unwrap();

                let scene = Scene::try_new(&context).unwrap();
                simulator.set_scene(&scene);

                let source = Source::try_new(&simulator).unwrap();
                simulator.add_source(&source);

                let simulation_shared_inputs = SimulationSharedInputs::new(
                    CoordinateSystem::default(),
                )
                .with_reflections(ReflectionsSharedInputs {
                    num_rays: 4096,
                    num_bounces: 16,
                    duration: 2.0,
                    order: 1,
                    irradiance_min_distance: 1.0,
                });
                simulator
                    .set_shared_reflections_inputs(&simulation_shared_inputs)
                    .unwrap();

                simulator.commit();

                assert!(simulator.run_reflections().is_ok());
                let reflection_effect_params = source.get_reflections_outputs().unwrap();

                let num_output_channels = num_ambisonics_channels(1);
                let reflection_effect_settings = ReflectionEffectSettings {
                    impulse_response_size,
                    num_channels: num_output_channels,
                };
                let mut reflection_effect = ReflectionEffect::<Convolution>::try_new(
                    &context,
                    &audio_settings,
                    &reflection_effect_settings,
                )
                .unwrap();

                let input_container = vec![0.5; audio_settings.frame_size as usize];
                let input_buffer = AudioBufferRef::try_from(input_container.as_slice()).unwrap();

                let mut output_container =
                    vec![0.0; num_output_channels as usize * input_buffer.num_samples()];
                let mut output_buffer =
                    AudioBufferMut::try_new(output_container.as_mut_slice(), 4 as usize).unwrap();

                assert!(
                    reflection_effect
                        .apply(&reflection_effect_params, &input_buffer, &mut output_buffer)
                        .is_ok()
                );
            }

            #[test]
            fn test_invalid_input_num_channels() {
                let context = Context::default();

                let audio_settings = AudioSettings::default();
                let impulse_response_size = 2 * audio_settings.sampling_rate;

                let simulation_settings = SimulationSettings::new(&audio_settings)
                    .with_reflections(ConvolutionSettings {
                        max_num_rays: 4096,
                        num_diffuse_samples: 32,
                        max_duration: 2.0,
                        max_num_sources: 8,
                        num_threads: 1,
                        max_order: 1,
                    });
                let mut simulator = Simulator::try_new(&context, &simulation_settings).unwrap();

                let scene = Scene::try_new(&context).unwrap();
                simulator.set_scene(&scene);

                let source = Source::try_new(&simulator).unwrap();
                simulator.add_source(&source);

                let simulation_shared_inputs = SimulationSharedInputs::new(
                    CoordinateSystem::default(),
                )
                .with_reflections(ReflectionsSharedInputs {
                    num_rays: 4096,
                    num_bounces: 16,
                    duration: 2.0,
                    order: 1,
                    irradiance_min_distance: 1.0,
                });
                simulator
                    .set_shared_reflections_inputs(&simulation_shared_inputs)
                    .unwrap();

                simulator.commit();

                assert!(simulator.run_reflections().is_ok());
                let reflection_effect_params = source.get_reflections_outputs().unwrap();

                let num_output_channels = num_ambisonics_channels(1);
                let reflection_effect_settings = ReflectionEffectSettings {
                    impulse_response_size,
                    num_channels: num_output_channels,
                };
                let mut reflection_effect = ReflectionEffect::<Convolution>::try_new(
                    &context,
                    &audio_settings,
                    &reflection_effect_settings,
                )
                .unwrap();

                let input_container = vec![0.5; 2 * audio_settings.frame_size as usize];
                let input_buffer =
                    AudioBufferRef::try_new(input_container.as_slice(), 2 as usize).unwrap();

                let mut output_container =
                    vec![0.0; num_output_channels as usize * input_buffer.num_samples()];
                let mut output_buffer =
                    AudioBufferMut::try_new(output_container.as_mut_slice(), 4 as usize).unwrap();

                assert_eq!(
                    reflection_effect.apply(
                        &reflection_effect_params,
                        &input_buffer,
                        &mut output_buffer
                    ),
                    Err(EffectError::InvalidInputChannels {
                        expected: ChannelRequirement::Exactly(1),
                        actual: 2
                    })
                );
            }

            #[test]
            fn test_invalid_output_num_channels() {
                let context = Context::default();

                let audio_settings = AudioSettings::default();
                let impulse_response_size = 2 * audio_settings.sampling_rate;

                let simulation_settings = SimulationSettings::new(&audio_settings)
                    .with_reflections(ConvolutionSettings {
                        max_num_rays: 4096,
                        num_diffuse_samples: 32,
                        max_duration: 2.0,
                        max_num_sources: 8,
                        num_threads: 1,
                        max_order: 1,
                    });
                let mut simulator = Simulator::try_new(&context, &simulation_settings).unwrap();

                let scene = Scene::try_new(&context).unwrap();
                simulator.set_scene(&scene);

                let source = Source::try_new(&simulator).unwrap();
                simulator.add_source(&source);

                let simulation_shared_inputs = SimulationSharedInputs::new(
                    CoordinateSystem::default(),
                )
                .with_reflections(ReflectionsSharedInputs {
                    num_rays: 4096,
                    num_bounces: 16,
                    duration: 2.0,
                    order: 1,
                    irradiance_min_distance: 1.0,
                });
                simulator
                    .set_shared_reflections_inputs(&simulation_shared_inputs)
                    .unwrap();

                simulator.commit();

                assert!(simulator.run_reflections().is_ok());
                let reflection_effect_params = source.get_reflections_outputs().unwrap();

                let num_output_channels = num_ambisonics_channels(1);
                let reflection_effect_settings = ReflectionEffectSettings {
                    impulse_response_size,
                    num_channels: num_output_channels,
                };
                let mut reflection_effect = ReflectionEffect::<Convolution>::try_new(
                    &context,
                    &audio_settings,
                    &reflection_effect_settings,
                )
                .unwrap();

                let input_container = vec![0.5; audio_settings.frame_size as usize];
                let input_buffer = AudioBufferRef::try_from(input_container.as_slice()).unwrap();

                let mut output_container = vec![0.0; (2 * input_buffer.num_samples()) as usize];
                let mut output_buffer =
                    AudioBufferMut::try_new(output_container.as_mut_slice(), 2 as usize).unwrap();

                assert_eq!(
                    reflection_effect.apply(
                        &reflection_effect_params,
                        &input_buffer,
                        &mut output_buffer
                    ),
                    Err(EffectError::InvalidOutputChannels {
                        expected: ChannelRequirement::Exactly(4),
                        actual: 2
                    })
                );
            }
        }

        mod apply_into_mixer {
            use super::*;

            #[test]
            fn test_valid() {
                let context = Context::default();

                let audio_settings = AudioSettings::default();
                let impulse_response_size = 2 * audio_settings.sampling_rate;

                let simulation_settings = SimulationSettings::new(&audio_settings)
                    .with_reflections(ConvolutionSettings {
                        max_num_rays: 4096,
                        num_diffuse_samples: 32,
                        max_duration: 2.0,
                        max_num_sources: 8,
                        num_threads: 1,
                        max_order: 1,
                    });
                let mut simulator = Simulator::try_new(&context, &simulation_settings).unwrap();

                let scene = Scene::try_new(&context).unwrap();
                simulator.set_scene(&scene);

                let source = Source::try_new(&simulator).unwrap();
                simulator.add_source(&source);

                let simulation_shared_inputs = SimulationSharedInputs::new(
                    CoordinateSystem::default(),
                )
                .with_reflections(ReflectionsSharedInputs {
                    num_rays: 4096,
                    num_bounces: 16,
                    duration: 2.0,
                    order: 1,
                    irradiance_min_distance: 1.0,
                });
                simulator
                    .set_shared_reflections_inputs(&simulation_shared_inputs)
                    .unwrap();

                simulator.commit();

                assert!(simulator.run_reflections().is_ok());
                let reflection_effect_params = source.get_reflections_outputs().unwrap();

                let num_output_channels = num_ambisonics_channels(1);
                let reflection_effect_settings = ReflectionEffectSettings {
                    impulse_response_size,
                    num_channels: num_output_channels,
                };
                let mut reflection_effect = ReflectionEffect::<Convolution>::try_new(
                    &context,
                    &audio_settings,
                    &reflection_effect_settings,
                )
                .unwrap();

                let input_container = vec![0.5; audio_settings.frame_size as usize];
                let input_buffer = AudioBufferRef::try_from(input_container.as_slice()).unwrap();

                let mut output_container =
                    vec![0.0; num_output_channels as usize * input_buffer.num_samples()];
                let mut output_buffer =
                    AudioBufferMut::try_new(output_container.as_mut_slice(), 4 as usize).unwrap();

                let mixer = ReflectionMixer::try_new(
                    &context,
                    &audio_settings,
                    &reflection_effect_settings,
                )
                .unwrap();

                assert!(
                    reflection_effect
                        .apply_into_mixer(
                            &reflection_effect_params,
                            &input_buffer,
                            &mut output_buffer,
                            &mixer
                        )
                        .is_ok()
                );
            }

            #[test]
            fn test_invalid_input_num_channels() {
                let context = Context::default();

                let audio_settings = AudioSettings::default();
                let impulse_response_size = 2 * audio_settings.sampling_rate;

                let simulation_settings = SimulationSettings::new(&audio_settings)
                    .with_reflections(ConvolutionSettings {
                        max_num_rays: 4096,
                        num_diffuse_samples: 32,
                        max_duration: 2.0,
                        max_num_sources: 8,
                        num_threads: 1,
                        max_order: 1,
                    });
                let mut simulator = Simulator::try_new(&context, &simulation_settings).unwrap();

                let scene = Scene::try_new(&context).unwrap();
                simulator.set_scene(&scene);

                let source = Source::try_new(&simulator).unwrap();
                simulator.add_source(&source);

                let simulation_shared_inputs = SimulationSharedInputs::new(
                    CoordinateSystem::default(),
                )
                .with_reflections(ReflectionsSharedInputs {
                    num_rays: 4096,
                    num_bounces: 16,
                    duration: 2.0,
                    order: 1,
                    irradiance_min_distance: 1.0,
                });
                simulator
                    .set_shared_reflections_inputs(&simulation_shared_inputs)
                    .unwrap();

                simulator.commit();

                assert!(simulator.run_reflections().is_ok());
                let reflection_effect_params = source.get_reflections_outputs().unwrap();

                let num_output_channels = num_ambisonics_channels(1);
                let reflection_effect_settings = ReflectionEffectSettings {
                    impulse_response_size,
                    num_channels: num_output_channels,
                };
                let mut reflection_effect = ReflectionEffect::<Convolution>::try_new(
                    &context,
                    &audio_settings,
                    &reflection_effect_settings,
                )
                .unwrap();

                let input_container = vec![0.5; 2 * audio_settings.frame_size as usize];
                let input_buffer =
                    AudioBufferRef::try_new(input_container.as_slice(), 2 as usize).unwrap();

                let mut output_container =
                    vec![0.0; num_output_channels as usize * input_buffer.num_samples()];
                let mut output_buffer =
                    AudioBufferMut::try_new(output_container.as_mut_slice(), 4 as usize).unwrap();

                let mixer = ReflectionMixer::try_new(
                    &context,
                    &audio_settings,
                    &reflection_effect_settings,
                )
                .unwrap();

                assert_eq!(
                    reflection_effect.apply_into_mixer(
                        &reflection_effect_params,
                        &input_buffer,
                        &mut output_buffer,
                        &mixer
                    ),
                    Err(EffectError::InvalidInputChannels {
                        expected: ChannelRequirement::Exactly(1),
                        actual: 2
                    })
                );
            }

            #[test]
            fn test_invalid_output_num_channels() {
                let context = Context::default();

                let audio_settings = AudioSettings::default();
                let impulse_response_size = 2 * audio_settings.sampling_rate;

                let simulation_settings = SimulationSettings::new(&audio_settings)
                    .with_reflections(ConvolutionSettings {
                        max_num_rays: 4096,
                        num_diffuse_samples: 32,
                        max_duration: 2.0,
                        max_num_sources: 8,
                        num_threads: 1,
                        max_order: 1,
                    });
                let mut simulator = Simulator::try_new(&context, &simulation_settings).unwrap();

                let scene = Scene::try_new(&context).unwrap();
                simulator.set_scene(&scene);

                let source = Source::try_new(&simulator).unwrap();
                simulator.add_source(&source);

                let simulation_shared_inputs = SimulationSharedInputs::new(
                    CoordinateSystem::default(),
                )
                .with_reflections(ReflectionsSharedInputs {
                    num_rays: 4096,
                    num_bounces: 16,
                    duration: 2.0,
                    order: 1,
                    irradiance_min_distance: 1.0,
                });
                simulator
                    .set_shared_reflections_inputs(&simulation_shared_inputs)
                    .unwrap();

                simulator.commit();

                assert!(simulator.run_reflections().is_ok());
                let reflection_effect_params = source.get_reflections_outputs().unwrap();

                let num_output_channels = num_ambisonics_channels(1);
                let reflection_effect_settings = ReflectionEffectSettings {
                    impulse_response_size,
                    num_channels: num_output_channels,
                };
                let mut reflection_effect = ReflectionEffect::<Convolution>::try_new(
                    &context,
                    &audio_settings,
                    &reflection_effect_settings,
                )
                .unwrap();

                let input_container = vec![0.5; audio_settings.frame_size as usize];
                let input_buffer = AudioBufferRef::try_from(input_container.as_slice()).unwrap();

                let mut output_container = vec![0.0; (2 * input_buffer.num_samples()) as usize];
                let mut output_buffer =
                    AudioBufferMut::try_new(output_container.as_mut_slice(), 2 as usize).unwrap();

                let mixer = ReflectionMixer::try_new(
                    &context,
                    &audio_settings,
                    &reflection_effect_settings,
                )
                .unwrap();

                assert_eq!(
                    reflection_effect.apply_into_mixer(
                        &reflection_effect_params,
                        &input_buffer,
                        &mut output_buffer,
                        &mixer
                    ),
                    Err(EffectError::InvalidOutputChannels {
                        expected: ChannelRequirement::Exactly(4),
                        actual: 2
                    })
                );
            }
        }

        mod tail {
            use super::*;

            #[test]
            fn test_valid() {
                let context = Context::default();

                let audio_settings = AudioSettings::default();
                let impulse_response_size = 2 * audio_settings.sampling_rate;

                let num_output_channels = num_ambisonics_channels(1);
                let reflection_effect_settings = ReflectionEffectSettings {
                    impulse_response_size,
                    num_channels: num_output_channels,
                };
                let reflection_effect = ReflectionEffect::<Convolution>::try_new(
                    &context,
                    &audio_settings,
                    &reflection_effect_settings,
                )
                .unwrap();

                let mut output_container =
                    vec![0.0; (num_output_channels * audio_settings.frame_size) as usize];
                let mut output_buffer = AudioBufferMut::try_new(
                    output_container.as_mut_slice(),
                    num_output_channels as usize,
                )
                .unwrap();

                assert!(reflection_effect.tail(&mut output_buffer).is_ok());
            }

            #[test]
            fn test_invalid_output_num_channels() {
                let context = Context::default();

                let audio_settings = AudioSettings::default();
                let impulse_response_size = 2 * audio_settings.sampling_rate;

                let num_output_channels = num_ambisonics_channels(1);
                let reflection_effect_settings = ReflectionEffectSettings {
                    impulse_response_size,
                    num_channels: num_output_channels,
                };
                let reflection_effect = ReflectionEffect::<Convolution>::try_new(
                    &context,
                    &audio_settings,
                    &reflection_effect_settings,
                )
                .unwrap();

                let mut output_container = vec![0.0; audio_settings.frame_size as usize];
                let mut output_buffer =
                    AudioBufferMut::try_from(output_container.as_mut_slice()).unwrap();

                assert_eq!(
                    reflection_effect.tail(&mut output_buffer),
                    Err(EffectError::InvalidOutputChannels {
                        expected: ChannelRequirement::Exactly(4),
                        actual: 1
                    })
                );
            }
        }

        mod tail_into_mixer {
            use super::*;

            #[test]
            fn test_valid() {
                let context = Context::default();

                let audio_settings = AudioSettings::default();
                let impulse_response_size = 2 * audio_settings.sampling_rate;

                let num_output_channels = num_ambisonics_channels(1);
                let reflection_effect_settings = ReflectionEffectSettings {
                    impulse_response_size,
                    num_channels: num_output_channels,
                };
                let reflection_effect = ReflectionEffect::<Convolution>::try_new(
                    &context,
                    &audio_settings,
                    &reflection_effect_settings,
                )
                .unwrap();

                let mut output_container =
                    vec![0.0; (num_output_channels * audio_settings.frame_size) as usize];
                let mut output_buffer = AudioBufferMut::try_new(
                    output_container.as_mut_slice(),
                    num_output_channels as usize,
                )
                .unwrap();

                let mixer = ReflectionMixer::try_new(
                    &context,
                    &audio_settings,
                    &reflection_effect_settings,
                )
                .unwrap();

                assert!(
                    reflection_effect
                        .tail_into_mixer(&mut output_buffer, &mixer)
                        .is_ok()
                );
            }

            #[test]
            fn test_invalid_output_num_channels() {
                let context = Context::default();

                let audio_settings = AudioSettings::default();
                let impulse_response_size = 2 * audio_settings.sampling_rate;

                let num_output_channels = num_ambisonics_channels(1);
                let reflection_effect_settings = ReflectionEffectSettings {
                    impulse_response_size,
                    num_channels: num_output_channels,
                };
                let reflection_effect = ReflectionEffect::<Convolution>::try_new(
                    &context,
                    &audio_settings,
                    &reflection_effect_settings,
                )
                .unwrap();

                let mut output_container = vec![0.0; audio_settings.frame_size as usize];
                let mut output_buffer =
                    AudioBufferMut::try_from(output_container.as_mut_slice()).unwrap();

                let mixer = ReflectionMixer::try_new(
                    &context,
                    &audio_settings,
                    &reflection_effect_settings,
                )
                .unwrap();

                assert_eq!(
                    reflection_effect.tail_into_mixer(&mut output_buffer, &mixer),
                    Err(EffectError::InvalidOutputChannels {
                        expected: ChannelRequirement::Exactly(4),
                        actual: 1
                    })
                );
            }
        }

        mod clone {
            use super::*;

            #[test]
            fn test_clone() {
                let context = Context::default();
                let audio_settings = AudioSettings::default();
                let impulse_response_size = 2 * audio_settings.sampling_rate;

                let num_output_channels = num_ambisonics_channels(1);
                let reflection_effect_settings = ReflectionEffectSettings {
                    impulse_response_size,
                    num_channels: num_output_channels,
                };

                let effect = ReflectionEffect::<Convolution>::try_new(
                    &context,
                    &audio_settings,
                    &reflection_effect_settings,
                )
                .unwrap();
                let clone = effect.clone();
                assert_eq!(effect.raw_ptr(), clone.raw_ptr());
                drop(effect);
                assert!(!clone.raw_ptr().is_null());
            }
        }
    }
}
