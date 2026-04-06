//! Acoustic geometry.
//!
//! This module provides components and systems for registering 3D acoustic geometry.
//! Geometry affects how sound propagates through a scene; surfaces reflect, absorb, and oclude
//! audio depending on their acoustic [`Material`] properties.
//!
//! # Scene hierarchy
//!
//! Every piece of geometry must live under a [`Scene`] ancestor.
//!
//! # Static vs. instanced geometry
//!
//! [`StaticMesh`] represents immovable geometry.
//! In contrast, [`InstancedMesh`] can undergo rigid-body motion by applying a transform to the
//! scene it references.
//!
//! Multiple instanced meshes may use the same underlying scene, i.e. reference the same geometry.

mod instanced_mesh;
mod scene;
mod static_mesh;

pub use instanced_mesh::*;
pub use scene::*;
pub use static_mesh::*;
