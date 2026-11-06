//! Handle storage selected by the Bevy integration.

use super::configuration::SimulationConfiguration;
use crate::binding::Static;
use crate::geometry::StaticMeshSettings;
use crate::geometry::scene::{GeometryBinding, Scene};
use crate::simulation::inputs::{SimulationParameters, StaticSimulationInputs};
use crate::wiring::simulation::pipeline::{StaticSimulation, StaticSourceWithInputs};
use crate::{Context, CoordinateSystem};
use bevy::ecs::system::{SystemParam, SystemParamItem};
use bevy::prelude::Entity;
use std::borrow::{Borrow, BorrowMut, Cow};

/// Handle selection that keeps source and simulator bindings inseparable.
pub trait ConfigurationBinding: Send + Sync + 'static {
    /// The error from instanced mesh creation.
    type InstancedMeshError: std::error::Error;
    /// The scene and mesh binding.
    type Geometry: GeometryBinding + 'static;
    /// The simulation pipeline binding.
    type Pipeline: crate::wiring::binding::PipelineBinding<Geometry = Self::Geometry>;
    /// The simulation input binding.
    type Inputs: crate::simulation::inputs::InputBinding<Parameters = Self::Parameters>;
    /// The simulation pipeline stored in the ECS resource.
    type Simulation: Borrow<crate::wiring::simulation::pipeline::Simulation<Entity, Self::Pipeline>>
        + BorrowMut<crate::wiring::simulation::pipeline::Simulation<Entity, Self::Pipeline>>
        + Send
        + Sync
        + 'static;
    /// The ECS access used for source and listener publication.
    type SimulationAccess: SystemParam + 'static;
    /// The source handle.
    type Source: Clone + Send + Sync + 'static;
    /// The per-source simulation parameters.
    type Parameters: Clone + Send + Sync + 'static;
    /// The configuration-wide defaults used by debug drawing.
    type DebugParameters;
    /// The source handle and its simulation inputs.
    type SourceWithInputs;
    /// The simulation inputs shared by sources and listeners.
    type SharedInputs: Borrow<crate::simulation::simulator::SimulationSharedInputs<Self::Inputs>>
        + BorrowMut<crate::simulation::simulator::SimulationSharedInputs<Self::Inputs>>
        + Default
        + Clone
        + Send
        + Sync
        + 'static;
    /// The simulator creation settings.
    type Settings: Send + Sync;
    /// The worker selection settings.
    type Runners: Send + Sync;
    /// The error returned by a simulation worker.
    type Error: std::error::Error + Send + Sync + 'static;
    /// The validation error returned by ECS operations.
    type IntegrationError: std::error::Error + Send + Sync + 'static;
    /// The direct simulation worker.
    type DirectSimulation: Send + Sync + 'static;
    /// The source reflections worker.
    type ReflectionsSimulation: Send + Sync + 'static;
    /// The worker for source reflections and listener reverb.
    type ReflectionsReverbSimulation: Send + Sync + 'static;
    /// The pathing simulation worker.
    type PathingSimulation: Send + Sync + 'static;

    /// Creates the pipeline with the selected settings.
    ///
    /// # Panics
    ///
    /// Panics if native simulator creation fails.
    fn simulation(context: &Context, settings: &Self::Settings) -> Self::Simulation;
    /// Returns shared source parameters for debug drawing.
    fn debug_parameters() -> Self::DebugParameters;
    /// Selects explicit parameters or the debug fallback for a source.
    fn source_debug_parameters<'a>(
        source: &Self::Source,
        parameters: Option<&'a Self::Parameters>,
        default_parameters: &'a Self::DebugParameters,
    ) -> Cow<'a, Self::Parameters>;
    /// Returns implicit source parameters for this configuration.
    fn implicit_parameters(source: &Self::Source) -> Self::Parameters;
    /// Returns default parameters for a listener source.
    fn listener_parameters(source: &Self::Source) -> Self::Parameters;
    /// Combines a source with its coordinates and simulation parameters.
    fn source_with_inputs(
        source: &Self::Source,
        coordinates: CoordinateSystem,
        parameters: Self::Parameters,
    ) -> Self::SourceWithInputs;
    /// Publishes the source snapshot.
    fn update_sources(
        simulation: &mut SystemParamItem<'_, '_, Self::SimulationAccess>,
        fill: impl FnOnce(&mut Vec<(Entity, Self::SourceWithInputs)>),
    ) -> Result<(), Self::IntegrationError>;
    /// Registers a source with the simulator.
    #[cfg_attr(feature = "dynamic", doc = "\n# Errors\n")]
    #[cfg_attr(
        feature = "dynamic",
        doc = "- [`crate::dynamic::SimulationError::ForeignSource`] for another simulator's source."
    )]
    fn add_source(
        simulation: &mut Self::Simulation,
        source: &Self::Source,
    ) -> Result<(), Self::IntegrationError>;
    /// Removes a source from the simulator.
    #[cfg_attr(feature = "dynamic", doc = "\n# Errors\n")]
    #[cfg_attr(
        feature = "dynamic",
        doc = "- [`crate::dynamic::SimulationError::ForeignSource`] for another simulator's source."
    )]
    fn remove_source(
        simulation: &mut Self::Simulation,
        source: &Self::Source,
    ) -> Result<(), Self::IntegrationError>;
    /// Selects the scene used by the simulator.
    #[cfg_attr(feature = "dynamic", doc = "\n# Errors\n")]
    #[cfg_attr(
        feature = "dynamic",
        doc = "- [`crate::dynamic::SimulationError::RayTracerMismatch`] for different ray tracers."
    )]
    #[cfg_attr(
        feature = "dynamic",
        doc = "- [`crate::dynamic::SimulationError::DeviceMismatch`] for incompatible devices."
    )]
    fn set_scene(
        simulation: &mut Self::Simulation,
        scene: &Scene<Self::Geometry>,
    ) -> Result<(), Self::IntegrationError>;

    /// Creates a static mesh for this scene.
    ///
    /// # Errors
    ///
    /// - [`crate::geometry::StaticMeshError::MaterialIndexCountMismatch`] for mismatched counts.
    /// - [`crate::geometry::StaticMeshError::VertexIndexOutOfBounds`] for a missing vertex.
    /// - [`crate::geometry::StaticMeshError::MaterialIndexOutOfBounds`] for a missing material.
    /// - [`crate::geometry::StaticMeshError::SteamAudio`] if native creation fails.
    fn static_mesh(
        scene: &Scene<Self::Geometry>,
        settings: &StaticMeshSettings,
    ) -> Result<
        crate::geometry::static_mesh::StaticMesh<Self::Geometry>,
        crate::geometry::StaticMeshError,
    >;
    /// Adds a static mesh created for this scene.
    ///
    /// # Panics
    ///
    /// Panics if the mesh uses a different runtime ray tracer.
    fn add_static_mesh(
        scene: &mut Scene<Self::Geometry>,
        mesh: crate::geometry::static_mesh::StaticMesh<Self::Geometry>,
    ) -> crate::geometry::StaticMeshHandle;
    /// Creates an instanced mesh for this scene.
    ///
    /// # Errors
    ///
    /// - [`crate::SteamAudioError::Unspecified`] for a native error in the static binding.
    /// - [`crate::SteamAudioError::OutOfMemory`] for allocation failure in the static binding.
    /// - [`crate::SteamAudioError::Initialization`] if a backend in the static binding cannot
    ///   initialize.
    #[cfg_attr(
        feature = "dynamic",
        doc = "- [`crate::dynamic::SceneError::SteamAudio`] if runtime mesh creation fails."
    )]
    #[cfg_attr(
        feature = "dynamic",
        doc = "- [`crate::dynamic::SceneError::RayTracerMismatch`] if runtime scenes use different backends."
    )]
    fn instanced_mesh(
        scene: &Scene<Self::Geometry>,
        settings: &crate::geometry::instanced_mesh::InstancedMeshSettings<Self::Geometry>,
    ) -> Result<
        crate::geometry::instanced_mesh::InstancedMesh<Self::Geometry>,
        Self::InstancedMeshError,
    >;
    /// Adds an instanced mesh created for this scene.
    ///
    /// # Panics
    ///
    /// Panics if the mesh uses a different runtime ray tracer.
    fn add_instanced_mesh(
        scene: &mut Scene<Self::Geometry>,
        mesh: crate::geometry::instanced_mesh::InstancedMesh<Self::Geometry>,
    ) -> crate::geometry::InstancedMeshHandle;
}

