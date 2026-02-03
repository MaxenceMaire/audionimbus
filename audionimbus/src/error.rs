/// A Steam Audio error.
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum SteamAudioError {
    /// An unspecified error occurred.
    Unspecified,

    /// The system ran out of memory.
    OutOfMemory,

    /// An error occurred while initializing an external dependency.
    Initialization,
}

impl std::error::Error for SteamAudioError {}

impl std::fmt::Display for SteamAudioError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match &self {
            Self::Unspecified => write!(f, "unspecified error",),
            Self::OutOfMemory => write!(f, "out of memory"),
            Self::Initialization => write!(f, "error while initializing an external dependency"),
        }
    }
}

pub const fn to_option_error(status: audionimbus_sys::IPLerror) -> Option<SteamAudioError> {
    match status {
        audionimbus_sys::IPLerror::IPL_STATUS_SUCCESS => None,
        audionimbus_sys::IPLerror::IPL_STATUS_FAILURE => Some(SteamAudioError::Unspecified),
        audionimbus_sys::IPLerror::IPL_STATUS_OUTOFMEMORY => Some(SteamAudioError::OutOfMemory),
        audionimbus_sys::IPLerror::IPL_STATUS_INITIALIZATION => {
            Some(SteamAudioError::Initialization)
        }
    }
}
