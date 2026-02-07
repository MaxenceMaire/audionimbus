use super::Vector3;

/// A ray in 3D space.
#[derive(Default, Debug, Copy, Clone, PartialEq)]
pub struct Ray {
    /// Origin of the ray.
    pub origin: Vector3,

    /// Unit vector direction of the ray.
    pub direction: Vector3,
}

impl From<Ray> for audionimbus_sys::IPLRay {
    fn from(ray: Ray) -> Self {
        Self {
            origin: ray.origin.into(),
            direction: ray.direction.into(),
        }
    }
}

impl From<audionimbus_sys::IPLRay> for Ray {
    fn from(ray: audionimbus_sys::IPLRay) -> Self {
        Self {
            origin: ray.origin.into(),
            direction: ray.direction.into(),
        }
    }
}
