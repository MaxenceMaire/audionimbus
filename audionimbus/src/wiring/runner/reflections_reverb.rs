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
use crate::wiring::input::{InputView, Reverb, StepInput};
use crate::wiring::{
    binding::{FrameBinding, InputBinding},
    simulation::pipeline::{SharedSources, SourceWithInputs},
    step::reflections_reverb::ReflectionsReverbOutput as BoundReflectionsReverbOutput,
    step::{AsReflectionsReverbInput, ReflectionsReverbInput, ReflectionsReverbOutput},
};
use std::collections::HashMap;
use std::hash::Hash;

#[cfg(doc)]
use crate::wiring::simulation::Simulation;

/// Input frame for reflections and reverb simulation.
pub type TypedReflectionsReverbFrame<SourceId, D, R, P, RE, LD = D, LP = P> =
    ReflectionsReverbFrame<SourceId, Static<(D, R, P, RE)>, Static<(LD, R, LP, RE)>>;

/// Frame storage selected by a source binding.
pub struct ReflectionsReverbFrame<SourceId, B: FrameBinding, L: InputBinding = B> {
    /// Shared reference to the current frame of sources, written by the game thread via
    /// [`Simulation::update_sources`].
    pub sources: SharedSources<SourceId, B>,
    /// Listener source.
    pub listener: Option<SourceWithInputs<L>>,
    /// Shared simulation inputs.
    pub shared_inputs: BoundSharedInputs<B::Inputs>,
}

impl<SourceId, E> Clear for BoundReflectionsReverbOutput<SourceId, E>
where
    SourceId: Hash + Eq,
    E: ReflectionBinding,
{
    fn clear(&mut self) {
        self.sources.clear();
        self.listener = None;
    }
}

impl<SourceId, E> Shrink for BoundReflectionsReverbOutput<SourceId, E>
where
    SourceId: Hash + Eq,
    E: ReflectionBinding,
{
    fn shrink(&mut self) {
        if self.sources.capacity() > self.sources.len() * 3 {
            self.sources.shrink_to_fit();
        }
    }
}

impl<SourceId, B: FrameBinding, L: InputBinding> Resolve
    for ReflectionsReverbFrame<SourceId, B, L>
{
    type Resolved<'a>
        = ResolvedReflectionsReverbFrame<'a, SourceId, B, L>
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

/// A snapshot of a [`ReflectionsReverbFrame`](crate::wiring::runner::ReflectionsReverbFrame) for use during a single simulation step.
///
/// Keeps the sources alive for the duration of the step.
pub type TypedResolvedReflectionsReverbFrame<'a, SourceId, D, R, P, RE, LD, LP> =
    ResolvedReflectionsReverbFrame<'a, SourceId, Static<(D, R, P, RE)>, Static<(LD, R, LP, RE)>>;

/// A retained source snapshot and borrowed shared inputs.
pub struct ResolvedReflectionsReverbFrame<'a, SourceId, B: InputBinding, L: InputBinding = B> {
    /// The retained published source buffers.
    guard: SourcesGuard<SourceId, B>,
    /// The listener supplied by the input frame.
    listener: Option<&'a SourceWithInputs<L>>,
    /// The shared parameters borrowed from the frame.
    shared_inputs: &'a BoundSharedInputs<B::Inputs>,
}

impl<SourceId, D, R, P, RE, LD, LP> AsReflectionsReverbInput<SourceId, D, R, P, RE>
    for ResolvedReflectionsReverbFrame<'_, SourceId, Static<(D, R, P, RE)>, Static<(LD, R, LP, RE)>>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    type LD = LD;
    type LP = LP;

    fn as_reflections_reverb_input(
        &self,
    ) -> ReflectionsReverbInput<'_, SourceId, D, R, P, RE, LD, LP> {
        ReflectionsReverbInput {
            sources: <Static<(D, R, P, RE)> as InputBinding>::sources(
                &self.guard,
                SimulationFlags::REFLECTIONS,
            ),
            listener: self.listener,
            shared_inputs: self.shared_inputs,
        }
    }
}

impl<SourceId, D, P, RE, LD, LP>
    Allocate<
        ResolvedReflectionsReverbFrame<
            '_,
            SourceId,
            Static<(D, Reflections, P, RE)>,
            Static<(LD, Reflections, LP, RE)>,
        >,
    > for ReflectionsReverbOutput<SourceId, RE>
where
    SourceId: Hash + Eq,
    RE: ReflectionEffectType + ReflectionEffectCompatible<Reflections, RE>,
{
    fn allocate(
        input: &ResolvedReflectionsReverbFrame<
            '_,
            SourceId,
            Static<(D, Reflections, P, RE)>,
            Static<(LD, Reflections, LP, RE)>,
        >,
    ) -> Self {
        Self {
            sources: HashMap::with_capacity(input.guard.len()),
            listener: None,
        }
    }
}

#[cfg(feature = "dynamic")]
impl<SourceId> InputView<SourceId, Dynamic<()>, Reverb>
    for ResolvedReflectionsReverbFrame<'_, SourceId, Dynamic<()>, Dynamic<()>>
{
    type Listener = Dynamic<()>;

    fn input(&self) -> StepInput<'_, SourceId, Dynamic<()>> {
        StepInput {
            sources: <Dynamic<()> as InputBinding>::sources(
                &self.guard,
                SimulationFlags::REFLECTIONS,
            ),
            listener: self.guard.listener.as_ref(),
            shared_inputs: self.shared_inputs,
        }
    }
}

#[cfg(feature = "dynamic")]
impl<SourceId: Hash + Eq> Allocate<ResolvedReflectionsReverbFrame<'_, SourceId, Dynamic<()>>>
    for crate::dynamic::ReflectionsReverbOutput<SourceId>
{
    fn allocate(input: &ResolvedReflectionsReverbFrame<'_, SourceId, Dynamic<()>>) -> Self {
        Self {
            sources: HashMap::with_capacity(input.guard.reflections.len()),
            listener: None,
        }
    }
}
