use super::Point;

/// A sphere.
/// Spheres are used to define a region of influence around a point.
#[derive(Debug)]
pub struct Sphere {
    /// The center.
    pub center: Point,

    /// The radius.
    pub radius: f32,
}

impl From<&Sphere> for audionimbus_sys::IPLSphere {
    fn from(sphere: &Sphere) -> Self {
        todo!()
    }
}
