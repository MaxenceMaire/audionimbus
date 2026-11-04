//! Reflection mixers and native output retrieval.

use super::binding::ReflectionBinding;
use super::effect::{ReflectionEffectParams, StaticReflectionEffectParams};
use super::{ReflectionEffectSettings, ReflectionEffectType};
use crate::ChannelRequirement;
use crate::audio_buffer::AudioBufferMut;
use crate::audio_settings::AudioSettings;
#[cfg(feature = "dynamic")]
use crate::binding::Dynamic;
use crate::binding::Static;
use crate::context::Context;
#[cfg(feature = "dynamic")]
use crate::dynamic::{ReflectionAlgorithm, ReflectionError};
use crate::effect::{AudioEffectState, EffectError};
use crate::error::{SteamAudioError, to_option_error};
use std::hash::{Hash, Hasher};

/// Mixes the outputs of multiple reflection effects, and generates a single sound field containing all the reflected sound reaching the listener.
///
/// Using this is optional. Depending on the reflection effect algorithm used, a reflection mixer may provide a reduction in CPU usage.
///
/// `ReflectionMixer` is a reference-counted handle to an underlying Steam Audio object.
/// Cloning it is cheap; it produces a new handle pointing to the same underlying object, while
/// incrementing a reference count.
/// The underlying object is destroyed when all handles are dropped.
pub type StaticReflectionMixer<T> = ReflectionMixer<Static<T>>;

/// A reflection mixer with a runtime-selected algorithm.
///
/// ```
/// use audionimbus::{AudioBufferMut, AudioEffectState};
/// use audionimbus::dynamic::{ReflectionEffectParams, ReflectionError, ReflectionMixer};
///
/// fn mix(
///     reflection_mixer: &mut ReflectionMixer,
///     reflection_effect_params: &ReflectionEffectParams,
///     output_buffer: &mut AudioBufferMut<'_>,
/// ) -> Result<AudioEffectState, ReflectionError> {
///     reflection_mixer.apply(reflection_effect_params, output_buffer)
/// }
/// ```
#[cfg(feature = "dynamic")]
pub type DynamicReflectionMixer = ReflectionMixer<Dynamic<ReflectionAlgorithm>>;

/// Reflection mixer storage for either API binding.
#[derive(Debug)]
pub struct ReflectionMixer<B: ReflectionBinding> {
    /// The native reflection handle.
    inner: audionimbus_sys::IPLReflectionMixer,

    /// Number of output channels required.
    num_output_channels: ChannelRequirement,

    /// The compile-time or runtime reflection binding.
    binding: B,
}

#[cfg(feature = "dynamic")]
impl DynamicReflectionMixer {
    /// Creates a reflection mixer with the selected algorithm.
    ///
    /// # Errors
    ///
    /// - [`ReflectionError::MixerUnsupported`] when parametric or hybrid reverb cannot use a mixer.
    /// - [`ReflectionError::SteamAudio`] when native creation fails.
    pub fn try_new(
        context: &Context,
        audio_settings: &AudioSettings,
        reflection_effect_settings: &ReflectionEffectSettings,
        algorithm: ReflectionAlgorithm,
    ) -> Result<Self, ReflectionError> {
        if !matches!(
            algorithm,
            ReflectionAlgorithm::Convolution | ReflectionAlgorithm::TrueAudioNext
        ) {
            return Err(ReflectionError::MixerUnsupported);
        }

        Self::try_new_shared(
            context,
            audio_settings,
            reflection_effect_settings,
            Dynamic(algorithm),
        )
        .map_err(ReflectionError::SteamAudio)
    }

    /// Returns the selected algorithm.
    pub const fn algorithm(&self) -> ReflectionAlgorithm {
        self.binding.0
    }

    /// Retrieves mixed reflections using parameters with the same algorithm.
    ///
    /// # Errors
    ///
    /// - [`ReflectionError::AlgorithmMismatch`] when the parameters use another algorithm.
    /// - [`ReflectionError::Effect`] when the output buffer has an invalid channel count.
    pub fn apply(
        &mut self,
        reflection_effect_params: &crate::dynamic::ReflectionEffectParams,
        output_buffer: &mut AudioBufferMut<'_>,
    ) -> Result<AudioEffectState, ReflectionError> {
        if self.algorithm() != reflection_effect_params.algorithm() {
            return Err(ReflectionError::AlgorithmMismatch {
                expected: self.algorithm(),
                actual: reflection_effect_params.algorithm(),
            });
        }

        self.apply_shared(reflection_effect_params, output_buffer)
            .map_err(ReflectionError::Effect)
    }
}

impl<T: ReflectionEffectType> StaticReflectionMixer<T> {
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
    pub const fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLReflectionMixer {
        &mut self.inner
    }
}

impl<B: ReflectionBinding> ReflectionMixer<B> {
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
            audionimbus_sys::iplReflectionMixerCreate(
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

        let reflection_mixer = Self {
            inner,
            num_output_channels,
            binding,
        };

        Ok(reflection_mixer)
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
}

impl<T: ReflectionEffectType> StaticReflectionMixer<T> {
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
    pub fn apply(
        &mut self,
        reflection_effect_params: &mut StaticReflectionEffectParams<T>,
        output_buffer: &mut AudioBufferMut<'_>,
    ) -> Result<AudioEffectState, EffectError> {
        self.apply_shared(reflection_effect_params, output_buffer)
    }
}

impl<B: ReflectionBinding> ReflectionMixer<B> {
    /// Retrieves mixed reflections.
    ///
    /// # Errors
    ///
    /// - [`EffectError::InvalidOutputChannels`] when the output channel count is invalid.
    fn apply_shared(
        &mut self,
        reflection_effect_params: &ReflectionEffectParams<B>,
        output_buffer: &mut AudioBufferMut<'_>,
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
        let audio_effect_state = unsafe {
            audionimbus_sys::iplReflectionMixerApply(
                self.raw_ptr(),
                &raw mut *reflection_effect_params.as_ffi(),
                &raw mut *output_buffer,
            )
        };
        let state = audio_effect_state.into();

        Ok(state)
    }
}

impl<B: ReflectionBinding> Drop for ReflectionMixer<B> {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplReflectionMixerRelease(&raw mut self.inner) }
    }
}

