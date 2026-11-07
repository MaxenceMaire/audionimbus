//! Components for spatial audio sources.

use super::binding::{ConfigurationBinding, DefaultConfiguration};
use super::configuration::SimulationConfiguration;
use super::error::{IntegrationErrorEvent, IntegrationOperation};
use super::simulation::{Simulation, SimulationSharedInputs};
use crate::error::SteamAudioError;
use crate::geometry::CoordinateSystem;
use crate::simulation::inputs::SimulationParameters;
use bevy::ecs::system::StaticSystemParam;
use bevy::prelude::{
    Add, Commands, Component, Entity, GlobalTransform, On, Query, Reflect, ReflectComponent,
    Remove, ResMut, With, Without,
};
use std::borrow::BorrowMut;
use std::ops::{Deref, DerefMut};

/// Spatial audio source component.
///
/// Attach this to any entity that should emit sound. The entity's [`GlobalTransform`] is used as the
/// source position each frame.
#[derive(Component, Reflect, Debug)]
#[reflect(Component, opaque)]
pub struct Source<C: ConfigurationBinding = DefaultConfiguration>(pub C::Source);

impl<C: ConfigurationBinding> Clone for Source<C> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<C: SimulationConfiguration> Source<C> {
    /// Creates a new source using the [`Simulation`] resource.
    ///
    /// Mirrors [`crate::simulation::Source::try_new`](crate::simulation::Source#method.try_new).
    pub fn try_new(simulation: &Simulation<C>) -> Result<Self, SteamAudioError> {
        crate::simulation::source::StaticSource::<
            C::Direct,
            C::Reflections,
            C::Pathing,
            C::ReflectionEffect,
        >::try_new(simulation.simulator())
        .map(Self)
    }
}

impl<C: ConfigurationBinding> Deref for Source<C> {
    type Target = C::Source;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<C: ConfigurationBinding> DerefMut for Source<C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<C: SimulationConfiguration>
    From<
        crate::simulation::source::StaticSource<
            C::Direct,
            C::Reflections,
            C::Pathing,
            C::ReflectionEffect,
        >,
    > for Source<C>
{
    fn from(
        source: crate::simulation::source::StaticSource<
            C::Direct,
            C::Reflections,
            C::Pathing,
            C::ReflectionEffect,
        >,
    ) -> Self {
        Self(source)
    }
}

/// Per-source simulation parameters.
///
/// Optional companion to [`Source`]. When absent, the integration uses implicit parameters for
/// that source.
#[derive(Component, Reflect, Debug)]
#[reflect(Component, opaque)]
pub struct SourceParameters<C: ConfigurationBinding = DefaultConfiguration>(pub C::Parameters);

impl<C: ConfigurationBinding> Clone for SourceParameters<C> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<C: SimulationConfiguration> From<SimulationParameters<C::Direct, C::Reflections, C::Pathing>>
    for SourceParameters<C>
{
    fn from(params: SimulationParameters<C::Direct, C::Reflections, C::Pathing>) -> Self {
        Self(params)
    }
}

impl<C: SimulationConfiguration> Default for SourceParameters<C> {
    fn default() -> Self {
        Self(C::implicit_source_parameters())
    }
}

/// Adds a `Source` to the simulator and requests a commit when it is spawned.
pub fn on_source_added<C: SimulationConfiguration>(
    event: On<Add, Source<C>>,
    query: Query<&Source<C>>,
    simulation: ResMut<Simulation<C>>,
) {
    try_on_source_added::<C>(event, query, simulation).unwrap_or_else(|event| match event.error {});
}

/// Registers a source and requests a commit after successful registration.
///
/// # Panics
///
/// Panics if the entity has no matching source component.
#[cfg_attr(
    feature = "dynamic",
    doc = "\n# Errors\n\n- [`crate::dynamic::SimulationError::ForeignSource`] if another simulator created the source."
)]
pub fn try_on_source_added<C: ConfigurationBinding>(
    event: On<Add, Source<C>>,
    query: Query<&Source<C>>,
    mut simulation: ResMut<Simulation<C>>,
) -> Result<(), IntegrationErrorEvent<C::IntegrationError>> {
    let source = query.get(event.entity).unwrap();
    C::add_source(&mut simulation.0, &source.0).map_err(|error| IntegrationErrorEvent {
        entity: Some(event.entity),
        operation: IntegrationOperation::AddSource,
        error,
    })?;
    simulation.request_simulator_commit();

    Ok(())
}

/// Removes a `Source` from the simulator and requests a commit when it is removed.
pub fn on_source_removed<C: SimulationConfiguration>(
    event: On<Remove, Source<C>>,
    query: Query<&Source<C>>,
    simulation: ResMut<Simulation<C>>,
) {
    try_on_source_removed::<C>(event, query, simulation)
        .unwrap_or_else(|event| match event.error {});
}

/// Removes a source and requests a commit after successful removal.
///
/// # Panics
///
/// Panics if the entity has no matching source component.
#[cfg_attr(
    feature = "dynamic",
    doc = "\n# Errors\n\n- [`crate::dynamic::SimulationError::ForeignSource`] if another simulator created the source."
)]
pub fn try_on_source_removed<C: ConfigurationBinding>(
    event: On<Remove, Source<C>>,
    query: Query<&Source<C>>,
    mut simulation: ResMut<Simulation<C>>,
) -> Result<(), IntegrationErrorEvent<C::IntegrationError>> {
    let source = query.get(event.entity).unwrap();
    C::remove_source(&mut simulation.0, &source.0).map_err(|error| IntegrationErrorEvent {
        entity: Some(event.entity),
        operation: IntegrationOperation::RemoveSource,
        error,
    })?;
    simulation.request_simulator_commit();

    Ok(())
}

