#[cfg(feature = "firewheel")]
use firewheel::diff::{Diff, Patch, RealtimeClone};

/// A point or vector in 3D space.
///
/// Steam Audio uses a right-handed coordinate system, with the positive x-axis pointing right, the positive y-axis pointing up, and the negative z-axis pointing ahead.
/// Position and direction data obtained from a game engine or audio engine must be properly transformed before being passed to any Steam Audio API function.
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "firewheel", derive(Diff, Patch, RealtimeClone))]
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

impl Default for Vector3 {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
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

impl From<audionimbus_sys::IPLVector3> for Vector3 {
    fn from(vector: audionimbus_sys::IPLVector3) -> Self {
        Self {
            x: vector.x,
            y: vector.y,
            z: vector.z,
        }
    }
}
