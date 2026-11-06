//! Bevy plugin and shared simulation inputs resource.

use super::binding::{ConfigurationBinding, DefaultConfiguration};
use super::configuration::{DefaultSimulationConfiguration, SimulationConfiguration};
#[cfg(feature = "dynamic")]
use super::error::report_integration_error;
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
use crate::simulation::settings::StaticSimulationSettings as SimulationSettings;
use crate::simulation::{
    ConvolutionSettings, Direct, DirectCompatible, DirectSimulationSettings, PathingCompatible,
    Reflections, ReflectionsCompatible, SimulationFlagsProvider,
};
use bevy::prelude::{App, IntoScheduleConfigs, IntoSystem, Plugin, PostUpdate, TransformSystems};
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
    C: ConfigurationBinding = DefaultConfiguration,
    RD: Runner = (),
    RR: Runner = (),
    RP: Runner = (),
> {
    simulation_settings: C::Settings,
    /// The worker selection settings.
    runners: C::Runners,
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
        simulation_settings: BoundSimulationSettings<DefaultSimulationConfiguration>,
    ) -> Self {
        Self {
            simulation_settings,
            runners: (),
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
        simulation_settings: BoundSimulationSettings<C>,
    ) -> InferredSpatialAudioPlugin<C>
    where
        C: SimulationConfiguration,
        C::Direct: ToRunner,
        C::Reflections: ToRunner,
        C::Pathing: ToRunner,
    {
        SpatialAudioPlugin {
            simulation_settings,
            runners: (),
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
    /// # #[cfg(feature = "static")]
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # use audionimbus::*;
    /// # use audionimbus::r#static::SimulationSettings;
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
    /// # }
    /// # #[cfg(not(feature = "static"))]
    /// # fn main() {}
    /// ```
    pub fn with_runners<RD2, RR2, RP2>(self) -> SpatialAudioPlugin<C, RD2, RR2, RP2>
    where
        RD2: Runner<SimulationType = RD::SimulationType>,
        RR2: Runner<SimulationType = RR::SimulationType>,
        RP2: Runner<SimulationType = RP::SimulationType>,
    {
        SpatialAudioPlugin {
            simulation_settings: self.simulation_settings,
            runners: (),
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<C, RD, RR, RP> Plugin for SpatialAudioPlugin<C, RD, RR, RP>
where
    C: PluginSystems + TypePath,
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

        let simulation = C::simulation(&context, &self.simulation_settings);

        world.insert_resource(Simulation::<C>(simulation));
        world.insert_resource(SimulationSharedInputs::<C>::default());

        let (sender, receiver) = error_channel::<C::Error>();
        world.insert_resource(sender);
        world.insert_non_send(receiver);

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
            sync_simulation_shared_inputs_listener::<C>
                .after(TransformSystems::Propagate)
                .in_set(SpatialAudioSet::SyncSimulationSharedInputs),
        );

        app.add_systems(
            PostUpdate,
            propagate_simulation_errors::<C::Error>.in_set(SpatialAudioSet::PropagateErrors),
        );

        RD::add_systems(app);
        RR::add_systems(app);
        RP::add_systems(app);

        app.add_observer(on_listener_added);
        app.add_observer(on_probe_batch_added::<C>);
        app.add_observer(on_probe_batch_removed::<C>);
        app.add_observer(on_static_mesh_removed::<C>);
        app.add_observer(on_instanced_mesh_removed::<C>);
        app.add_observer(on_scene_removed::<C>);

        use super::asset::{
            ProbeBatchAsset, ProbeBatchAssetLoader, SceneAsset, SceneAssetLoader,
            sync_probe_batches_from_assets, sync_scenes_from_assets,
        };
        use bevy::asset::{AssetApp, AssetServer};
        use bevy::prelude::resource_exists;
        if app.world().contains_resource::<AssetServer>() {
            app.init_asset::<ProbeBatchAsset>()
                .init_asset_loader::<ProbeBatchAssetLoader>()
                .init_asset::<SceneAsset>()
                .init_asset_loader::<SceneAssetLoader>();
            app.add_systems(
                PostUpdate,
                (sync_probe_batches_from_assets, sync_scenes_from_assets::<C>)
                    .run_if(resource_exists::<Context>)
                    .in_set(SpatialAudioSet::SyncAssets),
            );
        }

        C::add_systems(app, &self.runners);
    }
}

/// Simulation settings bound to a [`SimulationConfiguration`].
pub type BoundSimulationSettings<C> = SimulationSettings<
    <C as SimulationConfiguration>::RayTracer,
    <C as SimulationConfiguration>::Direct,
    <C as SimulationConfiguration>::Reflections,
    <C as SimulationConfiguration>::Pathing,
    <C as SimulationConfiguration>::ReflectionEffect,
>;

/// Spatial audio plugin inferred from its [`SimulationConfiguration`].
pub type InferredSpatialAudioPlugin<C> = SpatialAudioPlugin<
    C,
    <<C as SimulationConfiguration>::Direct as ToRunner>::Runner,
    <<C as SimulationConfiguration>::Reflections as ToRunner>::Runner,
    <<C as SimulationConfiguration>::Pathing as ToRunner>::Runner,
>;

/// Reflection workers selected for a runtime pipeline.
#[cfg(feature = "dynamic")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ReflectionsRunner {
    /// No reflection worker.
    Disabled,
    /// Source reflections without listener reverb.
    Sources,
    /// Source reflections and listener reverb.
    #[default]
    SourcesAndReverb,
}

/// Worker choices for a runtime pipeline.
#[cfg(feature = "dynamic")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RunnerSettings {
    /// Whether to start a direct worker.
    pub direct: bool,
    /// The reflection worker selection.
    pub reflections: ReflectionsRunner,
    /// Whether to start a pathing worker.
    pub pathing: bool,
}

