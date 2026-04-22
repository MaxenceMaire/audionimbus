use super::super::simulation::SourceWithInputs;
use super::{SimulationStep, SimulationStepError, SourceEntries};
use crate::effect::{ReflectionEffectParams, ReflectionEffectType};
use crate::ray_tracing::RayTracer;
use crate::simulation::{
    DirectCompatible, PathingCompatible, ReflectionEffectCompatible, Reflections,
    SimulationFlagsProvider, SimulationSharedInputs, Simulator,
};
use std::collections::HashMap;
use std::hash::Hash;
use std::marker::PhantomData;

/// Runs reflections and listener-centric reverb simulation simultaneously.
pub struct ReflectionsReverbStep<SourceId, T, D, P, RE>
where
    T: RayTracer,
{
    /// The [`Simulator`] used by the step.
    simulator: Simulator<T, D, Reflections, P, RE>,
    _source_id: PhantomData<fn() -> SourceId>,
}

impl<T, D, P, RE> ReflectionsReverbStep<(), T, D, P, RE>
where
    T: RayTracer,
{
    /// Creates a new reflections and reverb simulation step.
    pub fn new<SourceId>(
        simulator: Simulator<T, D, Reflections, P, RE>,
    ) -> ReflectionsReverbStep<SourceId, T, D, P, RE> {
        ReflectionsReverbStep {
            simulator,
            _source_id: PhantomData,
        }
    }
}

impl<SourceId, T, D, P, RE, I> SimulationStep<I> for ReflectionsReverbStep<SourceId, T, D, P, RE>
where
    T: 'static + RayTracer,
    D: 'static + Send + Sync + DirectCompatible<D> + SimulationFlagsProvider,
    P: 'static + Send + Sync + PathingCompatible<P> + SimulationFlagsProvider,
    RE: 'static + Send + Sync + ReflectionEffectCompatible<Reflections, RE> + ReflectionEffectType,
    (): DirectCompatible<D>
        + PathingCompatible<P>
        + DirectCompatible<I::LD>
        + PathingCompatible<I::LP>,
    SourceId: 'static + Clone + Send + Sync + Hash + Eq,
    I: AsReflectionsReverbInput<SourceId, D, Reflections, P, RE>,
    I::LD: 'static + Send + Sync + DirectCompatible<I::LD> + SimulationFlagsProvider,
    I::LP: 'static + Send + Sync + PathingCompatible<I::LP> + SimulationFlagsProvider,
{
    type Output = ReflectionsReverbOutput<SourceId, RE>;
    type Error = SimulationStepError;

    fn run(&mut self, frame: &I, output: &mut Self::Output) -> Result<(), Self::Error> {
        let input = frame.as_reflections_reverb_input();

        self.simulator
            .set_shared_reflections_inputs(input.shared_inputs)?;

        for (
            _,
            SourceWithInputs {
                source,
                simulation_inputs,
            },
        ) in input.sources
        {
            source.set_reflections_inputs(simulation_inputs)?;
        }

        if let Some(listener) = input.listener {
            listener
                .source
                .set_reflections_inputs(&listener.simulation_inputs)?;
        }

        self.simulator.run_reflections()?;

        for (id, SourceWithInputs { source, .. }) in input.sources.iter() {
            output
                .sources
                .insert(id.clone(), source.get_reflections_outputs()?);
        }

        if let Some(listener) = input.listener {
            output.listener = Some(listener.source.get_reflections_outputs()?);
        }

        Ok(())
    }
}

/// Reflections and reverb simulation inputs.
#[derive(Debug)]
pub struct ReflectionsReverbInput<'a, SourceId, D, R, P, RE, LD = D, LP = P>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    /// The spatial audio sources whose reflections to simulate.
    pub sources: &'a SourceEntries<SourceId, D, R, P, RE>,
    /// The listener, used for listener-centric reverb simulation.
    pub listener: Option<&'a SourceWithInputs<LD, R, LP, RE>>,
    /// Shared simulation inputs applying to all sources and the listener.
    pub shared_inputs: &'a SimulationSharedInputs<D, R, P>,
}

/// Implemented by any type that can produce a [`ReflectionsReverbInput`] view.
pub trait AsReflectionsReverbInput<SourceId, D, R, P, RE>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    type LD;
    type LP;

    /// Returns a view of this type as [`ReflectionsReverbInput`].
    fn as_reflections_reverb_input(
        &self,
    ) -> ReflectionsReverbInput<'_, SourceId, D, R, P, RE, Self::LD, Self::LP>;
}

/// Owned input for reflections and reverb simulation.
pub struct ReflectionsReverbInputOwned<SourceId, D, R, P, RE, LD = D, LP = P>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    /// The spatial audio sources whose reflections to simulate.
    pub sources: Vec<(SourceId, SourceWithInputs<D, R, P, RE>)>,
    /// The listener, used for listener-centric reverb simulation.
    pub listener: Option<SourceWithInputs<LD, R, LP, RE>>,
    /// Shared simulation inputs applying to all sources and the listener.
    pub shared_inputs: SimulationSharedInputs<D, R, P>,
}

impl<SourceId, D, R, P, RE, LD, LP> AsReflectionsReverbInput<SourceId, D, R, P, RE>
    for ReflectionsReverbInputOwned<SourceId, D, R, P, RE, LD, LP>
where
    RE: ReflectionEffectCompatible<R, RE>,
{
    type LD = LD;
    type LP = LP;

    fn as_reflections_reverb_input(
        &self,
    ) -> ReflectionsReverbInput<'_, SourceId, D, R, P, RE, LD, LP> {
        ReflectionsReverbInput {
            sources: self.sources.as_slice(),
            listener: self.listener.as_ref(),
            shared_inputs: &self.shared_inputs,
        }
    }
}

/// Combined per-source reflections and listener-centric reverb output.
#[derive(Debug)]
pub struct ReflectionsReverbOutput<SourceId, RE>
where
    SourceId: Hash + Eq,
    RE: ReflectionEffectType,
{
    /// Per-source reflection effect params.
    pub sources: HashMap<SourceId, ReflectionEffectParams<RE>>,
    /// Listener-centric reverb.
    ///
    /// `None` if no listener is present in the scene, or until the first simulation run with a
    /// listener completes.
    pub listener: Option<ReflectionEffectParams<RE>>,
}

impl<SourceId, RE> Default for ReflectionsReverbOutput<SourceId, RE>
where
    SourceId: Hash + Eq,
    RE: ReflectionEffectType,
{
    fn default() -> Self {
        Self {
            sources: HashMap::new(),
            listener: None,
        }
    }
}

unsafe impl<SourceId, RE> Send for ReflectionsReverbOutput<SourceId, RE>
where
    SourceId: Send + Hash + Eq,
    RE: ReflectionEffectType,
{
}

/// # Safety
///
/// It is technically unsafe because simulation threads might be rewriting the
/// IR buffer that [`ReflectionEffectParams`] points to (impulse_response field)
/// while the audio thread is still reading the previous version.
///
/// However the chance of an overlap is slim and a data race is likely inaudible.
unsafe impl<SourceId, RE> Sync for ReflectionsReverbOutput<SourceId, RE>
where
    SourceId: Sync + Hash + Eq,
    RE: ReflectionEffectType,
{
}
