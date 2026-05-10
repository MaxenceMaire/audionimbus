use audionimbus::Sample;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Stream, StreamConfig};

pub const FRAME_SIZE: u32 = 1024;
pub const SAMPLE_RATE: u32 = 48_000;
pub const NUM_CHANNELS: u32 = 2;

/// Opens the default output device and returns a [`StreamConfig`].
pub fn default_device() -> (cpal::Device, StreamConfig) {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .expect("no output device available");

    let config = StreamConfig {
        buffer_size: cpal::BufferSize::Fixed(FRAME_SIZE),
        sample_rate: cpal::SampleRate(SAMPLE_RATE),
        channels: NUM_CHANNELS as u16,
    };

    (device, config)
}

/// Builds, starts, and returns an output stream driven by `callback`.
pub fn start_output_stream<F>(device: &cpal::Device, config: &StreamConfig, callback: F) -> Stream
where
    F: FnMut(&mut [Sample], &cpal::OutputCallbackInfo) + Send + 'static,
{
    let mut fixed_output = FrameAdapter::new(config, callback);
    let stream = device
        .build_output_stream(
            config,
            move |output, info| fixed_output.render(output, info),
            |err| eprintln!("audio stream error: {err}"),
            None,
        )
        .expect("failed to build output stream");

    stream.play().expect("failed to start output stream");
    stream
}

/// Adapts CPAL's callback buffer size to the fixed block size used by Steam Audio.
///
/// CPAL doesn't guarantee [`cpal::BufferSize::Fixed`] is actually honored.
/// Some backends may still invoke the output callback with a different number of samples.
///
/// This wraps the render callback so it always sees exactly `FRAME_SIZE * channels` samples,
/// holding any unconsumed output in `buffer` until the next CPAL callback drains it.
struct FrameAdapter<F> {
    /// User render callback that fills exactly one fixed-size frame.
    callback: F,
    /// Block buffer.
    buffer: Vec<Sample>,
    /// Current read position in `buffer`.
    cursor: usize,
}

impl<F> FrameAdapter<F>
where
    F: FnMut(&mut [Sample], &cpal::OutputCallbackInfo),
{
    fn new(config: &StreamConfig, callback: F) -> Self {
        let buffer_len = FRAME_SIZE as usize * config.channels as usize;
        Self {
            callback,
            buffer: vec![0.0; buffer_len],
            // Start at the end so the first callback renders a fresh block.
            cursor: buffer_len,
        }
    }

    /// Fills CPAL's `output` buffer by consuming one or more fixed-size render blocks.
    fn render(&mut self, output: &mut [Sample], info: &cpal::OutputCallbackInfo) {
        let mut remaining = output;

        while !remaining.is_empty() {
            if self.cursor == self.buffer.len() {
                (self.callback)(&mut self.buffer, info);
                self.cursor = 0;
            }

            let n = remaining.len().min(self.buffer.len() - self.cursor);
            remaining[..n].copy_from_slice(&self.buffer[self.cursor..self.cursor + n]);
            remaining = &mut remaining[n..];
            self.cursor += n;
        }
    }
}
