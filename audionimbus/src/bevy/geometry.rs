//! Acoustic geometry.
//!
//! This module provides components and systems for registering 3D acoustic geometry.
//! Geometry affects how sound propagates through a scene; surfaces reflect, absorb, and oclude
//! audio depending on their acoustic [`Material`] properties.
//!
//! # Scene hierarchy
//!
//! Every piece of geometry must live under a [`Scene`] ancestor.
//!
//! # Static vs. instanced geometry
//!
//! [`StaticMesh`] represents immovable geometry.
//! In contrast, [`InstancedMesh`] can undergo rigid-body motion by applying a transform to the
//! scene it references.
//!
//! Multiple instanced meshes may use the same underlying scene, i.e. reference the same geometry.

use super::configuration::{DefaultSimulationConfiguration, SimulationConfiguration};
use crate::geometry::{
    InstancedMeshHandle, InstancedMeshSettings, Material, Matrix4, Point, StaticMeshHandle,
    StaticMeshSettings, Triangle,
};
use bevy::mesh::{Indices, VertexAttributeValues};
use bevy::prelude::{
    Add, Assets, Changed, ChildOf, Commands, Component, Entity, GlobalTransform, Mesh, Mesh3d, On,
    Query, Remove, Res, With, Without,
};
use std::collections::HashSet;

/// Component wrapping an [AudioNimbus scene](`crate::geometry::Scene`).
#[derive(Component, Clone, Debug)]
#[require(SceneStatus)]
pub struct Scene<C: SimulationConfiguration = DefaultSimulationConfiguration>(
    pub crate::geometry::Scene<C::RayTracer>,
);

/// Tracks whether a [`Scene`] has uncommitted geometry changes.
#[derive(Component, Default, Copy, Clone, Debug)]
pub(crate) struct SceneStatus {
    /// Whether [`commit`](`crate::geometry::Scene::commit`) needs to be called.
    commit_needed: bool,
}

/// Marker component that registers this entity's [`Mesh3d`] geometry as a static mesh.
///
/// The static mesh must be a descendant of a [`Scene`].
///
/// The geometry is extracted from the [`Mesh3d`] component of the entity.
///
/// If no scene ancestor exists yet, or the mesh asset is not yet loaded, registration is deferred
/// to the next frame.
#[derive(Component, Copy, Clone, Debug)]
#[require(Mesh3d)]
pub struct StaticMesh;

/// Internal bookkeeping component inserted alongside [`StaticMesh`] after succesful registration.
///
/// Stores the handle needed to remove the mesh from its scene when [`StaticMesh`] is removed or
/// the entity is despawned.
#[derive(Component, Debug)]
pub(crate) struct SpawnedStaticMesh {
    /// The scene entity this mesh was registered with.
    scene_entity: Entity,
    /// Handle used to reference and remove the mesh from the scene.
    handle: StaticMeshHandle,
}

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

/// Relationship target component that records which entities hold an [`InstancedMesh`] reference
/// to this scene.
///
/// Counterpart to [`InstancedMesh`]: an entity with [`Scene`] automatically receives `SubSceneOf`
/// once other entities reference it via [`InstancedMesh`].
#[derive(Component, Clone, Debug)]
#[relationship_target(relationship = InstancedMesh)]
pub struct SubSceneOf(Vec<Entity>);

