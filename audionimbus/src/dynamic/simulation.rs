//! Runtime-selected simulation and source participation.

use super::{RayTracerKind, ReflectionAlgorithm, ReflectionsParameters};
use crate::binding::Dynamic;
use crate::simulation::simulator::SimulationChoices;
use crate::simulation::source::{OutputSelection, SourceChoices};
use crate::simulation::{inputs, settings, simulator, source};
use crate::{DirectSimulationParameters, PathingSimulationParameters, SimulationFlags};
use std::fmt;

/// Creation settings with runtime simulation modes and algorithms.
pub type SimulationSettings = settings::SimulationSettings<Dynamic<SimulationChoices>>;

/// A simulator with runtime-selected modes, backend, and reflection algorithm.
///
/// Static and runtime simulation handles cannot be mixed.
///
/// ```
/// use audionimbus::{AudioSettings, Context, CoordinateSystem, DirectSimulationParameters,
///     DirectSimulationSettings, DistanceAttenuationModel, SimulationFlags};
/// use audionimbus::dynamic::{SimulationInputs, SimulationParameters, SimulationSettings,
///     SimulationSharedInputs, Simulator, Source};
/// let context = Context::default();
/// let settings = SimulationSettings::new(&AudioSettings::default())
///     .with_direct(DirectSimulationSettings::default());
/// let simulator = Simulator::try_new(&context, &settings)?;
/// let source = Source::try_new(&simulator, SimulationFlags::DIRECT)?;
/// simulator.try_add_source(&source)?;
/// simulator.commit();
/// source.set_inputs(&SimulationInputs {
///     source: CoordinateSystem::default(),
///     parameters: SimulationParameters {
///         direct: Some(DirectSimulationParameters::new()
///             .with_distance_attenuation(DistanceAttenuationModel::default())),
///         ..Default::default()
///     },
/// })?;
/// simulator.set_shared_inputs(SimulationFlags::DIRECT, &SimulationSharedInputs::default())?;
/// simulator.run_direct()?;
/// let parameters = source.get_direct_outputs()?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub type Simulator = simulator::Simulator<Dynamic<SimulationChoices>>;

/// A source using a subset of its creating simulator's available modes.
pub type Source = source::Source<Dynamic<SourceChoices>>;

/// Source coordinates and runtime participation parameters.
pub type SimulationInputs = inputs::SimulationInputs<Dynamic<()>>;

/// Listener coordinates and shared runtime simulation inputs.
pub type SimulationSharedInputs = simulator::SimulationSharedInputs<Dynamic<()>>;

/// Selected source outputs and their retained resources.
pub type SimulationOutputs = source::SimulationOutputs<Dynamic<OutputSelection>>;

/// Optional source participation parameters for each simulation mode.
#[derive(Clone, Default, Debug)]
pub struct SimulationParameters {
    /// The direct simulation parameters.
    pub direct: Option<DirectSimulationParameters>,
    /// The reflection simulation parameters and algorithm.
    pub reflections: Option<ReflectionsParameters>,
    /// The pathing simulation parameters and probes.
    pub pathing: Option<PathingSimulationParameters>,
}

impl SimulationParameters {
    /// Returns the modes enabled by these parameters.
    pub fn flags(&self) -> SimulationFlags {
        let mut flags = SimulationFlags::empty();
        if self.direct.is_some() {
            flags |= SimulationFlags::DIRECT;
        }
        if self.reflections.is_some() {
            flags |= SimulationFlags::REFLECTIONS;
        }
        if self.pathing.is_some() {
            flags |= SimulationFlags::PATHING;
        }

        flags
    }
}

/// Errors from runtime simulation construction, inputs, and output retrieval.
#[derive(Debug)]
pub enum SimulationError {
    /// A native operation failed.
    SteamAudio(crate::SteamAudioError),
    /// Simulation parameters exceed creation limits.
    ParameterValidation(crate::ParameterValidationError),
    /// Simulation requires a committed scene or probes.
    Simulation(crate::SimulationError),
    /// Requested modes are unavailable.
    DisabledModes(SimulationFlags),
    /// Requested flags contain unknown bits.
    UnknownFlags(u32),
    /// Required shared inputs are absent for these modes.
    MissingInputs(SimulationFlags),
    /// The scene and simulator use different ray tracers.
    RayTracerMismatch {
        /// The simulator's ray tracer.
        expected: RayTracerKind,
        /// The scene's ray tracer.
        actual: RayTracerKind,
    },
    /// Source inputs use a different reflection algorithm.
    AlgorithmMismatch {
        /// The source's reflection algorithm.
        expected: ReflectionAlgorithm,
        /// The input parameters' reflection algorithm.
        actual: ReflectionAlgorithm,
    },
    /// Related handles use different native devices.
    DeviceMismatch,
    /// A different simulator created the supplied source.
    ForeignSource,
}

