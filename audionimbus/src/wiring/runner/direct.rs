use super::super::simulation::SharedSources;
use super::super::step::{AsDirectInput, DirectInput};
use super::{Allocate, Resolve, SourcesGuard};
use crate::effect::DirectEffectParams;
use crate::simulation::{Direct, ReflectionEffectCompatible, SimulationSharedInputs};

#[cfg(doc)]
use super::super::simulation::Simulation;

/// Input frame for direct simulation.
pub struct DirectFrame<SourceId, D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    /// Shared reference to the current frame of sources, written by the game thread via
    /// [`Simulation::update`].
    pub sources: SharedSources<SourceId, D, R, P, RE>,
    /// Shared simulation inputs.
    pub shared_inputs: SimulationSharedInputs<D, R, P>,
}

impl<SourceId, D, R, P, RE> Resolve for DirectFrame<SourceId, D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    type Resolved<'a>
        = ResolvedDirectFrame<'a, SourceId, D, R, P, RE>
    where
        Self: 'a;

    fn resolve(&self) -> Self::Resolved<'_> {
        ResolvedDirectFrame {
            guard: self.sources.load(),
            shared_inputs: &self.shared_inputs,
        }
    }
}

/// A snapshot of a [`DirectFrame`] for use during a single simulation step.
///
/// Keeps the current sources alive for the duration of the step.
pub struct ResolvedDirectFrame<'a, SourceId, D, R, P, RE> {
    guard: SourcesGuard<SourceId, D, R, P, RE>,
    shared_inputs: &'a SimulationSharedInputs<D, R, P>,
}

impl<SourceId, D, R, P, RE> AsDirectInput<SourceId, D, R, P, RE>
    for ResolvedDirectFrame<'_, SourceId, D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    fn as_direct_input(&self) -> DirectInput<'_, SourceId, D, R, P, RE> {
        DirectInput {
            sources: &self.guard,
            shared_inputs: self.shared_inputs,
        }
    }
}

impl<SourceId, R, P, RE> Allocate<ResolvedDirectFrame<'_, SourceId, Direct, R, P, RE>>
    for Vec<(SourceId, DirectEffectParams)>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    fn allocate(input: &ResolvedDirectFrame<'_, SourceId, Direct, R, P, RE>) -> Self {
        Self::with_capacity(input.guard.len())
    }
}
