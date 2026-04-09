//! Asset integration for serialized acoustic data.

use crate::context::Context;
use crate::error::SteamAudioError;
use crate::serialized_object::SerializedObject;
use bevy::asset::{Asset, AssetLoader, LoadContext, io::Reader};
use bevy::reflect::TypePath;

/// Serialized scene data loaded through the asset system.
#[derive(Asset, TypePath, Clone, Debug)]
pub struct SceneAsset {
    bytes: Vec<u8>,
}

impl SceneAsset {
    /// Creates a new scene asset from serialized bytes.
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    /// Returns the serialized bytes backing this asset.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

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

/// Asset loader for [`SceneAsset`].
#[derive(Default, TypePath)]
pub struct SceneAssetLoader;

impl AssetLoader for SceneAssetLoader {
    type Asset = SceneAsset;
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
        Ok(SceneAsset::new(bytes))
    }

    fn extensions(&self) -> &[&str] {
        &["phononscene"]
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bevy::{
        DefaultSimulationConfiguration, ProbeBatch as BevyProbeBatch, Scene as BevyScene,
    };
    use crate::geometry::Scene as RawScene;
    use crate::probe::ProbeBatch as RawProbeBatch;
    use crate::ray_tracing::DefaultRayTracer;

    #[test]
    fn test_scene_asset_round_trip() {
        let context = Context::default();
        let scene = RawScene::<DefaultRayTracer>::try_new(&context).unwrap();
        let serialized_object = SerializedObject::try_new(&context).unwrap();
        unsafe {
            audionimbus_sys::iplSceneSave(scene.raw_ptr(), serialized_object.raw_ptr());
        }
        let asset = SceneAsset::new(serialized_object.to_vec());

        let loaded_scene =
            BevyScene::<DefaultSimulationConfiguration>::from_asset(&context, &asset).unwrap();

        assert!(!loaded_scene.raw_ptr().is_null());
    }

    #[test]
    fn test_probe_batch_asset_round_trip() {
        let context = Context::default();
        let probe_batch = RawProbeBatch::try_new(&context).unwrap();
        let mut serialized_object = SerializedObject::try_new(&context).unwrap();
        probe_batch.save(&mut serialized_object);
        let asset = ProbeBatchAsset::new(serialized_object.to_vec());

        let loaded_probe_batch = BevyProbeBatch::from_asset(&context, &asset).unwrap();

        assert!(!loaded_probe_batch.raw_ptr().is_null());
    }
}
