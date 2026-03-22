use crate::error::SteamAudioError;
use crate::simulation::{ParameterValidationError, SimulationError};

mod direct;
pub use direct::*;
mod pathing;
pub use pathing::*;
mod reflections;
pub use reflections::*;
mod reflections_reverb;
pub use reflections_reverb::*;

/// Defines the simulation logic for a single step.
pub trait SimulationStep<I>: Send + 'static {
    type Output: Send + Sync + 'static;
    type Error: std::error::Error;

    fn run(&mut self, input: &I, output: &mut Self::Output) -> Result<(), Self::Error>;
}

/// Errors that can occur during a simulation step.
#[derive(Debug)]
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
