//! Room acoustics and reverberation effects.

use super::audio_effect_state::AudioEffectState;
use super::equalizer::Equalizer;
use super::error::{ImpulseResponseSizeExceedsMaxError, NumChannelsExceedsMaxError};
use super::EffectError;
use crate::audio_buffer::{AudioBuffer, Sample};
use crate::audio_settings::AudioSettings;
use crate::context::Context;
use crate::device::true_audio_next::TrueAudioNextDevice;
use crate::error::{to_option_error, SteamAudioError};
use crate::ffi_wrapper::FFIWrapper;
use crate::Sealed;
use crate::{ChannelPointers, ChannelRequirement};
use std::marker::PhantomData;

/// Multi-channel convolution reverb.
///
/// Reflections reaching the listener are encoded in an Impulse Response (IR), which is a filter that records each reflection as it arrives.
/// This algorithm renders reflections with the most detail, but may result in significant CPU usage.
///
/// Using a [`ReflectionMixer`] with this algorithm provides a reduction in CPU usage.
#[derive(Debug)]
pub struct Convolution;

/// Parametric (or artificial) reverb, using feedback delay networks.
///
/// The reflected sound field is reduced to a few numbers that describe how reflected energy decays over time.
/// This is then used to drive an approximate model of reverberation in an indoor space.
/// This algorithm results in lower CPU usage, but cannot render individual echoes, especially in outdoor spaces.
///
/// A reflection mixer cannot be used with this algorithm.
#[derive(Debug)]
pub struct Parametric;

/// A hybrid of convolution and parametric reverb.
///
/// The initial portion of the IR is rendered using convolution reverb, but the later part is used to estimate a parametric reverb.
/// The point in the IR where this transition occurs can be controlled.
/// This algorithm allows a trade-off between rendering quality and CPU usage.
///
/// A reflection mixer cannot be used with this algorithm.
#[derive(Debug)]
pub struct Hybrid;

/// Multi-channel convolution reverb, using AMD TrueAudio Next for GPU acceleration.
///
/// This algorithm is similar to [`Convolution`], but uses the GPU instead of the CPU for processing, allowing significantly more sources to be processed.
///
/// A [`ReflectionMixer`] must be used with this algorithm, because the GPU will process convolution reverb at a single point in your audio processing pipeline.
#[derive(Debug)]
pub struct TrueAudioNext;

impl Sealed for Convolution {}
impl Sealed for Parametric {}
impl Sealed for Hybrid {}
impl Sealed for TrueAudioNext {}

/// Marker trait for effects that can use `apply()`.
pub trait CanApplyDirectly: Sealed {}
impl CanApplyDirectly for Convolution {}
impl CanApplyDirectly for Parametric {}
impl CanApplyDirectly for Hybrid {}

/// Marker trait for effects that can use `apply_into_mixer() and `tail_into_mixer()`.
pub trait CanUseReflectionMixer: Sealed {}
impl CanUseReflectionMixer for Convolution {}
impl CanUseReflectionMixer for TrueAudioNext {}

/// Reflection effect type. Can be:
/// - [`Convolution`]: Multi-channel convolution reverb
/// - [`Parametric`]: Parametric (or artificial) reverb, using feedback delay networks
/// - [`Hybrid`]: A hybrid of convolution and parametric reverb
/// - [`TrueAudioNext`]: Multi-channel convolution reverb, using AMD TrueAudio Next for GPU acceleration
pub trait ReflectionEffectType: Sealed {
    /// Returns the FFI enum value for this reflection effect type.
    fn to_ffi_type() -> audionimbus_sys::IPLReflectionEffectType;

    /// Converts reflection effect settings to the FFI representation.
    fn ffi_settings(
        settings: &ReflectionEffectSettings,
    ) -> audionimbus_sys::IPLReflectionEffectSettings {
        audionimbus_sys::IPLReflectionEffectSettings {
            type_: Self::to_ffi_type(),
            irSize: settings.impulse_response_size as i32,
            numChannels: settings.num_channels as i32,
        }
    }

    /// Returns the number of output channels required for this effect type.
    fn num_output_channels(settings: &ReflectionEffectSettings) -> ChannelRequirement;
}

impl ReflectionEffectType for Convolution {
    fn to_ffi_type() -> audionimbus_sys::IPLReflectionEffectType {
        audionimbus_sys::IPLReflectionEffectType::IPL_REFLECTIONEFFECTTYPE_CONVOLUTION
    }

    fn num_output_channels(settings: &ReflectionEffectSettings) -> ChannelRequirement {
        ChannelRequirement::Exactly(settings.num_channels)
    }
}

impl ReflectionEffectType for Parametric {
    fn to_ffi_type() -> audionimbus_sys::IPLReflectionEffectType {
        audionimbus_sys::IPLReflectionEffectType::IPL_REFLECTIONEFFECTTYPE_PARAMETRIC
    }

    fn ffi_settings(
        settings: &ReflectionEffectSettings,
    ) -> audionimbus_sys::IPLReflectionEffectSettings {
        audionimbus_sys::IPLReflectionEffectSettings {
            type_: Self::to_ffi_type(),
            irSize: settings.impulse_response_size as i32,
            numChannels: settings.num_channels as i32,
        }
    }

    fn num_output_channels(_settings: &ReflectionEffectSettings) -> ChannelRequirement {
        ChannelRequirement::AtLeast(1)
    }
}

impl ReflectionEffectType for Hybrid {
    fn to_ffi_type() -> audionimbus_sys::IPLReflectionEffectType {
        audionimbus_sys::IPLReflectionEffectType::IPL_REFLECTIONEFFECTTYPE_HYBRID
    }

    fn num_output_channels(settings: &ReflectionEffectSettings) -> ChannelRequirement {
        ChannelRequirement::Exactly(settings.num_channels)
    }
}

impl ReflectionEffectType for TrueAudioNext {
    fn to_ffi_type() -> audionimbus_sys::IPLReflectionEffectType {
        audionimbus_sys::IPLReflectionEffectType::IPL_REFLECTIONEFFECTTYPE_TAN
    }

