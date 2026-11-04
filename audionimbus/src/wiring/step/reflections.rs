use super::{SimulationStep, SimulationStepError, SourceEntries};
#[cfg(feature = "dynamic")]
use crate::binding::Dynamic;
use crate::binding::Static;
#[cfg(feature = "dynamic")]
use crate::effect::reflections::binding::ReflectionAlgorithm;
use crate::effect::reflections::{
    binding::ReflectionBinding, effect::ReflectionEffectParams as BoundReflectionEffectParams,
};
use crate::effect::{ReflectionEffectParams, ReflectionEffectType};
use crate::ray_tracing::RayTracer;
use crate::simulation::inputs::SimulationInputs as BoundInputs;
use crate::simulation::simulator::{
    SimulationSharedInputs as BoundSharedInputs, Simulator as BoundSimulator,
};
use crate::simulation::source::Source as BoundSource;
use crate::simulation::{
    DirectCompatible, PathingCompatible, ReflectionEffectCompatible, Reflections,
    SimulationFlagsProvider, SimulationSharedInputs, Simulator,
};
use crate::wiring::{
    SourceWithInputs,
    binding::{InputBinding, PipelineBinding},
    input::{InputView, StepInput},
};
use std::collections::HashMap;
use std::hash::Hash;
use std::marker::PhantomData;

/// Runs reflections simulation.
pub type StaticReflectionsStep<SourceId, T, D, P, RE> =
    ReflectionsStep<SourceId, Static<(T, D, Reflections, P, RE)>>;

/// A simulation step with binding-specific handles and errors.
pub struct ReflectionsStep<SourceId, B: PipelineBinding> {
    /// The simulator used by this step.
    simulator: BoundSimulator<B>,
    /// The source identifier marker.
    _source_id: PhantomData<fn() -> SourceId>,
}

impl<T, D, P, RE> StaticReflectionsStep<(), T, D, P, RE>
where
    T: RayTracer,
{
    /// Creates a new reflections simulation step.
    pub fn new<SourceId>(
        simulator: Simulator<T, D, Reflections, P, RE>,
    ) -> StaticReflectionsStep<SourceId, T, D, P, RE> {
        ReflectionsStep {
            simulator,
            _source_id: PhantomData,
        }
    }
}

impl<SourceId, B, I> SimulationStep<I> for ReflectionsStep<SourceId, B>
where
    B: ReflectionsBinding + 'static,
    BoundSimulator<B>: Send,
    SourceId: 'static + Clone + Send + Sync + Hash + Eq,
    I: InputView<SourceId, B::Sources, Reflections>,
    B::Sources: ReflectionsSourceBinding<B::Reflection, B::Error>,
{
    type Output = ReflectionsOutput<SourceId, B::Reflection>;
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

        B::run(&self.simulator)?;

        for (id, source_with_inputs) in input.sources {
            output.sources.insert(
                id.clone(),
                <B::Sources as ReflectionsSourceBinding<B::Reflection, B::Error>>::output(
                    &source_with_inputs.source,
                )?,
            );
        }

        Ok(())
    }
}

impl<SourceId, D, R, P, RE, I> InputView<SourceId, Static<(D, R, P, RE)>, Reflections> for I
where
    RE: ReflectionEffectCompatible<R, RE>,
    I: AsReflectionsInput<SourceId, D, R, P, RE>,
{
    type Listener = Static<(D, R, P, RE)>;

    fn input(&self) -> StepInput<'_, SourceId, Static<(D, R, P, RE)>, Self::Listener> {
        let input = self.as_reflections_input();
        StepInput {
            sources: input.sources,
            listener: None,
            shared_inputs: input.shared_inputs,
        }
    }
}

/// Reflections inputs.
#[derive(Debug)]
pub struct ReflectionsInput<'a, SourceId, D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    /// The spatial audio sources whose reflections to simulate.
    pub sources: &'a SourceEntries<SourceId, D, R, P, RE>,
    /// Shared simulation inputs applying to all sources and the listener.
    pub shared_inputs: &'a SimulationSharedInputs<D, R, P>,
}

