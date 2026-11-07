use super::super::asset::{SceneAsset, SceneAssetSource, with_serialized_object};
use super::super::binding::{ConfigurationBinding, DefaultConfiguration};
use super::super::configuration::SimulationConfiguration;
use super::super::error::{IntegrationErrorEvent, IntegrationOperation};
use super::super::simulation::Simulation;
use super::instanced_mesh::{InstancedMesh, SpawnedInstancedMesh, SubSceneOf};
use super::static_mesh::SpawnedStaticMesh;
use crate::callback::{CustomRayTracingCallbacks, ProgressCallback};
use crate::context::Context;
use crate::device::{EmbreeDevice, RadeonRaysDevice};
use crate::error::SteamAudioError;
use crate::ray_tracing::{CustomRayTracer, DefaultRayTracer, Embree, RadeonRays};
use crate::serialized_object::SerializedObject;
use bevy::ecs::{lifecycle::HookContext, world::DeferredWorld};
use bevy::prelude::{
    Add, ChildOf, Commands, Component, Entity, Local, On, Query, Reflect, ReflectComponent, Remove,
    Res, ResMut, Resource, With, Without, World,
};
use std::any::TypeId;
use std::collections::{HashMap, HashSet};
use std::ops::{Deref, DerefMut};

/// Component wrapping an [AudioNimbus scene](`crate::geometry::Scene`).
#[derive(Component, Reflect, Debug)]
#[reflect(Component, opaque)]
#[require(SceneStatus<C>)]
pub struct Scene<C: ConfigurationBinding = DefaultConfiguration>(
    pub crate::geometry::scene::Scene<C::Geometry>,
);

impl<C: ConfigurationBinding> Clone for Scene<C> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<C> Scene<C>
where
    C: SimulationConfiguration<RayTracer = DefaultRayTracer>,
{
    /// Creates a new scene.
    ///
    /// Mirrors [`Scene::<DefaultRayTracer>::try_new`](crate::geometry::Scene#method.try_new).
    pub fn try_new(context: &Context) -> Result<Self, SteamAudioError> {
        crate::geometry::scene::StaticScene::<DefaultRayTracer>::try_new(context).map(Self)
    }

    /// Loads a scene from a serialized object.
    ///
    /// Mirrors [`Scene::<DefaultRayTracer>::load`](crate::geometry::Scene#method.load).
    pub fn load(
        context: &Context,
        serialized_object: &SerializedObject,
    ) -> Result<Self, SteamAudioError> {
        crate::geometry::scene::StaticScene::<DefaultRayTracer>::load(context, serialized_object)
            .map(Self)
    }

    /// Loads a scene from a [`SceneAsset`].
    pub fn from_asset(context: &Context, asset: &SceneAsset) -> Result<Self, SteamAudioError> {
        with_serialized_object(context, asset.bytes().to_vec(), |serialized_object| {
            Self::load(context, serialized_object)
        })
    }

    /// Loads a scene from a serialized object with a progress callback.
    ///
    /// Mirrors [`Scene::<DefaultRayTracer>::load_with_progress`](crate::geometry::Scene#method.load_with_progress).
    pub fn load_with_progress(
        context: &Context,
        serialized_object: &SerializedObject,
        progress_callback: ProgressCallback,
    ) -> Result<Self, SteamAudioError> {
        crate::geometry::scene::StaticScene::<DefaultRayTracer>::load_with_progress(
            context,
            serialized_object,
            progress_callback,
        )
        .map(Self)
    }

    /// Loads a scene from a [`SceneAsset`] with a progress callback.
    pub fn from_asset_with_progress(
        context: &Context,
        asset: &SceneAsset,
        progress_callback: ProgressCallback,
    ) -> Result<Self, SteamAudioError> {
        with_serialized_object(context, asset.bytes().to_vec(), |serialized_object| {
            Self::load_with_progress(context, serialized_object, progress_callback)
        })
    }
}