    fn num_output_channels(settings: &ReflectionEffectSettings) -> ChannelRequirement {
        ChannelRequirement::Exactly(settings.num_channels)
    }
}

#[cfg(doc)]
use super::AmbisonicsDecodeEffect;
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
/// 2. Adding [`Source`]s to the scene (don't forget to commit the changes using [`Simulator::commit`]!)
/// 3. Running the simulation ([`Simulator::run_reflections`]) and retrieving the output for the source ([`Source::get_outputs`])
/// 4. Applying the reflection effect to the audio buffer (or to a [`ReflectionMixer`] for better performance with supported reflection algorithms) using the simulation output ([`SimulationOutputs::reflections`]) as params
///
/// ```
/// # use audionimbus::*;
/// let context = Context::default();
///
/// const SAMPLING_RATE: u32 = 48_000;
/// const FRAME_SIZE: u32 = 1024;
/// let audio_settings = AudioSettings {
///     sampling_rate: SAMPLING_RATE,
///     frame_size: FRAME_SIZE,
/// };
///
/// // Create a simulator with reflections.
/// let simulation_settings = SimulationSettings::new(SAMPLING_RATE, FRAME_SIZE, 1)
///     .with_reflections(ReflectionsSimulationSettings::Convolution {
///         max_num_rays: 4096,
///         num_diffuse_samples: 32,
///         max_duration: 2.0,
///         max_num_sources: 8,
///         num_threads: 2,
///     });
/// let mut simulator = Simulator::try_new(&context, &simulation_settings)?;
///
/// let scene = Scene::try_new(&context)?;
/// simulator.set_scene(&scene);
///
/// let mut source = Source::try_new(&simulator)?;
///
/// source.set_reflections_inputs(
///     &SimulationInputs::new(CoordinateSystem::default()).with_reflections(
///         ReflectionsSimulationParameters::Convolution {
///             baked_data_identifier: None,
///         },
///     ),
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
/// let params = source.get_reflection_outputs()?;
///
/// const NUM_CHANNELS: u32 = num_ambisonics_channels(1); // 1st order ambisonics
/// let mut effect = ReflectionEffect::<Convolution>::try_new(
///     &context,
///     &audio_settings,
///     &ReflectionEffectSettings {
///         impulse_response_size: 2 * SAMPLING_RATE, // 2 seconds
///         num_channels: NUM_CHANNELS,
///     },
/// )?;
///
/// let input = vec![0.5; FRAME_SIZE as usize];
/// let input_buffer = AudioBuffer::try_with_data(&input)?;
/// let mut output = vec![0.0; (NUM_CHANNELS * FRAME_SIZE) as usize]; // 4 channels
/// let output_buffer = AudioBuffer::try_with_data_and_settings(
///     &mut output,
///     AudioBufferSettings::with_num_channels(NUM_CHANNELS),
/// )?;
///
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
/// # use audionimbus::*;
/// let context = Context::default();
/// const SAMPLING_RATE: u32 = 48_000;
/// const FRAME_SIZE: u32 = 1024;
/// let audio_settings = AudioSettings {
///     sampling_rate: SAMPLING_RATE,
///     frame_size: FRAME_SIZE,
/// };
///
/// // Create simulator with reflections
/// let simulation_settings = SimulationSettings::new(SAMPLING_RATE, FRAME_SIZE, 1)
///     .with_reflections(ReflectionsSimulationSettings::Convolution {
///         max_num_rays: 2048,
///         num_diffuse_samples: 32,
///         max_duration: 2.0,
///         max_num_sources: 8,
///         num_threads: 2,
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
///     &SimulationInputs::new(listener_position) // Source at listener = reverb
///         .with_reflections(ReflectionsSimulationParameters::Convolution {
///             baked_data_identifier: None,
///         }),
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
/// let reverb_params = reverb_source.get_reflection_outputs()?;
///
/// const NUM_CHANNELS: u32 = num_ambisonics_channels(1); // 1st order ambisonics
/// let mut reverb_effect = ReflectionEffect::<Convolution>::try_new(
///     &context,
///     &audio_settings,
///     &ReflectionEffectSettings {
///         impulse_response_size: 2 * SAMPLING_RATE,
///         num_channels: NUM_CHANNELS,
///     },
/// )?;
///
/// let input = vec![0.5; FRAME_SIZE as usize];
/// let input_buffer = AudioBuffer::try_with_data(&input)?;
/// let mut reverb_output = vec![0.0; (NUM_CHANNELS * FRAME_SIZE) as usize];
/// let output_buffer = AudioBuffer::try_with_data_and_settings(
///     &mut reverb_output,
///     AudioBufferSettings::with_num_channels(NUM_CHANNELS),
/// )?;
///
/// let _ = reverb_effect.apply(&reverb_params, &input_buffer, &output_buffer);
///
/// // Mix with dry signal (e.g., 70% dry, 30% reverb)
/// // Then decode the ambisonics output for final playback
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug)]
pub struct ReflectionEffect<T: ReflectionEffectType> {
    inner: audionimbus_sys::IPLReflectionEffect,

    /// Number of output channels needed, i.e. as many channels as the impulse response specified
    /// when creating the effect.
    num_output_channels: ChannelRequirement,

    _marker: PhantomData<T>,
}

impl<T: ReflectionEffectType> ReflectionEffect<T> {
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
        let mut inner = std::ptr::null_mut();

