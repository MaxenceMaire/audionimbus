use crate::effect::reflections::{Convolution, ReflectionEffectType};
use crate::ray_tracing::{DefaultRayTracer, RayTracer};
use crate::simulation::{
    Direct, DirectCompatible, PathingCompatible, ReflectionEffectCompatible, Reflections,
    ReflectionsCompatible, SimulationFlagsProvider,
};

/// Bundles simulation type parameters.
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

pub struct DefaultSimulationConfiguration;

impl SimulationConfiguration for DefaultSimulationConfiguration {
    type RayTracer = DefaultRayTracer;
    type Direct = Direct;
    type Reflections = Reflections;
    type Pathing = ();
    type ReflectionEffect = Convolution;
}
