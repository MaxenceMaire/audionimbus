//! Audio effects for spatial audio processing.
//!
//! This module provides various audio effects:
//!
//! # Effect Categories
//!
//! ## Point Source Spatialization
//! - [`BinauralEffect`] - Spatialize a point source using an HRTF
//! - [`PanningEffect`] - Pan a point source to a multi-channel speaker layout based on its 3D positon
//! - [`VirtualSurroundEffect`] - Render surround mixes binaurally over headphones using HRTF
//!
//! ## Environmental Effects
//! - [`DirectEffect`] - Distance attenuation, air absorption, occlusion, transmission
//! - [`ReflectionEffect`] - Room acoustics and reverb
//! - [`PathEffect`] - Sound propagation paths around obstacles
//!
//! ## Ambisonics Processing
//! - [`AmbisonicsEncodeEffect`] - Encode point sources to ambisonics
//! - [`AmbisonicsDecodeEffect`] - Decode ambisonics to speakers/headphones
//! - [`AmbisonicsPanningEffect`] - Decode ambisonics by panning to speakers
//! - [`AmbisonicsBinauralEffect`] - Decode Ambisonics using HRTF rendering
//! - [`AmbisonicsRotationEffect`] - Rotate Ambisonics to listener's orientation
//!
//! # Typical Usage
//!
//! ```
//! use audionimbus::*;
//!
//! let context = Context::default();
//! let audio_settings = AudioSettings::default();
//! let hrtf = Hrtf::try_new(&context, &audio_settings, &HrtfSettings::default())?;
//!
//! let mut effect = BinauralEffect::try_new(
//!     &context,
//!     &audio_settings,
//!     &BinauralEffectSettings { hrtf: &hrtf }
//! )?;
//!
//! let params = BinauralEffectParams {
//!     direction: Direction::new(1.0, 0.0, 0.0), // Sound from the right
//!     interpolation: HrtfInterpolation::Nearest,
//!     spatial_blend: 1.0,
//!     hrtf: &hrtf,
//!     peak_delays: None,
//! };
//!
//! let input_buffer = AudioBuffer::try_with_data([1.0; 1024])?;
//! let mut output_container = vec![0.0; 2 * input_buffer.num_samples() as usize];
//! let mut output_buffer = AudioBuffer::try_with_data_and_settings(
//!     &mut output_container,
//!     AudioBufferSettings::with_num_channels(2),
//! )?;
//!
//! let _ = effect.apply(&params, &input_buffer, &mut output_buffer);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub mod ambisonics;
pub use ambisonics::*;

pub mod binaural;
pub use binaural::*;

pub mod direct;
pub use direct::*;

mod error;
pub use error::EffectError;

pub mod reflection;
pub use reflection::*;

pub mod panning;
pub use panning::*;

pub mod path;
pub use path::*;

pub mod virtual_surround;
pub use virtual_surround::*;

mod equalizer;
pub use equalizer::Equalizer;

mod audio_effect_state;
pub use audio_effect_state::AudioEffectState;