        let status = unsafe {
            audionimbus_sys::iplReflectionEffectCreate(
                context.raw_ptr(),
                &mut audionimbus_sys::IPLAudioSettings::from(audio_settings),
                &mut T::ffi_settings(reflection_effect_settings),
                &raw mut inner,
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        let num_output_channels = T::num_output_channels(reflection_effect_settings);

        let reflection_effect = Self {
            inner,
            num_output_channels,
            _marker: PhantomData,
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

    /// Returns a mutable reference to the raw FFI pointer.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub const fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLReflectionEffect {
        &mut self.inner
    }
}

impl<T: ReflectionEffectType + CanApplyDirectly> ReflectionEffect<T> {
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
    pub fn apply<I, O, PI: ChannelPointers, PO: ChannelPointers>(
        &mut self,
        reflection_effect_params: &ReflectionEffectParams<T>,
        input_buffer: &AudioBuffer<I, PI>,
        output_buffer: &AudioBuffer<O, PO>,
    ) -> Result<AudioEffectState, EffectError>
    where
        I: AsRef<[Sample]>,
        O: AsRef<[Sample]> + AsMut<[Sample]>,
    {
        let num_input_channels = input_buffer.num_channels();
        if num_input_channels != 1 {
            return Err(EffectError::InvalidInputChannels {
                expected: ChannelRequirement::Exactly(1),
                actual: num_input_channels,
            });
        }

        let num_output_channels = output_buffer.num_channels();
        if !self
            .num_output_channels
            .is_satisfied_by(num_output_channels)
        {
            return Err(EffectError::InvalidOutputChannels {
                expected: self.num_output_channels,
                actual: num_output_channels,
            });
        }

        let state = unsafe {
            audionimbus_sys::iplReflectionEffectApply(
                self.raw_ptr(),
                &raw mut *reflection_effect_params.as_ffi(),
                &raw mut *input_buffer.as_ffi(),
                &raw mut *output_buffer.as_ffi(),
                std::ptr::null_mut(),
            )
        }
        .into();

        Ok(state)
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
    pub fn tail<O>(&self, output_buffer: &AudioBuffer<O>) -> Result<AudioEffectState, EffectError>
    where
        O: AsRef<[Sample]> + AsMut<[Sample]>,
    {
        let num_output_channels = output_buffer.num_channels();
        if !self
            .num_output_channels
            .is_satisfied_by(num_output_channels)
        {
            return Err(EffectError::InvalidOutputChannels {
                expected: self.num_output_channels,
                actual: num_output_channels,
            });
        }

        let state = unsafe {
            audionimbus_sys::iplReflectionEffectGetTail(
                self.raw_ptr(),
                &raw mut *output_buffer.as_ffi(),
                std::ptr::null_mut(),
            )
        }
        .into();

        Ok(state)
    }
}

impl<T: ReflectionEffectType + CanUseReflectionMixer> ReflectionEffect<T> {
    /// Applies a reflection effect to an audio buffer.
    ///
    /// The output of this effect will be mixed into the given mixer.
    ///
    /// The mixed output can be retrieved elsewhere in the audio pipeline using [`ReflectionMixer::apply`].
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
    pub fn apply_into_mixer<I, O, PI: ChannelPointers, PO: ChannelPointers>(
        &mut self,
        reflection_effect_params: &ReflectionEffectParams<T>,
        input_buffer: &AudioBuffer<I, PI>,
        output_buffer: &AudioBuffer<O, PO>,
        mixer: &ReflectionMixer<T>,
    ) -> Result<AudioEffectState, EffectError>
    where
        I: AsRef<[Sample]>,
        O: AsRef<[Sample]> + AsMut<[Sample]>,
    {
        let num_input_channels = input_buffer.num_channels();
        if num_input_channels != 1 {
            return Err(EffectError::InvalidInputChannels {
                expected: ChannelRequirement::Exactly(1),
                actual: num_input_channels,
            });
        }

        let num_output_channels = output_buffer.num_channels();
        if !self
            .num_output_channels
            .is_satisfied_by(num_output_channels)
        {
            return Err(EffectError::InvalidOutputChannels {
                expected: self.num_output_channels,
                actual: num_output_channels,
            });
        }

        let state = unsafe {
            audionimbus_sys::iplReflectionEffectApply(
                self.raw_ptr(),
                &mut *reflection_effect_params.as_ffi(),
                &mut *input_buffer.as_ffi(),
                &mut *output_buffer.as_ffi(),
                mixer.raw_ptr(),
            )
        }
        .into();

        Ok(state)
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
    /// The output audio buffer must have as many channels as the impulse response specified when
    /// creating the effect (for convolution, hybrid, and TrueAudioNext) or at least one
    /// channel (for parametric).
    ///
    /// # Errors
    ///
    /// Returns [`EffectError`] if the output audio buffer does not have as many channels as the impulse response specified
    /// when creating the effect (for convolution, hybrid, and TrueAudioNext) or at lea one channel (for parametric).
    pub fn tail_into_mixer<O>(
        &self,
        output_buffer: &AudioBuffer<O>,
        mixer: &ReflectionMixer<T>,
    ) -> Result<AudioEffectState, EffectError>
    where
        O: AsRef<[Sample]> + AsMut<[Sample]>,
    {
        let num_output_channels = output_buffer.num_channels();
        if !self
            .num_output_channels
            .is_satisfied_by(num_output_channels)
        {
            return Err(EffectError::InvalidOutputChannels {
                expected: self.num_output_channels,
                actual: num_output_channels,
            });
        }

        let state = unsafe {
            audionimbus_sys::iplReflectionEffectGetTail(
                self.raw_ptr(),
                &raw mut *output_buffer.as_ffi(),
                mixer.raw_ptr(),
            )
        }
        .into();

        Ok(state)
    }
}

impl<T: ReflectionEffectType> Drop for ReflectionEffect<T> {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplReflectionEffectRelease(&raw mut self.inner) }
    }
}

unsafe impl<T: ReflectionEffectType> Send for ReflectionEffect<T> {}
unsafe impl<T: ReflectionEffectType> Sync for ReflectionEffect<T> {}

impl<T: ReflectionEffectType> Clone for ReflectionEffect<T> {
    /// Retains an additional reference to the reflection effect.
    ///
    /// The returned [`ReflectionEffect`] shares the same underlying Steam Audio object.
    fn clone(&self) -> Self {
        // SAFETY: The reflection effect will not be destroyed until all references are released.
        Self {
            inner: unsafe { audionimbus_sys::iplReflectionEffectRetain(self.inner) },
            num_output_channels: self.num_output_channels,
            _marker: PhantomData,
        }
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

/// Parameters for applying a reflection effect to an audio buffer.
#[derive(Debug, PartialEq)]
pub struct ReflectionEffectParams<T: ReflectionEffectType> {
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

    _marker: PhantomData<T>,
}

impl ReflectionEffectParams<Convolution> {
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
            _marker: PhantomData,
        }
    }
}

impl ReflectionEffectParams<Parametric> {
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
            _marker: PhantomData,
        }
    }
}