impl<C> Scene<C>
where
    C: SimulationConfiguration<RayTracer = Embree>,
{
    /// Creates a new scene with the Embree ray tracer.
    ///
    /// Mirrors [`Scene::<Embree>::try_with_embree`](crate::geometry::Scene#method.try_with_embree).
    pub fn try_with_embree(
        context: &Context,
        device: EmbreeDevice,
    ) -> Result<Self, SteamAudioError> {
        crate::geometry::scene::StaticScene::<Embree>::try_with_embree(context, device).map(Self)
    }

    /// Loads a scene from a serialized object using Embree.
    ///
    /// Mirrors [`Scene::<Embree>::load_embree`](crate::geometry::Scene#method.load_embree).
    pub fn load_embree(
        context: &Context,
        device: EmbreeDevice,
        serialized_object: &SerializedObject,
    ) -> Result<Self, SteamAudioError> {
        crate::geometry::scene::StaticScene::<Embree>::load_embree(
            context,
            device,
            serialized_object,
        )
        .map(Self)
    }

    /// Loads a scene from a [`SceneAsset`] using Embree.
    pub fn from_asset_with_embree(
        context: &Context,
        device: EmbreeDevice,
        asset: &SceneAsset,
    ) -> Result<Self, SteamAudioError> {
        with_serialized_object(context, asset.bytes().to_vec(), |serialized_object| {
            Self::load_embree(context, device, serialized_object)
        })
    }

    /// Loads a scene from a serialized object using Embree with a progress callback.
    ///
    /// Mirrors [`Scene::<Embree>::load_embree_with_progress`](crate::geometry::Scene#method.load_embree_with_progress).
    pub fn load_embree_with_progress(
        context: &Context,
        device: EmbreeDevice,
        serialized_object: &SerializedObject,
        progress_callback: ProgressCallback,
    ) -> Result<Self, SteamAudioError> {
        crate::geometry::scene::StaticScene::<Embree>::load_embree_with_progress(
            context,
            device,
            serialized_object,
            progress_callback,
        )
        .map(Self)
    }

    /// Loads a scene from a [`SceneAsset`] using Embree with a progress callback.
    pub fn from_asset_with_embree_progress(
        context: &Context,
        device: EmbreeDevice,
        asset: &SceneAsset,
        progress_callback: ProgressCallback,
    ) -> Result<Self, SteamAudioError> {
        with_serialized_object(context, asset.bytes().to_vec(), |serialized_object| {
            Self::load_embree_with_progress(context, device, serialized_object, progress_callback)
        })
    }
}

impl<C> Scene<C>
where
    C: SimulationConfiguration<RayTracer = RadeonRays>,
{
    /// Creates a new scene with the Radeon Rays ray tracer.
    ///
    /// Mirrors [`Scene::<RadeonRays>::try_with_radeon_rays`](crate::geometry::Scene#method.try_with_radeon_rays).
    pub fn try_with_radeon_rays(
        context: &Context,
        device: RadeonRaysDevice,
    ) -> Result<Self, SteamAudioError> {
        crate::geometry::scene::StaticScene::<RadeonRays>::try_with_radeon_rays(context, device)
            .map(Self)
    }

    /// Loads a scene from a serialized object using Radeon Rays.
    ///
    /// Mirrors [`Scene::<RadeonRays>::load_radeon_rays`](crate::geometry::Scene#method.load_radeon_rays).
    pub fn load_radeon_rays(
        context: &Context,
        device: RadeonRaysDevice,
        serialized_object: &SerializedObject,
    ) -> Result<Self, SteamAudioError> {
        crate::geometry::scene::StaticScene::<RadeonRays>::load_radeon_rays(
            context,
            device,
            serialized_object,
        )
        .map(Self)
    }

    /// Loads a scene from a [`SceneAsset`] using Radeon Rays.
    pub fn from_asset_with_radeon_rays(
        context: &Context,
        device: RadeonRaysDevice,
        asset: &SceneAsset,
    ) -> Result<Self, SteamAudioError> {
        with_serialized_object(context, asset.bytes().to_vec(), |serialized_object| {
            Self::load_radeon_rays(context, device, serialized_object)
        })
    }

    /// Loads a scene from a serialized object using Radeon Rays with a progress callback.
    ///
    /// Mirrors [`Scene::<RadeonRays>::load_radeon_rays_with_progress`](crate::geometry::Scene#method.load_radeon_rays_with_progress).
    pub fn load_radeon_rays_with_progress(
        context: &Context,
        device: RadeonRaysDevice,
        serialized_object: &SerializedObject,
        progress_callback: ProgressCallback,
    ) -> Result<Self, SteamAudioError> {
        crate::geometry::scene::StaticScene::<RadeonRays>::load_radeon_rays_with_progress(
            context,
            device,
            serialized_object,
            progress_callback,
        )
        .map(Self)
    }

    /// Loads a scene from a [`SceneAsset`] using Radeon Rays with a progress callback.
    pub fn from_asset_with_radeon_rays_progress(
        context: &Context,
        device: RadeonRaysDevice,
        asset: &SceneAsset,
        progress_callback: ProgressCallback,
    ) -> Result<Self, SteamAudioError> {
        with_serialized_object(context, asset.bytes().to_vec(), |serialized_object| {
            Self::load_radeon_rays_with_progress(
                context,
                device,
                serialized_object,
                progress_callback,
            )
        })
    }
}

