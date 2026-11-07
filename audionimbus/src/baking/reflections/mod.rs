//! Reflection baking.

/// Baker implementations whose binding selectors must stay inaccessible to downstream callers.
pub(crate) mod baker;
#[cfg(dynamic_api)]
pub use baker::DynamicReflectionsBaker as ReflectionsBaker;
#[cfg(static_api)]
pub use baker::StaticReflectionsBaker as ReflectionsBaker;
pub use baker::{ReflectionsBakeFlags, ReflectionsBakeParams};