impl<C: SimulationConfiguration> ConfigurationBinding for C {
    type InstancedMeshError = crate::SteamAudioError;
    type Geometry = Static<C::RayTracer>;
    type Pipeline = Static<(
        C::RayTracer,
        C::Direct,
        C::Reflections,
        C::Pathing,
        C::ReflectionEffect,
    )>;
    type Inputs = Static<(C::Direct, C::Reflections, C::Pathing)>;
    type Simulation = StaticSimulation<
        Entity,
        C::RayTracer,
        C::Direct,
        C::Reflections,
        C::Pathing,
        C::ReflectionEffect,
    >;
    type SimulationAccess = bevy::prelude::Res<'static, super::simulation::Simulation<C>>;
    type Source = crate::simulation::source::StaticSource<
        C::Direct,
        C::Reflections,
        C::Pathing,
        C::ReflectionEffect,
    >;
    type Parameters = SimulationParameters<C::Direct, C::Reflections, C::Pathing>;
    type DebugParameters = Self::Parameters;
    type SourceWithInputs =
        StaticSourceWithInputs<C::Direct, C::Reflections, C::Pathing, C::ReflectionEffect>;
    type SharedInputs = crate::simulation::simulator::StaticSimulationSharedInputs<
        C::Direct,
        C::Reflections,
        C::Pathing,
    >;
    type Settings = super::plugin::BoundSimulationSettings<C>;
    type Runners = ();
    type Error = crate::wiring::SimulationStepError;
    type IntegrationError = std::convert::Infallible;
    type DirectSimulation = crate::wiring::simulation::direct::StaticDirectSimulation<
        Entity,
        C::Reflections,
        C::Pathing,
        C::ReflectionEffect,
    >;
    type ReflectionsSimulation =
        crate::wiring::simulation::reflections::StaticReflectionsSimulation<
            Entity,
            C::Direct,
            C::Pathing,
            C::ReflectionEffect,
        >;
    type ReflectionsReverbSimulation =
        crate::wiring::simulation::reflections_reverb::StaticReflectionsReverbSimulation<
            Entity,
            C::Direct,
            C::Pathing,
            C::ReflectionEffect,
        >;
    type PathingSimulation = crate::wiring::simulation::pathing::StaticPathingSimulation<
        Entity,
        C::Direct,
        C::Reflections,
        C::ReflectionEffect,
    >;

