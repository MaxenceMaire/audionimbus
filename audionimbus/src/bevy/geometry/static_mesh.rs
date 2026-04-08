use super::super::configuration::SimulationConfiguration;
use super::scene::{Scene, SceneStatus, find_scene_ancestor};
use crate::geometry::{Material, Point, StaticMeshHandle, StaticMeshSettings, Triangle};
use bevy::asset::AssetId;
use bevy::mesh::{Indices, VertexAttributeValues};
use bevy::prelude::{
    Assets, ChildOf, Commands, Component, Entity, Mesh, Mesh3d, On, Query, Remove, Res, With,
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
    pub(super) scene_entity: Entity,
    /// Mesh asset used to build the registered static mesh.
    pub(super) mesh_asset_id: AssetId<Mesh>,
    /// Acoustic material used to build the registered static mesh.
    pub(super) material: Material,
    /// Handle used to reference and remove the mesh from the scene.
    pub(super) handle: StaticMeshHandle,
}

/// Synchronizes static mesh registrations with the current ECS state.
///
/// Entities are registered when they have a [`Scene`] ancestor and their [`Mesh3d`] asset is
/// loaded.
/// Existing registrations are rebuilt when:
/// - the entity moves under a different [`Scene`],
/// - the [`Mesh3d`] handle changes,
/// - or the acoustic [`Material`] changes.
///
/// Entities are skipped (and retried next frame) when:
/// - no ancestor [`Scene`] exists, or
/// - the [`Mesh3d`] asset is not yet loaded.
///
/// Geometry is sourced from the entity's [`Mesh3d`] asset.
/// [`Material::default`] is used when no [`Material`] component is present.
pub(crate) fn sync_static_meshes<C: SimulationConfiguration>(
    static_meshes: Query<
        (
            Entity,
            &Mesh3d,
            Option<&Material>,
            Option<&SpawnedStaticMesh>,
        ),
        With<StaticMesh>,
    >,
    parents: Query<&ChildOf>,
    mut scenes: Query<(&mut Scene<C>, &mut SceneStatus)>,
    mesh_assets: Res<Assets<Mesh>>,
    mut commands: Commands,
) {
    for (entity, mesh_3d, material_option, spawned_static_mesh) in &static_meshes {
        let scene_entity_option = find_scene_ancestor::<C>(entity, &parents, &scenes);
        let mesh_asset_id = mesh_3d.0.id();
        let material = material_option.copied().unwrap_or_default();

        let registration_is_current = spawned_static_mesh.is_some_and(|spawned_static_mesh| {
            scene_entity_option == Some(spawned_static_mesh.scene_entity)
                && spawned_static_mesh.mesh_asset_id == mesh_asset_id
                && spawned_static_mesh.material == material
        });
        if registration_is_current {
            continue;
        }

        if let Some(spawned_static_mesh) = spawned_static_mesh {
            // The simulator still holds the old registration, so remove it before rebuilding.
            deregister_static_mesh(spawned_static_mesh, &mut scenes);
        }

        let registration = scene_entity_option.and_then(|scene_entity| {
            try_register_static_mesh(
                entity,
                scene_entity,
                mesh_asset_id,
                material,
                mesh_3d,
                &mesh_assets,
                &mut scenes,
            )
        });

        let mut entity_commands = commands.entity(entity);
        match registration {
            Some(registration) => {
                entity_commands.insert(registration);
                if material_option.is_none() {
                    entity_commands.insert(material);
                }
            }
            None => {
                entity_commands.remove::<SpawnedStaticMesh>();
            }
        }
    }
}

/// Attempts to create and register a [`StaticMesh`] under its parent scene.
fn try_register_static_mesh<C: SimulationConfiguration>(
    entity: Entity,
    scene_entity: Entity,
    mesh_asset_id: AssetId<Mesh>,
    material: Material,
    mesh_3d: &Mesh3d,
    mesh_assets: &Assets<Mesh>,
    scenes: &mut Query<(&mut Scene<C>, &mut SceneStatus)>,
) -> Option<SpawnedStaticMesh> {
    // The mesh asset is not ready yet. Retry once it becomes available.
    let mesh = mesh_assets.get(&mesh_3d.0)?;

    let (vertices, triangles) = match extract_geometry(mesh) {
        Some(geometry) => geometry,
        None => {
            // The mesh cannot be represented as AudioNimbus geometry.
            bevy::log::warn_once!("Mesh3d incompatible with StaticMesh; skipping.");
            return None;
        }
    };

    // All triangles share the same material.
    let material_indices = vec![0; triangles.len()];

    let (mut scene, mut scene_status) = scenes.get_mut(scene_entity).ok()?;

    let inner_mesh = crate::geometry::StaticMesh::try_new(
        &scene.0,
        &StaticMeshSettings {
            vertices: &vertices,
            triangles: &triangles,
            material_indices: &material_indices,
            materials: &[material],
        },
    )
    .inspect_err(|error| {
        bevy::log::error!("failed to create static mesh for {entity:?}: {error:?}");
    })
    .ok()?;

    let handle = scene.0.add_static_mesh(inner_mesh);
    scene_status.commit_needed = true;

    Some(SpawnedStaticMesh {
        scene_entity,
        mesh_asset_id,
        material,
        handle,
    })
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

    deregister_static_mesh(static_mesh, &mut scenes);
}

fn deregister_static_mesh<C: SimulationConfiguration>(
    static_mesh: &SpawnedStaticMesh,
    scenes: &mut Query<(&mut Scene<C>, &mut SceneStatus)>,
) {
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
