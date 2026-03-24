use super::super::simulation::SourceWithInputs;
use super::{SimulationStep, SimulationStepError};
use crate::effect::{ReflectionEffectParams, ReflectionEffectType};
use crate::ray_tracing::RayTracer;
use crate::simulation::{
    DirectCompatible, PathingCompatible, ReflectionEffectCompatible, Reflections,
    SimulationFlagsProvider, SimulationSharedInputs, Simulator,
};
use std::marker::PhantomData;

/// Runs reflections simulation.
pub struct ReflectionsStep<SourceId, T, D, P, RE>
where
    T: RayTracer,
{
    /// The [`Simulator`] used by the step.
    simulator: Simulator<T, D, Reflections, P, RE>,
    _source_id: PhantomData<fn() -> SourceId>,
}

impl<T, D, P, RE> ReflectionsStep<(), T, D, P, RE>
where
    T: RayTracer,
{
    /// Creates a new reflections simulation step.
    pub fn new<SourceId>(
        simulator: Simulator<T, D, Reflections, P, RE>,
    ) -> ReflectionsStep<SourceId, T, D, P, RE> {
        ReflectionsStep {
            simulator,
            _source_id: PhantomData,
        }
    }
}

impl<SourceId, T, D, P, RE, I> SimulationStep<I> for ReflectionsStep<SourceId, T, D, P, RE>
where
    T: 'static + RayTracer,
    D: 'static + Send + Sync + DirectCompatible<D> + SimulationFlagsProvider,
    P: 'static + Send + Sync + PathingCompatible<P> + SimulationFlagsProvider,
    RE: 'static + Send + Sync + ReflectionEffectCompatible<Reflections, RE> + ReflectionEffectType,
    (): DirectCompatible<D> + PathingCompatible<P>,
    SourceId: 'static + Clone + Send + Sync,
    I: AsReflectionsInput<SourceId, D, Reflections, P, RE>,
{
    type Output = ReflectionsOutput<SourceId, RE>;
    type Error = SimulationStepError;

    fn run(&mut self, frame: &I, output: &mut Self::Output) -> Result<(), Self::Error> {
        let input = frame.as_reflections_input();

        self.simulator
            .set_shared_reflections_inputs(input.shared_inputs)?;

        for (
            _,
            SourceWithInputs {
                source,
                ref simulation_inputs,
            },
        ) in input.sources
        {
            source.set_reflections_inputs(simulation_inputs)?;
        }

        self.simulator.run_reflections()?;

        for (id, SourceWithInputs { source, .. }) in input.sources.iter() {
            output
                .sources
                .push((id.clone(), source.get_reflections_outputs()?));
        }

        Ok(())
    }
}

/// Reflections inputs.
#[derive(Debug)]
pub struct ReflectionsInput<'a, SourceId, D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    /// The spatial audio sources whose reflections to simulate.
    pub sources: &'a [(SourceId, SourceWithInputs<D, R, P, RE>)],
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
#[derive(Debug)]
pub struct ReflectionsOutput<SourceId, RE: ReflectionEffectType> {
    /// Per-source reflection effect params.
    pub sources: Vec<(SourceId, ReflectionEffectParams<RE>)>,
}

impl<SourceId, RE: ReflectionEffectType> Default for ReflectionsOutput<SourceId, RE> {
    fn default() -> Self {
        Self {
            sources: Vec::new(),
        }
    }
}

unsafe impl<SourceId: Send, RE: ReflectionEffectType> Send for ReflectionsOutput<SourceId, RE> {}

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
unsafe impl<SourceId: Sync, RE: ReflectionEffectType> Sync for ReflectionsOutput<SourceId, RE> {}
