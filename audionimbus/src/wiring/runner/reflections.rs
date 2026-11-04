use super::{Allocate, Clear, Resolve, Shrink, SourcesGuard};
use crate::SimulationFlags;
#[cfg(feature = "dynamic")]
use crate::binding::Dynamic;
use crate::binding::Static;
use crate::effect::ReflectionEffectType;
use crate::effect::reflections::binding::ReflectionBinding;
use crate::simulation::simulator::SimulationSharedInputs as BoundSharedInputs;
use crate::simulation::{ReflectionEffectCompatible, Reflections};
#[cfg(feature = "dynamic")]
use crate::wiring::input::{InputView, StepInput};
use crate::wiring::{
    binding::{FrameBinding, InputBinding},
    simulation::pipeline::SharedSources,
    step::reflections::ReflectionsOutput as BoundReflectionsOutput,
    step::{AsReflectionsInput, ReflectionsInput, ReflectionsInputOwned, ReflectionsOutput},
};
use std::collections::HashMap;
use std::hash::Hash;

#[cfg(doc)]
use crate::wiring::simulation::Simulation;

/// Input frame for reflections simulation.
pub type TypedReflectionsFrame<SourceId, D, R, P, RE> =
    ReflectionsFrame<SourceId, Static<(D, R, P, RE)>>;

/// Frame storage selected by a source binding.
pub struct ReflectionsFrame<SourceId, B: FrameBinding> {
    /// Shared reference to the current frame of sources, written by the game thread via
    /// [`Simulation::update_sources`].
    pub sources: SharedSources<SourceId, B>,
    /// Shared simulation inputs.
    pub shared_inputs: BoundSharedInputs<B::Inputs>,
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

impl<SourceId, E: ReflectionBinding> Clear for BoundReflectionsOutput<SourceId, E> {
    fn clear(&mut self) {
        self.sources.clear();
    }
}

impl<SourceId, E> Shrink for BoundReflectionsOutput<SourceId, E>
where
    SourceId: Hash + Eq,
    E: ReflectionBinding,
{
    fn shrink(&mut self) {
        if self.sources.capacity() > self.sources.len() * 2 {
            self.sources.shrink_to_fit();
        }
    }
}

impl<SourceId, B: FrameBinding> Resolve for ReflectionsFrame<SourceId, B> {
    type Resolved<'a>
        = ResolvedReflectionsFrame<'a, SourceId, B>
    where
        Self: 'a;

    fn resolve(&self) -> Self::Resolved<'_> {
        ResolvedReflectionsFrame {
            guard: self.sources.load(),
            shared_inputs: &self.shared_inputs,
        }
    }
}

/// A snapshot of a [`ReflectionsFrame`](crate::wiring::runner::ReflectionsFrame) for use during a single simulation step.
///
/// Keeps the sources alive for the duration of the step.
pub type TypedResolvedReflectionsFrame<'a, SourceId, D, R, P, RE> =
    ResolvedReflectionsFrame<'a, SourceId, Static<(D, R, P, RE)>>;

/// A retained source snapshot and borrowed shared inputs.
pub struct ResolvedReflectionsFrame<'a, SourceId, B: InputBinding> {
    /// The retained published source buffers.
    guard: SourcesGuard<SourceId, B>,
    /// The shared parameters borrowed from the frame.
    shared_inputs: &'a BoundSharedInputs<B::Inputs>,
}

impl<SourceId, D, R, P, RE> AsReflectionsInput<SourceId, D, R, P, RE>
    for ResolvedReflectionsFrame<'_, SourceId, Static<(D, R, P, RE)>>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    fn as_reflections_input(&self) -> ReflectionsInput<'_, SourceId, D, R, P, RE> {
        ReflectionsInput {
            sources: <Static<(D, R, P, RE)> as InputBinding>::sources(
                &self.guard,
                SimulationFlags::REFLECTIONS,
            ),
            shared_inputs: self.shared_inputs,
        }
    }
}

impl<SourceId, D, P, RE>
    Allocate<ResolvedReflectionsFrame<'_, SourceId, Static<(D, Reflections, P, RE)>>>
    for ReflectionsOutput<SourceId, RE>
where
    RE: ReflectionEffectType + ReflectionEffectCompatible<Reflections, RE>,
{
    fn allocate(
        input: &ResolvedReflectionsFrame<'_, SourceId, Static<(D, Reflections, P, RE)>>,
    ) -> Self {
        Self {
            sources: HashMap::with_capacity(input.guard.len()),
        }
    }
}

#[cfg(feature = "dynamic")]
impl<SourceId> InputView<SourceId, Dynamic<()>, crate::Reflections>
    for ResolvedReflectionsFrame<'_, SourceId, Dynamic<()>>
{
    type Listener = Dynamic<()>;

    fn input(&self) -> StepInput<'_, SourceId, Dynamic<()>> {
        StepInput {
            sources: <Dynamic<()> as InputBinding>::sources(
                &self.guard,
                SimulationFlags::REFLECTIONS,
            ),
            listener: None,
            shared_inputs: self.shared_inputs,
        }
    }
}

#[cfg(feature = "dynamic")]
impl<SourceId: Hash + Eq> Allocate<ResolvedReflectionsFrame<'_, SourceId, Dynamic<()>>>
    for crate::dynamic::ReflectionsOutput<SourceId>
{
    fn allocate(input: &ResolvedReflectionsFrame<'_, SourceId, Dynamic<()>>) -> Self {
        Self {
            sources: HashMap::with_capacity(input.guard.reflections.len()),
        }
    }
}
