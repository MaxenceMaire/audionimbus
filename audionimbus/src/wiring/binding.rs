//! Storage selected by simulation workers.

use super::simulation::pipeline::SourceWithInputs;
use crate::binding::Static;
use crate::simulation::inputs::InputBinding as CoreInputBinding;
use crate::simulation::simulator::SimulatorBinding;
use crate::simulation::source::SourceBinding;
use crate::simulation::{ReflectionEffectCompatible, SimulationFlags};

/// Source handles, parameters, and published buffers selected by a worker binding.
pub trait InputBinding {
    /// The source handle binding.
    type Source: SourceBinding;
    /// The source and shared input binding.
    type Inputs: CoreInputBinding;
    /// The published source buffers.
    type Sources<SourceId>: Default;

    /// Returns sources available to the selected worker.
    fn sources<SourceId>(
        sources: &Self::Sources<SourceId>,
        flags: SimulationFlags,
    ) -> &[(SourceId, SourceWithInputs<Self>)]
    where
        Self: Sized;
}

impl<D, R, P, RE> InputBinding for Static<(D, R, P, RE)> {
    type Source = Self;
    type Inputs = Static<(D, R, P)>;
    type Sources<SourceId> = Vec<(SourceId, SourceWithInputs<Self>)>;

    fn sources<SourceId>(
        sources: &Self::Sources<SourceId>,
        _: SimulationFlags,
    ) -> &[(SourceId, SourceWithInputs<Self>)] {
        sources
    }
}

/// Input bindings accepted by the existing public frame contracts.
pub trait FrameBinding: InputBinding {}

impl<D, R, P, RE> FrameBinding for Static<(D, R, P, RE)> where RE: ReflectionEffectCompatible<R, RE> {}

/// Simulator and source storage selected by a simulation pipeline.
pub trait PipelineBinding: SimulatorBinding {
    /// The source input and publication binding.
    type Sources: InputBinding;
}

impl<T: crate::ray_tracing::RayTracer, D, R, P, RE> PipelineBinding for Static<(T, D, R, P, RE)> {
    type Sources = Static<(D, R, P, RE)>;
}

#[cfg(feature = "dynamic")]
use crate::binding::Dynamic;
#[cfg(feature = "dynamic")]
use crate::simulation::simulator::SimulationChoices;
#[cfg(feature = "dynamic")]
use crate::simulation::source::SourceChoices;

#[cfg(feature = "dynamic")]
impl InputBinding for Dynamic<()> {
    type Source = Dynamic<SourceChoices>;
    type Inputs = Self;
    type Sources<SourceId> = SourcePartitions<SourceId>;

    fn sources<SourceId>(
        sources: &Self::Sources<SourceId>,
        flags: SimulationFlags,
    ) -> &[(SourceId, SourceWithInputs<Self>)] {
        if flags.contains(SimulationFlags::DIRECT) {
            &sources.direct
        } else if flags.contains(SimulationFlags::REFLECTIONS) {
            &sources.reflections
        } else {
            &sources.pathing
        }
    }
}

#[cfg(feature = "dynamic")]
impl FrameBinding for Dynamic<()> {}

#[cfg(feature = "dynamic")]
impl PipelineBinding for Dynamic<SimulationChoices> {
    type Sources = Dynamic<()>;
}

/// Runtime sources partitioned by their available simulation modes.
#[cfg(feature = "dynamic")]
pub struct SourcePartitions<SourceId> {
    /// The complete published source list.
    pub sources: Vec<(SourceId, SourceWithInputs<Dynamic<()>>)>,
    /// Sources available to direct simulation.
    pub direct: Vec<(SourceId, SourceWithInputs<Dynamic<()>>)>,
    /// Sources available to reflection simulation.
    pub reflections: Vec<(SourceId, SourceWithInputs<Dynamic<()>>)>,
    /// Sources available to pathing simulation.
    pub pathing: Vec<(SourceId, SourceWithInputs<Dynamic<()>>)>,
    /// The listener used by reflection reverb workers.
    pub listener: Option<SourceWithInputs<Dynamic<()>>>,
}

#[cfg(feature = "dynamic")]
impl<SourceId> Default for SourcePartitions<SourceId> {
    fn default() -> Self {
        Self {
            sources: Vec::new(),
            direct: Vec::new(),
            reflections: Vec::new(),
            pathing: Vec::new(),
            listener: None,
        }
    }
}

#[cfg(feature = "dynamic")]
impl<SourceId: Clone> SourcePartitions<SourceId> {
    /// Rebuilds each worker list from the sources' available modes.
    pub fn partition(&mut self) {
        self.direct.clear();
        self.reflections.clear();
        self.pathing.clear();

        for (id, source_with_inputs) in &self.sources {
            let flags = source_with_inputs.source.flags();
            if flags.contains(SimulationFlags::DIRECT) {
                self.direct.push((id.clone(), source_with_inputs.clone()));
            }
            if flags.contains(SimulationFlags::REFLECTIONS) {
                self.reflections
                    .push((id.clone(), source_with_inputs.clone()));
            }
            if flags.contains(SimulationFlags::PATHING) {
                self.pathing.push((id.clone(), source_with_inputs.clone()));
            }
        }
    }
}
