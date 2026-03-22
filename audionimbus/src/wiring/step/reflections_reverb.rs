use super::super::simulation::SourceWithInputs;
use super::{SimulationStep, SimulationStepError};
use crate::effect::{ReflectionEffectParams, ReflectionEffectType};
use crate::ray_tracing::RayTracer;
use crate::simulation::{
    DirectCompatible, PathingCompatible, ReflectionEffectCompatible, Reflections,
    SimulationFlagsProvider, SimulationSharedInputs, Simulator,
};

/// Runs reflections and listener-centric reverb simulation simultaneously.
pub struct ReflectionsReverbStep<T, D, P, RE>
where
    T: RayTracer,
{
    /// The [`Simulator`] used by the step.
    pub simulator: Simulator<T, D, Reflections, P, RE>,
}

impl<T, D, P, RE, I> SimulationStep<I> for ReflectionsReverbStep<T, D, P, RE>
where
    T: 'static + RayTracer,
    D: 'static + Send + Sync + DirectCompatible<D> + SimulationFlagsProvider,
    P: 'static + Send + Sync + PathingCompatible<P> + SimulationFlagsProvider,
    RE: 'static + Send + Sync + ReflectionEffectCompatible<Reflections, RE> + ReflectionEffectType,
    (): DirectCompatible<D> + PathingCompatible<P> + DirectCompatible<()> + PathingCompatible<()>,
    I: AsReflectionsReverbInput<D, Reflections, P, RE>,
{
    type Output = ReflectionsReverbOutput<RE>;
    type Error = SimulationStepError;

    fn run(&mut self, frame: &I, output: &mut Self::Output) -> Result<(), Self::Error> {
        let input = frame.as_reflections_reverb_input();

        self.simulator
            .set_shared_reflections_inputs(input.shared_inputs)?;

        for SourceWithInputs {
            source,
            simulation_inputs,
        } in input.sources
        {
            source.set_reflections_inputs(simulation_inputs)?;
        }

        input
            .listener
            .source
            .set_reflections_inputs(&input.listener.simulation_inputs)?;

        self.simulator.run_reflections()?;

        for SourceWithInputs { source, .. } in input.sources.iter() {
            output.sources.push(source.get_reflections_outputs()?);
        }

        output.listener = Some(input.listener.source.get_reflections_outputs()?);

        Ok(())
    }
}

/// Reflections and reverb simulation inputs.
#[derive(Debug)]
pub struct ReflectionsReverbInput<'a, D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    /// The spatial audio sources whose reflections to simulate.
    pub sources: &'a [SourceWithInputs<D, R, P, RE>],
    /// The listener, used for listener-centric reverb simulation.
    pub listener: &'a SourceWithInputs<(), R, (), RE>,
    /// Shared simulation inputs applying to all sources and the listener.
    pub shared_inputs: &'a SimulationSharedInputs<D, R, P>,
}

/// Implemented by any type that can produce a [`ReflectionsReverbInput`] view.
pub trait AsReflectionsReverbInput<D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    /// Returns a view of this type as [`ReflectionsReverbInput`].
    fn as_reflections_reverb_input(&self) -> ReflectionsReverbInput<'_, D, R, P, RE>;
}

/// Owned input for reflections and reverb simulation.
pub struct ReflectionsReverbInputOwned<D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    /// The spatial audio sources whose reflections to simulate.
    pub sources: Vec<SourceWithInputs<D, R, P, RE>>,
    /// The listener, used for listener-centric reverb simulation.
    pub listener: SourceWithInputs<(), R, (), RE>,
    /// Shared simulation inputs applying to all sources and the listener.
    pub shared_inputs: SimulationSharedInputs<D, R, P>,
}

impl<D, R, P, RE> AsReflectionsReverbInput<D, R, P, RE> for ReflectionsReverbInputOwned<D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    fn as_reflections_reverb_input(&self) -> ReflectionsReverbInput<'_, D, R, P, RE> {
        ReflectionsReverbInput {
            sources: self.sources.as_slice(),
            listener: &self.listener,
            shared_inputs: &self.shared_inputs,
        }
    }
}

/// Combined per-source reflections and listener-centric reverb output.
#[derive(Debug)]
pub struct ReflectionsReverbOutput<RE: ReflectionEffectType> {
    /// Per-source reflection effect params.
    pub sources: Vec<ReflectionEffectParams<RE>>,
    /// Listener-centric reverb.
    /// `None` until the first simulation run completes.
    pub listener: Option<ReflectionEffectParams<RE>>,
}

impl<RE: ReflectionEffectType> Default for ReflectionsReverbOutput<RE> {
    fn default() -> Self {
        Self {
            sources: Vec::default(),
            listener: None,
        }
    }
}

unsafe impl<RE: ReflectionEffectType> Send for ReflectionsReverbOutput<RE> {}

/// # Safety
///
/// It is technically unsafe because simulation threads might be rewriting the
/// IR buffer that [`ReflectionEffectParams`] points to (impulse_response field)
/// while the audio thread is still reading the previous version.
///
/// However the chance of an overlap is slim and a data race is likely inaudible.
unsafe impl<RE: ReflectionEffectType> Sync for ReflectionsReverbOutput<RE> {}