    fn simulation(context: &Context, settings: &Self::Settings) -> Self::Simulation {
        let simulator = crate::simulation::simulator::StaticSimulator::try_new(context, settings)
            .expect("failed to create simulator");

        StaticSimulation::new::<Entity>(simulator)
    }

    fn debug_parameters() -> Self::DebugParameters {
        C::implicit_source_parameters()
    }

    fn source_debug_parameters<'a>(
        _: &Self::Source,
        parameters: Option<&'a Self::Parameters>,
        default_parameters: &'a Self::DebugParameters,
    ) -> Cow<'a, Self::Parameters> {
        Cow::Borrowed(parameters.unwrap_or(default_parameters))
    }

    fn implicit_parameters(_: &Self::Source) -> Self::Parameters {
        C::implicit_source_parameters()
    }
    fn listener_parameters(_: &Self::Source) -> Self::Parameters {
        Self::Parameters::default()
    }

    fn source_with_inputs(
        source: &Self::Source,
        coordinates: CoordinateSystem,
        parameters: Self::Parameters,
    ) -> Self::SourceWithInputs {
        StaticSourceWithInputs {
            source: source.clone(),
            simulation_inputs: StaticSimulationInputs {
                source: coordinates,
                parameters,
            },
        }
    }

    fn update_sources(
        simulation: &mut SystemParamItem<'_, '_, Self::SimulationAccess>,
        fill: impl FnOnce(&mut Vec<(Entity, Self::SourceWithInputs)>),
    ) -> Result<(), Self::IntegrationError> {
        simulation.0.update_sources(fill);

        Ok(())
    }
    fn add_source(
        simulation: &mut Self::Simulation,
        source: &Self::Source,
    ) -> Result<(), Self::IntegrationError> {
        simulation.simulator.add_source(source);

        Ok(())
    }
    fn remove_source(
        simulation: &mut Self::Simulation,
        source: &Self::Source,
    ) -> Result<(), Self::IntegrationError> {
        simulation.simulator.remove_source(source);

        Ok(())
    }
    fn set_scene(
        simulation: &mut Self::Simulation,
        scene: &Scene<Self::Geometry>,
    ) -> Result<(), Self::IntegrationError> {
        simulation.simulator.set_scene(scene);

        Ok(())
    }
    fn static_mesh(
        scene: &Scene<Self::Geometry>,
        settings: &StaticMeshSettings,
    ) -> Result<
        crate::geometry::static_mesh::StaticMesh<Self::Geometry>,
        crate::geometry::StaticMeshError,
    > {
        crate::geometry::static_mesh::StaticStaticMesh::try_new(scene, settings)
    }
    fn add_static_mesh(
        scene: &mut Scene<Self::Geometry>,
        mesh: crate::geometry::static_mesh::StaticMesh<Self::Geometry>,
    ) -> crate::geometry::StaticMeshHandle {
        scene.add_static_mesh(mesh)
    }
    fn instanced_mesh(
        scene: &Scene<Self::Geometry>,
        settings: &crate::geometry::instanced_mesh::InstancedMeshSettings<Self::Geometry>,
    ) -> Result<
        crate::geometry::instanced_mesh::InstancedMesh<Self::Geometry>,
        Self::InstancedMeshError,
    > {
        crate::geometry::instanced_mesh::StaticInstancedMesh::try_new(scene, settings)
    }
    fn add_instanced_mesh(
        scene: &mut Scene<Self::Geometry>,
        mesh: crate::geometry::instanced_mesh::InstancedMesh<Self::Geometry>,
    ) -> crate::geometry::InstancedMeshHandle {
        scene.add_instanced_mesh(mesh)
    }
}

