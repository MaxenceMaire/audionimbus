use crate::context::Context;
use crate::ffi_wrapper::FFIWrapper;

/// An audio buffer.
///
/// All audio buffers passed to Steam Audio must be deinterleaved.
#[derive(Debug)]
pub struct AudioBuffer {
    /// Number of channels.
    pub num_channels: usize,

    /// Number of samples per channel.
    pub num_samples: usize,

    /// Sample data.
    pub data: Vec<Sample>,

    /// Pointers to sample data for each channel.
    pub channel_ptrs: Vec<*mut Sample>,
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

    /// Interleaves samples from the audio buffer.
    pub fn interleave(&mut self, context: &Context) -> Vec<f32> {
        let mut interleaved_data = vec![0.0; self.num_channels * self.num_samples];
        self.interleave_mut(context, &mut interleaved_data);
        interleaved_data
    }

    /// Mutates the `dst` buffer with interleaved data from the audio buffer.
    pub fn interleave_mut(&mut self, context: &Context, dst: &mut [Sample]) {
        let mut audio_buffer_ffi = self.as_ffi();

        unsafe {
            audionimbus_sys::iplAudioBufferInterleave(
                context.raw_ptr(),
                &mut *audio_buffer_ffi,
                dst.as_mut_ptr(),
            );
        }
    }

    /// Deinterleaves the `src` data into `Self`.
    pub fn deinterleave(&mut self, context: &Context, src: &mut [Sample]) {
        assert_eq!(
            src.len(),
            self.num_channels * self.num_samples,
            "input data size must match the required interleaved size"
        );

        let mut audio_buffer_ffi = self.as_ffi();

        unsafe {
            audionimbus_sys::iplAudioBufferDeinterleave(
                context.raw_ptr(),
                src.as_mut_ptr(),
                &mut *audio_buffer_ffi,
            )
        };
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

/// An audio sample.
pub type Sample = f32;

/// An audio channel.
pub type Channel = Vec<Sample>;

/// Deinterleaved sample data, i.e. sample data organized by channel.
pub type DeinterleavedChannelSamples = Vec<Channel>;
