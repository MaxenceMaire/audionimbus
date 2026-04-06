use super::super::configuration::SimulationConfiguration;
use super::scene::{Scene, SceneStatus, find_scene_ancestor};
use crate::geometry::{Material, Point, StaticMeshHandle, StaticMeshSettings, Triangle};
use bevy::mesh::{Indices, VertexAttributeValues};
use bevy::prelude::{
    Assets, ChildOf, Commands, Component, Entity, Mesh, Mesh3d, On, Query, Remove, Res, With,
    Without,
};

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
