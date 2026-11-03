//! Runtime-selected reflection algorithms and handles.

use crate::binding::Dynamic;
use crate::effect::reflections::{effect, mixer};
use crate::{
    ConvolutionParameters, ConvolutionSettings, EffectError, HybridParameters, HybridSettings,
    ParametricParameters, ParametricSettings, SteamAudioError, TrueAudioNextParameters,
    TrueAudioNextSettings,
};
use std::fmt;

/// A reflection algorithm selected at construction.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ReflectionAlgorithm {
    /// Multi-channel convolution reverb.
    Convolution,
    /// Parametric reverb.
    Parametric,
    /// Combined convolution and parametric reverb.
    Hybrid,
    /// GPU convolution through AMD TrueAudio Next.
    TrueAudioNext,
}

/// Simulation settings for a runtime-selected reflection algorithm.
#[derive(Clone, Debug)]
pub enum ReflectionsSettings {
    /// Multi-channel convolution settings.
    Convolution(ConvolutionSettings),
    /// Parametric reverb settings.
    Parametric(ParametricSettings),
    /// Combined convolution and parametric settings.
    Hybrid(HybridSettings),
    /// GPU convolution settings and devices.
    TrueAudioNext(TrueAudioNextSettings),
}

impl ReflectionsSettings {
    /// Returns the selected algorithm.
    pub const fn algorithm(&self) -> ReflectionAlgorithm {
        match self {
            Self::Convolution(_) => ReflectionAlgorithm::Convolution,
            Self::Parametric(_) => ReflectionAlgorithm::Parametric,
            Self::Hybrid(_) => ReflectionAlgorithm::Hybrid,
            Self::TrueAudioNext(_) => ReflectionAlgorithm::TrueAudioNext,
        }
    }
}

impl From<ConvolutionSettings> for ReflectionsSettings {
    fn from(convolution_settings: ConvolutionSettings) -> Self {
        Self::Convolution(convolution_settings)
    }
}

impl From<ParametricSettings> for ReflectionsSettings {
    fn from(parametric_settings: ParametricSettings) -> Self {
        Self::Parametric(parametric_settings)
    }
}

impl From<HybridSettings> for ReflectionsSettings {
    fn from(hybrid_settings: HybridSettings) -> Self {
        Self::Hybrid(hybrid_settings)
    }
}

impl From<TrueAudioNextSettings> for ReflectionsSettings {
    fn from(true_audio_next_settings: TrueAudioNextSettings) -> Self {
        Self::TrueAudioNext(true_audio_next_settings)
    }
}

/// Source parameters for a runtime-selected reflection algorithm.
#[derive(Clone, Debug)]
pub enum ReflectionsParameters {
    /// Multi-channel convolution parameters.
    Convolution(ConvolutionParameters),
    /// Parametric reverb parameters.
    Parametric(ParametricParameters),
    /// Combined convolution and parametric parameters.
    Hybrid(HybridParameters),
    /// GPU convolution parameters.
    TrueAudioNext(TrueAudioNextParameters),
}

impl ReflectionsParameters {
    /// Returns the selected algorithm.
    pub const fn algorithm(&self) -> ReflectionAlgorithm {
        match self {
            Self::Convolution(_) => ReflectionAlgorithm::Convolution,
            Self::Parametric(_) => ReflectionAlgorithm::Parametric,
            Self::Hybrid(_) => ReflectionAlgorithm::Hybrid,
            Self::TrueAudioNext(_) => ReflectionAlgorithm::TrueAudioNext,
        }
    }
}

impl From<ConvolutionParameters> for ReflectionsParameters {
    fn from(convolution_parameters: ConvolutionParameters) -> Self {
        Self::Convolution(convolution_parameters)
    }
}

impl From<ParametricParameters> for ReflectionsParameters {
    fn from(parametric_parameters: ParametricParameters) -> Self {
        Self::Parametric(parametric_parameters)
    }
}

impl From<HybridParameters> for ReflectionsParameters {
    fn from(hybrid_parameters: HybridParameters) -> Self {
        Self::Hybrid(hybrid_parameters)
    }
}

impl From<TrueAudioNextParameters> for ReflectionsParameters {
    fn from(true_audio_next_parameters: TrueAudioNextParameters) -> Self {
        Self::TrueAudioNext(true_audio_next_parameters)
    }
}

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
pub type ReflectionEffect = effect::ReflectionEffect<Dynamic<ReflectionAlgorithm>>;

/// Reflection outputs from a runtime-selected source.
pub type ReflectionEffectParams = effect::ReflectionEffectParams<Dynamic<ReflectionAlgorithm>>;

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
pub type ReflectionMixer = mixer::ReflectionMixer<Dynamic<ReflectionAlgorithm>>;

/// Errors from runtime reflection creation and processing.
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

impl std::error::Error for ReflectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SteamAudio(error) => Some(error),
            Self::Effect(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(doctest)]
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
