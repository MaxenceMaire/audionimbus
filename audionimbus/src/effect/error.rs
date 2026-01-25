use super::ambisonics::SpeakerLayout;

/// Errors that can occur when applying audio effects.
#[derive(Debug, PartialEq)]
pub enum EffectError {
    /// Input buffer has wrong number of channels.
    InvalidInputChannels { expected: u32, actual: u32 },

    /// Output buffer has wrong number of channels.
    InvalidOutputChannels { expected: u32, actual: u32 },

    /// Input and output channel counts must match but don't.
    InputOutputChannelMismatch { input: u32, output: u32 },

    /// Buffer doesn't have correct channels for ambisonic order.
    InvalidAmbisonicOrder {
        order: u32,
        buffer_channels: u32,
        required_channels: u32,
    },

    /// Buffer has wrong channels for speaker layout.
    InvalidSpeakerLayoutChannels {
        layout: SpeakerLayout,
        expected: u32,
        actual: u32,
    },

    /// Buffer is too small for the required samples.
    InsufficientBufferSize {
        required_samples: usize,
        actual_samples: usize,
    },
}

impl std::error::Error for EffectError {}

impl std::fmt::Display for EffectError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::InvalidInputChannels { expected, actual } => {
                write!(
                    f,
                    "invalid number of input channels: expected {}, got {}",
                    expected, actual
                )
            }
            Self::InvalidOutputChannels { expected, actual } => {
                write!(
                    f,
                    "invalid number of output channels: expected {}, got {}",
                    expected, actual
                )
            }
            Self::InputOutputChannelMismatch { input, output } => {
                write!(
                    f,
                    "input and output channel counts must match: input has {}, output has {}",
                    input, output
                )
            }
            Self::InvalidAmbisonicOrder {
                order,
                buffer_channels,
                required_channels,
            } => {
                write!(
                    f,
                    "ambisonic order {} requires {} channels, but buffer has {}",
                    order, required_channels, buffer_channels
                )
            }
            Self::InvalidSpeakerLayoutChannels {
                layout,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "speaker layout '{}' requires {} channels, but buffer has {}",
                    layout, expected, actual
                )
            }
            Self::InsufficientBufferSize {
                required_samples,
                actual_samples,
            } => {
                write!(
                    f,
                    "buffer too small: needs {} samples, has {}",
                    required_samples, actual_samples
                )
            }
        }
    }
}
