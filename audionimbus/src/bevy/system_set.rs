//! System set for spatial audio scheduling.

use bevy::prelude::{Reflect, SystemSet};

/// System sets used to group related spatial audio systems.
#[derive(SystemSet, Reflect, Debug, Clone, PartialEq, Eq, Hash)]
pub enum SpatialAudioSet {
    /// Asset instantiation.
    SyncAssets,
    /// Scene and geometry synchronization.
    SyncGeometry,
    /// Probe batch synchronization.
    SyncProbes,
    /// Source synchronization.
    SyncSources,
    /// Shared simulator input synchronization.
    SyncSimulationSharedInputs,
    /// Frame submission to background simulation threads.
    SyncFrames,
    /// Error propagation.
    PropagateErrors,
}
