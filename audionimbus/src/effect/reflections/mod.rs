//! Room acoustics and reverberation effects.

/// Bindings must stay inaccessible to prevent mixing static and runtime effect handles.
pub(crate) mod binding;
pub use binding::{
    CanApplyDirectly, CanUseReflectionMixer, Convolution, Hybrid, Parametric, ReflectionEffectType,
    TrueAudioNext,
};

/// Effect implementations whose bindings must stay inaccessible to downstream callers.
pub(crate) mod effect;
pub use effect::{
    ReflectionEffectIR, ReflectionEffectSettings, StaticReflectionEffect as ReflectionEffect,
    StaticReflectionEffectParams as ReflectionEffectParams,
};

/// Mixer implementations whose bindings must stay inaccessible to downstream callers.
pub(crate) mod mixer;
pub use mixer::StaticReflectionMixer as ReflectionMixer;