impl<C> Scene<C>
where
    C: SimulationConfiguration<RayTracer = CustomRayTracer>,
{
    /// Creates a new scene with a custom ray tracer.
    ///
    /// Mirrors [`Scene::<CustomRayTracer>::try_with_custom`](crate::geometry::Scene#method.try_with_custom).
    pub fn try_with_custom(
        context: &Context,
        callbacks: CustomRayTracingCallbacks,
    ) -> Result<Self, SteamAudioError> {
        crate::geometry::scene::StaticScene::<CustomRayTracer>::try_with_custom(context, callbacks)
            .map(Self)
    }

    /// Loads a scene from a serialized object using a custom ray tracer.
    ///
    /// Mirrors [`Scene::<CustomRayTracer>::load_custom`](crate::geometry::Scene#method.load_custom).
    pub fn load_custom(
        context: &Context,
        callbacks: CustomRayTracingCallbacks,
        serialized_object: &SerializedObject,
    ) -> Result<Self, SteamAudioError> {
        crate::geometry::scene::StaticScene::<CustomRayTracer>::load_custom(
            context,
            callbacks,
            serialized_object,
        )
        .map(Self)
    }

    /// Loads a scene from a [`SceneAsset`] using a custom ray tracer.
    pub fn from_asset_with_custom(
        context: &Context,
        callbacks: CustomRayTracingCallbacks,
        asset: &SceneAsset,
    ) -> Result<Self, SteamAudioError> {
        with_serialized_object(context, asset.bytes().to_vec(), |serialized_object| {
            Self::load_custom(context, callbacks, serialized_object)
        })
    }

    /// Loads a scene from a serialized object using a custom ray tracer with a progress callback.
    ///
    /// Mirrors [`Scene::<CustomRayTracer>::load_custom_with_progress`](crate::geometry::Scene#method.load_custom_with_progress).
    pub fn load_custom_with_progress(
        context: &Context,
        callbacks: CustomRayTracingCallbacks,
        serialized_object: &SerializedObject,
        progress_callback: ProgressCallback,
    ) -> Result<Self, SteamAudioError> {
        crate::geometry::scene::StaticScene::<CustomRayTracer>::load_custom_with_progress(
            context,
            callbacks,
            serialized_object,
            progress_callback,
        )
        .map(Self)
    }

    /// Loads a scene from a [`SceneAsset`] using a custom ray tracer with a progress callback.
    pub fn from_asset_with_custom_progress(
        context: &Context,
        callbacks: CustomRayTracingCallbacks,
        asset: &SceneAsset,
        progress_callback: ProgressCallback,
    ) -> Result<Self, SteamAudioError> {
        with_serialized_object(context, asset.bytes().to_vec(), |serialized_object| {
            Self::load_custom_with_progress(
                context,
                callbacks,
                serialized_object,
                progress_callback,
            )
        })
    }
}

