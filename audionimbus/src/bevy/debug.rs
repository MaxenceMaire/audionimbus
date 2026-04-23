//! Debug wireframe overlays.

use super::configuration::{DefaultSimulationConfiguration, SimulationConfiguration};
use super::geometry::{Scene, SpawnedInstancedMesh, SpawnedStaticMesh, SubSceneOf};
use super::system_set::SpatialAudioSet;
use bevy::mesh::{Indices, VertexAttributeValues};
use bevy::prelude::{
    App, Assets, Color, Component, Entity, Gizmos, GlobalTransform, IntoScheduleConfigs, Mat4,
    Mesh, Mesh3d, Plugin, PostUpdate, Query, Res, Resource, With,
};
use bevy::transform::TransformSystems;
use std::marker::PhantomData;

/// Optional plugin that draws acoustic geometry as wireframe overlays.
///
/// `C` must be the same as the [`SpatialAudioPlugin`](crate::bevy::SpatialAudioPlugin)'s.
pub struct SpatialAudioDebugPlugin<C: SimulationConfiguration = DefaultSimulationConfiguration> {
    _phantom: PhantomData<C>,
}

impl SpatialAudioDebugPlugin<DefaultSimulationConfiguration> {
    /// Creates a debug plugin for [`DefaultSimulationConfiguration`].
    pub fn new() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }

    /// Creates a debug plugin for a custom simulation configuration.
    pub fn with_config<C: SimulationConfiguration>() -> SpatialAudioDebugPlugin<C> {
        SpatialAudioDebugPlugin {
            _phantom: PhantomData,
        }
    }
}

impl Default for SpatialAudioDebugPlugin<DefaultSimulationConfiguration> {
    fn default() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<C: SimulationConfiguration + 'static> Plugin for SpatialAudioDebugPlugin<C> {
    fn build(&self, app: &mut App) {
        app.init_resource::<WireframePalette>();
        app.add_systems(
            PostUpdate,
            (draw_top_level_static_mesh, draw_instanced_sub_scene::<C>)
                .after(TransformSystems::Propagate)
                .after(SpatialAudioSet::SyncGeometry),
        );
    }
}

/// A cycling palette of colors used to distinguish scene geometry.
#[derive(Resource, Clone, Debug)]
pub struct WireframePalette {
    /// Cycling colors.
    pub colors: Vec<Color>,
}

impl WireframePalette {
    /// Creates a palette from a list of colors.
    ///
    /// # Panics
    ///
    /// Panics if `colors` is empty.
    pub fn new(colors: Vec<Color>) -> Self {
        assert!(
            !colors.is_empty(),
            "WireframePalette must contain at least one color"
        );
        Self { colors }
    }

    /// Returns the palette color assigned to `entity`.
    ///
    /// The assignment is stable for the lifetime of the entity.
    pub fn color_for(&self, entity: Entity) -> Color {
        self.colors[entity.index().index() as usize % self.colors.len()]
    }
}

impl Default for WireframePalette {
    fn default() -> Self {
        Self {
            colors: vec![
                Color::srgba(0.18, 0.95, 0.38, 0.75), // green
                Color::srgba(0.18, 0.82, 0.95, 0.75), // cyan
                Color::srgba(0.95, 0.85, 0.18, 0.75), // yellow
                Color::srgba(0.95, 0.35, 0.85, 0.75), // magenta
                Color::srgba(0.95, 0.52, 0.18, 0.75), // orange
                Color::srgba(0.18, 0.95, 0.82, 0.75), // teal
                Color::srgba(0.65, 0.45, 0.95, 0.75), // lavender
                Color::srgba(0.72, 0.95, 0.18, 0.75), // lime
            ],
        }
    }
}

impl From<Vec<Color>> for WireframePalette {
    fn from(colors: Vec<Color>) -> Self {
        Self { colors }
    }
}

/// Overrides the wireframe color for a specific entity.
#[derive(Component, Clone, Copy, Debug)]
pub struct WireframeColor(pub Color);

