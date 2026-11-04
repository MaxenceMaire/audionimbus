use super::{SimulationStep, SimulationStepError, SourceEntries};
#[cfg(feature = "dynamic")]
use crate::binding::Dynamic;
use crate::binding::Static;
use crate::effect::PathEffectParams;
use crate::ray_tracing::RayTracer;
use crate::simulation::inputs::SimulationInputs as BoundInputs;
use crate::simulation::simulator::{
    SimulationSharedInputs as BoundSharedInputs, Simulator as BoundSimulator,
};
use crate::simulation::source::Source as BoundSource;
use crate::simulation::{
    DirectCompatible, Pathing, ReflectionEffectCompatible, ReflectionsCompatible,
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

/// Runs pathing simulation.
pub type StaticPathingStep<SourceId, T, D, R, RE> =
    PathingStep<SourceId, Static<(T, D, R, Pathing, RE)>>;

/// A simulation step with binding-specific handles and errors.
pub struct PathingStep<SourceId, B: PipelineBinding> {
    /// The simulator used by this step.
    simulator: BoundSimulator<B>,
    /// The source identifier marker.
    _source_id: PhantomData<fn() -> SourceId>,
}

impl<T, D, R, RE> StaticPathingStep<(), T, D, R, RE>
where
    T: RayTracer,
{
    /// Creates a new pathing simulation step.
    pub fn new<SourceId>(
        simulator: Simulator<T, D, R, Pathing, RE>,
    ) -> StaticPathingStep<SourceId, T, D, R, RE> {
        PathingStep {
            simulator,
            _source_id: PhantomData,
        }
    }
}

impl<SourceId, B, I> SimulationStep<I> for PathingStep<SourceId, B>
where
    B: PathingBinding + 'static,
    BoundSimulator<B>: Send,
    SourceId: 'static + Clone + Send + Sync + Hash + Eq,
    I: InputView<SourceId, B::Sources, Pathing>,
{
    type Output = HashMap<SourceId, PathEffectParams>;
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

impl<SourceId, D, R, P, RE, I> InputView<SourceId, Static<(D, R, P, RE)>, Pathing> for I
where
    RE: ReflectionEffectCompatible<R, RE>,
    I: AsPathingInput<SourceId, D, R, P, RE>,
{
    type Listener = Static<(D, R, P, RE)>;

    fn input(&self) -> StepInput<'_, SourceId, Static<(D, R, P, RE)>, Self::Listener> {
        let input = self.as_pathing_input();
        StepInput {
            sources: input.sources,
            listener: None,
            shared_inputs: input.shared_inputs,
        }
    }
}

/// Pathing simulation inputs.
#[derive(Debug)]
pub struct PathingInput<'a, SourceId, D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    /// The spatial audio sources whose paths to simulate.
    pub sources: &'a SourceEntries<SourceId, D, R, P, RE>,
    /// Shared simulation inputs applying to all sources.
    pub shared_inputs: &'a SimulationSharedInputs<D, R, P>,
}

/// Implemented by any type that can produce a [`PathingInput`] view.
pub trait AsPathingInput<SourceId, D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    /// Returns a view of this type as [`PathingInput`].
    fn as_pathing_input(&self) -> PathingInput<'_, SourceId, D, R, P, RE>;
}

/// Owned input for pathing simulation.
pub struct PathingInputOwned<SourceId, D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    /// The spatial audio sources whose paths to simulate.
    pub sources: Vec<(SourceId, SourceWithInputs<D, R, P, RE>)>,
    /// Shared simulation inputs applying to all sources.
    pub shared_inputs: SimulationSharedInputs<D, R, P>,
}

impl<SourceId, D, R, P, RE> AsPathingInput<SourceId, D, R, P, RE>
    for PathingInputOwned<SourceId, D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    fn as_pathing_input(&self) -> PathingInput<'_, SourceId, D, R, P, RE> {
        PathingInput {
            sources: self.sources.as_slice(),
            shared_inputs: &self.shared_inputs,
        }
    }
}

/// Native operations selected by a pathing simulation binding.
pub trait PathingBinding: PipelineBinding {
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
    ) -> Result<PathEffectParams, Self::Error>;
}

impl<T, D, R, RE> PathingBinding for Static<(T, D, R, Pathing, RE)>
where
    T: 'static + RayTracer,
    D: 'static + Send + Sync + DirectCompatible<D> + SimulationFlagsProvider,
    R: 'static + Send + Sync + ReflectionsCompatible<R> + SimulationFlagsProvider,
    RE: 'static + Send + Sync + ReflectionEffectCompatible<R, RE>,
    (): DirectCompatible<D> + ReflectionsCompatible<R>,
{
    type Error = SimulationStepError;

    fn set_shared(
        simulator: &BoundSimulator<Self>,
        shared_inputs: &SimulationSharedInputs<D, R, Pathing>,
    ) -> Result<(), Self::Error> {
        Ok(simulator.set_shared_pathing_inputs(shared_inputs)?)
    }

    #[inline(always)]
    fn set_source(
        source: &crate::Source<D, R, Pathing, RE>,
        simulation_inputs: &crate::SimulationInputs<D, R, Pathing>,
    ) -> Result<(), Self::Error> {
        Ok(source.set_pathing_inputs(simulation_inputs)?)
    }

    #[inline(always)]
    fn run(simulator: &BoundSimulator<Self>) -> Result<(), Self::Error> {
        simulator.run_pathing()?;

        Ok(())
    }

    #[inline(always)]
    fn output(source: &crate::Source<D, R, Pathing, RE>) -> Result<PathEffectParams, Self::Error> {
        Ok(source.get_pathing_outputs()?)
    }
}

#[cfg(feature = "dynamic")]
impl PathingBinding for Dynamic<crate::simulation::simulator::SimulationChoices> {
    type Error = crate::dynamic::SimulationError;

    fn set_shared(
        simulator: &crate::dynamic::Simulator,
        shared_inputs: &crate::dynamic::SimulationSharedInputs,
    ) -> Result<(), Self::Error> {
        simulator.set_shared_inputs(crate::SimulationFlags::PATHING, shared_inputs)
    }

    fn set_source(
        source: &crate::dynamic::Source,
        simulation_inputs: &crate::dynamic::SimulationInputs,
    ) -> Result<(), Self::Error> {
        source.set_inputs_subset(crate::SimulationFlags::PATHING, simulation_inputs)
    }

    fn run(simulator: &crate::dynamic::Simulator) -> Result<(), Self::Error> {
        simulator.run_pathing()
    }

    fn output(source: &crate::dynamic::Source) -> Result<PathEffectParams, Self::Error> {
        source.get_pathing_outputs()
    }
}

impl<SourceId, B: PipelineBinding> From<BoundSimulator<B>> for PathingStep<SourceId, B> {
    fn from(simulator: BoundSimulator<B>) -> Self {
        Self {
            simulator,
            _source_id: PhantomData,
        }
    }
}
