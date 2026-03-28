use super::super::simulation::SharedSources;
use super::super::step::{AsPathingInput, PathingInput, PathingInputOwned};
use super::{Allocate, Resolve, SourcesGuard};
use crate::effect::PathEffectParams;
use crate::simulation::{Pathing, ReflectionEffectCompatible, SimulationSharedInputs};

#[cfg(doc)]
use super::super::simulation::Simulation;

/// Input frame for pathing simulation.
pub struct PathingFrame<SourceId, D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    /// Shared reference to the current frame of sources, written by the game thread via
    /// [`Simulation::update_sources`].
    pub sources: SharedSources<SourceId, D, R, P, RE>,
    /// Shared simulation inputs.
    pub shared_inputs: SimulationSharedInputs<D, R, P>,
}

impl<SourceId, D, R, RE> Allocate<PathingInputOwned<SourceId, D, R, Pathing, RE>>
    for Vec<(SourceId, PathEffectParams)>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    fn allocate(input: &PathingInputOwned<SourceId, D, R, Pathing, RE>) -> Self {
        Self::with_capacity(input.sources.len())
    }
}

impl<SourceId, D, R, P, RE> Resolve for PathingFrame<SourceId, D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    type Resolved<'a>
        = ResolvedPathingFrame<'a, SourceId, D, R, P, RE>
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
pub struct ResolvedPathingFrame<'a, SourceId, D, R, P, RE> {
    guard: SourcesGuard<SourceId, D, R, P, RE>,
    shared_inputs: &'a SimulationSharedInputs<D, R, P>,
}

impl<SourceId, D, R, P, RE> AsPathingInput<SourceId, D, R, P, RE>
    for ResolvedPathingFrame<'_, SourceId, D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    fn as_pathing_input(&self) -> PathingInput<'_, SourceId, D, R, P, RE> {
        PathingInput {
            sources: &self.guard,
            shared_inputs: self.shared_inputs,
        }
    }
}

impl<SourceId, D, R, RE> Allocate<ResolvedPathingFrame<'_, SourceId, D, R, Pathing, RE>>
    for Vec<(SourceId, PathEffectParams)>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    fn allocate(input: &ResolvedPathingFrame<'_, SourceId, D, R, Pathing, RE>) -> Self {
        Self::with_capacity(input.guard.len())
    }
}
