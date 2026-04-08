use super::super::configuration::{DefaultSimulationConfiguration, SimulationConfiguration};
use super::super::simulation::Simulation;
use super::instanced_mesh::{InstancedMesh, SpawnedInstancedMesh, SubSceneOf};
use super::static_mesh::SpawnedStaticMesh;
use crate::callback::{CustomRayTracingCallbacks, ProgressCallback};
use crate::context::Context;
use crate::device::{EmbreeDevice, RadeonRaysDevice};
use crate::error::SteamAudioError;
use crate::ray_tracing::{CustomRayTracer, DefaultRayTracer, Embree, RadeonRays};
use crate::serialized_object::SerializedObject;
use bevy::prelude::{
    Add, ChildOf, Commands, Component, Entity, Local, On, Query, Remove, ResMut, With, Without,
};
use std::ops::{Deref, DerefMut};

/// Component wrapping an [AudioNimbus scene](`crate::geometry::Scene`).
#[derive(Component, Clone, Debug)]
#[require(SceneStatus)]
pub struct Scene<C: SimulationConfiguration = DefaultSimulationConfiguration>(
    pub crate::geometry::Scene<C::RayTracer>,
);

impl<C> Scene<C>
where
    C: SimulationConfiguration<RayTracer = DefaultRayTracer>,
{
    /// Creates a new scene.
    ///
    /// Mirrors [`Scene::<DefaultRayTracer>::try_new`](crate::geometry::Scene::<DefaultRayTracer>::try_new).
    pub fn try_new(context: &Context) -> Result<Self, SteamAudioError> {
        crate::geometry::Scene::<DefaultRayTracer>::try_new(context).map(Self)
    }

    /// Loads a scene from a serialized object.
    ///
    /// Mirrors [`Scene::<DefaultRayTracer>::load`](crate::geometry::Scene::<DefaultRayTracer>::load).
    pub fn load(
        context: &Context,
        serialized_object: &SerializedObject,
    ) -> Result<Self, SteamAudioError> {
        crate::geometry::Scene::<DefaultRayTracer>::load(context, serialized_object).map(Self)
    }

    /// Loads a scene from a serialized object with a progress callback.
    ///
    /// Mirrors [`Scene::<DefaultRayTracer>::load_with_progress`](crate::geometry::Scene::<DefaultRayTracer>::load_with_progress).
    pub fn load_with_progress(
        context: &Context,
        serialized_object: &SerializedObject,
        progress_callback: ProgressCallback,
    ) -> Result<Self, SteamAudioError> {
        crate::geometry::Scene::<DefaultRayTracer>::load_with_progress(
            context,
            serialized_object,
            progress_callback,
        )
        .map(Self)
    }
}

impl<C> Scene<C>
where
    C: SimulationConfiguration<RayTracer = Embree>,
{
    /// Creates a new scene with the Embree ray tracer.
    ///
    /// Mirrors [`Scene::<Embree>::try_with_embree`](crate::geometry::Scene::<Embree>::try_with_embree).
    pub fn try_with_embree(
        context: &Context,
        device: EmbreeDevice,
    ) -> Result<Self, SteamAudioError> {
        crate::geometry::Scene::<Embree>::try_with_embree(context, device).map(Self)
    }

    /// Loads a scene from a serialized object using Embree.
    ///
    /// Mirrors [`Scene::<Embree>::load_embree`](crate::geometry::Scene::<Embree>::load_embree).
    pub fn load_embree(
        context: &Context,
        device: EmbreeDevice,
        serialized_object: &SerializedObject,
    ) -> Result<Self, SteamAudioError> {
        crate::geometry::Scene::<Embree>::load_embree(context, device, serialized_object).map(Self)
    }

    /// Loads a scene from a serialized object using Embree with a progress callback.
    ///
    /// Mirrors [`Scene::<Embree>::load_embree_with_progress`](crate::geometry::Scene::<Embree>::load_embree_with_progress).
    pub fn load_embree_with_progress(
        context: &Context,
        device: EmbreeDevice,
        serialized_object: &SerializedObject,
        progress_callback: ProgressCallback,
    ) -> Result<Self, SteamAudioError> {
        crate::geometry::Scene::<Embree>::load_embree_with_progress(
            context,
            device,
            serialized_object,
            progress_callback,
        )
        .map(Self)
    }
}