impl fmt::Display for SimulationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SteamAudio(error) => error.fmt(formatter),
            Self::ParameterValidation(error) => error.fmt(formatter),
            Self::Simulation(error) => error.fmt(formatter),
            Self::DisabledModes(flags) => {
                write!(formatter, "simulation modes are unavailable: {flags:?}")
            }
            Self::UnknownFlags(bits) => write!(formatter, "unknown simulation flags: {bits:#x}"),
            Self::MissingInputs(flags) => {
                write!(formatter, "missing shared simulation inputs: {flags:?}")
            }
            Self::RayTracerMismatch { expected, actual } => write!(
                formatter,
                "ray tracer mismatch: expected {expected:?}, got {actual:?}"
            ),
            Self::AlgorithmMismatch { expected, actual } => write!(
                formatter,
                "reflection algorithm mismatch: expected {expected:?}, got {actual:?}"
            ),
            Self::DeviceMismatch => write!(formatter, "simulation devices do not match"),
            Self::ForeignSource => write!(formatter, "another simulator created this source"),
        }
    }
}

impl std::error::Error for SimulationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SteamAudio(error) => Some(error),
            Self::ParameterValidation(error) => Some(error),
            Self::Simulation(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(doctest)]
/// Simulation binding checks.
///
/// ```compile_fail
/// use audionimbus::{Simulator, ray_tracing::DefaultRayTracer};
/// fn add(simulator: &Simulator<DefaultRayTracer>, source: &audionimbus::dynamic::Source) {
///     simulator.add_source(source);
/// }
/// ```
///
/// ```compile_fail
/// fn add(simulator: &audionimbus::dynamic::Simulator, source: &audionimbus::Source) {
///     simulator.try_add_source(source);
/// }
/// ```
///
/// ```compile_fail
/// use audionimbus::{Simulator, ray_tracing::DefaultRayTracer};
/// fn scene(simulator: &mut Simulator<DefaultRayTracer>, scene: &audionimbus::dynamic::Scene) {
///     simulator.set_scene(scene);
/// }
/// ```
///
/// ```compile_fail
/// fn scene(simulator: &mut audionimbus::dynamic::Simulator, scene: &audionimbus::Scene) {
///     simulator.try_set_scene(scene);
/// }
/// ```
///
/// ```compile_fail
/// fn inputs(source: &audionimbus::Source, inputs: &audionimbus::dynamic::SimulationInputs) {
///     source.set_inputs(inputs);
/// }
/// ```
///
/// ```compile_fail
/// fn inputs(source: &audionimbus::dynamic::Source, inputs: &audionimbus::SimulationInputs) {
///     source.set_inputs(inputs);
/// }
/// ```
///
/// ```compile_fail
/// fn settings(context: &audionimbus::Context, settings: &audionimbus::dynamic::SimulationSettings) {
///     audionimbus::Simulator::try_new(context, settings);
/// }
/// ```
///
/// ```compile_fail
/// fn settings(context: &audionimbus::Context, settings: &audionimbus::SimulationSettings<audionimbus::ray_tracing::DefaultRayTracer>) {
///     audionimbus::dynamic::Simulator::try_new(context, settings);
/// }
/// ```
///
/// ```compile_fail
/// fn outputs(source: &audionimbus::dynamic::Source) {
///     let _: audionimbus::SimulationOutputs<(), (), ()> = source.get_outputs().unwrap();
/// }
/// ```
///
/// ```compile_fail
/// fn outputs(source: &audionimbus::Source) {
///     let _: audionimbus::dynamic::SimulationOutputs = source.get_outputs().unwrap();
/// }
/// ```
///
/// ```compile_fail
/// fn mutate(simulator: &mut audionimbus::dynamic::Simulator) {
///     simulator.raw_ptr_mut();
/// }
/// ```
///
/// ```compile_fail
/// fn mutate(source: &mut audionimbus::dynamic::Source) {
///     source.raw_ptr_mut();
/// }
/// ```
struct SimulationBindings;
