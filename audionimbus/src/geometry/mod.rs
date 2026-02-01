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
//! use audionimbus::*;
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
//! let triangles = vec![
//!     Triangle::new(0, 1, 2),
//!     Triangle::new(0, 2, 3),
//! ];
//! let materials = vec![Material::CONCRETE];
//! let material_indices = vec![0, 0];
//!
//! // Create and add the mesh.
//! let mesh = StaticMesh::try_new(&scene, &StaticMeshSettings {
//!     vertices: &vertices,
//!     triangles: &triangles,
//!     material_indices: &material_indices,
//!     materials: &materials,
//! })?;
//! scene.add_static_mesh(mesh);
//! scene.commit();
//! # Ok::<(), SteamAudioError>(())
//! ```
//!
//! # Dynamic Geometry
//!
//! Use [`InstancedMesh`] for moving objects:
//!
//! ```
//! # use audionimbus::*;
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
//! let instanced = InstancedMesh::try_new(&scene, InstancedMeshSettings {
//!     sub_scene: &sub_scene,
//!     transform,
//! })?;
//! scene.add_instanced_mesh(instanced);
//! # Ok::<(), SteamAudioError>(())
//! ```

mod vector3;
pub use vector3::Vector3;

mod point;
pub use point::Point;

mod direction;
pub use direction::Direction;

mod coordinate_system;
pub use coordinate_system::CoordinateSystem;

mod matrix;
pub use matrix::{Matrix, Matrix3, Matrix4};

mod triangle;
pub use triangle::Triangle;

mod material;
pub use material::Material;

mod scene;
pub use scene::{
    relative_direction, InstancedMeshHandle, SaveableAsObj, SaveableAsSerialized, Scene,
    StaticMeshHandle,
};

mod static_mesh;
pub use static_mesh::{StaticMesh, StaticMeshSettings};

mod instanced_mesh;
pub use instanced_mesh::{InstancedMesh, InstancedMeshSettings};

mod sphere;
pub use sphere::Sphere;
