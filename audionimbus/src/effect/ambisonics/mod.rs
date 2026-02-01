//! Ambisonics encoding, decoding, and processing effects.

pub mod encode;
pub use encode::*;

pub mod decode;
pub use decode::*;

pub mod panning;
pub use panning::*;

pub mod binaural;
pub use binaural::*;

pub mod rotation;
pub use rotation::*;

mod speaker_layout;
pub use speaker_layout::SpeakerLayout;

mod r#type;
pub use r#type::AmbisonicsType;
