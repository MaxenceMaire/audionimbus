use super::reflections::{ReflectionsBinding, ReflectionsSourceBinding};
use super::{SimulationStep, SourceEntries};
#[cfg(feature = "dynamic")]
use crate::binding::Dynamic;
use crate::binding::Static;
#[cfg(doc)]
use crate::effect::reflections::ReflectionEffectParams;
#[cfg(feature = "dynamic")]
use crate::effect::reflections::binding::ReflectionAlgorithm;
use crate::effect::reflections::{
    binding::ReflectionBinding, effect::ReflectionEffectParams as BoundReflectionEffectParams,
};
use crate::ray_tracing::RayTracer;
use crate::simulation::simulator::Simulator as BoundSimulator;
use crate::simulation::simulator::{
    StaticSimulationSharedInputs as SimulationSharedInputs, StaticSimulator as Simulator,
};
use crate::simulation::{ReflectionEffectCompatible, Reflections};
use crate::wiring::binding::PipelineBinding;
use crate::wiring::input::{InputView, Reverb, StepInput};
use crate::wiring::simulation::pipeline::StaticSourceWithInputs as SourceWithInputs;
use std::collections::HashMap;
use std::hash::Hash;
use std::marker::PhantomData;

/// Runs reflections and listener-centric reverb simulation simultaneously.
pub type StaticReflectionsReverbStep<SourceId, T, D, P, RE> =
    ReflectionsReverbStep<SourceId, Static<(T, D, Reflections, P, RE)>>;

/// A simulation step with binding-specific handles and errors.
pub struct ReflectionsReverbStep<SourceId, B: PipelineBinding> {
    /// The simulator used by this step.
    simulator: BoundSimulator<B>,
    /// The source identifier marker.
    _source_id: PhantomData<fn() -> SourceId>,
}

impl<T, D, P, RE> StaticReflectionsReverbStep<(), T, D, P, RE>
where
    T: RayTracer,
{
    /// Creates a new reflections and reverb simulation step.
    pub fn new<SourceId>(
        simulator: Simulator<T, D, Reflections, P, RE>,
    ) -> StaticReflectionsReverbStep<SourceId, T, D, P, RE> {
        ReflectionsReverbStep {
            simulator,
            _source_id: PhantomData,
        }
    }
}

impl<SourceId, B, I> SimulationStep<I> for ReflectionsReverbStep<SourceId, B>
where
    B: ReflectionsBinding + 'static,
    BoundSimulator<B>: Send,
    SourceId: 'static + Clone + Send + Sync + Hash + Eq,
    I: InputView<SourceId, B::Sources, Reverb>,
    B::Sources: ReflectionsSourceBinding<B::Reflection, B::Error>,
    I::Listener: ReflectionsSourceBinding<B::Reflection, B::Error>,
{
    type Output = ReflectionsReverbOutput<SourceId, B::Reflection>;
    type Error = B::Error;

    fn run(&mut self, frame: &I, output: &mut Self::Output) -> Result<(), Self::Error> {
        let input = frame.input();
        B::set_shared(&self.simulator, input.shared_inputs)?;

        for (_, source_with_inputs) in input.sources {
            <B::Sources as ReflectionsSourceBinding<B::Reflection, B::Error>>::set_source(
                &source_with_inputs.source,
                &source_with_inputs.simulation_inputs,
            )?;
        }

        if let Some(listener) = input.listener {
            I::Listener::set_source(&listener.source, &listener.simulation_inputs)?;
        }

        B::run(&self.simulator)?;

        for (id, source_with_inputs) in input.sources {
            output.sources.insert(
                id.clone(),
                <B::Sources as ReflectionsSourceBinding<B::Reflection, B::Error>>::output(
                    &source_with_inputs.source,
                )?,
            );
        }

        if let Some(listener) = input.listener {
            output.listener = Some(I::Listener::output(&listener.source)?);
        }

        Ok(())
    }
}

impl<SourceId, D, R, P, RE, I> InputView<SourceId, Static<(D, R, P, RE)>, Reverb> for I
where
    RE: ReflectionEffectCompatible<R, RE>,
    I: AsReflectionsReverbInput<SourceId, D, R, P, RE>,
{
    type Listener = Static<(I::LD, R, I::LP, RE)>;

    fn input(&self) -> StepInput<'_, SourceId, Static<(D, R, P, RE)>, Self::Listener> {
        let input = self.as_reflections_reverb_input();
        StepInput {
            sources: input.sources,
            listener: input.listener,
            shared_inputs: input.shared_inputs,
        }
    }
}

