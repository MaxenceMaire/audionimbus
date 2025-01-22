/// A point or vector in 3D space.
///
/// Steam Audio uses a right-handed coordinate system, with the positive x-axis pointing right, the positive y-axis pointing up, and the negative z-axis pointing ahead.
/// Position and direction data obtained from a game engine or audio engine must be properly transformed before being passed to any Steam Audio API function.
#[derive(Copy, Clone, Debug)]
pub struct Vector3 {
    /// The x-coordinate.
    pub x: f32,

    /// The y-coordinate.
    pub y: f32,

    /// The z-coordinate.
    pub z: f32,
}

impl Vector3 {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
}

impl From<[f32; 3]> for Vector3 {
    fn from(vector: [f32; 3]) -> Self {
        Self {
            x: vector[0],
            y: vector[1],
            z: vector[2],
        }
    }
}

impl From<Vector3> for audionimbus_sys::IPLVector3 {
    fn from(vector: Vector3) -> Self {
        Self {
            x: vector.x,
            y: vector.y,
            z: vector.z,
        }
    }
}

/// A 3D coordinate system, expressed relative to a canonical coordinate system.
#[derive(Copy, Clone, Debug)]
pub struct CoordinateSystem3 {
    /// Unit vector pointing to the right (local +x axis).
    pub right: Vector3,

    /// Unit vector pointing upwards (local +y axis).
    pub up: Vector3,

    /// Unit vector pointing forwards (local -z axis).
    pub ahead: Vector3,

    /// The origin, relative to the canonical coordinate system.
    pub origin: Vector3,
}

impl From<CoordinateSystem3> for audionimbus_sys::IPLCoordinateSpace3 {
    fn from(coordinate_system: CoordinateSystem3) -> Self {
        Self {
            right: coordinate_system.right.into(),
            up: coordinate_system.up.into(),
            ahead: coordinate_system.ahead.into(),
            origin: coordinate_system.origin.into(),
        }
    }
}