/// Runtime handle selection for the Bevy integration.
#[cfg(feature = "dynamic")]
#[derive(bevy::reflect::TypePath)]
pub struct RuntimeConfiguration;

#[cfg(feature = "dynamic")]
impl ConfigurationBinding for RuntimeConfiguration {
    type InstancedMeshError = crate::dynamic::SceneError;
    type Geometry = crate::binding::Dynamic<crate::dynamic::RayTracerKind>;
    type Pipeline = crate::binding::Dynamic<crate::simulation::simulator::SimulationChoices>;
    type Inputs = crate::binding::Dynamic<()>;
    type Simulation = crate::dynamic::Simulation<Entity>;
    type SimulationAccess = bevy::prelude::ResMut<'static, super::simulation::Simulation<Self>>;
    type Source = crate::dynamic::Source;
    type Parameters = crate::dynamic::SimulationParameters;
    type DebugParameters = ();
    type SourceWithInputs = crate::dynamic::SourceWithInputs;
    type SharedInputs = crate::dynamic::SimulationSharedInputs;
    type Settings = crate::dynamic::SimulationSettings;
    type Runners = super::plugin::RunnerSettings;
    type Error = crate::dynamic::SimulationError;
    type IntegrationError = crate::dynamic::SimulationError;
    type DirectSimulation = crate::dynamic::DirectSimulation<Entity>;
    type ReflectionsSimulation = crate::dynamic::ReflectionsSimulation<Entity>;
    type ReflectionsReverbSimulation = crate::dynamic::ReflectionsReverbSimulation<Entity>;
    type PathingSimulation = crate::dynamic::PathingSimulation<Entity>;

    fn simulation(context: &Context, settings: &Self::Settings) -> Self::Simulation {
        let simulator = crate::dynamic::Simulator::try_new(context, settings)
            .expect("failed to create simulator");

        crate::dynamic::Simulation::new::<Entity>(simulator)
    }

    fn debug_parameters() -> Self::DebugParameters {}

