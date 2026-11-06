use super::{SimulationStep, SimulationStepError, SourceEntries};
#[cfg(feature = "dynamic")]
use crate::binding::Dynamic;
use crate::binding::Static;
use crate::effect::DirectEffectParams;
use crate::ray_tracing::RayTracer;
use crate::simulation::inputs::SimulationInputs as BoundInputs;
use crate::simulation::inputs::StaticSimulationInputs as SimulationInputs;
use crate::simulation::simulator::{
    SimulationSharedInputs as BoundSharedInputs, Simulator as BoundSimulator,
};
use crate::simulation::simulator::{
    StaticSimulationSharedInputs as SimulationSharedInputs, StaticSimulator as Simulator,
};
use crate::simulation::source::Source as BoundSource;
use crate::simulation::source::StaticSource as Source;
use crate::simulation::{
    Direct, PathingCompatible, ReflectionEffectCompatible, ReflectionsCompatible,
    SimulationFlagsProvider,
};
use crate::wiring::binding::{InputBinding, PipelineBinding};
use crate::wiring::input::{InputView, StepInput};
use crate::wiring::simulation::pipeline::StaticSourceWithInputs as SourceWithInputs;
use std::collections::HashMap;
use std::hash::Hash;
use std::marker::PhantomData;

/// Runs direct simulation.
pub type StaticDirectStep<SourceId, T, R, P, RE> =
    DirectStep<SourceId, Static<(T, Direct, R, P, RE)>>;

/// A simulation step with binding-specific handles and errors.
pub struct DirectStep<SourceId, B: PipelineBinding> {
    /// The simulator used by this step.
    simulator: BoundSimulator<B>,
    /// The source identifier marker.
    _source_id: PhantomData<fn() -> SourceId>,
}

impl<T, R, P, RE> StaticDirectStep<(), T, R, P, RE>
where
    T: RayTracer,
{
    /// Creates a new direct simulation step.
    pub fn new<SourceId>(
        simulator: Simulator<T, Direct, R, P, RE>,
    ) -> StaticDirectStep<SourceId, T, R, P, RE> {
        DirectStep {
            simulator,
            _source_id: PhantomData,
        }
    }
}

impl<SourceId, B, I> SimulationStep<I> for DirectStep<SourceId, B>
where
    B: DirectBinding + 'static,
    BoundSimulator<B>: Send,
    SourceId: 'static + Clone + Send + Sync + Hash + Eq,
    I: InputView<SourceId, B::Sources, Direct>,
{
    type Output = HashMap<SourceId, DirectEffectParams>;
    type Error = B::Error;

    fn run(&mut self, frame: &I, output: &mut Self::Output) -> Result<(), Self::Error> {
        let input = frame.input();
        B::set_shared(&self.simulator, input.shared_inputs)?;

        for (_, source_with_inputs) in input.sources {
            B::set_source(
                &source_with_inputs.source,
                &source_with_inputs.simulation_inputs,
            )?;
        }

        B::run(&self.simulator)?;

        for (id, source_with_inputs) in input.sources {
            output.insert(id.clone(), B::output(&source_with_inputs.source)?);
        }

        Ok(())
    }
}

impl<SourceId, D, R, P, RE, I> InputView<SourceId, Static<(D, R, P, RE)>, Direct> for I
where
    RE: ReflectionEffectCompatible<R, RE>,
    I: AsDirectInput<SourceId, D, R, P, RE>,
{
    type Listener = Static<(D, R, P, RE)>;

    fn input(&self) -> StepInput<'_, SourceId, Static<(D, R, P, RE)>, Self::Listener> {
        let input = self.as_direct_input();

        StepInput {
            sources: input.sources,
            listener: None,
            shared_inputs: input.shared_inputs,
        }
    }
}

/// Direct simulation inputs.
#[derive(Debug)]
pub struct DirectInput<'a, SourceId, D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    /// The spatial audio sources to simulate.
    pub sources: &'a SourceEntries<SourceId, D, R, P, RE>,
    /// Shared simulation inputs applying to all sources.
    pub shared_inputs: &'a SimulationSharedInputs<D, R, P>,
}

/// Implemented by any type that can produce a [`DirectInput`] view.
pub trait AsDirectInput<SourceId, D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    /// Returns a view of this type as [`DirectInput`].
    fn as_direct_input(&self) -> DirectInput<'_, SourceId, D, R, P, RE>;
}

