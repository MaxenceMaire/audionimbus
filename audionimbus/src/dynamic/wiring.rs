//! Simulation pipelines and workers with runtime choices.

use super::{RayTracerKind, ReflectionAlgorithm};
use crate::binding::Dynamic;
use crate::simulation::simulator::SimulationChoices;

/// A simulation pipeline with runtime source participation.
///
/// ```
/// use audionimbus::{AudioSettings, Context, DirectSimulationSettings};
/// use audionimbus::dynamic::{Simulation, SimulationSettings, Simulator};
/// let context = Context::default();
/// let settings = SimulationSettings::new(&AudioSettings::default())
///     .with_direct(DirectSimulationSettings::default());
/// let simulator = Simulator::try_new(&context, &settings)?;
/// let mut simulation = Simulation::new::<u32>(simulator);
/// let mut worker = simulation.spawn_direct(|error| eprintln!("{error}"))?;
/// simulation.shutdown();
/// worker.join().unwrap();
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub type Simulation<SourceId = ()> =
    crate::wiring::simulation::pipeline::Simulation<SourceId, Dynamic<SimulationChoices>>;

/// A runtime source paired with its simulation inputs.
pub type SourceWithInputs = crate::wiring::simulation::pipeline::SourceWithInputs<Dynamic<()>>;

/// A running direct simulation worker.
pub type DirectSimulation<SourceId> =
    crate::wiring::simulation::direct::DirectSimulation<SourceId, Dynamic<()>>;

/// A running reflection simulation worker.
pub type ReflectionsSimulation<SourceId> =
    crate::wiring::simulation::reflections::ReflectionsSimulation<
        SourceId,
        Dynamic<()>,
        Dynamic<ReflectionAlgorithm>,
    >;

/// A running reflection and listener reverb worker.
pub type ReflectionsReverbSimulation<SourceId> =
    crate::wiring::simulation::reflections_reverb::ReflectionsReverbSimulation<
        SourceId,
        Dynamic<()>,
        Dynamic<ReflectionAlgorithm>,
    >;

/// A running pathing simulation worker.
pub type PathingSimulation<SourceId> =
    crate::wiring::simulation::pathing::PathingSimulation<SourceId, Dynamic<()>>;

/// Per-source reflection parameters retained by a runtime worker.
pub type ReflectionsOutput<SourceId> =
    crate::wiring::step::reflections::ReflectionsOutput<SourceId, Dynamic<ReflectionAlgorithm>>;

/// Per-source reflections and optional listener reverb retained by a runtime worker.
pub type ReflectionsReverbOutput<SourceId> =
    crate::wiring::step::reflections_reverb::ReflectionsReverbOutput<
        SourceId,
        Dynamic<ReflectionAlgorithm>,
    >;

/// A simulation runner accepting runtime scene commits.
pub type SimulationRunner<I, O> =
    crate::wiring::runner::worker::SimulationRunner<I, O, Dynamic<RayTracerKind>>;

#[cfg(doctest)]
/// Worker binding checks.
///
/// ```compile_fail
/// use audionimbus::wiring::Simulation;
/// fn pipeline(simulator: audionimbus::dynamic::Simulator) {
///     Simulation::new::<u32>(simulator);
/// }
/// ```
///
/// ```compile_fail
/// fn inputs(worker: &audionimbus::dynamic::DirectSimulation<u32>) {
///     worker.set_input(audionimbus::SimulationSharedInputs::default());
/// }
/// ```
struct WiringBindings;
