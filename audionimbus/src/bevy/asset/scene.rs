//! Scene asset.

use super::with_serialized_object;
use crate::bevy::configuration::{DefaultSimulationConfiguration, SimulationConfiguration};
use crate::bevy::geometry::{MainScene, Scene};
use crate::callback::CustomRayTracingCallbacks;
use crate::context::Context;
use crate::device::{EmbreeDevice, RadeonRaysDevice};
use crate::error::SteamAudioError;
use crate::ray_tracing::{CustomRayTracer, DefaultRayTracer, Embree, RadeonRays};
use bevy::asset::{AssetLoader, LoadContext, io::Reader};
use bevy::prelude::{
    Asset, AssetEvent, Assets, Commands, Component, DetectChanges, Entity, Handle, Has,
    MessageReader, Query, Ref, Res, With,
};
use bevy::reflect::TypePath;
use std::collections::HashSet;
use std::sync::Arc;

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

/// Component that automatically instantiates a [`Scene`] from a [`SceneAsset`] when the backing
/// asset finishes loading or is hot-reloaded.
#[derive(Component)]
pub struct SceneAssetSource<C: SimulationConfiguration = DefaultSimulationConfiguration> {
    /// Asset handle.
    handle: Handle<SceneAsset>,
    /// Asset loader.
    loader: SceneLoader<C>,
}

impl<C> SceneAssetSource<C>
where
    C: SimulationConfiguration<RayTracer = DefaultRayTracer>,
{
    /// Creates a source with the default ray tracer.
    pub fn new(handle: Handle<SceneAsset>) -> Self {
        Self::with_loader(handle, |context, asset| {
            with_serialized_object(context, asset.bytes().to_vec(), |serialized_object| {
                crate::geometry::Scene::<DefaultRayTracer>::load(context, serialized_object)
            })
        })
    }
}

impl<C> SceneAssetSource<C>
where
    C: SimulationConfiguration<RayTracer = Embree>,
{
    /// Creates a source with the Embree ray tracer.
    pub fn with_embree(handle: Handle<SceneAsset>, device: EmbreeDevice) -> Self {
        Self::with_loader(handle, move |context, asset| {
            with_serialized_object(context, asset.bytes().to_vec(), |serialized_object| {
                crate::geometry::Scene::<Embree>::load_embree(
                    context,
                    device.clone(),
                    serialized_object,
                )
            })
        })
    }
}

impl<C> SceneAssetSource<C>
where
    C: SimulationConfiguration<RayTracer = RadeonRays>,
{
    /// Creates a source with the Radeon Rays ray tracer.
    pub fn with_radeon_rays(handle: Handle<SceneAsset>, device: RadeonRaysDevice) -> Self {
        Self::with_loader(handle, move |context, asset| {
            with_serialized_object(context, asset.bytes().to_vec(), |serialized_object| {
                crate::geometry::Scene::<RadeonRays>::load_radeon_rays(
                    context,
                    device.clone(),
                    serialized_object,
                )
            })
        })
    }
}

impl<C> SceneAssetSource<C>
where
    C: SimulationConfiguration<RayTracer = CustomRayTracer>,
{
    /// Creates a source with a custom ray tracer.
    pub fn with_custom(handle: Handle<SceneAsset>, callbacks: CustomRayTracingCallbacks) -> Self {
        Self::with_loader(handle, move |context, asset| {
            with_serialized_object(context, asset.bytes().to_vec(), |serialized_object| {
                crate::geometry::Scene::<CustomRayTracer>::load_custom(
                    context,
                    callbacks.clone(),
                    serialized_object,
                )
            })
        })
    }
}

impl<C: SimulationConfiguration> SceneAssetSource<C> {
    /// Creates a source component with a custom loader closure.
    fn with_loader(
        handle: Handle<SceneAsset>,
        loader: impl Fn(
            &Context,
            &SceneAsset,
        ) -> Result<crate::geometry::Scene<C::RayTracer>, SteamAudioError>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        Self {
            handle,
            loader: Arc::new(loader),
        }
    }
}

/// Scene asset loader.
type SceneLoader<C> = Arc<
    dyn Fn(
            &Context,
            &SceneAsset,
        ) -> Result<
            crate::geometry::Scene<<C as SimulationConfiguration>::RayTracer>,
            SteamAudioError,
        > + Send
        + Sync,
>;

/// Inserts or replaces [`Scene<C>`] on entities with [`SceneAssetSource<C>`] when their backing
/// asset becomes ready, is modified, or the stored handle changes.
#[allow(clippy::type_complexity)]
pub(crate) fn sync_scenes_from_assets<C: SimulationConfiguration>(
    mut commands: Commands,
    context: Res<Context>,
    scene_assets: Res<Assets<SceneAsset>>,
    mut asset_messages: MessageReader<AssetEvent<SceneAsset>>,
    query: Query<(Entity, Ref<SceneAssetSource<C>>, Has<Scene<C>>)>,
    main_scenes: Query<(), With<MainScene>>,
) {
    let changed_asset_ids = asset_messages
        .read()
        .filter_map(|event| match event {
            AssetEvent::LoadedWithDependencies { id } | AssetEvent::Modified { id } => Some(id),
            _ => None,
        })
        .collect::<HashSet<_>>();

    for (entity, source, has_scene) in &query {
        let needs_update =
            !has_scene || source.is_changed() || changed_asset_ids.contains(&source.handle.id());
        if !needs_update {
            continue;
        }

        let Some(asset) = scene_assets.get(&source.handle) else {
            continue;
        };

        match (source.loader)(&context, asset) {
            Ok(inner_scene) => {
                let had_main_scene = main_scenes.contains(entity);

                commands.entity(entity).insert(Scene::<C>(inner_scene));

                if had_main_scene {
                    // Explicit remove guarantees `On<Add, MainScene>` fires.
                    commands
                        .entity(entity)
                        .remove::<MainScene>()
                        .insert(MainScene);
                }
            }
            Err(error) => {
                bevy::log::error!("failed to load Scene from asset on {entity:?}: {error:?}");
            }
        }
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
