//! Runtime-selected configuration.

pub use crate::baking::pathing::baker::DynamicPathBaker as PathBaker;
pub use crate::baking::reflections::baker::{
    DynamicBakeError as BakeError, DynamicReflectionsBaker as ReflectionsBaker,
};
pub use crate::effect::reflections::binding::ReflectionAlgorithm;
pub use crate::effect::reflections::effect::{
    DynamicReflectionEffect as ReflectionEffect,
    DynamicReflectionEffectParams as ReflectionEffectParams, ReflectionError,
};
pub use crate::effect::reflections::mixer::DynamicReflectionMixer as ReflectionMixer;
pub use crate::geometry::instanced_mesh::{
    DynamicInstancedMesh as InstancedMesh, DynamicInstancedMeshSettings as InstancedMeshSettings,
};
pub use crate::geometry::scene::{
    DynamicScene as Scene, RayTracerKind, RayTracerSettings, SceneError,
};
pub use crate::geometry::static_mesh::DynamicStaticMesh as StaticMesh;
pub use crate::simulation::inputs::{
    DynamicSimulationInputs as SimulationInputs,
    DynamicSimulationParameters as SimulationParameters, ReflectionsParameters,
};
pub use crate::simulation::settings::{
    DynamicSimulationSettings as SimulationSettings, ReflectionsSettings,
};
pub use crate::simulation::simulator::{
    DynamicSimulationError as SimulationError,
    DynamicSimulationSharedInputs as SimulationSharedInputs, DynamicSimulator as Simulator,
};
pub use crate::simulation::source::{
    DynamicSimulationOutputs as SimulationOutputs, DynamicSource as Source,
};
#[cfg(feature = "wiring")]
pub use crate::wiring::{
    runner::worker::DynamicSimulationRunner as SimulationRunner,
    simulation::{
        direct::DynamicDirectSimulation as DirectSimulation,
        pathing::DynamicPathingSimulation as PathingSimulation,
        pipeline::{DynamicSimulation as Simulation, DynamicSourceWithInputs as SourceWithInputs},
        reflections::DynamicReflectionsSimulation as ReflectionsSimulation,
        reflections_reverb::DynamicReflectionsReverbSimulation as ReflectionsReverbSimulation,
    },
    step::{
        reflections::DynamicReflectionsOutput as ReflectionsOutput,
        reflections_reverb::DynamicReflectionsReverbOutput as ReflectionsReverbOutput,
    },
};

/// Bevy components and resources with runtime choices.
#[cfg(feature = "bevy")]
pub mod bevy {
    pub use crate::bevy::asset::{
        DynamicSceneAssetSource as SceneAssetSource, ProbeBatchAsset, ProbeBatchAssetLoader,
        ProbeBatchAssetSource, SceneAsset, SceneAssetLoader,
    };
    pub use crate::bevy::debug::{
        DynamicSpatialAudioDebugPlugin as SpatialAudioDebugPlugin, WireframeColor, WireframePalette,
    };
    pub use crate::bevy::error::{
        DynamicIntegrationErrorEvent as IntegrationErrorEvent,
        DynamicSimulationErrorEvent as SimulationErrorEvent, IntegrationOperation,
    };
    pub use crate::bevy::geometry::{
        DynamicScene as Scene, InstancedMesh, MainScene, StaticMesh, SubSceneOf,
    };
    pub use crate::bevy::hrtf::*;
    pub use crate::bevy::plugin::{
        DynamicSpatialAudioPlugin as SpatialAudioPlugin, ReflectionsRunner, RunnerSettings,
    };
    pub use crate::bevy::probe::{ProbeArray, ProbeBatch};
    pub use crate::bevy::runner::{
        DynamicDirectSimulation as DirectSimulation, DynamicPathingSimulation as PathingSimulation,
        DynamicReflectionsReverbSimulation as ReflectionsReverbSimulation,
        DynamicReflectionsSimulation as ReflectionsSimulation,
    };
    pub use crate::bevy::simulation::{
        DynamicSimulation as Simulation, DynamicSimulationSharedInputs as SimulationSharedInputs,
        SimulationThread,
    };
    pub use crate::bevy::source::{
        DynamicSource as Source, DynamicSourceParameters as SourceParameters, Listener,
    };
    pub use crate::bevy::system_set::SpatialAudioSet;
}