impl<C: ConfigurationBinding> Deref for Scene<C> {
    type Target = crate::geometry::scene::Scene<C::Geometry>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<C: ConfigurationBinding> DerefMut for Scene<C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<C: SimulationConfiguration> From<crate::geometry::scene::StaticScene<C::RayTracer>>
    for Scene<C>
{
    fn from(scene: crate::geometry::scene::StaticScene<C::RayTracer>) -> Self {
        Self(scene)
    }
}

/// Tracks whether a [`Scene`] has uncommitted geometry changes.
#[derive(Component, Copy, Clone, Debug)]
pub(crate) struct SceneStatus<C: ConfigurationBinding> {
    /// Whether [`commit`](`crate::geometry::Scene::commit`) needs to be called.
    pub commit_needed: bool,
    /// Whether an asset reload will replace this scene after removal.
    pub replacing_asset: bool,
    /// The configuration associated with these commit flags.
    _configuration: std::marker::PhantomData<fn() -> C>,
}

impl<C: ConfigurationBinding> Default for SceneStatus<C> {
    fn default() -> Self {
        Self {
            commit_needed: false,
            replacing_asset: false,
            _configuration: Default::default(),
        }
    }
}

/// Marks the [`Scene`] that the simulator should use for ray tracing.
///
/// Adding this component selects the scene for each configuration on the entity.
/// The marker stays on the previous entity while another configuration still selects it.
#[derive(Component, Reflect, Debug)]
#[reflect(Component)]
#[component(storage = "SparseSet", on_remove = Self::clear_selections)]
pub struct MainScene;

impl MainScene {
    /// Clears the entity's selections when its marker is removed.
    fn clear_selections(mut world: DeferredWorld, hook_context: HookContext) {
        if let Some(mut main_scenes) = world.get_resource_mut::<MainScenes>() {
            main_scenes
                .selections
                .retain(|_, entity| *entity != hook_context.entity);
            main_scenes
                .pending_assets
                .retain(|&(_, entity)| entity != hook_context.entity);
        }
    }
}

/// Main scene entities keyed by configuration.
///
/// Each selection must match the scene accepted by its simulator.
#[derive(Resource, Default)]
pub(crate) struct MainScenes {
    /// The active scene entity for each configuration.
    selections: HashMap<TypeId, Entity>,
    /// The configurations awaiting a marked entity's scene asset.
    pending_assets: HashSet<(TypeId, Entity)>,
}

impl MainScenes {
    /// Records a selection request for a scene asset that has not loaded.
    pub fn request_asset<C: ConfigurationBinding>(&mut self, entity: Entity) {
        self.pending_assets.insert((TypeId::of::<C>(), entity));
    }

    /// Returns whether loading this asset should select it for the configuration.
    pub fn selects_asset<C: ConfigurationBinding>(&self, entity: Entity) -> bool {
        self.selections.get(&TypeId::of::<C>()) == Some(&entity)
            || self.pending_assets.contains(&(TypeId::of::<C>(), entity))
    }

    /// Selects a scene for one configuration and requests a simulator commit.
    #[cfg_attr(feature = "dynamic", doc = "\n# Errors\n")]
    #[cfg_attr(
        feature = "dynamic",
        doc = "- [`crate::dynamic::SimulationError::RayTracerMismatch`] for a different ray tracer."
    )]
    #[cfg_attr(
        feature = "dynamic",
        doc = "- [`crate::dynamic::SimulationError::DeviceMismatch`] for incompatible devices."
    )]
    pub fn select<C: ConfigurationBinding>(
        &mut self,
        entity: Entity,
        scene: &Scene<C>,
        simulation: &mut Simulation<C>,
        commands: Commands,
    ) -> Result<(), IntegrationErrorEvent<C::IntegrationError>> {
        let configuration_id = TypeId::of::<C>();
        self.pending_assets.remove(&(configuration_id, entity));

        if let Err(error) = C::set_scene(&mut simulation.0, &scene.0) {
            Self::remove_unused_marker(entity, commands);

            return Err(IntegrationErrorEvent {
                entity: Some(entity),
                operation: IntegrationOperation::SetScene,
                error,
            });
        }

        if let Some(previous_entity) = self.selections.insert(configuration_id, entity)
            && previous_entity != entity
        {
            Self::remove_unused_marker(previous_entity, commands);
        }

        simulation.request_simulator_commit();

        Ok(())
    }

    /// Removes a marker with no active or pending selections.
    fn remove_unused_marker(entity: Entity, mut commands: Commands) {
        commands.queue(move |world: &mut World| {
            let main_scenes = world.resource::<Self>();
            if !main_scenes
                .selections
                .values()
                .any(|&selected_entity| selected_entity == entity)
                && !main_scenes
                    .pending_assets
                    .iter()
                    .any(|&(_, requested_entity)| requested_entity == entity)
                && let Ok(mut entity) = world.get_entity_mut(entity)
            {
                entity.remove::<MainScene>();
            }
        });
    }
}

