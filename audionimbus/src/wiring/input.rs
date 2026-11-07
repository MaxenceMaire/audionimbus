//! Borrowed inputs used by shared simulation steps.

use super::binding::InputBinding;
use super::simulation::pipeline::SourceWithInputs;
use crate::simulation::simulator::SimulationSharedInputs;

/// Sources, listener, and shared inputs retained during one worker iteration.
pub struct StepInput<'a, SourceId, B: InputBinding, L: InputBinding = B> {
    /// The selected source list.
    pub sources: &'a [(SourceId, SourceWithInputs<B>)],
    /// The optional listener source.
    pub listener: Option<&'a SourceWithInputs<L>>,
    /// The shared listener and simulation parameters.
    pub shared_inputs: &'a SimulationSharedInputs<B::Inputs>,
}

/// A borrowed frame accepted by one simulation mode.
pub trait InputView<SourceId, B: InputBinding, Mode> {
    /// The listener's source binding.
    type Listener: InputBinding;

    /// Returns the inputs retained for this iteration.
    fn input(&self) -> StepInput<'_, SourceId, B, Self::Listener>;
}

/// The listener-centric reflection input mode.
pub struct Reverb;
