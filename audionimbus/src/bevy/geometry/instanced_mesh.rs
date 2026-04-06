use super::super::configuration::SimulationConfiguration;
use super::scene::{Scene, SceneStatus, find_scene_ancestor};
use crate::geometry::{InstancedMeshHandle, InstancedMeshSettings, Matrix4};
use bevy::prelude::{
    Add, Changed, ChildOf, Commands, Component, Entity, GlobalTransform, On, Query, Remove,
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
    /// Handle used to reference, transform, and remove the instanced mesh from the scene.
    handle: InstancedMeshHandle,
}

/// Creates and registers an instanced mesh when [`InstancedMesh`] is added.
///
/// The sub-scene referenced by [`InstancedMesh`] must already carry a [`Scene`] component.
/// The initial transform is taken from [`GlobalTransform`].
pub(crate) fn on_instanced_mesh_added<C: SimulationConfiguration>(
    event: On<Add, InstancedMesh>,
    instanced_meshes: Query<(&InstancedMesh, &GlobalTransform)>,
    parents: Query<&ChildOf>,
    mut scenes: Query<(&mut Scene<C>, &mut SceneStatus)>,
    mut commands: Commands,
) {
    let entity = event.entity;

    let Some(scene_entity) = find_scene_ancestor::<C>(entity, &parents, &scenes) else {
        bevy::log::warn!("InstancedMesh on {entity:?} has no ancestor Scene; skipping.");
        return;
    };

    let Ok((instanced_mesh, global_transform)) = instanced_meshes.get(entity) else {
        return;
    };

    let sub_scene_entity = instanced_mesh.0;
    let transform = Matrix4::from(*global_transform);

    let sub_scene_clone = match scenes.get(sub_scene_entity) {
        Ok((sub_scene, _)) => sub_scene.0.clone(),
        Err(_) => {
            bevy::log::error!(
                "InstancedMesh on {entity:?} references {sub_scene_entity:?} which carries no Scene component."
            );
            return;
        }
    };

    let (mut parent_scene, mut parent_scene_status) = scenes.get_mut(scene_entity).unwrap();

    let inner_mesh = match crate::geometry::InstancedMesh::try_new(
        &parent_scene.0,
        &InstancedMeshSettings {
            sub_scene: sub_scene_clone,
            transform,
        },
    ) {
        Ok(mesh) => mesh,
        Err(error) => {
            bevy::log::error!("failed to create instanced mesh for {entity:?}: {error:?}");
            return;
        }
    };

    let handle = parent_scene.0.add_instanced_mesh(inner_mesh);

    parent_scene_status.commit_needed = true;

    commands.entity(entity).insert(SpawnedInstancedMesh {
        scene_entity,
        handle,
    });
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

    let Ok((mut scene, mut scene_status)) = scenes.get_mut(instanced_mesh.scene_entity) else {
        return;
    };

    scene.0.remove_instanced_mesh(instanced_mesh.handle);
    scene_status.commit_needed = true;
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
