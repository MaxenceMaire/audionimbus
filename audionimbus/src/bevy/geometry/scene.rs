use super::super::configuration::{DefaultSimulationConfiguration, SimulationConfiguration};
use super::super::simulation::Simulation;
use bevy::prelude::{ChildOf, Component, Entity, Query, ResMut};

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
    pub commit_needed: bool,
}

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
