//! Bevy plugin and shared simulation inputs resource.

use super::asset::{
    ProbeBatchAsset, ProbeBatchAssetLoader, SceneAsset, SceneAssetLoader,
    sync_probe_batches_from_assets, sync_scenes_from_assets,
};
use super::configuration::{DefaultSimulationConfiguration, SimulationConfiguration};
use super::error::{error_channel, propagate_simulation_errors};
use super::geometry::{
    commit_scenes, on_instanced_mesh_removed, on_main_scene_added, on_scene_removed,
    on_static_mesh_removed, sync_instanced_meshes, sync_static_meshes,
};
use super::probe::{commit_probe_batches, on_probe_batch_added, on_probe_batch_removed};
use super::runner::{Runner, Spawn, SyncFrame, ToRunner};
use super::simulation::{Simulation, SimulationSharedInputs};
use super::source::{
    on_listener_added, on_source_added, on_source_removed, sync_simulation_shared_inputs_listener,
    sync_sources,
};
use super::system_set::SpatialAudioSet;
use crate::audio_settings::AudioSettings;
use crate::context::Context;
use crate::simulation::{
    ConvolutionSettings, Direct, DirectCompatible, DirectSimulationSettings, PathingCompatible,
    Reflections, ReflectionsCompatible, SimulationFlagsProvider, SimulationSettings, Simulator,
};
use bevy::asset::{AssetApp, AssetServer};
use bevy::prelude::{
    App, Entity, IntoScheduleConfigs, Plugin, PostUpdate, TransformSystems, resource_exists,
};
use bevy::reflect::TypePath;

#[cfg(debug_assertions)]
use super::geometry::warn_missing_main_scene;
#[cfg(doc)]
use super::runner::{RunnerReflections, RunnerReflectionsReverb};

/// Bevy plugin that sets up the AudioNimbus spatial audio pipeline.
///
/// # Type parameters
///
/// | Parameter | Role |
/// |---|---|
/// | `C` | Simulation configuration (see [`SimulationConfiguration`]) |
/// | `RD` | Direct runner (`RunnerDirect` or `()`) |
/// | `RR` | Reflections runner (`RunnerReflections`, `RunnerReflectionsReverb`, or `()`) |
/// | `RP` | Pathing runner (`RunnerPathing` or `()`) |
pub struct SpatialAudioPlugin<
    C: SimulationConfiguration = DefaultSimulationConfiguration,
    RD: Runner = (),
    RR: Runner = (),
    RP: Runner = (),
> {
    simulation_settings: SimulationSettings<
        C::RayTracer,
        C::Direct,
        C::Reflections,
        C::Pathing,
        C::ReflectionEffect,
    >,
    _phantom: std::marker::PhantomData<(RD, RR, RP)>,
}