impl<C> Scene<C>
where
    C: SimulationConfiguration<RayTracer = RadeonRays>,
{
    /// Creates a new scene with the Radeon Rays ray tracer.
    ///
    /// Mirrors [`Scene::<RadeonRays>::try_with_radeon_rays`](crate::geometry::Scene::<RadeonRays>::try_with_radeon_rays).
    pub fn try_with_radeon_rays(
        context: &Context,
        device: RadeonRaysDevice,
    ) -> Result<Self, SteamAudioError> {
        crate::geometry::Scene::<RadeonRays>::try_with_radeon_rays(context, device).map(Self)
    }

    /// Loads a scene from a serialized object using Radeon Rays.
    ///
    /// Mirrors [`Scene::<RadeonRays>::load_radeon_rays`](crate::geometry::Scene::<RadeonRays>::load_radeon_rays).
    pub fn load_radeon_rays(
        context: &Context,
        device: RadeonRaysDevice,
        serialized_object: &SerializedObject,
    ) -> Result<Self, SteamAudioError> {
        crate::geometry::Scene::<RadeonRays>::load_radeon_rays(context, device, serialized_object)
            .map(Self)
    }

    /// Loads a scene from a serialized object using Radeon Rays with a progress callback.
    ///
    /// Mirrors [`Scene::<RadeonRays>::load_radeon_rays_with_progress`](crate::geometry::Scene::<RadeonRays>::load_radeon_rays_with_progress).
    pub fn load_radeon_rays_with_progress(
        context: &Context,
        device: RadeonRaysDevice,
        serialized_object: &SerializedObject,
        progress_callback: ProgressCallback,
    ) -> Result<Self, SteamAudioError> {
        crate::geometry::Scene::<RadeonRays>::load_radeon_rays_with_progress(
            context,
            device,
            serialized_object,
            progress_callback,
        )
        .map(Self)
    }
}

impl<C> Scene<C>
where
    C: SimulationConfiguration<RayTracer = CustomRayTracer>,
{
    /// Creates a new scene with a custom ray tracer.
    ///
    /// Mirrors [`Scene::<CustomRayTracer>::try_with_custom`](crate::geometry::Scene::<CustomRayTracer>::try_with_custom).
    pub fn try_with_custom(
        context: &Context,
        callbacks: CustomRayTracingCallbacks,
    ) -> Result<Self, SteamAudioError> {
        crate::geometry::Scene::<CustomRayTracer>::try_with_custom(context, callbacks).map(Self)
    }

    /// Loads a scene from a serialized object using a custom ray tracer.
    ///
    /// Mirrors [`Scene::<CustomRayTracer>::load_custom`](crate::geometry::Scene::<CustomRayTracer>::load_custom).
    pub fn load_custom(
        context: &Context,
        callbacks: CustomRayTracingCallbacks,
        serialized_object: &SerializedObject,
    ) -> Result<Self, SteamAudioError> {
        crate::geometry::Scene::<CustomRayTracer>::load_custom(
            context,
            callbacks,
            serialized_object,
        )
        .map(Self)
    }

    /// Loads a scene from a serialized object using a custom ray tracer with a progress callback.
    ///
    /// Mirrors [`Scene::<CustomRayTracer>::load_custom_with_progress`](crate::geometry::Scene::<CustomRayTracer>::load_custom_with_progress).
    pub fn load_custom_with_progress(
        context: &Context,
        callbacks: CustomRayTracingCallbacks,
        serialized_object: &SerializedObject,
        progress_callback: ProgressCallback,
    ) -> Result<Self, SteamAudioError> {
        crate::geometry::Scene::<CustomRayTracer>::load_custom_with_progress(
            context,
            callbacks,
            serialized_object,
            progress_callback,
        )
        .map(Self)
    }
}

