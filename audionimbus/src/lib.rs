//! [audionimbus-sys]: https://github.com/MaxenceMaire/audionimbus/tree/master/audionimbus-sys
//! [core-demo]: https://github.com/MaxenceMaire/audionimbus/tree/master/audionimbus/examples/core
//! [bevy-demo]: https://github.com/MaxenceMaire/audionimbus/tree/master/audionimbus/examples/bevy
//! [static-demo]: https://github.com/MaxenceMaire/audionimbus/blob/master/audionimbus/examples/core/static.rs
//! [dynamic-demo]: https://github.com/MaxenceMaire/audionimbus/blob/master/audionimbus/examples/core/dynamic.rs
//! [tests]: https://github.com/MaxenceMaire/audionimbus/tree/master/audionimbus/tests
//! [mit-license]: https://github.com/MaxenceMaire/audionimbus/blob/master/LICENSE-MIT
//! [apache-license]: https://github.com/MaxenceMaire/audionimbus/blob/master/LICENSE-APACHE
//!
#![cfg_attr(feature = "bevy", doc = "[bevy-module]: crate::bevy")]
#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, feature(doc_cfg))]

const NUM_BANDS: u32 = 3;

pub mod prelude;
pub use prelude::*;

#[cfg(feature = "static")]
#[cfg_attr(docsrs, doc(cfg(feature = "static")))]
pub mod r#static;

pub mod audio_buffer;
pub mod audio_settings;
pub mod baking;
mod binding;
pub mod callback;
pub mod context;
pub mod device;
#[cfg(feature = "dynamic")]
#[cfg_attr(docsrs, doc(cfg(feature = "dynamic")))]
pub mod dynamic;
pub mod effect;
mod error;
pub use error::SteamAudioError;
mod ffi_wrapper;
pub mod geometry;
pub mod hrtf;
pub mod model;
pub mod probe;
mod serialized_object;
pub use serialized_object::SerializedObject;
pub mod energy_field;
pub mod impulse_response;
pub mod ray_tracing;
pub mod reconstructor;
mod sealed;
pub mod simulation;
pub mod version;
use sealed::Sealed;
#[cfg(feature = "wiring")]
#[cfg_attr(docsrs, doc(cfg(feature = "wiring")))]
pub mod wiring;

#[cfg(feature = "fmod")]
#[path = "fmod.rs"]
mod fmod_integration;
#[cfg(all(feature = "fmod", dynamic_api))]
#[cfg_attr(docsrs, doc(cfg(feature = "fmod")))]
pub use fmod_integration::dynamic_api as fmod;
#[cfg(all(feature = "fmod", static_api))]
#[cfg_attr(docsrs, doc(cfg(feature = "fmod")))]
pub use fmod_integration::static_api as fmod;

#[cfg(feature = "wwise")]
#[path = "wwise.rs"]
mod wwise_integration;
#[cfg(all(feature = "wwise", dynamic_api))]
#[cfg_attr(docsrs, doc(cfg(feature = "wwise")))]
pub use wwise_integration::dynamic_api as wwise;
#[cfg(all(feature = "wwise", static_api))]
#[cfg_attr(docsrs, doc(cfg(feature = "wwise")))]
pub use wwise_integration::static_api as wwise;

#[cfg(feature = "bevy")]
#[cfg_attr(docsrs, doc(cfg(feature = "bevy")))]
pub mod bevy;

#[cfg(all(doctest, not(feature = "static")))]
/// The disabled static module.
///
/// ```compile_fail
/// use audionimbus::r#static;
/// ```
struct StaticFeature;
