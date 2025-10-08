#[cfg(feature = "firewheel")]
use firewheel::diff::{Diff, Patch, RealtimeClone};

/// An N-band equalizer, with band coefficients between 0.0 and 1.0.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "firewheel", derive(Diff, Patch, RealtimeClone))]
pub struct Equalizer<const N: usize>(pub [f32; N]);

impl<const N: usize> Default for Equalizer<N> {
    fn default() -> Self {
        Self([0.0; N])
    }
}

impl<const N: usize> std::ops::Deref for Equalizer<N> {
    type Target = [f32; N];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<const N: usize> std::ops::DerefMut for Equalizer<N> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
