use super::super::{AsDirectInput, DirectInput, DirectInputOwned, SourceWithInputs};
use super::Allocate;
use crate::effect::DirectEffectParams;
use crate::simulation::{Direct, ReflectionEffectCompatible, SimulationSharedInputs};
use object_pool::ReusableOwned;
use std::sync::Arc;

/// Input frame for direct simulation.
pub struct DirectFrame<D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    pub sources: Arc<ReusableOwned<Vec<SourceWithInputs<D, R, P, RE>>>>,
    pub shared_inputs: SimulationSharedInputs<D, R, P>,
}

impl<D, R, P, RE> AsDirectInput<D, R, P, RE> for DirectFrame<D, R, P, RE>
where
    D: Send + Sync + 'static,
    R: Send + Sync + 'static,
    P: Send + Sync + 'static,
    RE: Send + Sync + 'static + ReflectionEffectCompatible<R, RE>,
{
    fn as_direct_input(&self) -> DirectInput<'_, D, R, P, RE> {
        DirectInput {
            sources: self.sources.as_slice(),
            shared_inputs: &self.shared_inputs,
        }
    }
}

impl<R, P, RE> Allocate<DirectFrame<Direct, R, P, RE>> for Vec<DirectEffectParams>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    fn allocate(input: &DirectFrame<Direct, R, P, RE>) -> Self {
        Self::with_capacity(input.sources.len())
    }
}

impl<D, R, P, RE> Allocate<DirectInputOwned<D, R, P, RE>> for Vec<DirectEffectParams>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    fn allocate(input: &DirectInputOwned<D, R, P, RE>) -> Self {
        Self::with_capacity(input.sources.len())
    }
}