/// Implemented by any type that can produce a [`ReflectionsInput`] view.
pub trait AsReflectionsInput<SourceId, D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    /// Returns a view of this type as [`ReflectionsInput`].
    fn as_reflections_input(&self) -> ReflectionsInput<'_, SourceId, D, R, P, RE>;
}

/// Owned input for reflections simulation.
pub struct ReflectionsInputOwned<SourceId, D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    /// The spatial audio sources whose reflections to simulate.
    pub sources: Vec<(SourceId, SourceWithInputs<D, R, P, RE>)>,
    /// Shared simulation inputs applying to all sources and the listener.
    pub shared_inputs: SimulationSharedInputs<D, R, P>,
}

impl<SourceId, D, R, P, RE> AsReflectionsInput<SourceId, D, R, P, RE>
    for ReflectionsInputOwned<SourceId, D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    fn as_reflections_input(&self) -> ReflectionsInput<'_, SourceId, D, R, P, RE> {
        ReflectionsInput {
            sources: self.sources.as_slice(),
            shared_inputs: &self.shared_inputs,
        }
    }
}

/// Combined per-source reflections output.
pub type StaticReflectionsOutput<SourceId, RE> =
    ReflectionsOutput<SourceId, crate::binding::Static<RE>>;

/// Per-source reflection parameters retained by a runtime worker.
#[cfg(feature = "dynamic")]
pub type DynamicReflectionsOutput<SourceId> =
    ReflectionsOutput<SourceId, Dynamic<ReflectionAlgorithm>>;

/// Output parameters selected by a reflection binding.
#[derive(Debug)]
pub struct ReflectionsOutput<SourceId, B: ReflectionBinding> {
    /// Per-source reflection effect params.
    pub sources: HashMap<SourceId, BoundReflectionEffectParams<B>>,
}

impl<SourceId, B: ReflectionBinding> Default for ReflectionsOutput<SourceId, B> {
    fn default() -> Self {
        Self {
            sources: HashMap::new(),
        }
    }
}

unsafe impl<SourceId: Send, B: ReflectionBinding> Send for ReflectionsOutput<SourceId, B> {}

/// # Safety
///
/// `ReflectionsOutput` contains [`ReflectionEffectParams`] values that hold raw pointers to
/// impulse response buffers managed internally by Steam Audio.
/// When the simulation thread completes a new run, Steam Audio may overwrite these buffers before
/// the audio thread has finished reading the previous snapshot.
///
/// It can result in a single frame of corrupted reverb, which is inaudible in practice.
///
/// An alternative would be to copy the IR data on each simulation run, which is expensive.
/// The tradeoff is deliberate.
unsafe impl<SourceId: Sync, B: ReflectionBinding> Sync for ReflectionsOutput<SourceId, B> {}

/// Source operations for per-source reflections and listener reverb.
pub trait ReflectionsSourceBinding<Effect: ReflectionBinding, Error>: InputBinding {
    /// Updates the source's reflection inputs.
    fn set_source(
        source: &BoundSource<Self::Source>,
        simulation_inputs: &BoundInputs<Self::Inputs>,
    ) -> Result<(), Error>;
    /// Returns source-owned reflection parameters.
    fn output(
        source: &BoundSource<Self::Source>,
    ) -> Result<BoundReflectionEffectParams<Effect>, Error>;
}

impl<D, P, RE> ReflectionsSourceBinding<Static<RE>, SimulationStepError>
    for Static<(D, Reflections, P, RE)>
where
    D: 'static + Send + Sync + DirectCompatible<D> + SimulationFlagsProvider,
    P: 'static + Send + Sync + PathingCompatible<P> + SimulationFlagsProvider,
    RE: 'static + Send + Sync + ReflectionEffectCompatible<Reflections, RE> + ReflectionEffectType,
    (): DirectCompatible<D> + PathingCompatible<P>,
{
    #[inline(always)]
    fn set_source(
        source: &crate::Source<D, Reflections, P, RE>,
        simulation_inputs: &crate::SimulationInputs<D, Reflections, P>,
    ) -> Result<(), SimulationStepError> {
        Ok(source.set_reflections_inputs(simulation_inputs)?)
    }

    #[inline(always)]
    fn output(
        source: &crate::Source<D, Reflections, P, RE>,
    ) -> Result<ReflectionEffectParams<RE>, SimulationStepError> {
        Ok(source.get_reflections_outputs()?)
    }
}

