use super::{Allocate, Resolve, SourcesGuard};
use crate::SimulationFlags;
#[cfg(feature = "dynamic")]
use crate::binding::Dynamic;
use crate::binding::Static;
use crate::effect::PathEffectParams;
use crate::simulation::simulator::SimulationSharedInputs as BoundSharedInputs;
use crate::simulation::{Pathing, ReflectionEffectCompatible};
#[cfg(feature = "dynamic")]
use crate::wiring::input::{InputView, StepInput};
use crate::wiring::{
    binding::{FrameBinding, InputBinding},
    simulation::pipeline::SharedSources,
    step::{AsPathingInput, PathingInput, PathingInputOwned},
};
use std::collections::HashMap;
use std::hash::Hash;

#[cfg(doc)]
use crate::wiring::simulation::Simulation;

/// Input frame for pathing simulation.
pub type TypedPathingFrame<SourceId, D, R, P, RE> = PathingFrame<SourceId, Static<(D, R, P, RE)>>;

/// Frame storage selected by a source binding.
pub struct PathingFrame<SourceId, B: FrameBinding> {
    /// Shared reference to the current frame of sources, written by the game thread via
    /// [`Simulation::update_sources`].
    pub sources: SharedSources<SourceId, B>,
    /// Shared simulation inputs.
    pub shared_inputs: BoundSharedInputs<B::Inputs>,
}

impl<SourceId, D, R, RE> Allocate<PathingInputOwned<SourceId, D, R, Pathing, RE>>
    for Vec<(SourceId, PathEffectParams)>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    fn allocate(input: &PathingInputOwned<SourceId, D, R, Pathing, RE>) -> Self {
        Self::with_capacity(input.sources.len())
    }
}

impl<SourceId, B: FrameBinding> Resolve for PathingFrame<SourceId, B> {
    type Resolved<'a>
        = ResolvedPathingFrame<'a, SourceId, B>
    where
        Self: 'a;

    fn resolve(&self) -> Self::Resolved<'_> {
        ResolvedPathingFrame {
            guard: self.sources.load(),
            shared_inputs: &self.shared_inputs,
        }
    }
}

/// A snapshot of a [`PathingFrame`](crate::wiring::runner::PathingFrame) for use during a single simulation step.
///
/// Keeps the sources alive for the duration of the step.
pub type TypedResolvedPathingFrame<'a, SourceId, D, R, P, RE> =
    ResolvedPathingFrame<'a, SourceId, Static<(D, R, P, RE)>>;

/// A retained source snapshot and borrowed shared inputs.
pub struct ResolvedPathingFrame<'a, SourceId, B: InputBinding> {
    /// The retained published source buffers.
    guard: SourcesGuard<SourceId, B>,
    /// The shared parameters borrowed from the frame.
    shared_inputs: &'a BoundSharedInputs<B::Inputs>,
}

impl<SourceId, D, R, P, RE> AsPathingInput<SourceId, D, R, P, RE>
    for ResolvedPathingFrame<'_, SourceId, Static<(D, R, P, RE)>>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    fn as_pathing_input(&self) -> PathingInput<'_, SourceId, D, R, P, RE> {
        PathingInput {
            sources: <Static<(D, R, P, RE)> as InputBinding>::sources(
                &self.guard,
                SimulationFlags::PATHING,
            ),
            shared_inputs: self.shared_inputs,
        }
    }
}

impl<SourceId, D, R, RE> Allocate<ResolvedPathingFrame<'_, SourceId, Static<(D, R, Pathing, RE)>>>
    for HashMap<SourceId, PathEffectParams>
where
    SourceId: Hash + Eq,
    RE: ReflectionEffectCompatible<R, RE>,
{
    fn allocate(input: &ResolvedPathingFrame<'_, SourceId, Static<(D, R, Pathing, RE)>>) -> Self {
        Self::with_capacity(input.guard.len())
    }
}

#[cfg(feature = "dynamic")]
impl<SourceId> InputView<SourceId, Dynamic<()>, crate::Pathing>
    for ResolvedPathingFrame<'_, SourceId, Dynamic<()>>
{
    type Listener = Dynamic<()>;

    fn input(&self) -> StepInput<'_, SourceId, Dynamic<()>> {
        StepInput {
            sources: <Dynamic<()> as InputBinding>::sources(&self.guard, SimulationFlags::PATHING),
            listener: None,
            shared_inputs: self.shared_inputs,
        }
    }
}

#[cfg(feature = "dynamic")]
impl<SourceId: Hash + Eq> Allocate<ResolvedPathingFrame<'_, SourceId, Dynamic<()>>>
    for std::collections::HashMap<SourceId, crate::PathEffectParams>
{
    fn allocate(input: &ResolvedPathingFrame<'_, SourceId, Dynamic<()>>) -> Self {
        Self::with_capacity(input.guard.pathing.len())
    }
}
