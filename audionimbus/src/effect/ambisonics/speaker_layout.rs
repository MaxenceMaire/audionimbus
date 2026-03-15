use crate::geometry::Direction;
use std::fmt;
use std::ops::{Deref, DerefMut};

/// Describes a standard or custom speaker layout.
#[derive(Debug, PartialEq, Clone)]
pub enum SpeakerLayout {
    /// Mono.
    Mono,

    /// Stereo (left, right).
    Stereo,

    /// Front left, front right, rear left, rear right.
    Quadraphonic,

    /// Front left, front right, front center, LFE, rear left, rear right.
    Surround5_1,

    /// Front left, front right, front center, LFE, rear left, rear right, side left, side right.
    Surround7_1,

    /// User-defined speaker layout.
    Custom {
        /// Unit-length direction for each speaker.
        speaker_directions: Vec<Direction>,
    },
}

impl SpeakerLayout {
    /// Returns the name of this speaker layout.
    const fn name(&self) -> &'static str {
        match self {
            Self::Mono => "mono",
            Self::Stereo => "stereo",
            Self::Quadraphonic => "quadraphonic",
            Self::Surround5_1 => "5.1",
            Self::Surround7_1 => "7.1",
            Self::Custom { .. } => "custom",
        }
    }

    pub(crate) fn to_ffi(&self) -> SpeakerLayoutFfi {
        use audionimbus_sys::IPLSpeakerLayoutType::*;

        let (type_, mut directions) = match self {
            Self::Mono => (IPL_SPEAKERLAYOUTTYPE_MONO, vec![]),
            Self::Stereo => (IPL_SPEAKERLAYOUTTYPE_STEREO, vec![]),
            Self::Quadraphonic => (IPL_SPEAKERLAYOUTTYPE_QUADRAPHONIC, vec![]),
            Self::Surround5_1 => (IPL_SPEAKERLAYOUTTYPE_SURROUND_5_1, vec![]),
            Self::Surround7_1 => (IPL_SPEAKERLAYOUTTYPE_SURROUND_7_1, vec![]),
            Self::Custom { speaker_directions } => (
                IPL_SPEAKERLAYOUTTYPE_CUSTOM,
                speaker_directions.iter().copied().map(Into::into).collect(),
            ),
        };

        let num_speakers = directions.len() as i32;
        let speakers_ptr = if directions.is_empty() {
            std::ptr::null_mut()
        } else {
            directions.as_mut_ptr()
        };

        SpeakerLayoutFfi {
            layout: audionimbus_sys::IPLSpeakerLayout {
                type_,
                numSpeakers: num_speakers,
                speakers: speakers_ptr,
            },
            _directions: directions,
        }
    }
}

impl fmt::Display for SpeakerLayout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Custom { speaker_directions } => {
                write!(f, "{} ({} channels)", self.name(), speaker_directions.len())
            }
            _ => f.write_str(self.name()),
        }
    }
}

/// FFI speaker layout.
pub(crate) struct SpeakerLayoutFfi {
    layout: audionimbus_sys::IPLSpeakerLayout,
    /// Keeps the IPLVector3 whose pointer is used by the layout alive.
    _directions: Vec<audionimbus_sys::IPLVector3>,
}

impl Deref for SpeakerLayoutFfi {
    type Target = audionimbus_sys::IPLSpeakerLayout;
    fn deref(&self) -> &Self::Target {
        &self.layout
    }
}

impl DerefMut for SpeakerLayoutFfi {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.layout
    }
}
