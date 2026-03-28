use super::super::simulation::SourceWithInputs;
use super::{SimulationStep, SimulationStepError, SourceEntries};
use crate::effect::PathEffectParams;
use crate::ray_tracing::RayTracer;
use crate::simulation::{
    DirectCompatible, Pathing, ReflectionEffectCompatible, ReflectionsCompatible,
    SimulationFlagsProvider, SimulationSharedInputs, Simulator,
};
use std::marker::PhantomData;

/// Runs pathing simulation.
pub struct PathingStep<SourceId, T, D, R, RE>
where
    T: RayTracer,
{
    /// The [`Simulator`] used by the step.
    simulator: Simulator<T, D, R, Pathing, RE>,
    _source_id: PhantomData<fn() -> SourceId>,
}

impl<T, D, R, RE> PathingStep<(), T, D, R, RE>
where
    T: RayTracer,
{
    /// Creates a new pathing simulation step.
    pub fn new<SourceId>(
        simulator: Simulator<T, D, R, Pathing, RE>,
    ) -> PathingStep<SourceId, T, D, R, RE> {
        PathingStep {
            simulator,
            _source_id: PhantomData,
        }
    }
}

impl<SourceId, T, D, R, RE, I> SimulationStep<I> for PathingStep<SourceId, T, D, R, RE>
where
    T: 'static + RayTracer,
    D: 'static + Send + Sync + DirectCompatible<D> + SimulationFlagsProvider,
    R: 'static + Send + Sync + ReflectionsCompatible<R> + SimulationFlagsProvider,
    RE: 'static + Send + Sync + ReflectionEffectCompatible<R, RE>,
    (): DirectCompatible<D> + ReflectionsCompatible<R>,
    SourceId: 'static + Clone + Send + Sync,
    I: AsPathingInput<SourceId, D, R, Pathing, RE>,
{
    type Output = Vec<(SourceId, PathEffectParams)>;
    type Error = SimulationStepError;

    fn run(&mut self, frame: &I, output: &mut Self::Output) -> Result<(), Self::Error> {
        let input = frame.as_pathing_input();

        self.simulator
            .set_shared_pathing_inputs(input.shared_inputs)?;

        for (
            _,
            SourceWithInputs {
                source,
                simulation_inputs,
            },
        ) in input.sources
        {
            source.set_pathing_inputs(simulation_inputs)?;
        }

        self.simulator.run_pathing()?;

        for (id, SourceWithInputs { source, .. }) in input.sources.iter() {
            output.push((id.clone(), source.get_pathing_outputs()?));
        }

        Ok(())
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
