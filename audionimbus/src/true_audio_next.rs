use crate::error::{to_option_error, SteamAudioError};
use crate::open_cl::OpenClDevice;

#[derive(Debug)]
pub struct TrueAudioNextDevice(pub audionimbus_sys::IPLTrueAudioNextDevice);

/// Application-wide state for the TrueAudio Next convolution engine.
///
/// A TrueAudio Next device must be created before using any of Steam Audio’s TrueAudio Next convolution functionality.
impl TrueAudioNextDevice {
    pub fn new(
        open_cl_device: &OpenClDevice,
        settings: &TrueAudioNextDeviceSettings,
    ) -> Result<Self, SteamAudioError> {
        let true_audio_next_device = unsafe {
            let true_audio_next_device: *mut audionimbus_sys::IPLTrueAudioNextDevice =
                std::ptr::null_mut();
            let status = audionimbus_sys::iplTrueAudioNextDeviceCreate(
                open_cl_device.as_raw_ptr(),
                &mut audionimbus_sys::IPLTrueAudioNextDeviceSettings::from(settings),
                true_audio_next_device,
            );

            if let Some(error) = to_option_error(status) {
                return Err(error);
            }

            *true_audio_next_device
        };

        Ok(Self(true_audio_next_device))
    }
}

impl Drop for TrueAudioNextDevice {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplTrueAudioNextDeviceRelease(&mut self.0) }
    }
}

/// Settings used to create a TrueAudio Next device.
#[derive(Debug)]
pub struct TrueAudioNextDeviceSettings {
    /// The number of samples in an audio frame.
    pub frame_size: i32,

    /// The number of samples in the impulse responses that will be used for convolution.
    pub ir_size: i32,

    /// The Ambisonic order of the impulse responses that will be used for convolution.
    pub order: i32,

    /// The maximum number of sources that will use TrueAudio Next for convolution.
    pub max_sources: i32,
}

impl From<&TrueAudioNextDeviceSettings> for audionimbus_sys::IPLTrueAudioNextDeviceSettings {
    fn from(settings: &TrueAudioNextDeviceSettings) -> Self {
        todo!()
    }
}