/// Owned input for direct simulation.
pub struct DirectInputOwned<SourceId, D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    /// The spatial audio sources to simulate.
    pub sources: Vec<(SourceId, SourceWithInputs<D, R, P, RE>)>,
    /// Shared simulation inputs applying to all sources.
    pub shared_inputs: SimulationSharedInputs<D, R, P>,
}

impl<SourceId, D, R, P, RE> AsDirectInput<SourceId, D, R, P, RE>
    for DirectInputOwned<SourceId, D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    fn as_direct_input(&self) -> DirectInput<'_, SourceId, D, R, P, RE> {
        DirectInput {
            sources: self.sources.as_slice(),
            shared_inputs: &self.shared_inputs,
        }
    }
}

/// Native operations selected by a direct simulation binding.
pub trait DirectBinding: PipelineBinding {
    /// The errors returned by this worker binding.
    type Error: std::error::Error;

    /// Updates shared simulation inputs.
    fn set_shared(
        simulator: &BoundSimulator<Self>,
        shared_inputs: &BoundSharedInputs<<Self::Sources as InputBinding>::Inputs>,
    ) -> Result<(), Self::Error>
    where
        Self: Sized;
    /// Updates the source inputs for this mode.
    fn set_source(
        source: &BoundSource<<Self::Sources as InputBinding>::Source>,
        simulation_inputs: &BoundInputs<<Self::Sources as InputBinding>::Inputs>,
    ) -> Result<(), Self::Error>;
    /// Runs the selected simulation mode.
    fn run(simulator: &BoundSimulator<Self>) -> Result<(), Self::Error>
    where
        Self: Sized;
    /// Returns the selected source output.
    fn output(
        source: &BoundSource<<Self::Sources as InputBinding>::Source>,
    ) -> Result<DirectEffectParams, Self::Error>;
}

impl<T, R, P, RE> DirectBinding for Static<(T, Direct, R, P, RE)>
where
    T: 'static + RayTracer,
    R: 'static + Send + Sync + ReflectionsCompatible<R> + SimulationFlagsProvider,
    P: 'static + Send + Sync + PathingCompatible<P> + SimulationFlagsProvider,
    RE: 'static + Send + Sync + ReflectionEffectCompatible<R, RE>,
    (): ReflectionsCompatible<R> + PathingCompatible<P>,
{
    type Error = SimulationStepError;

    fn set_shared(
        simulator: &BoundSimulator<Self>,
        shared_inputs: &SimulationSharedInputs<Direct, R, P>,
    ) -> Result<(), Self::Error> {
        Ok(simulator.set_shared_direct_inputs(shared_inputs)?)
    }

    #[inline(always)]
    fn set_source(
        source: &Source<Direct, R, P, RE>,
        simulation_inputs: &SimulationInputs<Direct, R, P>,
    ) -> Result<(), Self::Error> {
        Ok(source.set_direct_inputs(simulation_inputs)?)
    }

    #[inline(always)]
    fn run(simulator: &BoundSimulator<Self>) -> Result<(), Self::Error> {
        simulator.run_direct();

        Ok(())
    }

    #[inline(always)]
    fn output(source: &Source<Direct, R, P, RE>) -> Result<DirectEffectParams, Self::Error> {
        Ok(source.get_direct_outputs()?)
    }
}

#[cfg(feature = "dynamic")]
impl DirectBinding for Dynamic<crate::simulation::simulator::SimulationChoices> {
    type Error = crate::dynamic::SimulationError;

    fn set_shared(
        simulator: &crate::dynamic::Simulator,
        shared_inputs: &crate::dynamic::SimulationSharedInputs,
    ) -> Result<(), Self::Error> {
        simulator.set_shared_inputs(crate::SimulationFlags::DIRECT, shared_inputs)
    }

    fn set_source(
        source: &crate::dynamic::Source,
        simulation_inputs: &crate::dynamic::SimulationInputs,
    ) -> Result<(), Self::Error> {
        source.set_inputs_subset(crate::SimulationFlags::DIRECT, simulation_inputs)
    }

    fn run(simulator: &crate::dynamic::Simulator) -> Result<(), Self::Error> {
        simulator.run_direct()
    }

    fn output(source: &crate::dynamic::Source) -> Result<DirectEffectParams, Self::Error> {
        source.get_direct_outputs()
    }
}

impl<SourceId, B: PipelineBinding> From<BoundSimulator<B>> for DirectStep<SourceId, B> {
    fn from(simulator: BoundSimulator<B>) -> Self {
        Self {
            simulator,
            _source_id: PhantomData,
        }
    }
}