    fn source_debug_parameters<'a>(
        source: &Self::Source,
        parameters: Option<&'a Self::Parameters>,
        _: &'a Self::DebugParameters,
    ) -> Cow<'a, Self::Parameters> {
        parameters.map_or_else(
            || Cow::Owned(Self::implicit_parameters(source)),
            Cow::Borrowed,
        )
    }

    fn implicit_parameters(source: &Self::Source) -> Self::Parameters {
        use crate::dynamic::{ReflectionAlgorithm, ReflectionsParameters};
        let direct = source
            .flags()
            .contains(crate::SimulationFlags::DIRECT)
            .then(|| {
                crate::simulation::DirectSimulationParameters::new()
                    .with_distance_attenuation(crate::model::DistanceAttenuationModel::default())
                    .with_air_absorption(crate::model::AirAbsorptionModel::default())
                    .with_occlusion(crate::simulation::Occlusion::new(
                        crate::simulation::OcclusionAlgorithm::Raycast,
                    ))
            });
        let reflections = source
            .reflection_algorithm()
            .map(|algorithm| match algorithm {
                ReflectionAlgorithm::Convolution => {
                    ReflectionsParameters::Convolution(Default::default())
                }
                ReflectionAlgorithm::Parametric => {
                    ReflectionsParameters::Parametric(Default::default())
                }
                ReflectionAlgorithm::Hybrid => ReflectionsParameters::Hybrid(Default::default()),
                ReflectionAlgorithm::TrueAudioNext => {
                    ReflectionsParameters::TrueAudioNext(Default::default())
                }
            });

        Self::Parameters {
            direct,
            reflections,
            pathing: None,
        }
    }

    fn listener_parameters(source: &Self::Source) -> Self::Parameters {
        Self::implicit_parameters(source)
    }
    fn source_with_inputs(
        source: &Self::Source,
        coordinates: CoordinateSystem,
        parameters: Self::Parameters,
    ) -> Self::SourceWithInputs {
        crate::dynamic::SourceWithInputs {
            source: source.clone(),
            simulation_inputs: crate::dynamic::SimulationInputs {
                source: coordinates,
                parameters,
            },
        }
    }

    fn update_sources(
        simulation: &mut SystemParamItem<'_, '_, Self::SimulationAccess>,
        fill: impl FnOnce(&mut Vec<(Entity, Self::SourceWithInputs)>),
    ) -> Result<(), Self::IntegrationError> {
        let simulation = bevy::prelude::DetectChangesMut::bypass_change_detection(simulation);

        simulation.0.update_sources(fill)
    }
    fn add_source(
        simulation: &mut Self::Simulation,
        source: &Self::Source,
    ) -> Result<(), Self::IntegrationError> {
        simulation.simulator.try_add_source(source)
    }
    fn remove_source(
        simulation: &mut Self::Simulation,
        source: &Self::Source,
    ) -> Result<(), Self::IntegrationError> {
        simulation.simulator.try_remove_source(source)
    }
    fn set_scene(
        simulation: &mut Self::Simulation,
        scene: &Scene<Self::Geometry>,
    ) -> Result<(), Self::IntegrationError> {
        simulation.simulator.try_set_scene(scene)
    }
    fn static_mesh(
        scene: &Scene<Self::Geometry>,
        settings: &StaticMeshSettings,
    ) -> Result<crate::dynamic::StaticMesh, crate::geometry::StaticMeshError> {
        crate::dynamic::StaticMesh::try_new(scene, settings)
    }
    fn add_static_mesh(
        scene: &mut Scene<Self::Geometry>,
        mesh: crate::dynamic::StaticMesh,
    ) -> crate::geometry::StaticMeshHandle {
        scene
            .try_add_static_mesh(mesh)
            .expect("mesh was created for this scene")
    }
    fn instanced_mesh(
        scene: &Scene<Self::Geometry>,
        settings: &crate::dynamic::InstancedMeshSettings,
    ) -> Result<crate::dynamic::InstancedMesh, Self::InstancedMeshError> {
        crate::dynamic::InstancedMesh::try_new(scene, settings)
    }
    fn add_instanced_mesh(
        scene: &mut Scene<Self::Geometry>,
        mesh: crate::dynamic::InstancedMesh,
    ) -> crate::geometry::InstancedMeshHandle {
        scene
            .try_add_instanced_mesh(mesh)
            .expect("mesh was created for this scene")
    }
}

/// The default static simulation configuration.
#[cfg(any(feature = "static", not(feature = "dynamic")))]
pub type DefaultConfiguration = super::configuration::DefaultSimulationConfiguration;

/// The default runtime simulation configuration.
#[cfg(all(feature = "dynamic", not(feature = "static")))]
pub type DefaultConfiguration = RuntimeConfiguration;
