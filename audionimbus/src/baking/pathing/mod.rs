//! Path baking.

/// Baker implementations whose binding selectors must stay inaccessible to downstream callers.
pub(crate) mod baker;
#[cfg(dynamic_api)]
pub use baker::DynamicPathBaker as PathBaker;
pub use baker::PathBakeParams;
#[cfg(static_api)]
pub use baker::StaticPathBaker as PathBaker;
