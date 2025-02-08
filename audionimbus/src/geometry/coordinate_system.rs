use super::{Point, Vector3};

/// A 3D coordinate system, expressed relative to a canonical coordinate system.
#[derive(Copy, Clone, Debug)]
pub struct CoordinateSystem {
    /// Unit vector pointing to the right (local +x axis).
    pub right: Vector3,

    /// Unit vector pointing upwards (local +y axis).
    pub up: Vector3,

    /// Unit vector pointing forwards (local -z axis).
    pub ahead: Vector3,

    /// The origin, relative to the canonical coordinate system.
    pub origin: Point,
}

impl Default for CoordinateSystem {
    fn default() -> Self {
        Self {
            right: Vector3::new(1.0, 0.0, 0.0),
            up: Vector3::new(0.0, 1.0, 0.0),
            ahead: Vector3::new(0.0, 0.0, 1.0),
            origin: Vector3::new(0.0, 0.0, 0.0),
        }
    }
}

impl From<CoordinateSystem> for audionimbus_sys::IPLCoordinateSpace3 {
    fn from(coordinate_system: CoordinateSystem) -> Self {
        Self {
            right: coordinate_system.right.into(),
            up: coordinate_system.up.into(),
            ahead: coordinate_system.ahead.into(),
            origin: coordinate_system.origin.into(),
        }
    }
}

impl From<audionimbus_sys::IPLCoordinateSpace3> for CoordinateSystem {
    fn from(coordinate_system: audionimbus_sys::IPLCoordinateSpace3) -> Self {
        Self {
            right: coordinate_system.right.into(),
            up: coordinate_system.up.into(),
            ahead: coordinate_system.ahead.into(),
            origin: coordinate_system.origin.into(),
        }
    }
}
