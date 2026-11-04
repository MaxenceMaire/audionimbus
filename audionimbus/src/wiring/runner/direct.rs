use super::{Allocate, Resolve, SourcesGuard};
use crate::SimulationFlags;
#[cfg(feature = "dynamic")]
use crate::binding::Dynamic;
use crate::binding::Static;
use crate::effect::DirectEffectParams;
use crate::simulation::simulator::SimulationSharedInputs as BoundSharedInputs;
use crate::simulation::{Direct, ReflectionEffectCompatible};
#[cfg(feature = "dynamic")]
use crate::wiring::input::{InputView, StepInput};
use crate::wiring::{
    binding::{FrameBinding, InputBinding},
    simulation::pipeline::SharedSources,
    step::{AsDirectInput, DirectInput},
};
use std::collections::HashMap;
use std::hash::Hash;

#[cfg(doc)]
use crate::wiring::simulation::Simulation;

/// Input frame for direct simulation.
pub type StaticDirectFrame<SourceId, D, R, P, RE> = DirectFrame<SourceId, Static<(D, R, P, RE)>>;

/// Frame storage selected by a source binding.
pub struct DirectFrame<SourceId, B: FrameBinding> {
    /// Shared reference to the current frame of sources, written by the game thread via
    /// [`Simulation::update_sources`].
    pub sources: SharedSources<SourceId, B>,
    /// Shared simulation inputs.
    pub shared_inputs: BoundSharedInputs<B::Inputs>,
}

impl<SourceId, B: FrameBinding> Resolve for DirectFrame<SourceId, B> {
    type Resolved<'a>
        = ResolvedDirectFrame<'a, SourceId, B>
    where
        Self: 'a;

    fn resolve(&self) -> Self::Resolved<'_> {
        ResolvedDirectFrame {
            guard: self.sources.load(),
            shared_inputs: &self.shared_inputs,
        }
    }
}

/// A snapshot of a [`DirectFrame`](crate::wiring::runner::DirectFrame) for use during a single simulation step.
///
/// Keeps the current sources alive for the duration of the step.
pub type StaticResolvedDirectFrame<'a, SourceId, D, R, P, RE> =
    ResolvedDirectFrame<'a, SourceId, Static<(D, R, P, RE)>>;

/// A retained source snapshot and borrowed shared inputs.
pub struct ResolvedDirectFrame<'a, SourceId, B: InputBinding> {
    /// The retained published source buffers.
    guard: SourcesGuard<SourceId, B>,
    /// The shared parameters borrowed from the frame.
    shared_inputs: &'a BoundSharedInputs<B::Inputs>,
}

impl<SourceId, D, R, P, RE> AsDirectInput<SourceId, D, R, P, RE>
    for ResolvedDirectFrame<'_, SourceId, Static<(D, R, P, RE)>>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    fn as_direct_input(&self) -> DirectInput<'_, SourceId, D, R, P, RE> {
        DirectInput {
            sources: <Static<(D, R, P, RE)> as InputBinding>::sources(
                &self.guard,
                SimulationFlags::DIRECT,
            ),
            shared_inputs: self.shared_inputs,
        }
    }
}

impl<SourceId, R, P, RE> Allocate<ResolvedDirectFrame<'_, SourceId, Static<(Direct, R, P, RE)>>>
    for HashMap<SourceId, DirectEffectParams>
where
    SourceId: Hash + Eq,
    RE: ReflectionEffectCompatible<R, RE>,
{
    fn allocate(input: &ResolvedDirectFrame<'_, SourceId, Static<(Direct, R, P, RE)>>) -> Self {
        Self::with_capacity(input.guard.len())
    }
}

#[cfg(feature = "dynamic")]
impl<SourceId> InputView<SourceId, Dynamic<()>, crate::Direct>
    for ResolvedDirectFrame<'_, SourceId, Dynamic<()>>
{
    type Listener = Dynamic<()>;

    fn input(&self) -> StepInput<'_, SourceId, Dynamic<()>> {
        StepInput {
            sources: <Dynamic<()> as InputBinding>::sources(&self.guard, SimulationFlags::DIRECT),
            listener: None,
            shared_inputs: self.shared_inputs,
        }
    }
}

#[cfg(feature = "dynamic")]
impl<SourceId: Hash + Eq> Allocate<ResolvedDirectFrame<'_, SourceId, Dynamic<()>>>
    for std::collections::HashMap<SourceId, crate::DirectEffectParams>
{
    fn allocate(input: &ResolvedDirectFrame<'_, SourceId, Dynamic<()>>) -> Self {
        Self::with_capacity(input.guard.direct.len())
    }
}
