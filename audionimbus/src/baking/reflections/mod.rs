//! Reflection baking.

/// Baker implementations whose binding selectors must stay inaccessible to downstream callers.
pub(crate) mod baker;
pub use baker::{
    ReflectionsBakeFlags, ReflectionsBakeParams, TypedReflectionsBaker as ReflectionsBaker,
};

#[cfg(test)]
pub use baker::tests;