impl
    SpatialAudioPlugin<
        DefaultSimulationConfiguration,
        <Direct as ToRunner>::Runner,
        <Reflections as ToRunner>::Runner,
        <() as ToRunner>::Runner,
    >
{
    /// Creates a new plugin using [`DefaultSimulationConfiguration`] and explicit simulation
    /// settings.
    ///
    /// Use [`default`](SpatialAudioPlugin::default) for the built-in starter configuration, or
    /// [`with_config`](SpatialAudioPlugin::with_config) to supply a custom [`SimulationConfiguration`].
    pub fn new(
        simulation_settings: SimulationSettings<
            <DefaultSimulationConfiguration as SimulationConfiguration>::RayTracer,
            <DefaultSimulationConfiguration as SimulationConfiguration>::Direct,
            <DefaultSimulationConfiguration as SimulationConfiguration>::Reflections,
            <DefaultSimulationConfiguration as SimulationConfiguration>::Pathing,
            <DefaultSimulationConfiguration as SimulationConfiguration>::ReflectionEffect,
        >,
    ) -> Self {
        Self {
            simulation_settings,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Creates a new plugin using the provided audio settings and the default simulation settings.
    pub fn with_audio_settings(audio_settings: &AudioSettings) -> Self {
        Self::new(
            SimulationSettings::new(audio_settings)
                .with_direct(DirectSimulationSettings::default())
                .with_reflections(ConvolutionSettings::default()),
        )
    }
}

impl Default
    for SpatialAudioPlugin<
        DefaultSimulationConfiguration,
        <Direct as ToRunner>::Runner,
        <Reflections as ToRunner>::Runner,
        <() as ToRunner>::Runner,
    >
{
    fn default() -> Self {
        Self::with_audio_settings(&AudioSettings::default())
    }
}

impl SpatialAudioPlugin<DefaultSimulationConfiguration, (), (), ()> {
    /// Creates a new plugin with a custom [`SimulationConfiguration`].
    ///
    /// Runners are inferred from the configuration's simulation modes.
    /// Use [`with_runners`](SpatialAudioPlugin::with_runners) to override them.
    pub fn with_config<C>(
        simulation_settings: SimulationSettings<
            C::RayTracer,
            C::Direct,
            C::Reflections,
            C::Pathing,
            C::ReflectionEffect,
        >,
    ) -> SpatialAudioPlugin<
        C,
        <C::Direct as ToRunner>::Runner,
        <C::Reflections as ToRunner>::Runner,
        <C::Pathing as ToRunner>::Runner,
    >
    where
        C: SimulationConfiguration,
        C::Direct: ToRunner,
        C::Reflections: ToRunner,
        C::Pathing: ToRunner,
    {
        SpatialAudioPlugin {
            simulation_settings,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<C, RD, RR, RP> SpatialAudioPlugin<C, RD, RR, RP>
where
    C: SimulationConfiguration,
    RD: Runner,
    RR: Runner,
    RP: Runner,
{
    /// Overrides the inferred runners with explicit runner types.
    ///
    /// Use this when the default [`ToRunner`] mapping doesn't match your needs, for example to use
    /// [`RunnerReflections`] instead of the default [`RunnerReflectionsReverb`] when you don't
    /// need listener-centric reverb.
    ///
    /// ```rust
    /// # use audionimbus::*;
    /// # use audionimbus::bevy::*;
    /// #
    /// # let audio_settings = AudioSettings::default();
    /// # let settings = SimulationSettings::new(&audio_settings)
    /// #     .with_direct(DirectSimulationSettings {
    /// #         max_num_occlusion_samples: 4,
    /// #     })
    /// #     .with_reflections(ConvolutionSettings {
    /// #         max_num_rays: 4096,
    /// #         num_diffuse_samples: 32,
    /// #         max_duration: 2.0,
    /// #         max_num_sources: 8,
    /// #         num_threads: 2,
    /// #         max_order: 1,
    /// #     });
    /// #
    /// # pub struct MyConfig;
    /// #
    /// # impl SimulationConfiguration for MyConfig {
    /// #     type RayTracer = DefaultRayTracer;
    /// #     type Direct = Direct;
    /// #     type Reflections = Reflections;
    /// #     type Pathing = ();
    /// #     type ReflectionEffect = Convolution;
    /// # }
    /// SpatialAudioPlugin::with_config::<MyConfig>(settings)
    ///     .with_runners::<RunnerDirect, RunnerReflections, ()>();
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn with_runners<RD2, RR2, RP2>(self) -> SpatialAudioPlugin<C, RD2, RR2, RP2>
    where
        RD2: Runner<SimulationType = RD::SimulationType>,
        RR2: Runner<SimulationType = RR::SimulationType>,
        RP2: Runner<SimulationType = RP::SimulationType>,
    {
        SpatialAudioPlugin {
            simulation_settings: self.simulation_settings,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<C, RD, RR, RP> Plugin for SpatialAudioPlugin<C, RD, RR, RP>
where
    C: SimulationConfiguration + TypePath,
    RD: 'static + Runner + Send + Sync + Spawn<C> + SyncFrame<C>,
    RR: 'static + Runner + Send + Sync + Spawn<C> + SyncFrame<C>,
    RP: 'static + Runner + Send + Sync + Spawn<C> + SyncFrame<C>,
    RD::SimulationType: 'static
        + Send
        + Sync
        + Clone
        + Default
        + DirectCompatible<RD::SimulationType>
        + SimulationFlagsProvider,
    RR::SimulationType: 'static
        + Send
        + Sync
        + Clone
        + Default
        + ReflectionsCompatible<RR::SimulationType>
        + SimulationFlagsProvider,
    RP::SimulationType: 'static
        + Send
        + Sync
        + Clone
        + Default
        + PathingCompatible<RP::SimulationType>
        + SimulationFlagsProvider,
    (): DirectCompatible<RD::SimulationType>
        + ReflectionsCompatible<RR::SimulationType>
        + PathingCompatible<RP::SimulationType>,
{
    fn build(&self, app: &mut App) {
        app.register_type::<SpatialAudioSet>()
            .register_type::<super::source::Source<C>>()
            .register_type::<super::source::SourceParameters<C>>()
            .register_type::<super::source::Listener>()
            .register_type::<super::geometry::Scene<C>>()
            .register_type::<super::geometry::MainScene>()
            .register_type::<super::geometry::StaticMesh>()
            .register_type::<super::geometry::InstancedMesh>()
            .register_type::<super::probe::ProbeArray>()
            .register_type::<super::probe::ProbeBatch>()
            .register_type::<super::asset::ProbeBatchAssetSource>();

        let world = app.world_mut();

        let context = world.get_resource_or_insert_with(Context::default).clone();

        let simulator = Simulator::try_new(&context, &self.simulation_settings)
            .expect("failed to create simulator");
        let simulation = crate::wiring::simulation::Simulation::new::<Entity>(simulator);

        world.insert_resource(Simulation::<C>(simulation));
        world.insert_resource(SimulationSharedInputs::<C>::default());

        let (sender, receiver) = error_channel();
        world.insert_resource(sender);
        world.insert_non_send_resource(receiver);

        RD::spawn(world);
        RR::spawn(world);
        RP::spawn(world);

        app.configure_sets(
            PostUpdate,
            (
                SpatialAudioSet::SyncAssets,
                (
                    SpatialAudioSet::SyncGeometry,
                    SpatialAudioSet::SyncProbes,
                    SpatialAudioSet::SyncSources,
                    SpatialAudioSet::SyncSimulationSharedInputs,
                ),
                SpatialAudioSet::SyncFrames,
                SpatialAudioSet::PropagateErrors,
            )
                .chain(),
        );

        app.add_systems(
            PostUpdate,
            (
                (
                    sync_static_meshes::<C>.after(TransformSystems::Propagate),
                    sync_instanced_meshes::<C>.after(TransformSystems::Propagate),
                    commit_scenes::<C>,
                )
                    .chain(),
                #[cfg(debug_assertions)]
                warn_missing_main_scene::<C>,
            )
                .in_set(SpatialAudioSet::SyncGeometry),
        );

        app.add_systems(
            PostUpdate,
            commit_probe_batches::<C>.in_set(SpatialAudioSet::SyncProbes),
        );

        app.add_systems(
            PostUpdate,
            sync_sources::<C>
                .after(TransformSystems::Propagate)
                .in_set(SpatialAudioSet::SyncSources),
        );

        app.add_systems(
            PostUpdate,
            sync_simulation_shared_inputs_listener::<C>
                .after(TransformSystems::Propagate)
                .in_set(SpatialAudioSet::SyncSimulationSharedInputs),
        );

        app.add_systems(
            PostUpdate,
            propagate_simulation_errors.in_set(SpatialAudioSet::PropagateErrors),
        );

        RD::add_systems(app);
        RR::add_systems(app);
        RP::add_systems(app);

        app.add_observer(on_source_added::<C>);
        app.add_observer(on_source_removed::<C>);
        app.add_observer(on_listener_added);
        app.add_observer(on_probe_batch_added::<C>);
        app.add_observer(on_probe_batch_removed::<C>);
        app.add_observer(on_static_mesh_removed::<C>);
        app.add_observer(on_instanced_mesh_removed::<C>);
        app.add_observer(on_main_scene_added::<C>);
        app.add_observer(on_scene_removed::<C>);

        if app.world().contains_resource::<AssetServer>() {
            app.init_asset::<SceneAsset>()
                .init_asset_loader::<SceneAssetLoader>();
            app.init_asset::<ProbeBatchAsset>()
                .init_asset_loader::<ProbeBatchAssetLoader>();

            app.add_systems(
                PostUpdate,
                (sync_scenes_from_assets::<C>, sync_probe_batches_from_assets)
                    .run_if(resource_exists::<Context>)
                    .in_set(SpatialAudioSet::SyncAssets),
            );
        }
    }
}
