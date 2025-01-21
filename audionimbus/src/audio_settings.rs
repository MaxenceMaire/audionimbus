/// Global settings for audio signal processing.
pub struct AudioSettings {
    /// Sampling rate, in Hz.
    pub sampling_rate: u32,

    /// Frame size, in samples.
    /// Independent of number of channels.
    pub frame_size: u32,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            sampling_rate: 48000,
            frame_size: 1024,
        }
    }
}

impl From<&AudioSettings> for audionimbus_sys::IPLAudioSettings {
    fn from(settings: &AudioSettings) -> Self {
        todo!()
    }
}
