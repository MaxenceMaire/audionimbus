use super::{Material, Vector3};

#[cfg(doc)]
use crate::ray_tracing::CustomRayTracer;

/// Information about a ray’s intersection with 3D geometry.
///
/// This information should be provided by ray tracer callbacks when using [`CustomRayTracer`].
/// Not all fields are required.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Hit {
    /// Distance along the ray from origin to hit point.
    /// Set to [`f32::INFINITY`] if nothing was hit.
    pub distance: f32,

    /// Index of the primitive hit by the ray.
    pub triangle_index: Option<u32>,

    /// Index of the scene object hit by the ray.
    pub object_index: Option<u32>,

    /// Index of the material associated with the primitive hit by the ray.
    pub material_index: Option<u32>,

    /// Unit length surface normal at the hit point. Ignored if nothing was hit.
    pub normal: Vector3,

    /// Material at the hit point.
    pub material: Option<Material>,
}

impl From<audionimbus_sys::IPLHit> for Hit {
    fn from(hit: audionimbus_sys::IPLHit) -> Self {
        Self {
            distance: hit.distance,
            triangle_index: if hit.triangleIndex == -1 {
                None
            } else {
                Some(hit.triangleIndex as u32)
            },
            object_index: if hit.objectIndex == -1 {
                None
            } else {
                Some(hit.objectIndex as u32)
            },
            material_index: if hit.materialIndex == -1 {
                None
            } else {
                Some(hit.materialIndex as u32)
            },
            normal: hit.normal.into(),
            material: unsafe { hit.material.as_ref().map(|material| material.into()) },
        }
    }
}
