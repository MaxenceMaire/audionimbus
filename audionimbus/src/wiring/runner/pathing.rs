use super::super::{AsPathingInput, PathingInput, PathingInputOwned, SourceWithInputs};
use super::Allocate;
use crate::effect::PathEffectParams;
use crate::simulation::{Pathing, ReflectionEffectCompatible, SimulationSharedInputs};
use object_pool::ReusableOwned;
use std::sync::Arc;

/// Input frame for pathing simulation.
pub struct PathingFrame<D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    pub sources: Arc<ReusableOwned<Vec<SourceWithInputs<D, R, P, RE>>>>,
    pub shared_inputs: SimulationSharedInputs<D, R, P>,
}

impl<D, R, P, RE> AsPathingInput<D, R, P, RE> for PathingFrame<D, R, P, RE>
where
    D: Send + Sync + 'static,
    R: Send + Sync + 'static,
    P: Send + Sync + 'static,
    RE: Send + Sync + 'static + ReflectionEffectCompatible<R, RE>,
{
    fn as_pathing_input(&self) -> PathingInput<'_, D, R, P, RE> {
        PathingInput {
            sources: self.sources.as_slice(),
            shared_inputs: &self.shared_inputs,
        }
    }
}

impl<D, R, RE> Allocate<PathingFrame<D, R, Pathing, RE>> for Vec<PathEffectParams>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    fn allocate(input: &PathingFrame<D, R, Pathing, RE>) -> Self {
        Self::with_capacity(input.sources.len())
    }
}

impl<D, R, RE> Allocate<PathingInputOwned<D, R, Pathing, RE>> for Vec<PathEffectParams>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    fn allocate(input: &PathingInputOwned<D, R, Pathing, RE>) -> Self {
        Self::with_capacity(input.sources.len())
    }
}