/// Draws wireframes for static meshes that belong to top-level scenes.
///
/// Static meshes whose containing scene is referenced as a sub-scene are skipped.
/// Those meshes are instead drawn by [`draw_instanced_sub_scene_wireframes`] once per spawned instance.
fn draw_top_level_static_mesh(
    static_meshes: Query<(Entity, &Mesh3d, &GlobalTransform, &SpawnedStaticMesh)>,
    sub_scenes: Query<(), With<SubSceneOf>>,
    mesh_assets: Res<Assets<Mesh>>,
    palette: Res<WireframePalette>,
    overrides: Query<&WireframeColor>,
    mut gizmos: Gizmos,
) {
    for (mesh_entity, mesh_3d, transform, spawned_static_mesh) in &static_meshes {
        if sub_scenes.contains(spawned_static_mesh.scene_entity) {
            continue;
        }

        let Some(mesh) = mesh_assets.get(&mesh_3d.0) else {
            continue;
        };

        let scene_color =
            resolve_scene_color(spawned_static_mesh.scene_entity, &overrides, &palette);
        let color = resolve_mesh_color(mesh_entity, scene_color, &overrides);

        draw_wireframe(mesh, transform.to_matrix(), color, &mut gizmos);
    }
}

/// Draws wireframes for sub-scene geometry once per spawned instanced mesh.
fn draw_instanced_sub_scene<C: SimulationConfiguration>(
    instanced_meshes: Query<(&GlobalTransform, &SpawnedInstancedMesh)>,
    scene_transforms: Query<&GlobalTransform, With<Scene<C>>>,
    static_meshes: Query<(Entity, &Mesh3d, &GlobalTransform, &SpawnedStaticMesh)>,
    mesh_assets: Res<Assets<Mesh>>,
    palette: Res<WireframePalette>,
    overrides: Query<&WireframeColor>,
    mut gizmos: Gizmos,
) {
    for (instance_transform, spawned_instanced_mesh) in &instanced_meshes {
        let sub_scene_entity = spawned_instanced_mesh.sub_scene_entity;

        let Ok(sub_scene_transform) = scene_transforms.get(sub_scene_entity) else {
            continue;
        };

        let instance_world = instance_transform.to_matrix();
        let sub_scene_inverse = sub_scene_transform.to_matrix().inverse();
        let scene_color =
            resolve_scene_color(spawned_instanced_mesh.scene_entity, &overrides, &palette);

        for (mesh_entity, mesh_3d, static_mesh_transform, spawned_static_mesh) in &static_meshes {
            if spawned_static_mesh.scene_entity != sub_scene_entity {
                continue;
            }

            let Some(mesh) = mesh_assets.get(&mesh_3d.0) else {
                continue;
            };

            let color = resolve_mesh_color(mesh_entity, scene_color, &overrides);
            let local_to_sub_scene = sub_scene_inverse * static_mesh_transform.to_matrix();
            let instance_world_transform = instance_world * local_to_sub_scene;
            draw_wireframe(mesh, instance_world_transform, color, &mut gizmos);
        }
    }
}

/// Draws the indexed triangles of `mesh` as wireframe lines, with all vertices transformed by `transform`.
fn draw_wireframe(mesh: &Mesh, transform: Mat4, color: Color, gizmos: &mut Gizmos) {
    let Some(positions) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else {
        return;
    };
    let VertexAttributeValues::Float32x3(positions) = positions else {
        return;
    };
    let Some(indices) = mesh.indices() else {
        return;
    };

    let vertices: Vec<_> = positions
        .iter()
        .map(|&[x, y, z]| transform.transform_point3((x, y, z).into()))
        .collect();

    let indices: Vec<usize> = match indices {
        Indices::U16(i) => i.iter().map(|&i| i as usize).collect(),
        Indices::U32(i) => i.iter().map(|&i| i as usize).collect(),
    };

    for chunk in indices.chunks_exact(3) {
        // SAFETY: chunk is guaranteed to be of length 3.
        let &[a, b, c] = chunk else { unreachable!() };
        gizmos.line(vertices[a], vertices[b], color);
        gizmos.line(vertices[b], vertices[c], color);
        gizmos.line(vertices[c], vertices[a], color);
    }
}

/// Resolves the wireframe color for a scene entity.
fn resolve_scene_color(
    scene_entity: Entity,
    overrides: &Query<&WireframeColor>,
    palette: &WireframePalette,
) -> Color {
    overrides
        .get(scene_entity)
        .map_or_else(|_| palette.color_for(scene_entity), |color| color.0)
}

/// Resolves the final wireframe color for a mesh entity.
fn resolve_mesh_color(
    mesh_entity: Entity,
    scene_color: Color,
    overrides: &Query<&WireframeColor>,
) -> Color {
    overrides
        .get(mesh_entity)
        .map_or(scene_color, |color| color.0)
}
