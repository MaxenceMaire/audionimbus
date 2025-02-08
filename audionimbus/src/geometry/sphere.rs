use super::Point;

/// A sphere.
/// Spheres are used to define a region of influence around a point.
#[derive(Copy, Clone, Debug)]
pub struct Sphere {
    /// The center.
    pub center: Point,

    /// The radius.
    pub radius: f32,
}

impl Default for Sphere {
    fn default() -> Self {
        Self {
            center: Point::default(),
            radius: f32::default(),
        }
    }
}

impl From<Sphere> for audionimbus_sys::IPLSphere {
    fn from(sphere: Sphere) -> Self {
        Self {
            center: sphere.center.into(),
            radius: sphere.radius.into(),
        }
    }
}
