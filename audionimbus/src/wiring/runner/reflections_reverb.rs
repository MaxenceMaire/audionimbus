use super::super::{
    AsReflectionsReverbInput, ReflectionsReverbInput, ReflectionsReverbInputOwned,
    ReflectionsReverbOutput, SourceWithInputs,
};
use super::{Allocate, Clear, Shrink};
use crate::effect::ReflectionEffectType;
use crate::simulation::{ReflectionEffectCompatible, Reflections, SimulationSharedInputs};
use object_pool::ReusableOwned;
use std::sync::Arc;

/// Input frame for reflections and reverb simulation.
pub struct ReflectionsReverbFrame<D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    pub sources: Arc<ReusableOwned<Vec<SourceWithInputs<D, R, P, RE>>>>,
    pub listener: SourceWithInputs<(), R, (), RE>,
    pub shared_inputs: SimulationSharedInputs<D, R, P>,
}

impl<D, R, P, RE> AsReflectionsReverbInput<D, R, P, RE> for ReflectionsReverbFrame<D, R, P, RE>
where
    D: Send + Sync + 'static,
    R: Send + Sync + 'static,
    P: Send + Sync + 'static,
    RE: Send + Sync + 'static + ReflectionEffectCompatible<R, RE>,
{
    fn as_reflections_reverb_input(&self) -> ReflectionsReverbInput<'_, D, R, P, RE> {
        ReflectionsReverbInput {
            sources: self.sources.as_slice(),
            listener: &self.listener,
            shared_inputs: &self.shared_inputs,
        }
    }
}

impl<D, P, RE> Allocate<ReflectionsReverbFrame<D, Reflections, P, RE>>
    for ReflectionsReverbOutput<RE>
where
    RE: ReflectionEffectType + ReflectionEffectCompatible<Reflections, RE>,
{
    fn allocate(input: &ReflectionsReverbFrame<D, Reflections, P, RE>) -> Self {
        Self {
            sources: Vec::with_capacity(input.sources.len()),
            listener: None,
        }
    }
}

impl<D, P, RE> Allocate<ReflectionsReverbInputOwned<D, Reflections, P, RE>>
    for ReflectionsReverbOutput<RE>
where
    RE: ReflectionEffectType + ReflectionEffectCompatible<Reflections, RE>,
{
    fn allocate(input: &ReflectionsReverbInputOwned<D, Reflections, P, RE>) -> Self {
        Self {
            sources: Vec::with_capacity(input.sources.len()),
            listener: None,
        }
    }
}

impl<RE: ReflectionEffectType> Clear for ReflectionsReverbOutput<RE> {
    fn clear(&mut self) {
        self.sources.clear();
        self.listener = None;
    }
}

impl<RE: ReflectionEffectType> Shrink for ReflectionsReverbOutput<RE> {
    fn shrink(&mut self) {
        if self.sources.capacity() > self.sources.len() * 3 {
            self.sources.shrink_to_fit();
        }
    }
}
