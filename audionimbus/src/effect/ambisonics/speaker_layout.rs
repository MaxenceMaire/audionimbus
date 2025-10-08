use crate::geometry::Direction;

/// Describes a standard or custom speaker layout.
#[derive(Debug, Clone)]
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

impl From<&SpeakerLayout> for audionimbus_sys::IPLSpeakerLayout {
    fn from(speaker_layout: &SpeakerLayout) -> Self {
        let (type_, num_speakers, mut speaker_directions) = match speaker_layout {
            SpeakerLayout::Mono => (
                audionimbus_sys::IPLSpeakerLayoutType::IPL_SPEAKERLAYOUTTYPE_MONO,
                i32::default(),
                vec![],
            ),
            SpeakerLayout::Stereo => (
                audionimbus_sys::IPLSpeakerLayoutType::IPL_SPEAKERLAYOUTTYPE_STEREO,
                i32::default(),
                vec![],
            ),
            SpeakerLayout::Quadraphonic => (
                audionimbus_sys::IPLSpeakerLayoutType::IPL_SPEAKERLAYOUTTYPE_QUADRAPHONIC,
                i32::default(),
                vec![],
            ),
            SpeakerLayout::Surround5_1 => (
                audionimbus_sys::IPLSpeakerLayoutType::IPL_SPEAKERLAYOUTTYPE_SURROUND_5_1,
                i32::default(),
                vec![],
            ),
            SpeakerLayout::Surround7_1 => (
                audionimbus_sys::IPLSpeakerLayoutType::IPL_SPEAKERLAYOUTTYPE_SURROUND_7_1,
                i32::default(),
                vec![],
            ),
            SpeakerLayout::Custom { speaker_directions } => (
                audionimbus_sys::IPLSpeakerLayoutType::IPL_SPEAKERLAYOUTTYPE_CUSTOM,
                i32::default(),
                speaker_directions
                    .clone()
                    .into_iter()
                    .map(audionimbus_sys::IPLVector3::from)
                    .collect(),
            ),
        };

        Self {
            type_,
            numSpeakers: num_speakers,
            speakers: speaker_directions.as_mut_ptr(),
        }
    }
}
