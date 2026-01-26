use crate::ChannelRequirement;

/// Errors that can occur when applying audio effects.
#[derive(Debug, PartialEq)]
pub enum EffectError {
    /// Input buffer has wrong number of channels.
    InvalidInputChannels {
        expected: ChannelRequirement,
        actual: u32,
    },

    /// Output buffer has wrong number of channels.
    InvalidOutputChannels {
        expected: ChannelRequirement,
        actual: u32,
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
        }
    }
}
