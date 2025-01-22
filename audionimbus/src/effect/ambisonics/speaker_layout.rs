use crate::geometry::Vector3;

/// Describes a standard or custom speaker layout.
#[derive(Debug)]
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
        speaker_directions: Vec<Vector3>,
    },
}