/// Commits modified scenes.
pub(crate) fn commit_scenes<C: ConfigurationBinding>(
    simulation: ResMut<Simulation<C>>,
    mut scenes: Query<(&mut Scene<C>, &mut SceneStatus<C>)>,
) {
    let scenes_iter = scenes.iter_mut();
    let mut scenes_pending_commit = Vec::with_capacity(scenes_iter.len());
    for (scene, mut scene_status) in scenes_iter {
        if scene_status.commit_needed {
            scenes_pending_commit.push(scene.0.clone());
            scene_status.commit_needed = false;
        }
    }

    simulation.request_scene_commits(scenes_pending_commit.as_slice());
}

/// Warns once in debug builds when scene entities exist but none is marked [`MainScene`].
#[cfg(debug_assertions)]
pub(crate) fn warn_missing_main_scene<C: ConfigurationBinding>(
    scenes: Query<Entity, (With<Scene<C>>, Without<SubSceneOf>)>,
    main_scenes: Res<MainScenes>,
    mut warned: Local<bool>,
) {
    if *warned || scenes.is_empty() || main_scenes.selections.contains_key(&TypeId::of::<C>()) {
        return;
    }

    *warned = true;
    bevy::log::warn!(
        "found Scene entities but no MainScene; add MainScene to the scene entity that should be used by the simulator."
    );
}

/// Selects the marked scene for this configuration.
pub(crate) fn on_main_scene_added<C: ConfigurationBinding>(
    event: On<Add, MainScene>,
    mut main_scenes: ResMut<MainScenes>,
    commands: Commands,
    mut simulation: ResMut<Simulation<C>>,
    scenes: Query<&Scene<C>>,
    asset_sources: Query<(), With<SceneAssetSource<C>>>,
) -> Result<(), IntegrationErrorEvent<C::IntegrationError>> {
    let Ok(scene) = scenes.get(event.entity) else {
        if asset_sources.contains(event.entity) {
            main_scenes.request_asset::<C>(event.entity);
        }

        return Ok(());
    };

    main_scenes.select(event.entity, scene, &mut simulation, commands)
}

/// Clears any object referencing a removed [`Scene`].
pub(crate) fn on_scene_removed<C: ConfigurationBinding>(
    event: On<Remove, Scene<C>>,
    static_meshes: Query<(Entity, &SpawnedStaticMesh<C>)>,
    instanced_meshes: Query<(Entity, &InstancedMesh, Option<&SpawnedInstancedMesh<C>>)>,
    mut scenes: Query<(&mut Scene<C>, &mut SceneStatus<C>)>,
    mut main_scenes: ResMut<MainScenes>,
    mut commands: Commands,
) {
    let removed_scene_entity = event.entity;
    let replacing_asset = if let Ok((_, mut scene_status)) = scenes.get_mut(removed_scene_entity) {
        std::mem::take(&mut scene_status.replacing_asset)
    } else {
        false
    };

    if !replacing_asset
        && main_scenes.selections.get(&TypeId::of::<C>()) == Some(&removed_scene_entity)
    {
        main_scenes.selections.remove(&TypeId::of::<C>());
        MainScenes::remove_unused_marker(removed_scene_entity, commands.reborrow());
    }

    for (entity, spawned_static_mesh) in &static_meshes {
        if spawned_static_mesh.scene_entity == removed_scene_entity {
            commands.entity(entity).remove::<SpawnedStaticMesh<C>>();
        }
    }

    for (entity, instanced_mesh, spawned_instanced_mesh_option) in &instanced_meshes {
        let references_removed_scene = instanced_mesh.0 == removed_scene_entity
            || spawned_instanced_mesh_option.is_some_and(|spawned_instanced_mesh| {
                spawned_instanced_mesh.scene_entity == removed_scene_entity
                    || spawned_instanced_mesh.sub_scene_entity == removed_scene_entity
            });
        if !references_removed_scene {
            continue;
        }

        // If the removed scene was only the sub-scene, clean up the surviving parent scene too.
        if let Some(spawned_instanced_mesh) = spawned_instanced_mesh_option
            && spawned_instanced_mesh.scene_entity != removed_scene_entity
            && let Ok((mut scene, mut scene_status)) =
                scenes.get_mut(spawned_instanced_mesh.scene_entity)
        {
            scene.0.remove_instanced_mesh(spawned_instanced_mesh.handle);
            scene_status.commit_needed = true;
        }

        commands.entity(entity).remove::<SpawnedInstancedMesh<C>>();
    }
}