/// Native simulation operations selected by a reflection binding.
pub trait ReflectionsBinding: PipelineBinding {
    /// The reflection parameter binding returned by this worker.
    type Reflection: ReflectionBinding + 'static;
    /// The errors returned by this worker binding.
    type Error: std::error::Error;

    /// Updates shared reflection inputs.
    fn set_shared(
        simulator: &BoundSimulator<Self>,
        shared_inputs: &BoundSharedInputs<<Self::Sources as InputBinding>::Inputs>,
    ) -> Result<(), Self::Error>
    where
        Self: Sized;
    /// Runs reflection simulation.
    fn run(simulator: &BoundSimulator<Self>) -> Result<(), Self::Error>
    where
        Self: Sized;
}

impl<T, D, P, RE> ReflectionsBinding for Static<(T, D, Reflections, P, RE)>
where
    T: 'static + RayTracer,
    D: 'static + Send + Sync + DirectCompatible<D> + SimulationFlagsProvider,
    P: 'static + Send + Sync + PathingCompatible<P> + SimulationFlagsProvider,
    RE: 'static + Send + Sync + ReflectionEffectCompatible<Reflections, RE> + ReflectionEffectType,
    (): DirectCompatible<D> + PathingCompatible<P>,
{
    type Reflection = Static<RE>;
    type Error = SimulationStepError;

    fn set_shared(
        simulator: &BoundSimulator<Self>,
        shared_inputs: &SimulationSharedInputs<D, Reflections, P>,
    ) -> Result<(), Self::Error> {
        Ok(simulator.set_shared_reflections_inputs(shared_inputs)?)
    }

    #[inline(always)]
    fn run(simulator: &BoundSimulator<Self>) -> Result<(), Self::Error> {
        Ok(simulator.run_reflections()?)
    }
}

#[cfg(feature = "dynamic")]
impl
    ReflectionsSourceBinding<
        Dynamic<crate::dynamic::ReflectionAlgorithm>,
        crate::dynamic::SimulationError,
    > for Dynamic<()>
{
    fn set_source(
        source: &crate::dynamic::Source,
        simulation_inputs: &crate::dynamic::SimulationInputs,
    ) -> Result<(), crate::dynamic::SimulationError> {
        source.set_inputs_subset(crate::SimulationFlags::REFLECTIONS, simulation_inputs)
    }

    fn output(
        source: &crate::dynamic::Source,
    ) -> Result<crate::dynamic::ReflectionEffectParams, crate::dynamic::SimulationError> {
        source.get_reflections_outputs()
    }
}

#[cfg(feature = "dynamic")]
impl ReflectionsBinding for Dynamic<crate::simulation::simulator::SimulationChoices> {
    type Reflection = Dynamic<crate::dynamic::ReflectionAlgorithm>;
    type Error = crate::dynamic::SimulationError;

    fn set_shared(
        simulator: &crate::dynamic::Simulator,
        shared_inputs: &crate::dynamic::SimulationSharedInputs,
    ) -> Result<(), Self::Error> {
        simulator.set_shared_inputs(crate::SimulationFlags::REFLECTIONS, shared_inputs)
    }

    fn run(simulator: &crate::dynamic::Simulator) -> Result<(), Self::Error> {
        simulator.run_reflections()
    }
}

impl<SourceId, B: PipelineBinding> From<BoundSimulator<B>> for ReflectionsStep<SourceId, B> {
    fn from(simulator: BoundSimulator<B>) -> Self {
        Self {
            simulator,
            _source_id: PhantomData,
        }
    }
}