/// Marks an entity as the simulator listener.
///
/// The entity's [`GlobalTransform`] is copied into [`SimulationSharedInputs`] each frame.
///
/// If the entity also carries [`Source`], it is used as the listener-centric reverb source.
///
/// At most one entity should carry this component.
/// Adding it to a new entity removes it from any entity that previously held it.
#[derive(Component, Reflect, Debug)]
#[reflect(Component)]
#[component(storage = "SparseSet")]
pub struct Listener;

/// Enforces the [`Listener`] exclusivity invariant.
///
/// When `Listener` is added to an entity, this observer removes it from any other entity
/// currently holding it, making the new entity the active listener.
pub(crate) fn on_listener_added(
    event: On<Add, Listener>,
    listeners: Query<Entity, With<Listener>>,
    mut commands: Commands,
) {
    for entity in &listeners {
        if entity != event.entity {
            commands.entity(entity).remove::<Listener>();
        }
    }
}

/// Synchronizes the listener transform into shared simulation inputs.
pub(crate) fn sync_simulation_shared_inputs_listener<C: ConfigurationBinding>(
    listeners: Query<&GlobalTransform, With<Listener>>,
    mut shared_inputs: ResMut<SimulationSharedInputs<C>>,
) {
    let inputs: &mut crate::simulation::simulator::SimulationSharedInputs<C::Inputs> =
        shared_inputs.0.borrow_mut();
    inputs.set_listener(
        listeners
            .iter()
            .next()
            .map_or_else(CoordinateSystem::default, |listener| (*listener).into()),
    );
}

/// Publishes a new snapshot of sources.
#[allow(clippy::type_complexity)]
pub(crate) fn sync_sources<C: ConfigurationBinding>(
    mut query: Query<
        (
            Entity,
            &GlobalTransform,
            &Source<C>,
            Option<&SourceParameters<C>>,
        ),
        Without<Listener>,
    >,
    mut simulation: StaticSystemParam<C::SimulationAccess>,
) -> Result<(), IntegrationErrorEvent<C::IntegrationError>> {
    C::update_sources(&mut simulation, |snapshot| {
        for (entity, global_transform, source, simulation_parameters) in query.iter_mut() {
            let parameters = simulation_parameters.map_or_else(
                || C::implicit_parameters(&source.0),
                |params| params.0.clone(),
            );
            snapshot.push((
                entity,
                C::source_with_inputs(&source.0, (*global_transform).into(), parameters),
            ));
        }
    })
    .map_err(|error| IntegrationErrorEvent {
        entity: None,
        operation: IntegrationOperation::PublishSources,
        error,
    })
}

/// Runtime source component.
#[cfg(feature = "dynamic")]
pub type DynamicSource = Source<super::binding::RuntimeConfiguration>;

/// Runtime source parameters component.
///
/// Pathing requires explicit parameters with a probe batch.
#[cfg(feature = "dynamic")]
pub type DynamicSourceParameters = SourceParameters<super::binding::RuntimeConfiguration>;

#[cfg(feature = "dynamic")]
impl DynamicSource {
    /// Creates a source with the selected simulation modes.
    ///
    /// # Errors
    ///
    /// - [`crate::dynamic::SimulationError::UnknownFlags`] for unknown mode bits.
    /// - [`crate::dynamic::SimulationError::DisabledModes`] for unavailable modes.
    /// - [`crate::dynamic::SimulationError::SteamAudio`] if native creation fails.
    pub fn try_with_flags(
        simulation: &super::simulation::DynamicSimulation,
        flags: crate::SimulationFlags,
    ) -> Result<Self, crate::dynamic::SimulationError> {
        crate::dynamic::Source::try_new(simulation.simulator(), flags).map(Self)
    }
}

#[cfg(feature = "dynamic")]
impl From<crate::dynamic::Source> for DynamicSource {
    fn from(source: crate::dynamic::Source) -> Self {
        Self(source)
    }
}

#[cfg(feature = "dynamic")]
impl From<crate::dynamic::SimulationParameters> for DynamicSourceParameters {
    fn from(parameters: crate::dynamic::SimulationParameters) -> Self {
        Self(parameters)
    }
}

/// ```compile_fail
/// fn reject(source: audionimbus::r#static::Source<audionimbus::Direct>) {
///     let _ = audionimbus::dynamic::bevy::Source::from(source);
/// }
/// ```
/// ```compile_fail
/// fn reject(source: audionimbus::dynamic::Source) {
///     let _ = audionimbus::r#static::bevy::Source::from(source);
/// }
/// ```
/// ```compile_fail
/// fn reject(simulation: &audionimbus::r#static::bevy::Simulation) {
///     let _ = audionimbus::dynamic::bevy::Source::try_with_flags(
///         simulation, audionimbus::SimulationFlags::DIRECT);
/// }
/// ```
/// ```compile_fail
/// fn reject(simulation: &audionimbus::dynamic::bevy::Simulation) {
///     let _ = audionimbus::r#static::bevy::Source::try_new(simulation);
/// }
/// ```
/// ```compile_fail
/// fn reject(scene: audionimbus::r#static::Scene) {
///     let _ = audionimbus::dynamic::bevy::Scene::from(scene);
/// }
/// ```
/// ```compile_fail
/// fn reject(scene: audionimbus::dynamic::Scene) {
///     let _ = audionimbus::r#static::bevy::Scene::from(scene);
/// }
/// ```
#[cfg(all(doctest, feature = "static", feature = "dynamic"))]
struct BindingBoundary;
