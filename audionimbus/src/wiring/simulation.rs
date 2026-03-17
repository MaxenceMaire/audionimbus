use crate::simulation::{SimulationInputs, Source};

/// A pair of source and simulation inputs.
#[derive(Clone, Debug)]
pub struct SourceWithInputs<D, R, P, RE> {
    /// Spatial audio source.
    pub source: Source<D, R, P, RE>,
    /// Simulation inputs for the associated source.
    pub simulation_inputs: SimulationInputs<D, R, P>,
}
