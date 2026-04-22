use super::super::simulation::{SharedSources, SourceWithInputs};
use super::super::step::{
    AsReflectionsReverbInput, ReflectionsReverbInput, ReflectionsReverbOutput,
};
use super::{Allocate, Clear, Resolve, Shrink, SourcesGuard};
use crate::effect::ReflectionEffectType;
use crate::simulation::{ReflectionEffectCompatible, Reflections, SimulationSharedInputs};
use std::collections::HashMap;
use std::hash::Hash;

#[cfg(doc)]
use super::super::simulation::Simulation;

/// Input frame for reflections and reverb simulation.
pub struct ReflectionsReverbFrame<SourceId, D, R, P, RE, LD = D, LP = P>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    /// Shared reference to the current frame of sources, written by the game thread via
    /// [`Simulation::update_sources`].
    pub sources: SharedSources<SourceId, D, R, P, RE>,
    /// Listener source.
    pub listener: Option<SourceWithInputs<LD, R, LP, RE>>,
    /// Shared simulation inputs.
    pub shared_inputs: SimulationSharedInputs<D, R, P>,
}

impl<SourceId, RE> Clear for ReflectionsReverbOutput<SourceId, RE>
where
    SourceId: Hash + Eq,
    RE: ReflectionEffectType,
{
    fn clear(&mut self) {
        self.sources.clear();
        self.listener = None;
    }
}

impl<SourceId, RE> Shrink for ReflectionsReverbOutput<SourceId, RE>
where
    SourceId: Hash + Eq,
    RE: ReflectionEffectType,
{
    fn shrink(&mut self) {
        if self.sources.capacity() > self.sources.len() * 3 {
            self.sources.shrink_to_fit();
        }
    }
}

impl<SourceId, D, R, P, RE, LD, LP> Resolve
    for ReflectionsReverbFrame<SourceId, D, R, P, RE, LD, LP>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    type Resolved<'a>
        = ResolvedReflectionsReverbFrame<'a, SourceId, D, R, P, RE, LD, LP>
    where
        Self: 'a;

    fn resolve(&self) -> Self::Resolved<'_> {
        ResolvedReflectionsReverbFrame {
            guard: self.sources.load(),
            listener: self.listener.as_ref(),
            shared_inputs: &self.shared_inputs,
        }
    }
}

/// A snapshot of a [`ReflectionsReverbFrame`] for use during a single simulation step.
///
/// Keeps the sources alive for the duration of the step.
pub struct ResolvedReflectionsReverbFrame<'a, SourceId, D, R, P, RE, LD, LP> {
    guard: SourcesGuard<SourceId, D, R, P, RE>,
    listener: Option<&'a SourceWithInputs<LD, R, LP, RE>>,
    shared_inputs: &'a SimulationSharedInputs<D, R, P>,
}

impl<SourceId, D, R, P, RE, LD, LP> AsReflectionsReverbInput<SourceId, D, R, P, RE>
    for ResolvedReflectionsReverbFrame<'_, SourceId, D, R, P, RE, LD, LP>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    type LD = LD;
    type LP = LP;

    fn as_reflections_reverb_input(
        &self,
    ) -> ReflectionsReverbInput<'_, SourceId, D, R, P, RE, LD, LP> {
        ReflectionsReverbInput {
            sources: &self.guard,
            listener: self.listener,
            shared_inputs: self.shared_inputs,
        }
    }
}

impl<SourceId, D, P, RE, LD, LP>
    Allocate<ResolvedReflectionsReverbFrame<'_, SourceId, D, Reflections, P, RE, LD, LP>>
    for ReflectionsReverbOutput<SourceId, RE>
where
    SourceId: Hash + Eq,
    RE: ReflectionEffectType + ReflectionEffectCompatible<Reflections, RE>,
{
    fn allocate(
        input: &ResolvedReflectionsReverbFrame<'_, SourceId, D, Reflections, P, RE, LD, LP>,
    ) -> Self {
        Self {
            sources: HashMap::with_capacity(input.guard.len()),
            listener: None,
        }
    }
}
