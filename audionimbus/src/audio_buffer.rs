use crate::ffi_wrapper::FFIWrapper;

/// An audio buffer.
///
/// All audio buffers passed to Steam Audio must be deinterleaved.
#[derive(Debug)]
pub struct AudioBuffer {
    num_channels: usize,
    num_samples: usize,
    data: Vec<f32>,
    channel_ptrs: Vec<*mut f32>,
}

impl AudioBuffer {
    pub fn with_num_channels_and_num_samples(num_channels: usize, num_samples: usize) -> Self {
        let mut data: Vec<f32> = vec![0.0; num_channels * num_samples];

        let mut channel_ptrs: Vec<*mut f32> = Vec::with_capacity(num_channels);
        for i in 0..num_channels {
            let channel_ptr = data.as_mut_ptr().wrapping_add(i * num_samples);
            channel_ptrs.push(channel_ptr);
        }

        Self {
            num_channels,
            num_samples,
            data,
            channel_ptrs,
        }
    }

    pub(crate) fn as_ffi(&mut self) -> FFIWrapper<'_, audionimbus_sys::IPLAudioBuffer, Self> {
        let audio_buffer = audionimbus_sys::IPLAudioBuffer {
            numChannels: self.num_channels as i32,
            numSamples: self.num_samples as i32,
            data: self.channel_ptrs.as_mut_ptr(),
        };

        FFIWrapper::new(audio_buffer)
    }
}

impl From<DeinterleavedChannelSamples> for AudioBuffer {
    fn from(channels: DeinterleavedChannelSamples) -> Self {
        let num_channels = channels.len();
        let num_samples = channels.first().map_or(0, |channel| channel.len());
        let mut data: Vec<Sample> = channels.into_iter().flatten().collect();

        let mut channel_ptrs: Vec<*mut Sample> = Vec::with_capacity(num_channels);
        for i in 0..num_channels {
            let channel_ptr = data.as_mut_ptr().wrapping_add(i * num_samples);
            channel_ptrs.push(channel_ptr);
        }

        Self {
            num_channels,
            num_samples,
            data,
            channel_ptrs,
        }
    }
}

pub type Sample = f32;
pub type Channel = Vec<Sample>;
pub type DeinterleavedChannelSamples = Vec<Channel>;
