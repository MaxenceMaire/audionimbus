use super::super::SourceWithInputs;
use super::{SimulationStep, SimulationStepError};
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
    type Error = SimulationStepError;

    fn run(&mut self, frame: &I, output: &mut Self::Output) -> Result<(), Self::Error> {
        let input = frame.as_reflections_input();

        self.simulator
            .set_shared_reflections_inputs(input.shared_inputs)?;

        for SourceWithInputs {
            source,
            simulation_inputs,
        } in input.sources
        {
            source.set_reflections_inputs(simulation_inputs)?;
        }

        self.simulator.run_reflections()?;

        for SourceWithInputs { source, .. } in input.sources.iter() {
            output.sources.push(source.get_reflections_outputs()?);
        }

        Ok(())
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
pub trait AsReflectionsInput<D, R, P, RE>
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
    RE: ReflectionEffectCompatible<R, RE>,
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
/// `ReflectionsOutput` contains [`ReflectionEffectParams`] values that hold raw pointers to
/// impulse response buffers managed internally by Steam Audio.
/// When the simulation thread completes a new run, Steam Audio may overwrite these buffers before
/// the audio thread has finished reading the previous snapshot.
///
/// It can result in a single frame of corrupted reverb, which is inaudible in practice.
///
/// An alternative would be to copy the IR data on each simulation run, which is expensive.
/// The tradeoff is deliberate.
unsafe impl<RE: ReflectionEffectType> Sync for ReflectionsOutput<RE> {}
