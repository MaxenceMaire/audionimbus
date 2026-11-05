//! Simulation prerequisites and parameter validation errors.

/// Errors that can occur during simulation operations.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum SimulationError {
    /// Attempted to run pathing simulation without any probe batches committed.
    ///
    /// Pathing requires at least one probe batch to be added to the simulator
    /// via `Simulator::add_probe_batch` and committed via `Simulator::commit`
    /// before running simulations.
    PathingWithoutProbes,

    /// Attempted to run reflection simulation without a scene set.
    ///
    /// Reflection simulation requires a [`crate::geometry::Scene`] to be set on the simulator via
    /// `Simulator::set_scene` and committed via `Simulator::commit` before
    /// running simulations.
    ReflectionsWithoutScene,
}

impl std::error::Error for SimulationError {}

impl std::fmt::Display for SimulationError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match &self {
            Self::PathingWithoutProbes => {
                write!(f, "running pathing on a simulator with no probes")
            }
            Self::ReflectionsWithoutScene => {
                write!(f, "running reflections on a simulator with no scene set")
            }
        }
    }
}

/// Errors that can occur during parameter validation.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ParameterValidationError {
    /// Number of occlusion samples exceeds the maximum set during initialization.
    OcclusionSamplesExceedsMax {
        /// The requested number of samples.
        requested: u32,
        /// The maximum allowed number of samples.
        max: u32,
    },

    /// Number of rays exceeds the maximum set during initialization.
    NumRaysExceedsMax {
        /// The requested number of rays.
        requested: u32,
        /// The maximum allowed number of rays.
        max: u32,
    },

    /// Duration exceeds the maximum set during initialization.
    DurationExceedsMax {
        /// The requested duration in seconds.
        requested: f32,
        /// The maximum allowed duration in seconds.
        max: f32,
    },
}

impl std::error::Error for ParameterValidationError {}

impl std::fmt::Display for ParameterValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::OcclusionSamplesExceedsMax { requested, max } => {
                write!(
                    f,
                    "requested {} occlusion samples, but maximum is {} (set during simulator initialization)",
                    requested, max
                )
            }
            Self::NumRaysExceedsMax { requested, max } => {
                write!(
                    f,
                    "requested {} rays, but maximum is {} (set during simulator initialization)",
                    requested, max
                )
            }
            Self::DurationExceedsMax { requested, max } => {
                write!(
                    f,
                    "requested duration of {}s, but maximum is {}s (set during simulator initialization)",
                    requested, max
                )
            }
        }
    }
}
