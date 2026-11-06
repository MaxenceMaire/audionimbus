//! Reflection baking.

/// Baker implementations whose binding selectors must stay inaccessible to downstream callers.
pub(crate) mod baker;
#[cfg(all(feature = "dynamic", not(feature = "static")))]
pub use baker::DynamicReflectionsBaker as ReflectionsBaker;
#[cfg(any(feature = "static", not(feature = "dynamic")))]
pub use baker::StaticReflectionsBaker as ReflectionsBaker;
pub use baker::{ReflectionsBakeFlags, ReflectionsBakeParams};
