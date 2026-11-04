//! Serialization of native bake operations.

use std::sync::Mutex;

/// The shared lock for native reflection and path bakes.
pub static BAKE_LOCK: Mutex<()> = Mutex::new(());

#[cfg(test)]
mod tests {
    use crate::baking::{pathing, reflections};

    #[test]
    fn test_bakers() {
        // Run test cases sequentially to avoid BakeError::BakeInProgress.
        pathing::tests::test_bake();
        reflections::tests::test_bake();
    }
}
