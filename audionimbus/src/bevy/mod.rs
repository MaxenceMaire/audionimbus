use crate::geometry::CoordinateSystem;
use bevy::prelude::Transform;

pub mod configuration;
pub mod plugin;
pub mod runner;
pub mod simulation;
pub mod source;
pub mod system_set;

pub use configuration::*;
pub use plugin::*;
pub use runner::*;
pub use simulation::*;
pub use source::*;
pub use system_set::*;

fn coordinate_system_from_transform(transform: Transform) -> CoordinateSystem {
    CoordinateSystem {
        right: transform.right().to_array().into(),
        up: transform.up().to_array().into(),
        ahead: transform.forward().to_array().into(),
        origin: transform.translation.to_array().into(),
    }
}