unsafe impl<B: ReflectionBinding> Send for ReflectionMixer<B> {}
unsafe impl<B: ReflectionBinding> Sync for ReflectionMixer<B> {}

impl<B: ReflectionBinding> Clone for ReflectionMixer<B> {
    /// Retains an additional reference to the reflection mixer.
    ///
    /// The returned [`ReflectionMixer`](crate::ReflectionMixer) shares the same underlying Steam Audio object.
    fn clone(&self) -> Self {
        // SAFETY: The reflection mixer will not be destroyed until all references are released.
        Self {
            inner: unsafe { audionimbus_sys::iplReflectionMixerRetain(self.inner) },
            num_output_channels: self.num_output_channels,
            binding: self.binding,
        }
    }
}

impl<B: ReflectionBinding> PartialEq for ReflectionMixer<B> {
    fn eq(&self, other: &Self) -> bool {
        self.raw_ptr() == other.raw_ptr()
    }
}

impl<B: ReflectionBinding> Eq for ReflectionMixer<B> {}

impl<B: ReflectionBinding> Hash for ReflectionMixer<B> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::ptr::hash(self.raw_ptr(), state);
    }
}

#[cfg(test)]
mod tests {
    use crate::*;

    mod reflection_mixer {
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
                let mut reflection_effect_params = source.get_reflections_outputs().unwrap();

                let num_output_channels = num_ambisonics_channels(1);
                let reflection_effect_settings = ReflectionEffectSettings {
                    impulse_response_size,
                    num_channels: num_output_channels,
                };

                let mut mixer = ReflectionMixer::<Convolution>::try_new(
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

                assert!(
                    mixer
                        .apply(&mut reflection_effect_params, &mut output_buffer)
                        .is_ok()
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
                let mut reflection_effect_params = source.get_reflections_outputs().unwrap();

                let num_output_channels = num_ambisonics_channels(1);
                let reflection_effect_settings = ReflectionEffectSettings {
                    impulse_response_size,
                    num_channels: num_output_channels,
                };

                let mut mixer = ReflectionMixer::<Convolution>::try_new(
                    &context,
                    &audio_settings,
                    &reflection_effect_settings,
                )
                .unwrap();

                let mut output_container = vec![0.0; (2 * audio_settings.frame_size) as usize];
                let mut output_buffer =
                    AudioBufferMut::try_new(output_container.as_mut_slice(), 2 as usize).unwrap();

                assert_eq!(
                    mixer.apply(&mut reflection_effect_params, &mut output_buffer),
                    Err(EffectError::InvalidOutputChannels {
                        expected: ChannelRequirement::Exactly(4),
                        actual: 2
                    })
                );
            }

            #[test]
            fn test_parametric_valid() {
                let context = Context::default();

                let audio_settings = AudioSettings::default();
                let impulse_response_size = 2 * audio_settings.sampling_rate;

                let simulation_settings = SimulationSettings::new(&audio_settings)
                    .with_reflections(ParametricSettings {
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
                let mut reflection_effect_params = source.get_reflections_outputs().unwrap();

                let num_output_channels = num_ambisonics_channels(1);
                let reflection_effect_settings = ReflectionEffectSettings {
                    impulse_response_size,
                    num_channels: num_output_channels,
                };

                let mut mixer = ReflectionMixer::<Parametric>::try_new(
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

                assert!(
                    mixer
                        .apply(&mut reflection_effect_params, &mut output_buffer)
                        .is_ok()
                );
            }

            #[test]
            fn test_parametric_at_least_one_channel() {
                let context = Context::default();

                let audio_settings = AudioSettings::default();
                let impulse_response_size = 2 * audio_settings.sampling_rate;

                let simulation_settings = SimulationSettings::new(&audio_settings)
                    .with_reflections(ParametricSettings {
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
                let mut reflection_effect_params = source.get_reflections_outputs().unwrap();

                let num_output_channels = num_ambisonics_channels(1);
                let reflection_effect_settings = ReflectionEffectSettings {
                    impulse_response_size,
                    num_channels: num_output_channels,
                };

                let mut mixer = ReflectionMixer::<Parametric>::try_new(
                    &context,
                    &audio_settings,
                    &reflection_effect_settings,
                )
                .unwrap();

                // Test with 1 channel (minimum for parametric)
                let mut output_container = vec![0.0; audio_settings.frame_size as usize];
                let mut output_buffer =
                    AudioBufferMut::try_from(output_container.as_mut_slice()).unwrap();

                assert!(
                    mixer
                        .apply(&mut reflection_effect_params, &mut output_buffer)
                        .is_ok()
                );
            }
        }

        mod reset {
            use super::*;

            #[test]
            fn test_reset() {
                let context = Context::default();

                let audio_settings = AudioSettings::default();
                let impulse_response_size = 2 * audio_settings.sampling_rate;

                let num_output_channels = num_ambisonics_channels(1);
                let reflection_effect_settings = ReflectionEffectSettings {
                    impulse_response_size,
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
                let context = Context::default();

                let audio_settings = AudioSettings::default();
                let impulse_response_size = 2 * audio_settings.sampling_rate;

                let num_output_channels = num_ambisonics_channels(1);
                let reflection_effect_settings = ReflectionEffectSettings {
                    impulse_response_size,
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
