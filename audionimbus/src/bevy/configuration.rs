//! Simulation type configuration.

use crate::effect::reflections::{Convolution, ReflectionEffectType};
use crate::ray_tracing::{DefaultRayTracer, RayTracer};
use crate::simulation::{
    Direct, DirectCompatible, PathingCompatible, ReflectionEffectCompatible, Reflections,
    ReflectionsCompatible, SimulationFlagsProvider,
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
}

/// Default simulation configuration: [`DefaultRayTracer`], direct, reflections via convolution, no
/// pathing.
pub struct DefaultSimulationConfiguration;

impl SimulationConfiguration for DefaultSimulationConfiguration {
    type RayTracer = DefaultRayTracer;
    type Direct = Direct;
    type Reflections = Reflections;
    type Pathing = ();
    type ReflectionEffect = Convolution;
}
