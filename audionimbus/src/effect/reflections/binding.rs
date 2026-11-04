//! Native settings selected by reflection bindings.

use super::ReflectionEffectSettings;
#[cfg(doc)]
use super::ReflectionMixer;
use crate::ChannelRequirement;
use crate::Sealed;
#[cfg(feature = "dynamic")]
use crate::binding::Dynamic;
use crate::binding::Static;
use crate::simulation::{
    ConvolutionParameters, HybridParameters, ParametricParameters, ReflectionsSimulationParameters,
    TrueAudioNextParameters,
};

/// Multi-channel convolution reverb.
///
/// Reflections reaching the listener are encoded in an Impulse Response (IR), which is a filter that records each reflection as it arrives.
/// This algorithm renders reflections with the most detail, but may result in significant CPU usage.
///
/// Using a [`ReflectionMixer`] with this algorithm provides a reduction in CPU usage.
#[derive(Default, Debug, Copy, Clone)]
pub struct Convolution;

/// Parametric (or artificial) reverb, using feedback delay networks.
///
/// The reflected sound field is reduced to a few numbers that describe how reflected energy decays over time.
/// This is then used to drive an approximate model of reverberation in an indoor space.
/// This algorithm results in lower CPU usage, but cannot render individual echoes, especially in outdoor spaces.
///
/// A reflection mixer cannot be used with this algorithm.
#[derive(Default, Debug, Copy, Clone)]
pub struct Parametric;

/// A hybrid of convolution and parametric reverb.
///
/// The initial portion of the IR is rendered using convolution reverb, but the later part is used to estimate a parametric reverb.
/// The point in the IR where this transition occurs can be controlled.
/// This algorithm allows a trade-off between rendering quality and CPU usage.
///
/// A reflection mixer cannot be used with this algorithm.
#[derive(Default, Debug, Copy, Clone)]
pub struct Hybrid;

/// Multi-channel convolution reverb, using AMD TrueAudio Next for GPU acceleration.
///
/// This algorithm is similar to [`Convolution`], but uses the GPU instead of the CPU for processing, allowing significantly more sources to be processed.
///
/// A [`ReflectionMixer`] must be used with this algorithm, because the GPU will process convolution reverb at a single point in your audio processing pipeline.
#[derive(Default, Debug, Copy, Clone)]
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
    /// The source parameters used by this algorithm.
    type SimulationParameters: ReflectionsSimulationParameters + Default;

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
    type SimulationParameters = ConvolutionParameters;

    fn to_ffi_type() -> audionimbus_sys::IPLReflectionEffectType {
        audionimbus_sys::IPLReflectionEffectType::IPL_REFLECTIONEFFECTTYPE_CONVOLUTION
    }

    fn num_output_channels(settings: &ReflectionEffectSettings) -> ChannelRequirement {
        ChannelRequirement::Exactly(settings.num_channels)
    }
}

impl ReflectionEffectType for Parametric {
    type SimulationParameters = ParametricParameters;

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
    type SimulationParameters = HybridParameters;

    fn to_ffi_type() -> audionimbus_sys::IPLReflectionEffectType {
        audionimbus_sys::IPLReflectionEffectType::IPL_REFLECTIONEFFECTTYPE_HYBRID
    }

    fn num_output_channels(settings: &ReflectionEffectSettings) -> ChannelRequirement {
        ChannelRequirement::Exactly(settings.num_channels)
    }
}

impl ReflectionEffectType for TrueAudioNext {
    type SimulationParameters = TrueAudioNextParameters;

    fn to_ffi_type() -> audionimbus_sys::IPLReflectionEffectType {
        audionimbus_sys::IPLReflectionEffectType::IPL_REFLECTIONEFFECTTYPE_TAN
    }

    fn num_output_channels(settings: &ReflectionEffectSettings) -> ChannelRequirement {
        ChannelRequirement::Exactly(settings.num_channels)
    }
}

/// Storage and native settings selected by a reflection API binding.
pub trait ReflectionBinding: Copy {
    /// Returns the native reflection algorithm.
    fn ffi_type(self) -> audionimbus_sys::IPLReflectionEffectType;

    /// Returns the native effect settings.
    fn ffi_settings(
        self,
        settings: &ReflectionEffectSettings,
    ) -> audionimbus_sys::IPLReflectionEffectSettings;

    /// Returns the required output channels.
    fn num_output_channels(self, settings: &ReflectionEffectSettings) -> ChannelRequirement;
}

impl<T: ReflectionEffectType> ReflectionBinding for Static<T> {
    fn ffi_type(self) -> audionimbus_sys::IPLReflectionEffectType {
        T::to_ffi_type()
    }

    fn ffi_settings(
        self,
        settings: &ReflectionEffectSettings,
    ) -> audionimbus_sys::IPLReflectionEffectSettings {
        T::ffi_settings(settings)
    }

    fn num_output_channels(self, settings: &ReflectionEffectSettings) -> ChannelRequirement {
        T::num_output_channels(settings)
    }
}

/// A reflection algorithm selected at construction.
#[cfg(feature = "dynamic")]
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

#[cfg(feature = "dynamic")]
impl ReflectionBinding for Dynamic<ReflectionAlgorithm> {
    fn ffi_type(self) -> audionimbus_sys::IPLReflectionEffectType {
        match self.0 {
            ReflectionAlgorithm::Convolution => Convolution::to_ffi_type(),
            ReflectionAlgorithm::Parametric => Parametric::to_ffi_type(),
            ReflectionAlgorithm::Hybrid => Hybrid::to_ffi_type(),
            ReflectionAlgorithm::TrueAudioNext => TrueAudioNext::to_ffi_type(),
        }
    }

    fn ffi_settings(
        self,
        settings: &ReflectionEffectSettings,
    ) -> audionimbus_sys::IPLReflectionEffectSettings {
        audionimbus_sys::IPLReflectionEffectSettings {
            type_: self.ffi_type(),
            irSize: settings.impulse_response_size as i32,
            numChannels: settings.num_channels as i32,
        }
    }

    fn num_output_channels(self, settings: &ReflectionEffectSettings) -> ChannelRequirement {
        match self.0 {
            ReflectionAlgorithm::Parametric => ChannelRequirement::AtLeast(1),
            _ => ChannelRequirement::Exactly(settings.num_channels),
        }
    }
}
