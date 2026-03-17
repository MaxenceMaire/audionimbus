use super::super::SourceWithInputs;
use super::SimulationStep;
use crate::effect::{DirectEffectParams, ReflectionEffectType};
use crate::ray_tracing::RayTracer;
use crate::simulation::{
    Direct, PathingCompatible, ReflectionEffectCompatible, ReflectionsCompatible,
    SimulationFlagsProvider, SimulationSharedInputs, Simulator,
};

pub struct DirectStep<T, R, P, RE>
where
    T: RayTracer,
{
    pub simulator: Simulator<T, Direct, R, P, RE>,
}

impl<T, R, P, RE, I> SimulationStep<I> for DirectStep<T, R, P, RE>
where
    T: 'static + RayTracer,
    R: 'static + Send + Sync + ReflectionsCompatible<R> + SimulationFlagsProvider,
    P: 'static + Send + Sync + PathingCompatible<P> + SimulationFlagsProvider,
    RE: 'static + Send + Sync + ReflectionEffectCompatible<R, RE> + ReflectionEffectType,
    (): ReflectionsCompatible<R> + PathingCompatible<P>,
    I: AsDirectInput<Direct, R, P, RE>,
{
    type Output = Vec<DirectEffectParams>;

    fn run(&mut self, frame: &I, output: &mut Self::Output) {
        let input = frame.as_direct_input();

        self.simulator
            .set_shared_direct_inputs(input.shared_inputs)
            .unwrap();

        for SourceWithInputs {
            source,
            simulation_inputs,
        } in input.sources
        {
            source.set_direct_inputs(simulation_inputs).unwrap();
        }

        self.simulator.run_direct();

        output.extend(
            input
                .sources
                .iter()
                .map(|SourceWithInputs { source, .. }| source.get_direct_outputs().unwrap()),
        );
    }
}

/// Direct simulation inputs.
#[derive(Debug)]
pub struct DirectInput<'a, D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    /// The spatial audio sources to simulate.
    pub sources: &'a [SourceWithInputs<D, R, P, RE>],
    /// Shared simulation inputs applying to all sources.
    pub shared_inputs: &'a SimulationSharedInputs<D, R, P>,
}

/// Implemented by any type that can produce a [`DirectInput`] view.
pub trait AsDirectInput<D, R, P, RE>: Send + Sync + 'static
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    fn as_direct_input(&self) -> DirectInput<'_, D, R, P, RE>;
}

/// Owned input for direct simulation.
pub struct DirectInputOwned<D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    /// The spatial audio sources to simulate.
    pub sources: Vec<SourceWithInputs<D, R, P, RE>>,
    /// Shared simulation inputs applying to all sources.
    pub shared_inputs: SimulationSharedInputs<D, R, P>,
}

impl<D, R, P, RE> AsDirectInput<D, R, P, RE> for DirectInputOwned<D, R, P, RE>
where
    D: Send + Sync + 'static,
    R: Send + Sync + 'static,
    P: Send + Sync + 'static,
    RE: Send + Sync + 'static + ReflectionEffectCompatible<R, RE>,
{
    fn as_direct_input(&self) -> DirectInput<'_, D, R, P, RE> {
        DirectInput {
            sources: self.sources.as_slice(),
            shared_inputs: &self.shared_inputs,
        }
    }
}
