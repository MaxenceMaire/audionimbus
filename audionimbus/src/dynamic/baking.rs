//! Bakers for runtime scenes.

use super::RayTracerKind;
use crate::binding::Dynamic;
use std::fmt;

/// A reflection baker borrowing runtime backend settings.
///
/// ```
/// use audionimbus::dynamic::{RayTracerSettings, ReflectionsBaker};
/// let ray_tracer_settings = RayTracerSettings::Default;
/// let reflections_baker = ReflectionsBaker::new(&ray_tracer_settings);
/// ```
pub type ReflectionsBaker<'a> =
    crate::baking::reflections::baker::ReflectionsBaker<'a, Dynamic<RayTracerKind>>;

/// A path baker for runtime scenes.
pub type PathBaker = crate::baking::pathing::baker::PathBaker<Dynamic<RayTracerKind>>;

/// Errors from runtime reflection baking.
#[derive(Debug, Eq, PartialEq)]
pub enum BakeError {
    /// Another native bake is already in progress.
    Bake(crate::BakeError),
    /// The scene and reflection baker use different ray tracers.
    RayTracerMismatch {
        /// The baker's ray tracer.
        expected: RayTracerKind,
        /// The scene's ray tracer.
        actual: RayTracerKind,
    },
    /// The scene and reflection baker use different devices.
    DeviceMismatch,
}

impl fmt::Display for BakeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bake(error) => error.fmt(formatter),
            Self::RayTracerMismatch { expected, actual } => write!(
                formatter,
                "bake ray tracer mismatch: expected {expected:?}, got {actual:?}"
            ),
            Self::DeviceMismatch => write!(formatter, "bake devices do not match"),
        }
    }
}

impl std::error::Error for BakeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Bake(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(doctest)]
/// Baker binding checks.
///
/// ```compile_fail
/// use audionimbus::{Context, ProbeBatch, ReflectionsBaker, ReflectionsBakeParams, ray_tracing::DefaultRayTracer};
/// fn bake(context: &Context, probes: &mut ProbeBatch, scene: &audionimbus::dynamic::Scene, params: ReflectionsBakeParams) {
///     ReflectionsBaker::<DefaultRayTracer>::new().bake(context, probes, scene, params);
/// }
/// ```
///
/// ```compile_fail
/// use audionimbus::{Context, ProbeBatch, ReflectionsBakeParams, Scene};
/// fn bake(baker: &audionimbus::dynamic::ReflectionsBaker<'_>, context: &Context, probes: &mut ProbeBatch, scene: &Scene, params: ReflectionsBakeParams) {
///     baker.bake(context, probes, scene, params);
/// }
/// ```
///
/// ```compile_fail
/// use audionimbus::{Context, ProbeBatch, PathBaker, PathBakeParams};
/// fn bake(context: &Context, probes: &mut ProbeBatch, scene: &audionimbus::dynamic::Scene, params: PathBakeParams) {
///     PathBaker::new().bake(context, probes, scene, params);
/// }
/// ```
///
/// ```compile_fail
/// use audionimbus::{Context, ProbeBatch, PathBakeParams, Scene};
/// fn bake(context: &Context, probes: &mut ProbeBatch, scene: &Scene, params: PathBakeParams) {
///     audionimbus::dynamic::PathBaker::new().bake(context, probes, scene, params);
/// }
/// ```
///
/// ```compile_fail
/// use audionimbus::{ReflectionsBaker, dynamic::RayTracerKind};
/// let _: Option<ReflectionsBaker<'static, RayTracerKind>> = None;
/// ```
///
/// ```compile_fail
/// use audionimbus::{PathBaker, dynamic::RayTracerKind};
/// let _: Option<PathBaker<RayTracerKind>> = None;
/// ```
struct BakingBindings;