/// Reflections and reverb simulation inputs.
#[derive(Debug)]
pub struct ReflectionsReverbInput<'a, SourceId, D, R, P, RE, LD = D, LP = P>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    /// The spatial audio sources whose reflections to simulate.
    pub sources: &'a SourceEntries<SourceId, D, R, P, RE>,
    /// The listener, used for listener-centric reverb simulation.
    pub listener: Option<&'a SourceWithInputs<LD, R, LP, RE>>,
    /// Shared simulation inputs applying to all sources and the listener.
    pub shared_inputs: &'a SimulationSharedInputs<D, R, P>,
}

/// Implemented by any type that can produce a [`ReflectionsReverbInput`] view.
pub trait AsReflectionsReverbInput<SourceId, D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    type LD;
    type LP;

    /// Returns a view of this type as [`ReflectionsReverbInput`].
    fn as_reflections_reverb_input(
        &self,
    ) -> ReflectionsReverbInput<'_, SourceId, D, R, P, RE, Self::LD, Self::LP>;
}

/// Owned input for reflections and reverb simulation.
pub struct ReflectionsReverbInputOwned<SourceId, D, R, P, RE, LD = D, LP = P>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    /// The spatial audio sources whose reflections to simulate.
    pub sources: Vec<(SourceId, SourceWithInputs<D, R, P, RE>)>,
    /// The listener, used for listener-centric reverb simulation.
    pub listener: Option<SourceWithInputs<LD, R, LP, RE>>,
    /// Shared simulation inputs applying to all sources and the listener.
    pub shared_inputs: SimulationSharedInputs<D, R, P>,
}

impl<SourceId, D, R, P, RE, LD, LP> AsReflectionsReverbInput<SourceId, D, R, P, RE>
    for ReflectionsReverbInputOwned<SourceId, D, R, P, RE, LD, LP>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    type LD = LD;
    type LP = LP;

    fn as_reflections_reverb_input(
        &self,
    ) -> ReflectionsReverbInput<'_, SourceId, D, R, P, RE, LD, LP> {
        ReflectionsReverbInput {
            sources: self.sources.as_slice(),
            listener: self.listener.as_ref(),
            shared_inputs: &self.shared_inputs,
        }
    }
}

/// Combined per-source reflections and listener-centric reverb output.
pub type StaticReflectionsReverbOutput<SourceId, RE> =
    ReflectionsReverbOutput<SourceId, crate::binding::Static<RE>>;

/// Per-source reflections and optional listener reverb retained by a runtime worker.
#[cfg(feature = "dynamic")]
pub type DynamicReflectionsReverbOutput<SourceId> =
    ReflectionsReverbOutput<SourceId, Dynamic<ReflectionAlgorithm>>;

/// Output parameters selected by a reflection binding.
#[derive(Debug)]
pub struct ReflectionsReverbOutput<SourceId, B: ReflectionBinding>
where
    SourceId: Hash + Eq,
{
    /// Per-source reflection effect params.
    pub sources: HashMap<SourceId, BoundReflectionEffectParams<B>>,
    /// Listener-centric reverb.
    ///
    /// `None` if no listener is present in the scene, or until the first simulation run with a
    /// listener completes.
    pub listener: Option<BoundReflectionEffectParams<B>>,
}

impl<SourceId, B> Default for ReflectionsReverbOutput<SourceId, B>
where
    SourceId: Hash + Eq,
    B: ReflectionBinding,
{
    fn default() -> Self {
        Self {
            sources: HashMap::new(),
            listener: None,
        }
    }
}

unsafe impl<SourceId, B> Send for ReflectionsReverbOutput<SourceId, B>
where
    SourceId: Send + Hash + Eq,
    B: ReflectionBinding,
{
}

/// # Safety
///
/// It is technically unsafe because simulation threads might be rewriting the
/// IR buffer that [`ReflectionEffectParams`] points to (impulse_response field)
/// while the audio thread is still reading the previous version.
///
/// However the chance of an overlap is slim and a data race is likely inaudible.
unsafe impl<SourceId, B> Sync for ReflectionsReverbOutput<SourceId, B>
where
    SourceId: Sync + Hash + Eq,
    B: ReflectionBinding,
{
}

impl<SourceId, B: PipelineBinding> From<BoundSimulator<B>> for ReflectionsReverbStep<SourceId, B> {
    fn from(simulator: BoundSimulator<B>) -> Self {
        Self {
            simulator,
            _source_id: PhantomData,
        }
    }
}
