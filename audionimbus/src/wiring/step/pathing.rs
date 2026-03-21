use super::super::SourceWithInputs;
use super::{SimulationStep, SimulationStepError};
use crate::effect::PathEffectParams;
use crate::ray_tracing::RayTracer;
use crate::simulation::{
    DirectCompatible, Pathing, ReflectionEffectCompatible, ReflectionsCompatible,
    SimulationFlagsProvider, SimulationSharedInputs, Simulator,
};

pub struct PathingStep<T, D, R, RE>
where
    T: RayTracer,
{
    pub simulator: Simulator<T, D, R, Pathing, RE>,
}

impl<T, D, R, RE, I> SimulationStep<I> for PathingStep<T, D, R, RE>
where
    T: 'static + RayTracer,
    D: 'static + Send + Sync + DirectCompatible<D> + SimulationFlagsProvider,
    R: 'static + Send + Sync + ReflectionsCompatible<R> + SimulationFlagsProvider,
    RE: 'static + Send + Sync + ReflectionEffectCompatible<R, RE>,
    (): DirectCompatible<D> + ReflectionsCompatible<R>,
    I: AsPathingInput<D, R, Pathing, RE>,
{
    type Output = Vec<PathEffectParams>;
    type Error = SimulationStepError;

    fn run(&mut self, frame: &I, output: &mut Self::Output) -> Result<(), Self::Error> {
        let input = frame.as_pathing_input();

        self.simulator
            .set_shared_pathing_inputs(input.shared_inputs)?;

        for SourceWithInputs {
            source,
            simulation_inputs,
        } in input.sources
        {
            source.set_pathing_inputs(&simulation_inputs)?;
        }

        self.simulator.run_pathing()?;

        for SourceWithInputs { source, .. } in input.sources.iter() {
            output.push(source.get_pathing_outputs()?);
        }

        Ok(())
    }
}

/// Pathing simulation inputs.
#[derive(Debug)]
pub struct PathingInput<'a, D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    /// The spatial audio sources whose paths to simulate.
    pub sources: &'a [SourceWithInputs<D, R, P, RE>],
    /// Shared simulation inputs applying to all sources.
    pub shared_inputs: &'a SimulationSharedInputs<D, R, P>,
}

/// Implemented by any type that can produce a [`PathingInput`] view.
pub trait AsPathingInput<D, R, P, RE>: Send + Sync + 'static
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    fn as_pathing_input(&self) -> PathingInput<'_, D, R, P, RE>;
}

/// Owned input for pathing simulation.
pub struct PathingInputOwned<D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    /// The spatial audio sources whose paths to simulate.
    pub sources: Vec<SourceWithInputs<D, R, P, RE>>,
    /// Shared simulation inputs applying to all sources.
    pub shared_inputs: SimulationSharedInputs<D, R, P>,
}

impl<D, R, P, RE> AsPathingInput<D, R, P, RE> for PathingInputOwned<D, R, P, RE>
where
    D: Send + Sync + 'static,
    R: Send + Sync + 'static,
    P: Send + Sync + 'static,
    RE: Send + Sync + 'static + ReflectionEffectCompatible<R, RE>,
{
    fn as_pathing_input(&self) -> PathingInput<'_, D, R, P, RE> {
        PathingInput {
            sources: self.sources.as_slice(),
            shared_inputs: &self.shared_inputs,
        }
    }
}
