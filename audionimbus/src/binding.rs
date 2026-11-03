//! Compile-time and runtime API bindings.

use std::marker::PhantomData;

/// A statically selected configuration.
#[derive(Debug, PartialEq, Eq)]
pub struct Static<C>(PhantomData<C>);

impl<C> Static<C> {
    /// Creates a static binding.
    pub const fn new() -> Self {
        Self(PhantomData)
    }
}

impl<C> Copy for Static<C> {}

impl<C> Clone for Static<C> {
    fn clone(&self) -> Self {
        *self
    }
}

/// A configuration selected at runtime.
#[cfg(feature = "dynamic")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Dynamic<C>(pub C);

impl<C: Default> Default for Static<C> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "dynamic")]
impl<C: Default> Default for Dynamic<C> {
    fn default() -> Self {
        Self(C::default())
    }
}
