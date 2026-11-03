//! Spatial audio simulation (direct simulation, reflections, pathing).
//!
//! # Multi-Threading Architecture
//!
//! Simulations are designed to run on separate threads from audio processing.
//! Different simulation types can run concurrently without blocking each other.
//! For example, `Simulator::run_direct` and `Simulator::run_reflections` can be called from
//! separate threads simultaneously.
//!
//! Running a simulation may take 50-500ms.
//! For this reason, a typical architecture is to run a dedicated simulation thread (or one per
//! simulation type) alongside the main real-time audio thread.
//! Simulation outputs are then passed down from the simulation thread(s) to the audio thread via
//! lock-free communication (triple buffer, lock-free queue).
//!
//! ```text
//! ┌────────────────────────────┐       ┌──────────────────┐       ┌─────────────────┐
//! │ Simulation thread(s)       │       │ Lock-free        │       │ Audio thread    │
//! │                            │──────▶│ communication    │──────▶│ (real-time)     │
//! │                            │       │                  │       │                 │
//! │ Source::set_inputs         │       │ Triple buffer    │       │ Apply effects   │
//! │ Simulator::run_direct      │       │ or               │       │                 │
//! │ Simulator::run_reflections │       │ lock-free queue  │       │                 │
//! │ Simulator::run_pathing     │       │                  │       │                 │
//! │ Source::get_outputs        │       │                  │       │                 │
//! └────────────────────────────┘       └──────────────────┘       └─────────────────┘
//!  Can take 50-500ms                    Non-blocking               Must complete in
//!  Can block                                                       a few ms
//! ```
//!
//! Never set inputs (`Source::set_inputs`, `Source::set_inputs_subset`, etc.) or retrieve
//! outputs (`Source::get_outputs`, `Source::get_outputs_subset`, etc.) from an audio thread as
//! it will block and cause audio glitches.
//!
//! Refer to the `wiring` feature to help set up this architecture and avoid common pitfalls.

/// Implementations whose bindings must stay inaccessible to downstream callers.
pub(crate) mod simulator;
pub use simulator::{
    PathingVisualizationCallback, ReflectionsSharedInputs,
    TypedSimulationSharedInputs as SimulationSharedInputs, TypedSimulator as Simulator,
};

/// Implementations whose bindings must stay inaccessible to downstream callers.
pub(crate) mod source;
pub use source::{TypedSimulationOutputs as SimulationOutputs, TypedSource as Source};

/// Implementations whose bindings must stay inaccessible to downstream callers.
pub(crate) mod settings;
pub use settings::{
    ConvolutionSettings, DirectSimulationSettings, HybridSettings, ParametricSettings,
    PathingSimulationSettings, ReflectionsAlgorithm, TrueAudioNextSettings,
    TypedSimulationSettings as SimulationSettings,
};

/// Implementations whose bindings must stay inaccessible to downstream callers.
pub(crate) mod inputs;
pub use inputs::{
    ConvolutionParameters, DirectSimulationParameters, HybridParameters, Occlusion,
    OcclusionAlgorithm, ParametricParameters, PathingSimulationParameters,
    ReflectionsSimulationParameters, SimulationParameters, TransmissionParameters,
    TrueAudioNextParameters, TypedSimulationInputs as SimulationInputs,
};

mod modes;
pub use modes::{
    Direct, DirectCompatible, Pathing, PathingCompatible, ReflectionEffectCompatible, Reflections,
    ReflectionsCompatible, SimulationFlags, SimulationFlagsProvider, SimulationType,
};

mod error;
pub use error::{ParameterValidationError, SimulationError};
