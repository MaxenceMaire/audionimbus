//! Probe asset.

use bevy::asset::{Asset, AssetLoader, LoadContext, io::Reader};
use bevy::reflect::TypePath;

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
