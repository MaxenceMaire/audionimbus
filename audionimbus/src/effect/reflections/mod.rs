//! Room acoustics and reverberation effects.

mod binding;
pub use binding::{
    CanApplyDirectly, CanUseReflectionMixer, Convolution, Hybrid, Parametric, ReflectionEffectType,
    TrueAudioNext,
};

/// Effect implementations whose bindings must stay inaccessible to downstream callers.
pub(crate) mod effect;
pub use effect::{
    ReflectionEffectIR, ReflectionEffectSettings, TypedReflectionEffect as ReflectionEffect,
    TypedReflectionEffectParams as ReflectionEffectParams,
};

/// Mixer implementations whose bindings must stay inaccessible to downstream callers.
pub(crate) mod mixer;
pub use mixer::TypedReflectionMixer as ReflectionMixer;
