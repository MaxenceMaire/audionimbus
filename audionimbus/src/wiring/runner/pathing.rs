use super::super::{AsPathingInput, PathingInput, PathingInputOwned, SharedSources, SourcesGuard};
use super::{Allocate, Resolve};
use crate::effect::PathEffectParams;
use crate::simulation::{Pathing, ReflectionEffectCompatible, SimulationSharedInputs};

#[cfg(doc)]
use super::super::Simulation;

/// Input frame for pathing simulation.
pub struct PathingFrame<D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    /// Shared reference to the current frame of sources, written by the game thread via
    /// [`Simulation::update`].
    pub sources: SharedSources<D, R, P, RE>,
    /// Shared simulation inputs.
    pub shared_inputs: SimulationSharedInputs<D, R, P>,
}

impl<D, R, RE> Allocate<PathingInputOwned<D, R, Pathing, RE>> for Vec<PathEffectParams>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    fn allocate(input: &PathingInputOwned<D, R, Pathing, RE>) -> Self {
        Self::with_capacity(input.sources.len())
    }
}

impl<D, R, P, RE> Resolve for PathingFrame<D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    type Resolved<'a>
        = ResolvedPathingFrame<'a, D, R, P, RE>
    where
        Self: 'a;

    fn resolve(&self) -> Self::Resolved<'_> {
        ResolvedPathingFrame {
            guard: self.sources.load(),
            shared_inputs: &self.shared_inputs,
        }
    }
}

/// A snapshot of a [`PathingFrame`] for use during a single simulation step.
///
/// Keeps the sources alive for the duration of the step.
pub struct ResolvedPathingFrame<'a, D, R, P, RE> {
    guard: SourcesGuard<D, R, P, RE>,
    shared_inputs: &'a SimulationSharedInputs<D, R, P>,
}

impl<D, R, P, RE> AsPathingInput<D, R, P, RE> for ResolvedPathingFrame<'_, D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    fn as_pathing_input(&self) -> PathingInput<'_, D, R, P, RE> {
        PathingInput {
            sources: &self.guard,
            shared_inputs: self.shared_inputs,
        }
    }
}

impl<D, R, RE> Allocate<ResolvedPathingFrame<'_, D, R, Pathing, RE>> for Vec<PathEffectParams>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    fn allocate(input: &ResolvedPathingFrame<'_, D, R, Pathing, RE>) -> Self {
        Self::with_capacity(input.guard.len())
    }
}
