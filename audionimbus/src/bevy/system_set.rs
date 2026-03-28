use bevy::prelude::SystemSet;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum SpatialAudioSet {
    SyncSources,
    SyncFrames,
}
