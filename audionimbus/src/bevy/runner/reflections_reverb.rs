//! Reflections and reverb simulation runner.

use super::super::binding::{ConfigurationBinding, DefaultConfiguration};
use super::super::configuration::SimulationConfiguration;
#[cfg(feature = "dynamic")]
use super::super::error::report_integration_error;
use super::super::error::{IntegrationErrorEvent, IntegrationOperation};
use super::super::error::{SimulationErrorEvent, SimulationErrorSender};
use super::super::simulation::{Simulation, SimulationSharedInputs, SimulationThread};
use super::super::source::{Listener, Source, SourceParameters};
use super::super::system_set::SpatialAudioSet;
use super::{Runner, Spawn, SyncFrame, ToRunner};
use crate::sealed::Sealed;
use crate::simulation::{DirectCompatible, PathingCompatible, Reflections};
use crate::wiring::ReflectionsReverbFrame;
use bevy::ecs::system::{StaticSystemParam, SystemParamItem};
use bevy::prelude::{
    App, Entity, GlobalTransform, IntoScheduleConfigs, IntoSystem, PostUpdate, Query, Res,
    Resource, With, World, resource_exists,
};
use bevy::transform::TransformSystems;
use std::ops::{Deref, DerefMut};

/// Runner for reflections and listener-centric reverb simulation.
pub struct RunnerReflectionsReverb;

impl Sealed for RunnerReflectionsReverb {}
impl Runner for RunnerReflectionsReverb {
    type SimulationType = Reflections;
}

impl ToRunner for Reflections {
    type Runner = RunnerReflectionsReverb;
}

impl<C: ReflectionsReverbSpawn> Spawn<C> for RunnerReflectionsReverb {
    fn spawn(world: &mut World) {
        let error_sender = world
            .resource::<SimulationErrorSender<C::Error>>()
            .0
            .clone();
        let runner = C::spawn(&mut world.resource_mut::<Simulation<C>>().0, move |error| {
            let _ = error_sender.try_send(SimulationErrorEvent {
                thread: SimulationThread::ReflectionsReverb,
                error,
            });
        });
        world.insert_resource(ReflectionsReverbSimulation::<C>(runner));
    }
}

impl<C> SyncFrame<C> for RunnerReflectionsReverb
where
    C: ReflectionsReverbSync,
{
    fn add_systems(app: &mut App) {
        C::add_systems(app);
    }
}

/// Publishes a new frame to the reflections-reverb thread, including the listener source when one
/// is present.
#[allow(clippy::type_complexity)]
fn sync_reflections_reverb_frame<C>(
    query: Query<
        (
            Entity,
            &GlobalTransform,
            &Source<C>,
            Option<&SourceParameters<C>>,
        ),
        With<Listener>,
    >,
    mut simulation: StaticSystemParam<C::SimulationAccess>,
    reflections_reverb: Res<ReflectionsReverbSimulation<C>>,
    shared_inputs: Res<SimulationSharedInputs<C>>,
) -> Result<(), IntegrationErrorEvent<C::IntegrationError>>
where
    C: ReflectionsReverbSync,
{
    let listener = query.iter().next();
    let entity = listener.map(|(entity, ..)| entity);
    let listener = listener.map(|(_, global_transform, source, parameters)| {
        C::source_with_inputs(
            &source.0,
            (*global_transform).into(),
            parameters.map_or_else(
                || C::listener_parameters(&source.0),
                |params| params.0.clone(),
            ),
        )
    });
    C::sync(
        &reflections_reverb.0,
        &mut simulation,
        &shared_inputs.0,
        listener,
    )
    .map_err(|error| IntegrationErrorEvent {
        entity,
        operation: IntegrationOperation::PublishListener,
        error,
    })
}

/// Resource wrapping a [`wiring::ReflectionsReverbSimulation`](crate::wiring::simulation::ReflectionsReverbSimulation).
#[derive(Resource)]
pub struct ReflectionsReverbSimulation<C: ConfigurationBinding = DefaultConfiguration>(
    pub C::ReflectionsReverbSimulation,
);

impl<C: ConfigurationBinding> Deref for ReflectionsReverbSimulation<C> {
    type Target = C::ReflectionsReverbSimulation;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<C: ConfigurationBinding> DerefMut for ReflectionsReverbSimulation<C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Frame publication for configurations that enable reflections reverb.
pub trait ReflectionsReverbSync: ConfigurationBinding {
    /// Registers frame publication with the selected error handler.
    fn add_systems(app: &mut App);

