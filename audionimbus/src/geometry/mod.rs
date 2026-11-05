//! 3D geometry and scene management for acoustic simulation.
//!
//! # Overview
//!
//! This module provides types for building 3D scenes that Steam Audio uses for
//! acoustic simulations like occlusion, reflection, and reverb.
//!
//! # Building a Scene
//!
//! ```
//! # #[cfg(feature = "static")]
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use audionimbus::*;
//! use audionimbus::r#static::{Scene, StaticMesh};
//!
//! let context = Context::default();
//! let mut scene = Scene::try_new(&context)?;
//!
//! // Define geometry.
//! let vertices = vec![
//!     Point::new(0.0, 0.0, 0.0),
//!     Point::new(10.0, 0.0, 0.0),
//!     Point::new(10.0, 10.0, 0.0),
//!     Point::new(0.0, 10.0, 0.0),
//! ];
//! let triangles = vec![Triangle::new(0, 1, 2), Triangle::new(0, 2, 3)];
//! let materials = vec![Material::CONCRETE];
//! let material_indices = vec![0, 0];
//!
//! // Create and add the mesh.
//! let mesh = StaticMesh::try_new(
//!     &scene,
//!     &StaticMeshSettings {
//!         vertices: &vertices,
//!         triangles: &triangles,
//!         material_indices: &material_indices,
//!         materials: &materials,
//!     },
//! )?;
//! scene.add_static_mesh(mesh);
//! scene.commit();
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! # }
//! # #[cfg(not(feature = "static"))]
//! # fn main() {}
//! ```
//!
//! # Dynamic Geometry
//!
//! Use [`InstancedMesh`] for moving objects:
//!
//! ```
//! # #[cfg(feature = "static")]
//! # fn main() -> Result<(), audionimbus::SteamAudioError> {
//! # use audionimbus::*;
//! # use audionimbus::r#static::{InstancedMesh, InstancedMeshSettings, Scene};
//! # let context = Context::default();
//! # let mut scene = Scene::try_new(&context)?;
//! # let sub_scene = Scene::try_new(&context)?;
//! let transform = Matrix::new([
//!     [1.0, 0.0, 0.0, 5.0],
//!     [0.0, 1.0, 0.0, 0.0],
//!     [0.0, 0.0, 1.0, 0.0],
//!     [0.0, 0.0, 0.0, 1.0],
//! ]);
//!
//! let instanced = InstancedMesh::try_new(
//!     &scene,
//!     &InstancedMeshSettings {
//!         sub_scene,
//!         transform,
//!     },
//! )?;
//! scene.add_instanced_mesh(instanced);
//! # Ok::<(), SteamAudioError>(())
//! # }
//! # #[cfg(not(feature = "static"))]
//! # fn main() {}
//! ```

mod vector3;
pub use vector3::Vector3;

mod point;
pub use point::Point;

mod direction;
pub use direction::Direction;

mod coordinate_system;
pub use coordinate_system::CoordinateSystem;

mod hit;
pub use hit::Hit;

mod matrix;
pub use matrix::{Matrix, Matrix3, Matrix4};

mod triangle;
pub use triangle::Triangle;

mod material;
pub use material::Material;

mod ray;
pub use ray::Ray;

/// Geometry implementations whose bindings must stay inaccessible to downstream callers.
pub(crate) mod scene;
#[cfg(all(feature = "dynamic", not(feature = "static")))]
pub use scene::DynamicScene as Scene;
#[cfg(any(feature = "static", not(feature = "dynamic")))]
pub use scene::StaticScene as Scene;
pub use scene::{
    InstancedMeshHandle, SaveableAsObj, SaveableAsSerialized, StaticMeshHandle, relative_direction,
};

/// Mesh implementations whose bindings must stay inaccessible to downstream callers.
pub(crate) mod static_mesh;
#[cfg(all(feature = "dynamic", not(feature = "static")))]
pub use static_mesh::DynamicStaticMesh as StaticMesh;
#[cfg(any(feature = "static", not(feature = "dynamic")))]
pub use static_mesh::StaticStaticMesh as StaticMesh;
pub use static_mesh::{StaticMeshError, StaticMeshSettings};

/// Instance implementations whose bindings must stay inaccessible to downstream callers.
pub(crate) mod instanced_mesh;
#[cfg(all(feature = "dynamic", not(feature = "static")))]
pub use instanced_mesh::{
    DynamicInstancedMesh as InstancedMesh, DynamicInstancedMeshSettings as InstancedMeshSettings,
};
#[cfg(any(feature = "static", not(feature = "dynamic")))]
pub use instanced_mesh::{
    StaticInstancedMesh as InstancedMesh, StaticInstancedMeshSettings as InstancedMeshSettings,
};

mod sphere;
pub use sphere::Sphere;

#[cfg(feature = "dynamic")]
pub use scene::{RayTracerKind, RayTracerSettings, SceneError};