impl<C: SimulationConfiguration> Deref for Scene<C> {
    type Target = crate::geometry::Scene<C::RayTracer>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<C: SimulationConfiguration> DerefMut for Scene<C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<C: SimulationConfiguration> From<crate::geometry::Scene<C::RayTracer>> for Scene<C> {
    fn from(scene: crate::geometry::Scene<C::RayTracer>) -> Self {
        Self(scene)
    }
}

/// Tracks whether a [`Scene`] has uncommitted geometry changes.
#[derive(Component, Default, Copy, Clone, Debug)]
pub(crate) struct SceneStatus {
    /// Whether [`commit`](`crate::geometry::Scene::commit`) needs to be called.
    pub commit_needed: bool,
}

/// Marks the [`Scene`] that the simulator should use for ray tracing.
///
/// At most one entity should carry this component at a time.
/// Adding it to a new entity will automatically remove it from whichever entity previously held
/// it.
/// The last entity to receive `MainScene` becomes the active scene.
#[derive(Component, Debug)]
#[component(storage = "SparseSet")]
pub struct MainScene;

/// Commits modified scenes.
pub(crate) fn commit_scenes<C: SimulationConfiguration>(
    simulation: ResMut<Simulation<C>>,
    mut scenes: Query<(&mut Scene<C>, &mut SceneStatus)>,
) {
    let scenes_iter = scenes.iter_mut();
    let mut scenes_pending_commit = Vec::with_capacity(scenes_iter.len());
    for (scene, mut scene_status) in scenes_iter {
        if scene_status.commit_needed {
            scenes_pending_commit.push(scene.0.clone());
            scene_status.commit_needed = false;
        }
    }

    simulation
        .0
        .request_scene_commits(scenes_pending_commit.as_slice());
}

/// Warns once in debug builds when scene entities exist but none is marked [`MainScene`].
#[cfg(debug_assertions)]
pub(crate) fn warn_missing_main_scene<C: SimulationConfiguration>(
    scenes: Query<Entity, (With<Scene<C>>, Without<SubSceneOf>)>,
    main_scenes: Query<Entity, With<MainScene>>,
    mut warned: Local<bool>,
) {
    if *warned || scenes.is_empty() || !main_scenes.is_empty() {
        return;
    }

    *warned = true;
    bevy::log::warn!(
        "found Scene entities but no MainScene; add MainScene to the scene entity that should be used by the simulator."
    );
}

/// Enforces the [`MainScene`] exclusivity invariant and notifies the simulator.
///
/// When `MainScene` is added to an entity, this observer removes it from any other entity
/// currently holding it, ensuring the simulator always has exactly one active scene.
/// The newly marked scene is then set on the simulator and a commit is requested.
pub(crate) fn on_main_scene_added<C: SimulationConfiguration>(
    event: On<Add, MainScene>,
    main_scenes: Query<Entity, With<MainScene>>,
    mut commands: Commands,
    mut simulation: ResMut<Simulation<C>>,
    scenes: Query<&Scene<C>>,
) {
    for entity in &main_scenes {
        if entity != event.entity {
            commands.entity(entity).remove::<MainScene>();
        }
    }

    if let Ok(scene) = scenes.get(event.entity) {
        simulation.0.simulator.set_scene(&scene.0);
        simulation.0.request_simulator_commit();
    }
}

/// Clears any object referencing a removed [`Scene`].
pub(crate) fn on_scene_removed<C: SimulationConfiguration>(
    event: On<Remove, Scene<C>>,
    static_meshes: Query<(Entity, &SpawnedStaticMesh)>,
    instanced_meshes: Query<(Entity, &InstancedMesh, Option<&SpawnedInstancedMesh>)>,
    mut scenes: Query<(&mut Scene<C>, &mut SceneStatus)>,
    main_scenes: Query<(), With<MainScene>>,
    mut commands: Commands,
) {
    let removed_scene_entity = event.entity;

    if main_scenes.contains(removed_scene_entity)
        && let Ok(mut removed_scene_commands) = commands.get_entity(removed_scene_entity)
    {
        removed_scene_commands.remove::<MainScene>();
    }

    for (entity, spawned_static_mesh) in &static_meshes {
        if spawned_static_mesh.scene_entity == removed_scene_entity {
            commands.entity(entity).remove::<SpawnedStaticMesh>();
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

        commands.entity(entity).remove::<SpawnedInstancedMesh>();
    }
}

/// Walks the parent chain from `start` (inclusive) and returns the first entity that carries a
/// scene, or `None` if none exists.
pub(super) fn find_scene_ancestor<C: SimulationConfiguration>(
    entity: Entity,
    parents: &Query<&ChildOf>,
    scenes: &Query<(&mut Scene<C>, &mut SceneStatus)>,
) -> Option<Entity> {
    if scenes.contains(entity) {
        return Some(entity);
    }

    for ancestor in parents.iter_ancestors(entity) {
        if scenes.contains(ancestor) {
            return Some(ancestor);
        }
    }

    None
}
