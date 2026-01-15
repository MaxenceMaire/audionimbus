pub const STEAMAUDIO_VERSION: usize = audionimbus_sys::STEAMAUDIO_VERSION as usize;
pub const STEAMAUDIO_VERSION_MAJOR: usize = audionimbus_sys::STEAMAUDIO_VERSION_MAJOR as usize;
pub const STEAMAUDIO_VERSION_MINOR: usize = audionimbus_sys::STEAMAUDIO_VERSION_MINOR as usize;
pub const STEAMAUDIO_VERSION_PATCH: usize = audionimbus_sys::STEAMAUDIO_VERSION_PATCH as usize;

/// The version of the Steam Audio library.
#[derive(Copy, Clone, Debug)]
pub struct SteamAudioVersion {
    pub major: usize,
    pub minor: usize,
    pub patch: usize,
}

impl From<SteamAudioVersion> for u32 {
    fn from(version: SteamAudioVersion) -> Self {
        ((version.major << 16) + (version.minor << 8) + version.patch) as u32
    }
}

impl Default for SteamAudioVersion {
    fn default() -> Self {
        Self {
            major: STEAMAUDIO_VERSION_MAJOR,
            minor: STEAMAUDIO_VERSION_MINOR,
            patch: STEAMAUDIO_VERSION_PATCH,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod try_new {
        use super::*;

        #[test]
        fn test_version_to_u32() {
            let version = SteamAudioVersion {
                major: 4,
                minor: 8,
                patch: 0,
            };

            let version_u32: u32 = version.into();

            assert_eq!(version_u32, 264192);
        }
    }
}
