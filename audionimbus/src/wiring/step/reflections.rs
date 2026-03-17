use super::super::SourceWithInputs;
use super::SimulationStep;
use crate::effect::{ReflectionEffectParams, ReflectionEffectType};
use crate::ray_tracing::RayTracer;
use crate::simulation::{
    DirectCompatible, PathingCompatible, ReflectionEffectCompatible, Reflections,
    SimulationFlagsProvider, SimulationSharedInputs, Simulator,
};

pub struct ReflectionsStep<T, D, P, RE>
where
    T: RayTracer,
{
    pub simulator: Simulator<T, D, Reflections, P, RE>,
}

impl<T, D, P, RE, I> SimulationStep<I> for ReflectionsStep<T, D, P, RE>
where
    T: 'static + RayTracer,
    D: 'static + Send + Sync + DirectCompatible<D> + SimulationFlagsProvider,
    P: 'static + Send + Sync + PathingCompatible<P> + SimulationFlagsProvider,
    RE: 'static + Send + Sync + ReflectionEffectCompatible<Reflections, RE> + ReflectionEffectType,
    (): DirectCompatible<D> + PathingCompatible<P>,
    I: AsReflectionsInput<D, Reflections, P, RE>,
{
    type Output = ReflectionsOutput<RE>;

    fn run(&mut self, frame: &I, output: &mut Self::Output) {
        let input = frame.as_reflections_input();

        self.simulator
            .set_shared_reflections_inputs(input.shared_inputs)
            .unwrap();

        for SourceWithInputs {
            source,
            simulation_inputs,
        } in input.sources
        {
            source.set_reflections_inputs(simulation_inputs).unwrap();
        }

        self.simulator.run_reflections().unwrap();

        output.sources.extend(
            input
                .sources
                .iter()
                .map(|SourceWithInputs { source, .. }| source.get_reflections_outputs().unwrap()),
        );
    }
}

/// Reflections inputs.
#[derive(Debug)]
pub struct ReflectionsInput<'a, D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    /// The spatial audio sources whose reflections to simulate.
    pub sources: &'a [SourceWithInputs<D, R, P, RE>],
    /// Shared simulation inputs applying to all sources and the listener.
    pub shared_inputs: &'a SimulationSharedInputs<D, R, P>,
}

/// Implemented by any type that can produce a [`ReflectionsInput`] view.
pub trait AsReflectionsInput<D, R, P, RE>: Send + Sync + 'static
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    fn as_reflections_input(&self) -> ReflectionsInput<'_, D, R, P, RE>;
}

/// Owned input for reflections simulation.
pub struct ReflectionsInputOwned<D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    /// The spatial audio sources whose reflections to simulate.
    pub sources: Vec<SourceWithInputs<D, R, P, RE>>,
    /// Shared simulation inputs applying to all sources and the listener.
    pub shared_inputs: SimulationSharedInputs<D, R, P>,
}

impl<D, R, P, RE> AsReflectionsInput<D, R, P, RE> for ReflectionsInputOwned<D, R, P, RE>
where
    D: Send + Sync + 'static,
    R: Send + Sync + 'static,
    P: Send + Sync + 'static,
    RE: Send + Sync + 'static + ReflectionEffectCompatible<R, RE>,
{
    fn as_reflections_input(&self) -> ReflectionsInput<'_, D, R, P, RE> {
        ReflectionsInput {
            sources: self.sources.as_slice(),
            shared_inputs: &self.shared_inputs,
        }
    }
}

/// Combined per-source reflections output.
#[derive(Default, Debug)]
pub struct ReflectionsOutput<RE: ReflectionEffectType> {
    /// Per-source reflection effect params.
    pub sources: Vec<ReflectionEffectParams<RE>>,
}

unsafe impl<RE: ReflectionEffectType> Send for ReflectionsOutput<RE> {}

/// # Safety
///
/// It is technically unsafe because simulation threads might be rewriting the
/// IR buffer that [`ReflectionEffectParams`] points to (impulse_response field)
/// while the audio thread is still reading the previous version.
///
/// However the chance of an overlap is slim and a data race is likely inaudible.
unsafe impl<RE: ReflectionEffectType> Sync for ReflectionsOutput<RE> {}