    /// Publishes inputs for the next worker iteration.
    fn sync(
        worker: &Self::ReflectionsReverbSimulation,
        simulation: &mut SystemParamItem<'_, '_, Self::SimulationAccess>,
        inputs: &Self::SharedInputs,
        listener: Option<Self::SourceWithInputs>,
    ) -> Result<(), Self::IntegrationError>;
}

impl<C> ReflectionsReverbSync for C
where
    C: SimulationConfiguration<Reflections = Reflections>,
{
    fn add_systems(app: &mut App) {
        app.add_systems(
            PostUpdate,
            sync_reflections_reverb_frame::<C>
                .map(|outcome| outcome.unwrap_or_else(|event| match event.error {}))
                .run_if(resource_exists::<ReflectionsReverbSimulation<C>>)
                .after(TransformSystems::Propagate)
                .in_set(SpatialAudioSet::SyncFrames),
        );
    }

    fn sync(
        worker: &Self::ReflectionsReverbSimulation,
        simulation: &mut SystemParamItem<'_, '_, Self::SimulationAccess>,
        inputs: &Self::SharedInputs,
        listener: Option<Self::SourceWithInputs>,
    ) -> Result<(), Self::IntegrationError> {
        worker.set_input(ReflectionsReverbFrame {
            sources: simulation.sources.clone(),
            listener,
            shared_inputs: inputs.clone(),
        });

        Ok(())
    }
}

/// Runtime reflections and listener reverb resource.
#[cfg(feature = "dynamic")]
pub type DynamicReflectionsReverbSimulation =
    ReflectionsReverbSimulation<super::super::binding::RuntimeConfiguration>;

#[cfg(feature = "dynamic")]
impl ReflectionsReverbSync for super::super::binding::RuntimeConfiguration {
    fn add_systems(app: &mut App) {
        app.add_systems(
            PostUpdate,
            sync_reflections_reverb_frame::<Self>
                .pipe(report_integration_error)
                .run_if(resource_exists::<ReflectionsReverbSimulation<Self>>)
                .after(TransformSystems::Propagate)
                .in_set(SpatialAudioSet::SyncFrames),
        );
    }

    fn sync(
        worker: &Self::ReflectionsReverbSimulation,
        simulation: &mut SystemParamItem<'_, '_, Self::SimulationAccess>,
        inputs: &Self::SharedInputs,
        listener: Option<Self::SourceWithInputs>,
    ) -> Result<(), Self::IntegrationError> {
        let simulation = bevy::prelude::DetectChangesMut::bypass_change_detection(simulation);
        let listener = listener.filter(|listener| {
            listener
                .source
                .flags()
                .contains(crate::SimulationFlags::REFLECTIONS)
        });
        simulation.set_listener(listener)?;
        worker.set_input(inputs.clone());

        Ok(())
    }
}

/// Worker startup for configurations that enable reflections reverb.
pub trait ReflectionsReverbSpawn: ConfigurationBinding {
    /// Starts the selected worker.
    ///
    /// # Panics
    ///
    /// Panics if the selected runtime mode is disabled.
    fn spawn(
        simulation: &mut Self::Simulation,
        on_error: impl Fn(Self::Error) + Send + 'static,
    ) -> Self::ReflectionsReverbSimulation;
}

impl<C> ReflectionsReverbSpawn for C
where
    C: SimulationConfiguration<Reflections = Reflections>,
    (): DirectCompatible<<C as SimulationConfiguration>::Direct>
        + PathingCompatible<<C as SimulationConfiguration>::Pathing>,
{
    fn spawn(
        simulation: &mut Self::Simulation,
        on_error: impl Fn(Self::Error) + Send + 'static,
    ) -> Self::ReflectionsReverbSimulation {
        simulation.spawn_reflections_reverb(on_error)
    }
}

#[cfg(feature = "dynamic")]
impl ReflectionsReverbSpawn for super::super::binding::RuntimeConfiguration {
    fn spawn(
        simulation: &mut Self::Simulation,
        on_error: impl Fn(Self::Error) + Send + 'static,
    ) -> Self::ReflectionsReverbSimulation {
        simulation
            .spawn_reflections_reverb(on_error)
            .expect("worker mode is disabled")
    }
}
