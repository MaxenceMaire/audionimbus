use super::super::{
    AsReflectionsReverbInput, ReflectionsReverbInput, ReflectionsReverbOutput, SourceWithInputs,
};
use super::{Allocate, Clear, Resolve, Shrink};
use crate::effect::ReflectionEffectType;
use crate::simulation::{ReflectionEffectCompatible, Reflections, SimulationSharedInputs};
use arc_swap::ArcSwap;
use object_pool::ReusableOwned;
use std::sync::Arc;

#[cfg(doc)]
use super::super::Simulation;

/// Input frame for reflections and reverb simulation.
pub struct ReflectionsReverbFrame<D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    /// Shared reference to the current frame of sources, written by the game thread via
    /// [`Simulation::update`].
    pub sources: Arc<ArcSwap<ReusableOwned<Vec<SourceWithInputs<D, R, P, RE>>>>>,
    /// Listener source.
    pub listener: SourceWithInputs<(), R, (), RE>,
    /// Shared simulation inputs.
    pub shared_inputs: SimulationSharedInputs<D, R, P>,
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

impl<D, R, P, RE> Resolve for ReflectionsReverbFrame<D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    type Resolved<'a>
        = ResolvedReflectionsReverbFrame<'a, D, R, P, RE>
    where
        Self: 'a;

    fn resolve(&self) -> Self::Resolved<'_> {
        ResolvedReflectionsReverbFrame {
            guard: self.sources.load(),
            listener: &self.listener,
            shared_inputs: &self.shared_inputs,
        }
    }
}

/// A snapshot of a [`ReflectionsReverbFrame`] for use during a single simulation step.
///
/// Keeps the sources alive for the duration of the step.
pub struct ResolvedReflectionsReverbFrame<'a, D, R, P, RE> {
    guard: arc_swap::Guard<Arc<ReusableOwned<Vec<SourceWithInputs<D, R, P, RE>>>>>,
    listener: &'a SourceWithInputs<(), R, (), RE>,
    shared_inputs: &'a SimulationSharedInputs<D, R, P>,
}

impl<D, R, P, RE> AsReflectionsReverbInput<D, R, P, RE>
    for ResolvedReflectionsReverbFrame<'_, D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    fn as_reflections_reverb_input(&self) -> ReflectionsReverbInput<'_, D, R, P, RE> {
        ReflectionsReverbInput {
            sources: &self.guard,
            listener: self.listener,
            shared_inputs: self.shared_inputs,
        }
    }
}

impl<D, P, RE> Allocate<ResolvedReflectionsReverbFrame<'_, D, Reflections, P, RE>>
    for ReflectionsReverbOutput<RE>
where
    RE: ReflectionEffectType + ReflectionEffectCompatible<Reflections, RE>,
{
    fn allocate(input: &ResolvedReflectionsReverbFrame<'_, D, Reflections, P, RE>) -> Self {
        Self {
            sources: Vec::with_capacity(input.guard.len()),
            listener: None,
        }
    }
}
