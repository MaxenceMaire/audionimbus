//! Probe asset.

use crate::bevy::ProbeBatch;
use crate::context::Context;
use bevy::asset::{AssetLoader, LoadContext, io::Reader};
use bevy::prelude::{
    Asset, AssetEvent, Assets, Commands, Component, DetectChanges, Entity, Handle, Has,
    MessageReader, Query, Ref, Reflect, ReflectComponent, Res,
};
use bevy::reflect::TypePath;
use std::collections::HashSet;

/// Serialized probe batch data loaded through the asset system.
#[derive(Asset, TypePath, Clone, Debug)]
pub struct ProbeBatchAsset {
    bytes: Vec<u8>,
}

impl ProbeBatchAsset {
    /// Creates a new probe batch asset from serialized bytes.
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    /// Returns the serialized bytes backing this asset.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Asset loader for [`ProbeBatchAsset`].
#[derive(Default, TypePath)]
pub struct ProbeBatchAssetLoader;

impl AssetLoader for ProbeBatchAssetLoader {
    type Asset = ProbeBatchAsset;
    type Settings = ();
    type Error = std::io::Error;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        Ok(ProbeBatchAsset::new(bytes))
    }

    fn extensions(&self) -> &[&str] {
        &["phononprobebatch", "phononprobes"]
    }
}

/// Component that automatically instantiates a [`ProbeBatch`] from a [`ProbeBatchAsset`] when the
/// backing asset finishes loading or is hot-reloaded.
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct ProbeBatchAssetSource {
    /// Asset handle.
    pub handle: Handle<ProbeBatchAsset>,
}

impl ProbeBatchAssetSource {
    /// Creates a new probe batch.
    pub fn new(handle: Handle<ProbeBatchAsset>) -> Self {
        Self { handle }
    }
}

/// Inserts or replaces [`ProbeBatch`] on entities with [`ProbeBatchAssetSource`] when their
/// backing asset becomes ready, is modified, or the stored handle changes.
pub(crate) fn sync_probe_batches_from_assets(
    mut commands: Commands,
    context: Res<Context>,
    probe_batch_assets: Res<Assets<ProbeBatchAsset>>,
    mut asset_messages: MessageReader<AssetEvent<ProbeBatchAsset>>,
    query: Query<(Entity, Ref<ProbeBatchAssetSource>, Has<ProbeBatch>)>,
) {
    let changed_asset_ids = asset_messages
        .read()
        .filter_map(|event| match event {
            AssetEvent::LoadedWithDependencies { id } | AssetEvent::Modified { id } => Some(id),
            _ => None,
        })
        .collect::<HashSet<_>>();

    if changed_asset_ids.is_empty() {
        return;
    }

    for (entity, source, has_probe_batch) in &query {
        let needs_update = !has_probe_batch
            || source.is_changed()
            || changed_asset_ids.contains(&source.handle.id());
        if !needs_update {
            continue;
        }

        let Some(asset) = probe_batch_assets.get(&source.handle) else {
            continue;
        };

        match ProbeBatch::from_asset(&context, asset) {
            Ok(probe_batch) => {
                commands.entity(entity).insert(probe_batch);
            }
            Err(error) => {
                bevy::log::error!("failed to load ProbeBatch from asset on {entity:?}: {error:?}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bevy::ProbeBatch;
    use crate::context::Context;
    use crate::serialized_object::SerializedObject;

    #[test]
    fn test_probe_batch_asset_round_trip() {
        let context = Context::default();
        let probe_batch = crate::probe::ProbeBatch::try_new(&context).unwrap();
        let mut serialized_object = SerializedObject::try_new(&context).unwrap();
        probe_batch.save(&mut serialized_object);
        let asset = ProbeBatchAsset::new(serialized_object.to_vec());

        let loaded_probe_batch = ProbeBatch::from_asset(&context, &asset).unwrap();

        assert!(!loaded_probe_batch.raw_ptr().is_null());
    }
}
