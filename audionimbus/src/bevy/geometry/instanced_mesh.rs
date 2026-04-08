use super::super::configuration::SimulationConfiguration;
use super::scene::{Scene, SceneStatus, find_scene_ancestor};
use crate::geometry::{InstancedMeshHandle, InstancedMeshSettings, Matrix4};
use bevy::prelude::{
    Changed, ChildOf, Commands, Component, Entity, GlobalTransform, On, Query, Remove,
};

/// Component that instantiates a sub-scene as movable acoustic geometry within a parent scene.
///
/// The wrapped [`Entity`] must carry a [`Scene`] component to be used as a sub-scene.
/// The sub-scene is placed into the parent scene at this entity's [`GlobalTransform`].
///
/// Multiple [`InstancedMesh`]s may reference the same sub-scene for shared geometry (e.g.
/// identical props placed throughout a level).
#[derive(Component, Copy, Clone, Debug)]
#[require(GlobalTransform)]
#[relationship(relationship_target = SubSceneOf)]
pub struct InstancedMesh(pub Entity);

/// Relationship target component that records which entities hold an [`InstancedMesh`] reference
/// to this scene.
///
/// Counterpart to [`InstancedMesh`]: an entity with [`Scene`] automatically receives `SubSceneOf`
/// once other entities reference it via [`InstancedMesh`].
#[derive(Component, Clone, Debug)]
#[relationship_target(relationship = InstancedMesh)]
pub struct SubSceneOf(Vec<Entity>);

/// Internal bookkeeping component inserted alongside [`InstancedMesh`] after succesful
/// registration.
///
/// Stores the information needed to remove the instanced mesh from its parent scene when
/// [`InstancedMesh`] is removed or the entity is despawned.
#[derive(Component, Debug)]
pub(crate) struct SpawnedInstancedMesh {
    /// The parent scene entity this instanced mesh was registered with.
    scene_entity: Entity,
    /// The sub-scene entity this instanced mesh references.
    sub_scene_entity: Entity,
    /// Handle used to reference, transform, and remove the instanced mesh from the scene.
    handle: InstancedMeshHandle,
}

/// Synchronizes instanced mesh registrations with the current ECS state.
///
/// Entities are registered when they have a parent [`Scene`] ancestor and reference an entity that
/// carries a [`Scene`] component.
/// Existing registrations are rebuilt when the entity moves under a different parent [`Scene`], or
/// the referenced sub-scene changes.
///
/// Pending entities are retried every frame until both scenes exist.
pub(crate) fn sync_instanced_meshes<C: SimulationConfiguration>(
    instanced_meshes: Query<(
        Entity,
        &InstancedMesh,
        &GlobalTransform,
        Option<&SpawnedInstancedMesh>,
    )>,
    parents: Query<&ChildOf>,
    mut scenes: Query<(&mut Scene<C>, &mut SceneStatus)>,
    mut commands: Commands,
) {
    for (entity, instanced_mesh, global_transform, spawned_instanced_mesh_option) in
        &instanced_meshes
    {
        let scene_entity_option = find_scene_ancestor::<C>(entity, &parents, &scenes);
        let sub_scene_entity = instanced_mesh.0;

        let registration_is_current =
            spawned_instanced_mesh_option.is_some_and(|spawned_instanced_mesh| {
                scene_entity_option == Some(spawned_instanced_mesh.scene_entity)
                    && spawned_instanced_mesh.sub_scene_entity == sub_scene_entity
            });
        if registration_is_current {
            continue;
        }

        // Remove any stale registration before rebuilding.
        if let Some(spawned_instanced_mesh) = spawned_instanced_mesh_option {
            deregister_instanced_mesh(spawned_instanced_mesh, &mut scenes);
        }

        let registration = scene_entity_option.and_then(|scene_entity| {
            try_register_instanced_mesh(
                entity,
                scene_entity,
                sub_scene_entity,
                global_transform,
                &mut scenes,
            )
        });

        match registration {
            Some(registration) => commands.entity(entity).insert(registration),
            None => commands.entity(entity).remove::<SpawnedInstancedMesh>(),
        };
    }
}

/// Attempts to create and register an [`InstancedMesh`] under its parent scene.
fn try_register_instanced_mesh<C: SimulationConfiguration>(
    entity: Entity,
    scene_entity: Entity,
    sub_scene_entity: Entity,
    global_transform: &GlobalTransform,
    scenes: &mut Query<(&mut Scene<C>, &mut SceneStatus)>,
) -> Option<SpawnedInstancedMesh> {
    let sub_scene = scenes.get(sub_scene_entity).ok()?.0.0.clone();

    let (mut parent_scene, mut parent_scene_status) = scenes.get_mut(scene_entity).ok()?;

    let inner_mesh = crate::geometry::InstancedMesh::try_new(
        &parent_scene.0,
        &InstancedMeshSettings {
            sub_scene,
            transform: Matrix4::from(*global_transform),
        },
    )
    .inspect_err(|error| {
        bevy::log::error!("failed to create instanced mesh for {entity:?}: {error:?}");
    })
    .ok()?;

    let handle = parent_scene.0.add_instanced_mesh(inner_mesh);
    parent_scene_status.commit_needed = true;

    Some(SpawnedInstancedMesh {
        scene_entity,
        sub_scene_entity,
        handle,
    })
}

/// Deregisters the instanced mesh when [`InstancedMesh`] is removed or the entity is despawned.
pub(crate) fn on_instanced_mesh_removed<C: SimulationConfiguration>(
    event: On<Remove, InstancedMesh>,
    handles: Query<&SpawnedInstancedMesh>,
    mut scenes: Query<(&mut Scene<C>, &mut SceneStatus)>,
) {
    let entity = event.entity;

    let Ok(instanced_mesh) = handles.get(entity) else {
        return;
    };

    deregister_instanced_mesh(instanced_mesh, &mut scenes);
}

/// Pushes updated transforms for all instanced meshes whose [`GlobalTransform`] changed.
pub(crate) fn sync_instanced_mesh_transforms<C: SimulationConfiguration>(
    changed_meshes: Query<(&GlobalTransform, &SpawnedInstancedMesh), Changed<GlobalTransform>>,
    mut scenes: Query<(&mut Scene<C>, &mut SceneStatus)>,
) {
    for (global_transform, instanced_mesh) in &changed_meshes {
        let Ok((mut scene, mut scene_status)) = scenes.get_mut(instanced_mesh.scene_entity) else {
            continue;
        };

        scene.0.update_instanced_mesh_transform(
            instanced_mesh.handle,
            Matrix4::from(*global_transform),
        );

        scene_status.commit_needed = true;
    }
}

/// Removes a registered instanced mesh from its parent scene.
fn deregister_instanced_mesh<C: SimulationConfiguration>(
    instanced_mesh: &SpawnedInstancedMesh,
    scenes: &mut Query<(&mut Scene<C>, &mut SceneStatus)>,
) {
    let Ok((mut scene, mut scene_status)) = scenes.get_mut(instanced_mesh.scene_entity) else {
        return;
    };

    scene.0.remove_instanced_mesh(instanced_mesh.handle);
    scene_status.commit_needed = true;
}
