use super::super::{
    AsReflectionsInput, ReflectionsInput, ReflectionsInputOwned, ReflectionsOutput,
    SourceWithInputs,
};
use super::{Allocate, Clear, Shrink};
use crate::effect::ReflectionEffectType;
use crate::simulation::{ReflectionEffectCompatible, Reflections, SimulationSharedInputs};
use object_pool::ReusableOwned;
use std::sync::Arc;

/// Input frame for reflections simulation.
pub struct ReflectionsFrame<D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    pub sources: Arc<ReusableOwned<Vec<SourceWithInputs<D, R, P, RE>>>>,
    pub listener: SourceWithInputs<D, R, P, RE>,
    pub shared_inputs: SimulationSharedInputs<D, R, P>,
}

impl<D, R, P, RE> AsReflectionsInput<D, R, P, RE> for ReflectionsFrame<D, R, P, RE>
where
    D: Send + Sync + 'static,
    R: Send + Sync + 'static,
    P: Send + Sync + 'static,
    RE: Send + Sync + 'static + ReflectionEffectCompatible<R, RE>,
{
    fn as_reflections_input(&self) -> ReflectionsInput<'_, D, R, P, RE> {
        ReflectionsInput {
            sources: self.sources.as_slice(),
            shared_inputs: &self.shared_inputs,
        }
    }
}

impl<D, P, RE> Allocate<ReflectionsFrame<D, Reflections, P, RE>> for ReflectionsOutput<RE>
where
    RE: ReflectionEffectType + ReflectionEffectCompatible<Reflections, RE>,
{
    fn allocate(input: &ReflectionsFrame<D, Reflections, P, RE>) -> Self {
        Self {
            sources: Vec::with_capacity(input.sources.len()),
        }
    }
}

impl<D, P, RE> Allocate<ReflectionsInputOwned<D, Reflections, P, RE>> for ReflectionsOutput<RE>
where
    RE: ReflectionEffectType + ReflectionEffectCompatible<Reflections, RE>,
{
    fn allocate(input: &ReflectionsInputOwned<D, Reflections, P, RE>) -> Self {
        Self {
            sources: Vec::with_capacity(input.sources.len()),
        }
    }
}

impl<RE: ReflectionEffectType> Clear for ReflectionsOutput<RE> {
    fn clear(&mut self) {
        self.sources.clear();
    }
}

impl<RE: ReflectionEffectType> Shrink for ReflectionsOutput<RE> {
    fn shrink(&mut self) {
        if self.sources.capacity() > self.sources.len() * 3 {}
    }
}
