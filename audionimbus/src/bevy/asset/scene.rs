//! Scene asset.

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bevy::configuration::DefaultSimulationConfiguration;
    use crate::bevy::geometry::Scene;
    use crate::context::Context;
    use crate::ray_tracing::DefaultRayTracer;
    use crate::serialized_object::SerializedObject;

    #[test]
    fn test_scene_asset_round_trip() {
        let context = Context::default();
        let scene = crate::geometry::Scene::<DefaultRayTracer>::try_new(&context).unwrap();
        let serialized_object = SerializedObject::try_new(&context).unwrap();
        unsafe {
            audionimbus_sys::iplSceneSave(scene.raw_ptr(), serialized_object.raw_ptr());
        }
        let asset = SceneAsset::new(serialized_object.to_vec());

        let loaded_scene =
            Scene::<DefaultSimulationConfiguration>::from_asset(&context, &asset).unwrap();

        assert!(!loaded_scene.raw_ptr().is_null());
    }
}
