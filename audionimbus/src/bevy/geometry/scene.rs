use super::super::configuration::{DefaultSimulationConfiguration, SimulationConfiguration};
use super::super::simulation::Simulation;
use super::SubSceneOf;
use bevy::prelude::{
    Add, ChildOf, Commands, Component, Entity, Local, On, Query, ResMut, With, Without,
};

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

/// Marks the [`Scene`] that the simulator should use for ray tracing.
///
/// At most one entity should carry this component at a time.
/// Adding it to a new entity will automatically remove it from whichever entity previously held
/// it.
/// The last entity to receive `MainScene` becomes the active scene.
///
/// The plugin automatically spawns a default scene entity tagged with both [`DefaultScene`] and
/// [`MainScene`].
/// For simple use cases, simply place geometry under it.
/// For level transitions or custom setups, spawn your own [`Scene`] and add [`MainScene`] to it.
/// The previous main scene will lose the marker automatically.
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
