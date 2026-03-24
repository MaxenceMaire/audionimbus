use super::super::simulation::SourceWithInputs;
use super::{SimulationStep, SimulationStepError};
use crate::effect::{DirectEffectParams, ReflectionEffectType};
use crate::ray_tracing::RayTracer;
use crate::simulation::{
    Direct, PathingCompatible, ReflectionEffectCompatible, ReflectionsCompatible,
    SimulationFlagsProvider, SimulationSharedInputs, Simulator,
};
use std::marker::PhantomData;

/// Runs direct simulation.
pub struct DirectStep<SourceId, T, R, P, RE>
where
    T: RayTracer,
{
    /// The [`Simulator`] used by the step.
    simulator: Simulator<T, Direct, R, P, RE>,
    _source_id: PhantomData<fn() -> SourceId>,
}

impl<T, R, P, RE> DirectStep<(), T, R, P, RE>
where
    T: RayTracer,
{
    /// Creates a new direct simulation step.
    pub fn new<SourceId>(
        simulator: Simulator<T, Direct, R, P, RE>,
    ) -> DirectStep<SourceId, T, R, P, RE> {
        DirectStep {
            simulator,
            _source_id: PhantomData,
        }
    }
}

impl<SourceId, T, R, P, RE, I> SimulationStep<I> for DirectStep<SourceId, T, R, P, RE>
where
    T: 'static + RayTracer,
    R: 'static + Send + Sync + ReflectionsCompatible<R> + SimulationFlagsProvider,
    P: 'static + Send + Sync + PathingCompatible<P> + SimulationFlagsProvider,
    RE: 'static + Send + Sync + ReflectionEffectCompatible<R, RE> + ReflectionEffectType,
    (): ReflectionsCompatible<R> + PathingCompatible<P>,
    I: AsDirectInput<SourceId, Direct, R, P, RE>,
    SourceId: 'static + Clone + Send + Sync,
{
    type Output = Vec<(SourceId, DirectEffectParams)>;
    type Error = SimulationStepError;

    fn run(&mut self, frame: &I, output: &mut Self::Output) -> Result<(), Self::Error> {
        let input = frame.as_direct_input();

        self.simulator
            .set_shared_direct_inputs(input.shared_inputs)?;

        for (
            _,
            SourceWithInputs {
                source,
                ref simulation_inputs,
            },
        ) in input.sources
        {
            source.set_direct_inputs(simulation_inputs)?;
        }

        self.simulator.run_direct();

        for (id, SourceWithInputs { source, .. }) in input.sources.iter() {
            output.push((id.clone(), source.get_direct_outputs()?));
        }

        Ok(())
    }
}

/// Direct simulation inputs.
#[derive(Debug)]
pub struct DirectInput<'a, SourceId, D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    /// The spatial audio sources to simulate.
    pub sources: &'a [(SourceId, SourceWithInputs<D, R, P, RE>)],
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