impl ReflectionEffectParams<Hybrid> {
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
            _marker: PhantomData,
        }
    }
}

impl ReflectionEffectParams<TrueAudioNext> {
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
            _marker: PhantomData,
        }
    }
}

impl<T: ReflectionEffectType> ReflectionEffectParams<T> {
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
    /// # Safety
    ///
    /// The `ir` pointer in `params` must remain valid for the lifetime of the params.
    pub(crate) unsafe fn from_ffi_unchecked(
        params: audionimbus_sys::IPLReflectionEffectParams,
        source: audionimbus_sys::IPLSource,
    ) -> Self {
        let device = if params.tanDevice.is_null() {
            None
        } else {
            Some(TrueAudioNextDevice(
                audionimbus_sys::iplTrueAudioNextDeviceRetain(params.tanDevice),
            ))
        };

        let source = audionimbus_sys::iplSourceRetain(source);

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
            _marker: PhantomData,
        }
    }
}

unsafe impl<T: ReflectionEffectType> Send for ReflectionEffectParams<T> {}

impl<T: ReflectionEffectType> Drop for ReflectionEffectParams<T> {
    fn drop(&mut self) {
        if !self._source.is_null() {
            unsafe { audionimbus_sys::iplSourceRelease(&mut self._source) };
        }
    }
}

/// The impulse response of [`ReflectionEffectParams`].
#[derive(Debug, Eq, PartialEq)]
pub struct ReflectionEffectIR(pub audionimbus_sys::IPLReflectionEffectIR);

unsafe impl Send for ReflectionEffectIR {}

impl<T: ReflectionEffectType> ReflectionEffectParams<T> {
    pub(crate) fn as_ffi(
        &self,
    ) -> FFIWrapper<'_, audionimbus_sys::IPLReflectionEffectParams, Self> {
        let device_ptr = self
            .true_audio_next_device
            .as_ref()
            .map(|d| d.raw_ptr())
            .unwrap_or(std::ptr::null_mut());

        let reflection_effect_params = audionimbus_sys::IPLReflectionEffectParams {
            type_: T::to_ffi_type(),
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

/// Mixes the outputs of multiple reflection effects, and generates a single sound field containing all the reflected sound reaching the listener.
///
/// Using this is optional. Depending on the reflection effect algorithm used, a reflection mixer may provide a reduction in CPU usage.
///
/// `ReflectionMixer` is a reference-counted handle to an underlying Steam Audio object.
/// Cloning it is cheap; it produces a new handle pointing to the same underlying object, while
/// incrementing a reference count.
/// The underlying object is destroyed when all handles are dropped.
#[derive(Debug)]
pub struct ReflectionMixer<T: ReflectionEffectType> {
    inner: audionimbus_sys::IPLReflectionMixer,

    /// Number of output channels required.
    num_output_channels: ChannelRequirement,

    _marker: PhantomData<T>,
}

impl<T: ReflectionEffectType> ReflectionMixer<T> {
    /// Creates a new reflection mixer and returns a handle to it.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if mixer creation fails.
    pub fn try_new(
        context: &Context,
        audio_settings: &AudioSettings,
        reflection_effect_settings: &ReflectionEffectSettings,
    ) -> Result<Self, SteamAudioError> {
        let mut inner = std::ptr::null_mut();

        let status = unsafe {
            audionimbus_sys::iplReflectionMixerCreate(
                context.raw_ptr(),
                &mut audionimbus_sys::IPLAudioSettings::from(audio_settings),
                &mut T::ffi_settings(reflection_effect_settings),
                &raw mut inner,
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        let num_output_channels = T::num_output_channels(reflection_effect_settings);

        let reflection_mixer = Self {
            inner,
            num_output_channels,
            _marker: PhantomData,
        };

        Ok(reflection_mixer)
    }

    /// Retrieves the contents of the reflection mixer and places it into the audio buffer.
    ///
    /// The output audio buffer must have as many channels as the impulse response specified when
    /// creating the effect (for convolution, hybrid, and TrueAudioNext) or at least one channel
    /// (for parametric).
    ///
    /// # Errors
    ///
    /// Returns [`EffectError`] if the output audio buffer does not have as many channels as the
    /// impulse impulse response specified when creating the effect (for convolution, hybrid, and
    /// TrueAudioNext) or at least one channel (for parametric).
    pub fn apply<O, PO: ChannelPointers>(
        &mut self,
        reflection_effect_params: &mut ReflectionEffectParams<T>,
        output_buffer: &AudioBuffer<O, PO>,
    ) -> Result<AudioEffectState, EffectError>
    where
        O: AsRef<[Sample]> + AsMut<[Sample]>,
    {
        let num_output_channels = output_buffer.num_channels();
        if !self
            .num_output_channels
            .is_satisfied_by(num_output_channels)
        {
            return Err(EffectError::InvalidOutputChannels {
                expected: self.num_output_channels,
                actual: num_output_channels,
            });
        }

        let audio_effect_state = unsafe {
            audionimbus_sys::iplReflectionMixerApply(
                self.raw_ptr(),
                &raw mut *reflection_effect_params.as_ffi(),
                &raw mut *output_buffer.as_ffi(),
            )
        };
        let state = audio_effect_state.into();

        Ok(state)
    }

    /// Resets the internal processing state of a reflection mixer.
    pub fn reset(&mut self) {
        unsafe { audionimbus_sys::iplReflectionMixerReset(self.raw_ptr()) };
    }

    /// Returns the raw FFI pointer to the underlying reflection mixer.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub const fn raw_ptr(&self) -> audionimbus_sys::IPLReflectionMixer {
        self.inner
    }

    /// Returns a mutable reference to the raw FFI pointer.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub const fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLReflectionMixer {
        &mut self.inner
    }
}

impl<T: ReflectionEffectType> Drop for ReflectionMixer<T> {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplReflectionMixerRelease(&raw mut self.inner) }
    }
}