/// Creates and registers pending [`StaticMesh`] entities.
///
/// An entity is pending when it has [`StaticMesh`] but not yet [`SpawnedStaticMesh`].
/// Entities are skipped (and retried next frame) when:
/// - no ancestor [`Scene`] exists, or
/// - the [`Mesh3d`] asset is not yet loaded.
///
/// Geometry is sourced from the entity's [`Mesh3d`] asset.
/// [`Material::default`] is used when no [`Material`] component is present.
pub(crate) fn register_static_meshes<C: SimulationConfiguration>(
    pending_meshes: Query<
        (Entity, &Mesh3d, Option<&Material>),
        (With<StaticMesh>, Without<SpawnedStaticMesh>),
    >,
    parents: Query<&ChildOf>,
    mut scenes: Query<(&mut Scene<C>, &mut SceneStatus)>,
    mesh_assets: Res<Assets<Mesh>>,
    mut commands: Commands,
) {
    for (entity, mesh_3d, material_option) in &pending_meshes {
        let Some(scene_entity) = find_scene_ancestor::<C>(entity, &parents, &scenes) else {
            // No scene ancestor yet; entity may be reparented later.
            continue;
        };

        let Some(mesh) = mesh_assets.get(&mesh_3d.0) else {
            // Asset not ready; retry next frame.
            continue;
        };

        let Some((vertices, triangles)) = extract_geometry(mesh) else {
            bevy::log::warn!("Mesh3d incompatible with StaticMesh; skipping.");
            continue;
        };

        let material = material_option.copied().unwrap_or_default();

        // All triangles share the same material.
        let material_indices = vec![0; triangles.len()];

        let Ok((mut scene, mut scene_status)) = scenes.get_mut(scene_entity) else {
            continue;
        };

        let inner_mesh = match crate::geometry::StaticMesh::try_new(
            &scene.0,
            &StaticMeshSettings {
                vertices: &vertices,
                triangles: &triangles,
                material_indices: &material_indices,
                materials: &[material],
            },
        ) {
            Ok(mesh) => mesh,
            Err(error) => {
                bevy::log::error!("failed to create static mesh for {entity:?}: {error:?}");
                continue;
            }
        };

        let handle = scene.0.add_static_mesh(inner_mesh);

        scene_status.commit_needed = true;

        let mut entity_commands = commands.entity(entity);
        entity_commands.insert(SpawnedStaticMesh {
            scene_entity,
            handle,
        });
        if material_option.is_none() {
            entity_commands.insert(material);
        }
    }
}

/// Deregisters the static mesh when [`StaticMesh`] is removed or the entity is despawned.
pub(crate) fn on_static_mesh_removed<C: SimulationConfiguration>(
    event: On<Remove, StaticMesh>,
    handles: Query<&SpawnedStaticMesh>,
    mut scenes: Query<(&mut Scene<C>, &mut SceneStatus)>,
) {
    let entity = event.entity;

    let Ok(static_mesh) = handles.get(entity) else {
        // The mesh was never registered (skipped on add).
        return;
    };

    // Scene may have been despawned before the mesh entity.
    let Ok((mut scene, mut scene_status)) = scenes.get_mut(static_mesh.scene_entity) else {
        return;
    };

    scene.0.remove_static_mesh(static_mesh.handle);
    scene_status.commit_needed = true;
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

/// Commits modified scenes.
pub(crate) fn commit_scenes<C: SimulationConfiguration>(
    mut scenes: Query<(&mut Scene<C>, &mut SceneStatus)>,
) {
    let scenes_iter = scenes.iter_mut();
    let mut scenes_pending_commit = HashSet::with_capacity(scenes_iter.len());
    for (scene, mut scene_status) in scenes_iter {
        if scene_status.commit_needed {
            scenes_pending_commit.insert(scene.0.clone());
            scene_status.commit_needed = false;
        }
    }
}

/// Walks the parent chain from `start` (inclusive) and returns the first entity that carries a
/// scene, or `None` if none exists.
fn find_scene_ancestor<C: SimulationConfiguration>(
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

/// Extracts vertex positions and triangle indices from a [`Mesh`].
///
/// Returns `None` when:
/// - the mesh has no position attribute,
/// - the position attribute is not [`VertexAttributeValues::Float32x3`],
/// - or the mesh has no index buffer.
fn extract_geometry(mesh: &Mesh) -> Option<(Vec<Point>, Vec<Triangle>)> {
    let positions = mesh.attribute(Mesh::ATTRIBUTE_POSITION)?;

    let vertices: Vec<Point> = match positions {
        VertexAttributeValues::Float32x3(vertices) => vertices
            .iter()
            .map(|&[x, y, z]| Point::new(x, y, z))
            .collect(),
        _ => {
            return None;
        }
    };

    let triangles: Vec<Triangle> = match mesh.indices()? {
        Indices::U16(indices) => indices
            .chunks_exact(3)
            .map(|c| Triangle::new(c[0] as i32, c[1] as i32, c[2] as i32))
            .collect(),
        Indices::U32(indices) => indices
            .chunks_exact(3)
            .map(|c| Triangle::new(c[0] as i32, c[1] as i32, c[2] as i32))
            .collect(),
    };

    Some((vertices, triangles))
}
