//! Path baking.

/// Baker implementations whose binding selectors must stay inaccessible to downstream callers.
pub(crate) mod baker;
pub use baker::{PathBakeParams, StaticPathBaker as PathBaker};

#[cfg(test)]
pub use baker::tests;