/// Walks the parent chain from `start` (inclusive) and returns the first entity that carries a
/// scene, or `None` if none exists.
pub(super) fn find_scene_ancestor<C: ConfigurationBinding>(
    entity: Entity,
    parents: &Query<&ChildOf>,
    scenes: &Query<(&mut Scene<C>, &mut SceneStatus<C>)>,
) -> Option<Entity> {
    if scenes.contains(entity) {
        return Some(entity);
    }

    parents
        .iter_ancestors(entity)
        .find(|&ancestor| scenes.contains(ancestor))
}

/// Runtime scene component.
#[cfg(feature = "dynamic")]
pub type DynamicScene = Scene<super::super::binding::RuntimeConfiguration>;

#[cfg(feature = "dynamic")]
impl DynamicScene {
    /// Creates a scene with the selected ray tracer.
    ///
    /// # Errors
    ///
    /// - [`SteamAudioError::Unspecified`] if native scene creation fails.
    /// - [`SteamAudioError::OutOfMemory`] if native allocation fails.
    /// - [`SteamAudioError::Initialization`] if an external dependency cannot initialize.
    pub fn try_with_ray_tracer(
        context: &Context,
        settings: crate::dynamic::RayTracerSettings,
    ) -> Result<Self, SteamAudioError> {
        crate::dynamic::Scene::try_new(context, settings).map(Self)
    }

    /// Loads a scene with the selected ray tracer.
    ///
    /// # Errors
    ///
    /// - [`SteamAudioError::Unspecified`] if native scene loading fails.
    /// - [`SteamAudioError::OutOfMemory`] if native allocation fails.
    /// - [`SteamAudioError::Initialization`] if an external dependency cannot initialize.
    pub fn load_with_ray_tracer(
        context: &Context,
        settings: crate::dynamic::RayTracerSettings,
        serialized_object: &SerializedObject,
    ) -> Result<Self, SteamAudioError> {
        crate::dynamic::Scene::load(context, settings, serialized_object).map(Self)
    }

    /// Loads a scene with a progress callback.
    ///
    /// # Errors
    ///
    /// - [`SteamAudioError::Unspecified`] if native scene loading fails.
    /// - [`SteamAudioError::OutOfMemory`] if native allocation fails.
    /// - [`SteamAudioError::Initialization`] if an external dependency cannot initialize.
    pub fn load_with_ray_tracer_and_progress(
        context: &Context,
        settings: crate::dynamic::RayTracerSettings,
        serialized_object: &SerializedObject,
        progress_callback: ProgressCallback,
    ) -> Result<Self, SteamAudioError> {
        crate::dynamic::Scene::load_with_progress(
            context,
            settings,
            serialized_object,
            progress_callback,
        )
        .map(Self)
    }
}

#[cfg(feature = "dynamic")]
impl From<crate::dynamic::Scene> for DynamicScene {
    fn from(scene: crate::dynamic::Scene) -> Self {
        Self(scene)
    }
}
