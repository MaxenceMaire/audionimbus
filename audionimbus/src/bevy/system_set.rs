//! System set for spatial audio scheduling.

use bevy::prelude::SystemSet;

/// System sets used internally to order spatial audio updates.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum SpatialAudioSet {
    SyncGeometry,
    SyncSources,
    SyncSimulationSharedInputs,
    SyncFrames,
    PropagateErrors,
}
