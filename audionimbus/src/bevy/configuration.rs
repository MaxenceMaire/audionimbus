//! Simulation type configuration.

use crate::effect::reflections::{Convolution, ReflectionEffectType};
use crate::model::{AirAbsorptionModel, DistanceAttenuationModel};
use crate::ray_tracing::{DefaultRayTracer, RayTracer};
use crate::simulation::{
    ConvolutionParameters, Direct, DirectCompatible, DirectSimulationParameters, Occlusion,
    OcclusionAlgorithm, PathingCompatible, ReflectionEffectCompatible, Reflections,
    ReflectionsCompatible, SimulationFlagsProvider, SimulationParameters,
};

/// Bundles the type parameters that define a simulation pipeline.
pub trait SimulationConfiguration: 'static + Send + Sync {
    type RayTracer: 'static + RayTracer + Send + Sync;
    type Direct: 'static
        + DirectCompatible<Self::Direct>
        + SimulationFlagsProvider
        + Send
        + Sync
        + Clone
        + Default;
    type Reflections: 'static
        + ReflectionsCompatible<Self::Reflections>
        + SimulationFlagsProvider
        + Send
        + Sync
        + Clone
        + Default;
    type Pathing: 'static
        + PathingCompatible<Self::Pathing>
        + SimulationFlagsProvider
        + Send
        + Sync
        + Clone
        + Default;
    type ReflectionEffect: 'static
        + ReflectionEffectCompatible<Self::Reflections, Self::ReflectionEffect>
        + ReflectionEffectType
        + Send
        + Sync
        + Clone
        + Default;

    /// Returns the implicit source parameters associated with this simulation configuration.
    fn implicit_source_parameters()
    -> SimulationParameters<Self::Direct, Self::Reflections, Self::Pathing> {
        SimulationParameters::default()
    }
}

/// Default simulation configuration: [`DefaultRayTracer`], direct, reflections via convolution, no
/// pathing.
#[derive(Copy, Clone)]
pub struct DefaultSimulationConfiguration;

impl SimulationConfiguration for DefaultSimulationConfiguration {
    type RayTracer = DefaultRayTracer;
    type Direct = Direct;
    type Reflections = Reflections;
    type Pathing = ();
    type ReflectionEffect = Convolution;

    fn implicit_source_parameters() -> SimulationParameters<Direct, Reflections, ()> {
        SimulationParameters::new()
            .with_direct(
                DirectSimulationParameters::new()
                    .with_distance_attenuation(DistanceAttenuationModel::default())
                    .with_air_absorption(AirAbsorptionModel::default())
                    .with_occlusion(Occlusion::new(OcclusionAlgorithm::Raycast)),
            )
            .with_reflections(ConvolutionParameters {
                baked_data_identifier: None,
            })
    }
}
