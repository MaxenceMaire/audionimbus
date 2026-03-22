use super::super::{
    AsReflectionsInput, ReflectionsInput, ReflectionsInputOwned, ReflectionsOutput,
    SourceWithInputs,
};
use super::{Allocate, Clear, Resolve, Shrink};
use crate::effect::ReflectionEffectType;
use crate::simulation::{ReflectionEffectCompatible, Reflections, SimulationSharedInputs};
use arc_swap::ArcSwap;
use object_pool::ReusableOwned;
use std::sync::Arc;

/// Input frame for reflections simulation.
pub struct ReflectionsFrame<D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    pub sources: Arc<ArcSwap<ReusableOwned<Vec<SourceWithInputs<D, R, P, RE>>>>>,
    pub shared_inputs: SimulationSharedInputs<D, R, P>,
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
        if self.sources.capacity() > self.sources.len() * 3 {
            self.sources.shrink_to_fit();
        }
    }
}

impl<D, R, P, RE> Resolve for ReflectionsFrame<D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    type Resolved<'a>
        = ResolvedReflectionsFrame<'a, D, R, P, RE>
    where
        Self: 'a;

    fn resolve(&self) -> Self::Resolved<'_> {
        ResolvedReflectionsFrame {
            guard: self.sources.load(),
            shared_inputs: &self.shared_inputs,
        }
    }
}

pub struct ResolvedReflectionsFrame<'a, D, R, P, RE> {
    guard: arc_swap::Guard<Arc<ReusableOwned<Vec<SourceWithInputs<D, R, P, RE>>>>>,
    shared_inputs: &'a SimulationSharedInputs<D, R, P>,
}

impl<D, R, P, RE> AsReflectionsInput<D, R, P, RE> for ResolvedReflectionsFrame<'_, D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    fn as_reflections_input(&self) -> ReflectionsInput<'_, D, R, P, RE> {
        ReflectionsInput {
            sources: &self.guard,
            shared_inputs: self.shared_inputs,
        }
    }
}

impl<D, P, RE> Allocate<ResolvedReflectionsFrame<'_, D, Reflections, P, RE>>
    for ReflectionsOutput<RE>
where
    RE: ReflectionEffectType + ReflectionEffectCompatible<Reflections, RE>,
{
    fn allocate(input: &ResolvedReflectionsFrame<'_, D, Reflections, P, RE>) -> Self {
        Self {
            sources: Vec::with_capacity(input.guard.len()),
        }
    }
}