unsafe impl<T: ReflectionEffectType> Send for ReflectionMixer<T> {}
unsafe impl<T: ReflectionEffectType> Sync for ReflectionMixer<T> {}

impl<T: ReflectionEffectType> Clone for ReflectionMixer<T> {
    /// Retains an additional reference to the reflection mixer.
    ///
    /// The returned [`ReflectionMixer`] shares the same underlying Steam Audio object.
    fn clone(&self) -> Self {
        // SAFETY: The reflection mixer will not be destroyed until all references are released.
        Self {
            inner: unsafe { audionimbus_sys::iplReflectionMixerRetain(self.inner) },
            num_output_channels: self.num_output_channels,
            _marker: PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::*;

    mod reflection_effect {
        use super::*;

        mod apply {
            use super::*;

            #[test]
            fn test_valid() {
                const SAMPLING_RATE: u32 = 48000;
                const FRAME_SIZE: u32 = 1024;
                const IR_SIZE: u32 = 2 * SAMPLING_RATE;
                const MAX_ORDER: u32 = 1;

                let context = Context::default();

                let audio_settings = AudioSettings::default();

                let simulation_settings =
                    SimulationSettings::new(SAMPLING_RATE, FRAME_SIZE, MAX_ORDER).with_reflections(
                        ReflectionsSimulationSettings::Convolution {
                            max_num_rays: 4096,
                            num_diffuse_samples: 32,
                            max_duration: 2.0,
                            max_num_sources: 8,
                            num_threads: 1,
                        },
                    );
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
                let reflection_effect_params = source.get_reflection_outputs().unwrap();

                let num_output_channels = num_ambisonics_channels(1);
                let reflection_effect_settings = ReflectionEffectSettings {
                    impulse_response_size: IR_SIZE,
                    num_channels: num_output_channels,
                };
                let mut reflection_effect = ReflectionEffect::<Convolution>::try_new(
                    &context,
                    &audio_settings,
                    &reflection_effect_settings,
                )
                .unwrap();

                let input_container = vec![0.5; FRAME_SIZE as usize];
                let input_buffer = AudioBuffer::try_with_data(&input_container).unwrap();

                let mut output_container =
                    vec![0.0; (num_output_channels * input_buffer.num_samples()) as usize];
                let output_buffer = AudioBuffer::try_with_data_and_settings(
                    &mut output_container,
                    AudioBufferSettings::with_num_channels(4),
                )
                .unwrap();

                assert!(reflection_effect
                    .apply(&reflection_effect_params, &input_buffer, &output_buffer)
                    .is_ok());
            }

            #[test]
            fn test_invalid_input_num_channels() {
                const SAMPLING_RATE: u32 = 48000;
                const FRAME_SIZE: u32 = 1024;
                const IR_SIZE: u32 = 2 * SAMPLING_RATE;
                const MAX_ORDER: u32 = 1;

                let context = Context::default();

                let audio_settings = AudioSettings::default();

                let simulation_settings =
                    SimulationSettings::new(SAMPLING_RATE, FRAME_SIZE, MAX_ORDER).with_reflections(
                        ReflectionsSimulationSettings::Convolution {
                            max_num_rays: 4096,
                            num_diffuse_samples: 32,
                            max_duration: 2.0,
                            max_num_sources: 8,
                            num_threads: 1,
                        },
                    );
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
                let reflection_effect_params = source.get_reflection_outputs().unwrap();

                let num_output_channels = num_ambisonics_channels(1);
                let reflection_effect_settings = ReflectionEffectSettings {
                    impulse_response_size: IR_SIZE,
                    num_channels: num_output_channels,
                };
                let mut reflection_effect = ReflectionEffect::<Convolution>::try_new(
                    &context,
                    &audio_settings,
                    &reflection_effect_settings,
                )
                .unwrap();

                let mut input_container = vec![0.5; 2 * FRAME_SIZE as usize];
                let input_buffer = AudioBuffer::try_with_data_and_settings(
                    &mut input_container,
                    AudioBufferSettings::with_num_channels(2),
                )
                .unwrap();

                let mut output_container =
                    vec![0.0; (num_output_channels * input_buffer.num_samples()) as usize];
                let output_buffer = AudioBuffer::try_with_data_and_settings(
                    &mut output_container,
                    AudioBufferSettings::with_num_channels(4),
                )
                .unwrap();

                assert_eq!(
                    reflection_effect.apply(
                        &reflection_effect_params,
                        &input_buffer,
                        &output_buffer
                    ),
                    Err(EffectError::InvalidInputChannels {
                        expected: ChannelRequirement::Exactly(1),
                        actual: 2
                    })
                );
            }

            #[test]
            fn test_invalid_output_num_channels() {
                const SAMPLING_RATE: u32 = 48000;
                const FRAME_SIZE: u32 = 1024;
                const IR_SIZE: u32 = 2 * SAMPLING_RATE;
                const MAX_ORDER: u32 = 1;

                let context = Context::default();

                let audio_settings = AudioSettings::default();

                let simulation_settings =
                    SimulationSettings::new(SAMPLING_RATE, FRAME_SIZE, MAX_ORDER).with_reflections(
                        ReflectionsSimulationSettings::Convolution {
                            max_num_rays: 4096,
                            num_diffuse_samples: 32,
                            max_duration: 2.0,
                            max_num_sources: 8,
                            num_threads: 1,
                        },
                    );
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
                let reflection_effect_params = source.get_reflection_outputs().unwrap();

                let num_output_channels = num_ambisonics_channels(1);
                let reflection_effect_settings = ReflectionEffectSettings {
                    impulse_response_size: IR_SIZE,
                    num_channels: num_output_channels,
                };
                let mut reflection_effect = ReflectionEffect::<Convolution>::try_new(
                    &context,
                    &audio_settings,
                    &reflection_effect_settings,
                )
                .unwrap();

                let input_container = vec![0.5; FRAME_SIZE as usize];
                let input_buffer = AudioBuffer::try_with_data(&input_container).unwrap();

                let mut output_container = vec![0.0; (2 * input_buffer.num_samples()) as usize];
                let output_buffer = AudioBuffer::try_with_data_and_settings(
                    &mut output_container,
                    AudioBufferSettings::with_num_channels(2),
                )
                .unwrap();

                assert_eq!(
                    reflection_effect.apply(
                        &reflection_effect_params,
                        &input_buffer,
                        &output_buffer
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
                const SAMPLING_RATE: u32 = 48000;
                const FRAME_SIZE: u32 = 1024;
                const IR_SIZE: u32 = 2 * SAMPLING_RATE;
                const MAX_ORDER: u32 = 1;

                let context = Context::default();

                let audio_settings = AudioSettings::default();

                let simulation_settings =
                    SimulationSettings::new(SAMPLING_RATE, FRAME_SIZE, MAX_ORDER).with_reflections(
                        ReflectionsSimulationSettings::Convolution {
                            max_num_rays: 4096,
                            num_diffuse_samples: 32,
                            max_duration: 2.0,
                            max_num_sources: 8,
                            num_threads: 1,
                        },
                    );
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
                let reflection_effect_params = source.get_reflection_outputs().unwrap();

                let num_output_channels = num_ambisonics_channels(1);
                let reflection_effect_settings = ReflectionEffectSettings {
                    impulse_response_size: IR_SIZE,
                    num_channels: num_output_channels,
                };
                let mut reflection_effect = ReflectionEffect::<Convolution>::try_new(
                    &context,
                    &audio_settings,
                    &reflection_effect_settings,
                )
                .unwrap();

                let input_container = vec![0.5; FRAME_SIZE as usize];
                let input_buffer = AudioBuffer::try_with_data(&input_container).unwrap();

                let mut output_container =
                    vec![0.0; (num_output_channels * input_buffer.num_samples()) as usize];
                let output_buffer = AudioBuffer::try_with_data_and_settings(
                    &mut output_container,
                    AudioBufferSettings::with_num_channels(4),
                )
                .unwrap();

                let mixer = ReflectionMixer::try_new(
                    &context,
                    &audio_settings,
                    &reflection_effect_settings,
                )
                .unwrap();

                assert!(reflection_effect
                    .apply_into_mixer(
                        &reflection_effect_params,
                        &input_buffer,
                        &output_buffer,
                        &mixer
                    )
                    .is_ok());
            }

            #[test]
            fn test_invalid_input_num_channels() {
                const SAMPLING_RATE: u32 = 48000;
                const FRAME_SIZE: u32 = 1024;
                const IR_SIZE: u32 = 2 * SAMPLING_RATE;
                const MAX_ORDER: u32 = 1;

                let context = Context::default();

                let audio_settings = AudioSettings::default();

                let simulation_settings =
                    SimulationSettings::new(SAMPLING_RATE, FRAME_SIZE, MAX_ORDER).with_reflections(
                        ReflectionsSimulationSettings::Convolution {
                            max_num_rays: 4096,
                            num_diffuse_samples: 32,
                            max_duration: 2.0,
                            max_num_sources: 8,
                            num_threads: 1,
                        },
                    );
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
                let reflection_effect_params = source.get_reflection_outputs().unwrap();

                let num_output_channels = num_ambisonics_channels(1);
                let reflection_effect_settings = ReflectionEffectSettings {
                    impulse_response_size: IR_SIZE,
                    num_channels: num_output_channels,
                };
                let mut reflection_effect = ReflectionEffect::<Convolution>::try_new(
                    &context,
                    &audio_settings,
                    &reflection_effect_settings,
                )
                .unwrap();

                let mut input_container = vec![0.5; 2 * FRAME_SIZE as usize];
                let input_buffer = AudioBuffer::try_with_data_and_settings(
                    &mut input_container,
                    AudioBufferSettings::with_num_channels(2),
                )
                .unwrap();

                let mut output_container =
                    vec![0.0; (num_output_channels * input_buffer.num_samples()) as usize];
                let output_buffer = AudioBuffer::try_with_data_and_settings(
                    &mut output_container,
                    AudioBufferSettings::with_num_channels(4),
                )
                .unwrap();

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
                        &output_buffer,
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
                const SAMPLING_RATE: u32 = 48000;
                const FRAME_SIZE: u32 = 1024;
                const IR_SIZE: u32 = 2 * SAMPLING_RATE;
                const MAX_ORDER: u32 = 1;

                let context = Context::default();

                let audio_settings = AudioSettings::default();

                let simulation_settings =
                    SimulationSettings::new(SAMPLING_RATE, FRAME_SIZE, MAX_ORDER).with_reflections(
                        ReflectionsSimulationSettings::Convolution {
                            max_num_rays: 4096,
                            num_diffuse_samples: 32,
                            max_duration: 2.0,
                            max_num_sources: 8,
                            num_threads: 1,
                        },
                    );
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
                let reflection_effect_params = source.get_reflection_outputs().unwrap();

                let num_output_channels = num_ambisonics_channels(1);
                let reflection_effect_settings = ReflectionEffectSettings {
                    impulse_response_size: IR_SIZE,
                    num_channels: num_output_channels,
                };
                let mut reflection_effect = ReflectionEffect::<Convolution>::try_new(
                    &context,
                    &audio_settings,
                    &reflection_effect_settings,
                )
                .unwrap();

                let mut input_container = vec![0.5; FRAME_SIZE as usize];
                let input_buffer = AudioBuffer::try_with_data(&mut input_container).unwrap();

                let mut output_container = vec![0.0; (2 * input_buffer.num_samples()) as usize];
                let output_buffer = AudioBuffer::try_with_data_and_settings(
                    &mut output_container,
                    AudioBufferSettings::with_num_channels(2),
                )
                .unwrap();

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
                        &output_buffer,
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
                const SAMPLING_RATE: u32 = 48000;
                const FRAME_SIZE: u32 = 1024;
                const IR_SIZE: u32 = 2 * SAMPLING_RATE;

                let context = Context::default();

                let audio_settings = AudioSettings::default();

                let num_output_channels = num_ambisonics_channels(1);
                let reflection_effect_settings = ReflectionEffectSettings {
                    impulse_response_size: IR_SIZE,
                    num_channels: num_output_channels,
                };
                let reflection_effect = ReflectionEffect::<Convolution>::try_new(
                    &context,
                    &audio_settings,
                    &reflection_effect_settings,
                )
                .unwrap();

                let mut output_container = vec![0.0; (num_output_channels * FRAME_SIZE) as usize];
                let output_buffer = AudioBuffer::try_with_data_and_settings(
                    &mut output_container,
                    AudioBufferSettings::with_num_channels(num_output_channels),
                )
                .unwrap();

                assert!(reflection_effect.tail(&output_buffer).is_ok());
            }

            #[test]
            fn test_invalid_output_num_channels() {
                const SAMPLING_RATE: u32 = 48000;
                const FRAME_SIZE: u32 = 1024;
                const IR_SIZE: u32 = 2 * SAMPLING_RATE;

                let context = Context::default();

                let audio_settings = AudioSettings::default();

                let num_output_channels = num_ambisonics_channels(1);
                let reflection_effect_settings = ReflectionEffectSettings {
                    impulse_response_size: IR_SIZE,
                    num_channels: num_output_channels,
                };
                let reflection_effect = ReflectionEffect::<Convolution>::try_new(
                    &context,
                    &audio_settings,
                    &reflection_effect_settings,
                )
                .unwrap();

                let mut output_container = vec![0.0; FRAME_SIZE as usize];
                let output_buffer = AudioBuffer::try_with_data(&mut output_container).unwrap();

                assert_eq!(
                    reflection_effect.tail(&output_buffer),
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
                const SAMPLING_RATE: u32 = 48000;
                const FRAME_SIZE: u32 = 1024;
                const IR_SIZE: u32 = 2 * SAMPLING_RATE;

                let context = Context::default();

                let audio_settings = AudioSettings::default();

                let num_output_channels = num_ambisonics_channels(1);
                let reflection_effect_settings = ReflectionEffectSettings {
                    impulse_response_size: IR_SIZE,
                    num_channels: num_output_channels,
                };
                let reflection_effect = ReflectionEffect::<Convolution>::try_new(
                    &context,
                    &audio_settings,
                    &reflection_effect_settings,
                )
                .unwrap();

                let mut output_container = vec![0.0; (num_output_channels * FRAME_SIZE) as usize];
                let output_buffer = AudioBuffer::try_with_data_and_settings(
                    &mut output_container,
                    AudioBufferSettings::with_num_channels(num_output_channels),
                )
                .unwrap();

                let mixer = ReflectionMixer::try_new(
                    &context,
                    &audio_settings,
                    &reflection_effect_settings,
                )
                .unwrap();

                assert!(reflection_effect
                    .tail_into_mixer(&output_buffer, &mixer)
                    .is_ok());
            }

            #[test]
            fn test_invalid_output_num_channels() {
                const SAMPLING_RATE: u32 = 48000;
                const FRAME_SIZE: u32 = 1024;
                const IR_SIZE: u32 = 2 * SAMPLING_RATE;

                let context = Context::default();

                let audio_settings = AudioSettings::default();

                let num_output_channels = num_ambisonics_channels(1);
                let reflection_effect_settings = ReflectionEffectSettings {
                    impulse_response_size: IR_SIZE,
                    num_channels: num_output_channels,
                };
                let reflection_effect = ReflectionEffect::<Convolution>::try_new(
                    &context,
                    &audio_settings,
                    &reflection_effect_settings,
                )
                .unwrap();

                let mut output_container = vec![0.0; FRAME_SIZE as usize];
                let output_buffer = AudioBuffer::try_with_data(&mut output_container).unwrap();

                let mixer = ReflectionMixer::try_new(
                    &context,
                    &audio_settings,
                    &reflection_effect_settings,
                )
                .unwrap();

                assert_eq!(
                    reflection_effect.tail_into_mixer(&output_buffer, &mixer),
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
                const SAMPLING_RATE: u32 = 48000;
                const IR_SIZE: u32 = 2 * SAMPLING_RATE;

                let context = Context::default();
                let audio_settings = AudioSettings::default();

                let num_output_channels = num_ambisonics_channels(1);
                let reflection_effect_settings = ReflectionEffectSettings {
                    impulse_response_size: IR_SIZE,
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

    mod reflection_mixer {
        use super::*;

        mod apply {
            use super::*;

            #[test]
            fn test_valid() {
                const SAMPLING_RATE: u32 = 48000;
                const FRAME_SIZE: u32 = 1024;
                const IR_SIZE: u32 = 2 * SAMPLING_RATE;
                const MAX_ORDER: u32 = 1;

                let context = Context::default();

                let audio_settings = AudioSettings::default();

                let simulation_settings =
                    SimulationSettings::new(SAMPLING_RATE, FRAME_SIZE, MAX_ORDER).with_reflections(
                        ReflectionsSimulationSettings::Convolution {
                            max_num_rays: 4096,
                            num_diffuse_samples: 32,
                            max_duration: 2.0,
                            max_num_sources: 8,
                            num_threads: 1,
                        },
                    );
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
                let mut reflection_effect_params = source.get_reflection_outputs().unwrap();

                let num_output_channels = num_ambisonics_channels(1);
                let reflection_effect_settings = ReflectionEffectSettings {
                    impulse_response_size: IR_SIZE,
                    num_channels: num_output_channels,
                };

                let mut mixer = ReflectionMixer::<Convolution>::try_new(
                    &context,
                    &audio_settings,
                    &reflection_effect_settings,
                )
                .unwrap();

                let mut output_container = vec![0.0; (num_output_channels * FRAME_SIZE) as usize];
                let output_buffer = AudioBuffer::try_with_data_and_settings(
                    &mut output_container,
                    AudioBufferSettings::with_num_channels(num_output_channels),
                )
                .unwrap();

                assert!(mixer
                    .apply(&mut reflection_effect_params, &output_buffer)
                    .is_ok());
            }

            #[test]
            fn test_invalid_output_num_channels() {
                const SAMPLING_RATE: u32 = 48000;
                const FRAME_SIZE: u32 = 1024;
                const IR_SIZE: u32 = 2 * SAMPLING_RATE;
                const MAX_ORDER: u32 = 1;

                let context = Context::default();

                let audio_settings = AudioSettings::default();

                let simulation_settings =
                    SimulationSettings::new(SAMPLING_RATE, FRAME_SIZE, MAX_ORDER).with_reflections(
                        ReflectionsSimulationSettings::Convolution {
                            max_num_rays: 4096,
                            num_diffuse_samples: 32,
                            max_duration: 2.0,
                            max_num_sources: 8,
                            num_threads: 1,
                        },
                    );
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
                let mut reflection_effect_params = source.get_reflection_outputs().unwrap();

                let num_output_channels = num_ambisonics_channels(1);
                let reflection_effect_settings = ReflectionEffectSettings {
                    impulse_response_size: IR_SIZE,
                    num_channels: num_output_channels,
                };

                let mut mixer = ReflectionMixer::<Convolution>::try_new(
                    &context,
                    &audio_settings,
                    &reflection_effect_settings,
                )
                .unwrap();

                let mut output_container = vec![0.0; (2 * FRAME_SIZE) as usize];
                let output_buffer = AudioBuffer::try_with_data_and_settings(
                    &mut output_container,
                    AudioBufferSettings::with_num_channels(2),
                )
                .unwrap();

                assert_eq!(
                    mixer.apply(&mut reflection_effect_params, &output_buffer),
                    Err(EffectError::InvalidOutputChannels {
                        expected: ChannelRequirement::Exactly(4),
                        actual: 2
                    })
                );
            }

            #[test]
            fn test_parametric_valid() {
                const SAMPLING_RATE: u32 = 48000;
                const FRAME_SIZE: u32 = 1024;
                const IR_SIZE: u32 = 2 * SAMPLING_RATE;
                const MAX_ORDER: u32 = 1;

                let context = Context::default();

                let audio_settings = AudioSettings::default();

                let simulation_settings =
                    SimulationSettings::new(SAMPLING_RATE, FRAME_SIZE, MAX_ORDER).with_reflections(
                        ReflectionsSimulationSettings::Parametric {
                            max_num_rays: 4096,
                            num_diffuse_samples: 32,
                            max_duration: 2.0,
                            max_num_sources: 8,
                            num_threads: 1,
                        },
                    );
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
                let mut reflection_effect_params = source.get_reflection_outputs().unwrap();

                let num_output_channels = num_ambisonics_channels(1);
                let reflection_effect_settings = ReflectionEffectSettings {
                    impulse_response_size: IR_SIZE,
                    num_channels: num_output_channels,
                };

                let mut mixer = ReflectionMixer::<Convolution>::try_new(
                    &context,
                    &audio_settings,
                    &reflection_effect_settings,
                )
                .unwrap();

                let mut output_container = vec![0.0; (num_output_channels * FRAME_SIZE) as usize];
                let output_buffer = AudioBuffer::try_with_data_and_settings(
                    &mut output_container,
                    AudioBufferSettings::with_num_channels(num_output_channels),
                )
                .unwrap();

                assert!(mixer
                    .apply(&mut reflection_effect_params, &output_buffer)
                    .is_ok());
            }

            #[test]
            fn test_parametric_at_least_one_channel() {
                const SAMPLING_RATE: u32 = 48000;
                const FRAME_SIZE: u32 = 1024;
                const IR_SIZE: u32 = 2 * SAMPLING_RATE;
                const MAX_ORDER: u32 = 1;

                let context = Context::default();

                let audio_settings = AudioSettings::default();

                let simulation_settings =
                    SimulationSettings::new(SAMPLING_RATE, FRAME_SIZE, MAX_ORDER).with_reflections(
                        ReflectionsSimulationSettings::Parametric {
                            max_num_rays: 4096,
                            num_diffuse_samples: 32,
                            max_duration: 2.0,
                            max_num_sources: 8,
                            num_threads: 1,
                        },
                    );
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
                let mut reflection_effect_params = source.get_reflection_outputs().unwrap();

                let num_output_channels = num_ambisonics_channels(1);
                let reflection_effect_settings = ReflectionEffectSettings {
                    impulse_response_size: IR_SIZE,
                    num_channels: num_output_channels,
                };

                let mut mixer = ReflectionMixer::<Parametric>::try_new(
                    &context,
                    &audio_settings,
                    &reflection_effect_settings,
                )
                .unwrap();

                // Test with 1 channel (minimum for parametric)
                let mut output_container = vec![0.0; FRAME_SIZE as usize];
                let output_buffer = AudioBuffer::try_with_data(&mut output_container).unwrap();

                assert!(mixer
                    .apply(&mut reflection_effect_params, &output_buffer)
                    .is_ok());
            }
        }

        mod reset {
            use super::*;

            #[test]
            fn test_reset() {
                const SAMPLING_RATE: u32 = 48000;
                const IR_SIZE: u32 = 2 * SAMPLING_RATE;

                let context = Context::default();

                let audio_settings = AudioSettings::default();

                let num_output_channels = num_ambisonics_channels(1);
                let reflection_effect_settings = ReflectionEffectSettings {
                    impulse_response_size: IR_SIZE,
                    num_channels: num_output_channels,
                };

                let mut mixer = ReflectionMixer::<Convolution>::try_new(
                    &context,
                    &audio_settings,
                    &reflection_effect_settings,
                )
                .unwrap();

                mixer.reset();
            }
        }

        mod clone {
            use super::*;

            #[test]
            fn test_clone() {
                const SAMPLING_RATE: u32 = 48000;
                const IR_SIZE: u32 = 2 * SAMPLING_RATE;

                let context = Context::default();

                let audio_settings = AudioSettings::default();

                let num_output_channels = num_ambisonics_channels(1);
                let reflection_effect_settings = ReflectionEffectSettings {
                    impulse_response_size: IR_SIZE,
                    num_channels: num_output_channels,
                };

                let mixer = ReflectionMixer::<Convolution>::try_new(
                    &context,
                    &audio_settings,
                    &reflection_effect_settings,
                )
                .unwrap();
                let clone = mixer.clone();
                assert_eq!(mixer.raw_ptr(), clone.raw_ptr());
                drop(mixer);
                assert!(!clone.raw_ptr().is_null());
            }
        }
    }
}
