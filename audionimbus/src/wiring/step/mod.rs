//! Simulation step implementations and their input/output types.
//!
//! Each step type ([`DirectStep`], [`ReflectionsStep`], [`ReflectionsReverbStep`],
//! [`PathingStep`]) implements [`SimulationStep`].

use crate::error::SteamAudioError;
use crate::simulation::ParameterValidationError;
use crate::simulation::error::SimulationError;
use crate::wiring::simulation::pipeline::StaticSourceWithInputs as SourceWithInputs;

/// Bindings must stay inaccessible to prevent mixing static and runtime handles.
pub(crate) mod direct;
pub use direct::{AsDirectInput, DirectInput, DirectInputOwned, StaticDirectStep as DirectStep};
/// Bindings must stay inaccessible to prevent mixing static and runtime handles.
pub(crate) mod pathing;
pub use pathing::{
    AsPathingInput, PathingInput, PathingInputOwned, StaticPathingStep as PathingStep,
};
/// Bindings must stay inaccessible to prevent mixing static and runtime handles.
pub(crate) mod reflections;
#[cfg(all(feature = "dynamic", not(feature = "static")))]
pub use reflections::DynamicReflectionsOutput as ReflectionsOutput;
#[cfg(any(feature = "static", not(feature = "dynamic")))]
pub use reflections::StaticReflectionsOutput as ReflectionsOutput;
pub use reflections::{
    AsReflectionsInput, ReflectionsInput, ReflectionsInputOwned,
    StaticReflectionsStep as ReflectionsStep,
};
/// Bindings must stay inaccessible to prevent mixing static and runtime handles.
pub(crate) mod reflections_reverb;
#[cfg(all(feature = "dynamic", not(feature = "static")))]
pub use reflections_reverb::DynamicReflectionsReverbOutput as ReflectionsReverbOutput;
#[cfg(any(feature = "static", not(feature = "dynamic")))]
pub use reflections_reverb::StaticReflectionsReverbOutput as ReflectionsReverbOutput;
pub use reflections_reverb::{
    AsReflectionsReverbInput, ReflectionsReverbInput, ReflectionsReverbInputOwned,
    StaticReflectionsReverbStep as ReflectionsReverbStep,
};

/// Defines the simulation logic for a single step.
pub trait SimulationStep<I>: Send + 'static {
    /// The output type produced by this step.
    type Output: Send + Sync + 'static;

    /// The error type returned on step failure.
    type Error: std::error::Error;

    /// Runs one iteration of the simulation step.
    fn run(&mut self, input: &I, output: &mut Self::Output) -> Result<(), Self::Error>;
}

/// Errors that can occur during a simulation step.
#[derive(Copy, Clone, Debug)]
pub enum SimulationStepError {
    /// Parameter validation error.
    ParameterValidation(ParameterValidationError),
    /// Generic Steam Audio error.
    SteamAudio(SteamAudioError),
    /// Simulation error.
    Simulation(SimulationError),
}

impl std::error::Error for SimulationStepError {}

impl std::fmt::Display for SimulationStepError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ParameterValidation(error) => write!(f, "parameter validation error: {error}"),
            Self::SteamAudio(error) => write!(f, "Steam Audio error: {error}"),
            Self::Simulation(e) => write!(f, "simulation error: {e}"),
        }
    }
}

impl From<ParameterValidationError> for SimulationStepError {
    fn from(error: ParameterValidationError) -> Self {
        Self::ParameterValidation(error)
    }
}

impl From<SteamAudioError> for SimulationStepError {
    fn from(error: SteamAudioError) -> Self {
        Self::SteamAudio(error)
    }
}

impl From<SimulationError> for SimulationStepError {
    fn from(error: SimulationError) -> Self {
        Self::Simulation(error)
    }
}

/// A slice of sources paired with their simulation inputs.
pub type SourceEntries<SourceId, D, R, P, RE> = [(SourceId, SourceWithInputs<D, R, P, RE>)];
