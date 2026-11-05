//! Convenient re-exports of commonly used types and traits.
//!
//! This prelude intentionally shadows types with aliases bound to [`DefaultSimulationConfiguration`].
//! Import the corresponding items from their defining modules when you need a custom
//! [`SimulationConfiguration`].
//!
//! It also re-exports a curated set of core AudioNimbus types that are commonly paired with the
//! Bevy integration.

use super::{
    asset, configuration, debug, error, geometry, hrtf, plugin, probe, runner, simulation, source,
    system_set,
};

pub use crate::audio_buffer::{
    AudioBuffer, AudioBufferMut, AudioBufferRef, Sample, num_ambisonics_channels,
};
pub use crate::audio_settings::AudioSettings;
pub use crate::context::Context;
pub use crate::effect::ambisonics::SpeakerLayout;
pub use crate::effect::ambisonics::decode::{
    AmbisonicsDecodeEffect, AmbisonicsDecodeEffectParams, AmbisonicsDecodeEffectSettings, Rendering,
};
pub use crate::effect::binaural::{BinauralEffect, BinauralEffectParams, BinauralEffectSettings};
pub use crate::effect::direct::{DirectEffect, DirectEffectParams, DirectEffectSettings};
pub use crate::effect::reflections::{
    Convolution, ReflectionEffect, ReflectionEffectParams, ReflectionEffectSettings,
};
pub use crate::geometry::{CoordinateSystem, Direction};
pub use crate::hrtf::{HrtfInterpolation, HrtfSettings};
pub use crate::simulation::{
    ConvolutionSettings, DirectSimulationSettings, ReflectionsSharedInputs, SimulationSettings,
};
pub use crate::wiring::{ReflectionsReverbOutput, SharedSimulationOutput};

pub use asset::{
    ProbeBatchAsset, ProbeBatchAssetLoader, ProbeBatchAssetSource, SceneAsset, SceneAssetLoader,
};
pub use configuration::*;
pub use debug::{SpatialAudioDebugPlugin, WireframeColor, WireframePalette};
pub use error::*;
pub use geometry::{InstancedMesh, MainScene, StaticMesh, SubSceneOf};
pub use hrtf::*;
#[cfg(all(feature = "dynamic", not(feature = "static")))]
pub use plugin::DynamicSpatialAudioPlugin as SpatialAudioPlugin;
#[cfg(any(feature = "static", not(feature = "dynamic")))]
pub use plugin::SpatialAudioPlugin;
#[cfg(feature = "dynamic")]
pub use plugin::{ReflectionsRunner, RunnerSettings};
pub use probe::*;
pub use runner::{
    Runner, RunnerDirect, RunnerPathing, RunnerReflections, RunnerReflectionsReverb, Spawn,
    SyncFrame, ToRunner,
};
pub use simulation::*;
pub use source::{Listener, on_source_added, on_source_removed};
pub use system_set::*;

/// Default-configuration alias for [`asset::SceneAssetSource`].
///
/// Import [`asset::SceneAssetSource`] directly when you need a custom [`SimulationConfiguration`].
pub type SceneAssetSource = asset::SceneAssetSource<configuration::DefaultSimulationConfiguration>;

/// Default-configuration alias for [`geometry::Scene`].
///
/// Import [`geometry::Scene`] directly when you need a custom [`SimulationConfiguration`].
#[cfg(any(feature = "static", not(feature = "dynamic")))]
pub type Scene = geometry::Scene<configuration::DefaultSimulationConfiguration>;

/// Default-configuration alias for [`source::Source`].
///
/// Import [`source::Source`] directly when you need a custom [`SimulationConfiguration`].
#[cfg(any(feature = "static", not(feature = "dynamic")))]
pub type Source = source::Source<configuration::DefaultSimulationConfiguration>;

/// Default-configuration alias for [`source::SourceParameters`].
///
/// Import [`source::SourceParameters`] directly when you need a custom [`SimulationConfiguration`].
#[cfg(any(feature = "static", not(feature = "dynamic")))]
pub type SourceParameters = source::SourceParameters<configuration::DefaultSimulationConfiguration>;

/// Default-configuration alias for [`runner::DirectSimulation`].
///
/// Import [`runner::DirectSimulation`] directly when you need a custom [`SimulationConfiguration`].
#[cfg(any(feature = "static", not(feature = "dynamic")))]
pub type DirectSimulation = runner::DirectSimulation<configuration::DefaultSimulationConfiguration>;

/// Default-configuration alias for [`runner::ReflectionsSimulation`].
///
/// Import [`runner::ReflectionsSimulation`] directly when you need a custom
/// [`SimulationConfiguration`].
#[cfg(any(feature = "static", not(feature = "dynamic")))]
pub type ReflectionsSimulation =
    runner::ReflectionsSimulation<configuration::DefaultSimulationConfiguration>;

/// Default-configuration alias for [`runner::ReflectionsReverbSimulation`].
///
/// Import [`runner::ReflectionsReverbSimulation`] directly when you need a custom
/// [`SimulationConfiguration`].
#[cfg(any(feature = "static", not(feature = "dynamic")))]
pub type ReflectionsReverbSimulation =
    runner::ReflectionsReverbSimulation<configuration::DefaultSimulationConfiguration>;

/// Default-configuration alias for [`runner::PathingSimulation`].
///
/// Import [`runner::PathingSimulation`] directly when you need a custom [`SimulationConfiguration`].
#[cfg(any(feature = "static", not(feature = "dynamic")))]
pub type PathingSimulation =
    runner::PathingSimulation<configuration::DefaultSimulationConfiguration>;

#[cfg(all(feature = "dynamic", not(feature = "static")))]
pub use geometry::DynamicScene as Scene;

#[cfg(all(feature = "dynamic", not(feature = "static")))]
pub use source::DynamicSource as Source;

#[cfg(all(feature = "dynamic", not(feature = "static")))]
pub use source::DynamicSourceParameters as SourceParameters;

#[cfg(all(feature = "dynamic", not(feature = "static")))]
pub use runner::DynamicDirectSimulation as DirectSimulation;

#[cfg(all(feature = "dynamic", not(feature = "static")))]
pub use runner::DynamicReflectionsSimulation as ReflectionsSimulation;

#[cfg(all(feature = "dynamic", not(feature = "static")))]
pub use runner::DynamicReflectionsReverbSimulation as ReflectionsReverbSimulation;

#[cfg(all(feature = "dynamic", not(feature = "static")))]
pub use runner::DynamicPathingSimulation as PathingSimulation;
