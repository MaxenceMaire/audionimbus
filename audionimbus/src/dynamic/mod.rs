//! Runtime-selected configuration.

mod geometry;
pub use geometry::{
    InstancedMesh, InstancedMeshSettings, RayTracerKind, RayTracerSettings, Scene, SceneError,
    StaticMesh,
};

mod reflections;
pub use reflections::{
    ReflectionAlgorithm, ReflectionEffect, ReflectionEffectParams, ReflectionError,
    ReflectionMixer, ReflectionsParameters, ReflectionsSettings,
};
