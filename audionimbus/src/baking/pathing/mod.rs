//! Path baking.

/// Baker implementations whose binding selectors must stay inaccessible to downstream callers.
pub(crate) mod baker;
#[cfg(all(feature = "dynamic", not(feature = "static")))]
pub use baker::DynamicPathBaker as PathBaker;
pub use baker::PathBakeParams;
#[cfg(any(feature = "static", not(feature = "dynamic")))]
pub use baker::StaticPathBaker as PathBaker;
