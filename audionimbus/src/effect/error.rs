use crate::ChannelRequirement;

/// Errors that can occur when applying audio effects.
#[derive(Debug, PartialEq, Eq)]
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
                    "invalid number of input channels: expected {expected}, got {actual}",
                )
            }
            Self::InvalidOutputChannels { expected, actual } => {
                write!(
                    f,
                    "invalid number of output channels: expected {expected}, got {actual}",
                )
            }
        }
    }
}

/// Error returned when the requested number of channels exceeds the maximum set during effect creation.
#[derive(Debug, Clone, PartialEq)]
pub struct NumChannelsExceedsMaxError {
    pub requested: u32,
    pub max: u32,
}

impl std::error::Error for NumChannelsExceedsMaxError {}

impl std::fmt::Display for NumChannelsExceedsMaxError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "requested {} channels, but maximum is {} (set during effect creation)",
            self.requested, self.max
        )
    }
}

/// Error returned when the requested impulse response size exceeds the maximum set during effect creation.
#[derive(Debug, Clone, PartialEq)]
pub struct ImpulseResponseSizeExceedsMaxError {
    pub requested: u32,
    pub max: u32,
}

impl std::error::Error for ImpulseResponseSizeExceedsMaxError {}

impl std::fmt::Display for ImpulseResponseSizeExceedsMaxError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "requested impulse response size of {}, but maximum is {} (set during effect creation)",
            self.requested, self.max
        )
    }
}
