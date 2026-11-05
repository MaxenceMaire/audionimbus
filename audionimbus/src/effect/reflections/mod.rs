//! Room acoustics and reverberation effects.

/// Bindings must stay inaccessible to prevent mixing static and runtime effect handles.
pub(crate) mod binding;
pub use binding::{
    CanApplyDirectly, CanUseReflectionMixer, Convolution, Hybrid, Parametric, ReflectionEffectType,
    TrueAudioNext,
};

/// Effect implementations whose bindings must stay inaccessible to downstream callers.
pub(crate) mod effect;
#[cfg(all(feature = "dynamic", not(feature = "static")))]
pub use effect::{
    DynamicReflectionEffect as ReflectionEffect,
    DynamicReflectionEffectParams as ReflectionEffectParams,
};
pub use effect::{ReflectionEffectIR, ReflectionEffectSettings};
#[cfg(any(feature = "static", not(feature = "dynamic")))]
pub use effect::{
    StaticReflectionEffect as ReflectionEffect,
    StaticReflectionEffectParams as ReflectionEffectParams,
};

/// Mixer implementations whose bindings must stay inaccessible to downstream callers.
pub(crate) mod mixer;
#[cfg(all(feature = "dynamic", not(feature = "static")))]
pub use mixer::DynamicReflectionMixer as ReflectionMixer;
#[cfg(any(feature = "static", not(feature = "dynamic")))]
pub use mixer::StaticReflectionMixer as ReflectionMixer;

#[cfg(feature = "dynamic")]
pub use binding::ReflectionAlgorithm;
#[cfg(feature = "dynamic")]
pub use effect::ReflectionError;
