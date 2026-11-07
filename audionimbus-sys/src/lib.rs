//! [audionimbus]: https://github.com/MaxenceMaire/audionimbus/tree/master/audionimbus
//! [steam-audio]: https://github.com/ValveSoftware/steam-audio
//! [mit-license]: https://github.com/MaxenceMaire/audionimbus/blob/master/LICENSE-MIT
//! [apache-license]: https://github.com/MaxenceMaire/audionimbus/blob/master/LICENSE-APACHE
//!
#![doc = include_str!("../README.md")]

mod phonon;
pub use phonon::*;

impl From<bool> for IPLbool {
    fn from(value: bool) -> Self {
        match value {
            true => Self::IPL_TRUE,
            false => Self::IPL_FALSE,
        }
    }
}

impl From<IPLbool> for bool {
    fn from(value: IPLbool) -> Self {
        match value {
            IPLbool::IPL_TRUE => true,
            IPLbool::IPL_FALSE => false,
        }
    }
}

#[cfg(feature = "fmod")]
pub mod fmod;
#[cfg(feature = "fmod")]
pub use fmod::*;

#[cfg(feature = "wwise")]
pub mod wwise;
#[cfg(feature = "wwise")]
pub use wwise::*;
