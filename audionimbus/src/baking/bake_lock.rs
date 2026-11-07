//! Serialization of native bake operations.

use std::sync::Mutex;

/// The shared lock for native reflection and path bakes.
pub static BAKE_LOCK: Mutex<()> = Mutex::new(());
