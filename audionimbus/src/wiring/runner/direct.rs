use super::super::{AsDirectInput, DirectInput, SourceWithInputs};
use super::{Allocate, Resolve};
use crate::effect::DirectEffectParams;
use crate::simulation::{Direct, ReflectionEffectCompatible, SimulationSharedInputs};
use arc_swap::ArcSwap;
use object_pool::ReusableOwned;
use std::sync::Arc;

#[cfg(doc)]
use super::super::Simulation;

/// Input frame for direct simulation.
pub struct DirectFrame<D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    /// Shared reference to the current frame of sources, written by the game thread via
    /// [`Simulation::update`].
    pub sources: Arc<ArcSwap<ReusableOwned<Vec<SourceWithInputs<D, R, P, RE>>>>>,
    /// Shared simulation inputs.
    pub shared_inputs: SimulationSharedInputs<D, R, P>,
}

impl<D, R, P, RE> Resolve for DirectFrame<D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    type Resolved<'a>
        = ResolvedDirectFrame<'a, D, R, P, RE>
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
pub struct ResolvedDirectFrame<'a, D, R, P, RE> {
    guard: arc_swap::Guard<Arc<ReusableOwned<Vec<SourceWithInputs<D, R, P, RE>>>>>,
    shared_inputs: &'a SimulationSharedInputs<D, R, P>,
}

impl<D, R, P, RE> AsDirectInput<D, R, P, RE> for ResolvedDirectFrame<'_, D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    fn as_direct_input(&self) -> DirectInput<'_, D, R, P, RE> {
        DirectInput {
            sources: &self.guard,
            shared_inputs: self.shared_inputs,
        }
    }
}

impl<R, P, RE> Allocate<ResolvedDirectFrame<'_, Direct, R, P, RE>> for Vec<DirectEffectParams>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    fn allocate(input: &ResolvedDirectFrame<'_, Direct, R, P, RE>) -> Self {
        Self::with_capacity(input.guard.len())
    }
}