/// Spatial audio plugin with runtime simulation choices.
#[cfg(feature = "dynamic")]
pub type DynamicSpatialAudioPlugin = SpatialAudioPlugin<super::binding::RuntimeConfiguration>;

#[cfg(feature = "dynamic")]
impl DynamicSpatialAudioPlugin {
    /// Creates a plugin with workers inferred from the available simulation modes.
    pub fn with_settings(simulation_settings: crate::dynamic::SimulationSettings) -> Self {
        let flags = simulation_settings.flags();
        let runners = RunnerSettings {
            direct: flags.contains(crate::SimulationFlags::DIRECT),
            reflections: if flags.contains(crate::SimulationFlags::REFLECTIONS) {
                ReflectionsRunner::SourcesAndReverb
            } else {
                ReflectionsRunner::Disabled
            },
            pathing: flags.contains(crate::SimulationFlags::PATHING),
        };

        Self {
            simulation_settings,
            runners,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Overrides the inferred workers.
    ///
    /// Selected workers require their mode in the simulation settings.
    pub fn with_runners(mut self, runners: RunnerSettings) -> Self {
        self.runners = runners;
        self
    }
}

/// System registration selected by the integration binding.
pub trait PluginSystems: ConfigurationBinding {
    /// Registers publication, observers, workers, and mode-specific scene assets.
    fn add_systems(app: &mut App, runners: &Self::Runners);
}

impl<C: SimulationConfiguration> PluginSystems for C {
    fn add_systems(app: &mut App, _: &()) {
        app.add_systems(
            PostUpdate,
            sync_sources::<C>
                .map(|outcome| outcome.unwrap_or_else(|event| match event.error {}))
                .after(TransformSystems::Propagate)
                .in_set(SpatialAudioSet::SyncSources),
        );
        app.add_observer(on_source_added::<C>);
        app.add_observer(on_source_removed::<C>);
        app.add_observer(
            on_main_scene_added::<C>
                .map(|outcome| outcome.unwrap_or_else(|event| match event.error {})),
        );
    }
}

#[cfg(feature = "dynamic")]
impl PluginSystems for super::binding::RuntimeConfiguration {
    fn add_systems(app: &mut App, runners: &Self::Runners) {
        use super::runner::{
            RunnerDirect, RunnerPathing, RunnerReflections, RunnerReflectionsReverb,
        };
        if runners.direct {
            <RunnerDirect as Spawn<Self>>::spawn(app.world_mut());
            <RunnerDirect as SyncFrame<Self>>::add_systems(app);
        }
        match runners.reflections {
            ReflectionsRunner::Disabled => {}
            ReflectionsRunner::Sources => {
                <RunnerReflections as Spawn<Self>>::spawn(app.world_mut());
                <RunnerReflections as SyncFrame<Self>>::add_systems(app);
            }
            ReflectionsRunner::SourcesAndReverb => {
                <RunnerReflectionsReverb as Spawn<Self>>::spawn(app.world_mut());
                <RunnerReflectionsReverb as SyncFrame<Self>>::add_systems(app);
            }
        }
        if runners.pathing {
            <RunnerPathing as Spawn<Self>>::spawn(app.world_mut());
            <RunnerPathing as SyncFrame<Self>>::add_systems(app);
        }
        app.add_systems(
            PostUpdate,
            sync_sources::<Self>
                .pipe(report_integration_error)
                .after(TransformSystems::Propagate)
                .in_set(SpatialAudioSet::SyncSources),
        );
        app.add_observer(super::source::try_on_source_added::<Self>.pipe(report_integration_error));
        app.add_observer(
            super::source::try_on_source_removed::<Self>.pipe(report_integration_error),
        );
        app.add_observer(on_main_scene_added::<Self>.pipe(report_integration_error));
    }
}
