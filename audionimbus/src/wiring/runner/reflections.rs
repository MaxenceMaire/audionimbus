use super::super::simulation::SharedSources;
use super::super::step::{
    AsReflectionsInput, ReflectionsInput, ReflectionsInputOwned, ReflectionsOutput,
};
use super::{Allocate, Clear, Resolve, Shrink, SourcesGuard};
use crate::effect::ReflectionEffectType;
use crate::simulation::{ReflectionEffectCompatible, Reflections, SimulationSharedInputs};
use std::collections::HashMap;
use std::hash::Hash;

#[cfg(doc)]
use super::super::simulation::Simulation;

/// Input frame for reflections simulation.
pub struct ReflectionsFrame<SourceId, D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    /// Shared reference to the current frame of sources, written by the game thread via
    /// [`Simulation::update_sources`].
    pub sources: SharedSources<SourceId, D, R, P, RE>,
    /// Shared simulation inputs.
    pub shared_inputs: SimulationSharedInputs<D, R, P>,
}

impl<SourceId, D, P, RE> Allocate<ReflectionsInputOwned<SourceId, D, Reflections, P, RE>>
    for ReflectionsOutput<SourceId, RE>
where
    RE: ReflectionEffectType + ReflectionEffectCompatible<Reflections, RE>,
{
    fn allocate(input: &ReflectionsInputOwned<SourceId, D, Reflections, P, RE>) -> Self {
        Self {
            sources: HashMap::with_capacity(input.sources.len()),
        }
    }
}

impl<SourceId, RE: ReflectionEffectType> Clear for ReflectionsOutput<SourceId, RE> {
    fn clear(&mut self) {
        self.sources.clear();
    }
}

impl<SourceId, RE> Shrink for ReflectionsOutput<SourceId, RE>
where
    SourceId: Hash + Eq,
    RE: ReflectionEffectType,
{
    fn shrink(&mut self) {
        if self.sources.capacity() > self.sources.len() * 2 {
            self.sources.shrink_to_fit();
        }
    }
}

impl<SourceId, D, R, P, RE> Resolve for ReflectionsFrame<SourceId, D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    type Resolved<'a>
        = ResolvedReflectionsFrame<'a, SourceId, D, R, P, RE>
    where
        Self: 'a;

    fn resolve(&self) -> Self::Resolved<'_> {
        ResolvedReflectionsFrame {
            guard: self.sources.load(),
            shared_inputs: &self.shared_inputs,
        }
    }
}

/// A snapshot of a [`ReflectionsFrame`] for use during a single simulation step.
///
/// Keeps the sources alive for the duration of the step.
pub struct ResolvedReflectionsFrame<'a, SourceId, D, R, P, RE> {
    guard: SourcesGuard<SourceId, D, R, P, RE>,
    shared_inputs: &'a SimulationSharedInputs<D, R, P>,
}

impl<SourceId, D, R, P, RE> AsReflectionsInput<SourceId, D, R, P, RE>
    for ResolvedReflectionsFrame<'_, SourceId, D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    fn as_reflections_input(&self) -> ReflectionsInput<'_, SourceId, D, R, P, RE> {
        ReflectionsInput {
            sources: &self.guard,
            shared_inputs: self.shared_inputs,
        }
    }
}

impl<SourceId, D, P, RE> Allocate<ResolvedReflectionsFrame<'_, SourceId, D, Reflections, P, RE>>
    for ReflectionsOutput<SourceId, RE>
where
    RE: ReflectionEffectType + ReflectionEffectCompatible<Reflections, RE>,
{
    fn allocate(input: &ResolvedReflectionsFrame<'_, SourceId, D, Reflections, P, RE>) -> Self {
        Self {
            sources: HashMap::with_capacity(input.guard.len()),
        }
    }
}
