use super::open_cl::OpenClDevice;
use crate::error::{to_option_error, SteamAudioError};

/// Application-wide state for the TrueAudio Next convolution engine.
///
/// A TrueAudio Next device must be created before using any of Steam Audio’s TrueAudio Next convolution functionality.
#[derive(Debug)]
pub struct TrueAudioNextDevice(pub(crate) audionimbus_sys::IPLTrueAudioNextDevice);

impl TrueAudioNextDevice {
    pub fn new(
        open_cl_device: &OpenClDevice,
        settings: &TrueAudioNextDeviceSettings,
    ) -> Result<Self, SteamAudioError> {
        let mut true_audio_next_device = Self(std::ptr::null_mut());

        let status = unsafe {
            audionimbus_sys::iplTrueAudioNextDeviceCreate(
                open_cl_device.raw_ptr(),
                &mut audionimbus_sys::IPLTrueAudioNextDeviceSettings::from(settings),
                true_audio_next_device.raw_ptr_mut(),
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        Ok(true_audio_next_device)
    }

    pub fn null() -> Self {
        Self(std::ptr::null_mut())
    }

    pub fn raw_ptr(&self) -> audionimbus_sys::IPLTrueAudioNextDevice {
        self.0
    }

    pub fn from_raw_ptr(ptr: audionimbus_sys::IPLTrueAudioNextDevice) -> Self {
        Self(ptr)
    }

    pub fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLTrueAudioNextDevice {
        &mut self.0
    }
}

impl Clone for TrueAudioNextDevice {
    fn clone(&self) -> Self {
        unsafe {
            audionimbus_sys::iplTrueAudioNextDeviceRetain(self.0);
        }
        Self(self.0)
    }
}

impl Drop for TrueAudioNextDevice {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplTrueAudioNextDeviceRelease(&mut self.0) }
    }
}

unsafe impl Send for TrueAudioNextDevice {}
unsafe impl Sync for TrueAudioNextDevice {}

/// Settings used to create a TrueAudio Next device.
#[derive(Debug)]
pub struct TrueAudioNextDeviceSettings {
    /// The number of samples in an audio frame.
    pub frame_size: usize,

    /// The number of samples in the impulse responses that will be used for convolution.
    pub impulse_response_size: usize,

    /// The Ambisonic order of the impulse responses that will be used for convolution.
    pub order: usize,

    /// The maximum number of sources that will use TrueAudio Next for convolution.
    pub max_sources: usize,
}

impl From<&TrueAudioNextDeviceSettings> for audionimbus_sys::IPLTrueAudioNextDeviceSettings {
    fn from(settings: &TrueAudioNextDeviceSettings) -> Self {
        Self {
            frameSize: settings.frame_size as i32,
            irSize: settings.impulse_response_size as i32,
            order: settings.order as i32,
            maxSources: settings.max_sources as i32,
        }
    }
}
