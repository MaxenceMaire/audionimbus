//! Asset integration for serialized acoustic data.

use crate::context::Context;
use crate::error::SteamAudioError;
use crate::serialized_object::SerializedObject;

mod probe;
mod scene;

pub use probe::*;
pub use scene::*;

/// Constructs a [`SerializedObject`] from an owned byte buffer and passes it to `load`.
pub(crate) fn with_serialized_object<T>(
    context: &Context,
    mut bytes: Vec<u8>,
    load: impl FnOnce(&SerializedObject) -> Result<T, SteamAudioError>,
) -> Result<T, SteamAudioError> {
    let serialized_object = SerializedObject::try_with_buffer(context, &mut bytes)?;
    load(&serialized_object)
}

/// Constructs a [`SerializedObject`] from an owned byte buffer and passes it mutably to `load`.
pub(crate) fn with_serialized_object_mut<T>(
    context: &Context,
    mut bytes: Vec<u8>,
    load: impl FnOnce(&mut SerializedObject) -> Result<T, SteamAudioError>,
) -> Result<T, SteamAudioError> {
    let mut serialized_object = SerializedObject::try_with_buffer(context, &mut bytes)?;
    load(&mut serialized_object)
}
