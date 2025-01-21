use crate::error::{to_option_error, SteamAudioError};
use crate::version::STEAMAUDIO_VERSION;

/// A context object, which controls low-level operations of Steam Audio.
///
/// Typically, a context is specified once during the execution of the client program, before calling any other API functions.
#[derive(Debug)]
pub struct Context(pub audionimbus_sys::IPLContext);

impl Context {
    pub fn try_new(settings: &ContextSettings) -> Result<Self, SteamAudioError> {
        let context = unsafe {
            let context: *mut audionimbus_sys::IPLContext = std::ptr::null_mut();
            let status = audionimbus_sys::iplContextCreate(
                &mut audionimbus_sys::IPLContextSettings::from(settings),
                context,
            );

            if let Some(error) = to_option_error(status) {
                return Err(error);
            }

            *context
        };

        Ok(Self(context))
    }

    pub fn as_raw_mut(&mut self) -> *mut audionimbus_sys::IPLContext {
        &mut self.0
    }
}

impl std::ops::Deref for Context {
    type Target = audionimbus_sys::IPLContext;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for Context {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplContextRelease(&mut self.0) }
    }
}

/// Settings used to create a [`Context`].
#[derive(Debug)]
pub struct ContextSettings {
    // TODO: add other fields from IPLContextSettings.
    /// The API version.
    ///
    /// Context creation will fail if `phonon.dll` does not implement a compatible version of the API.
    /// Typically, this should be set to [`STEAMAUDIO_VERSION`].
    pub version: u32,
}

impl Default for ContextSettings {
    fn default() -> Self {
        Self {
            version: STEAMAUDIO_VERSION,
        }
    }
}

impl From<&ContextSettings> for audionimbus_sys::IPLContextSettings {
    fn from(settings: &ContextSettings) -> Self {
        todo!()
    }
}
