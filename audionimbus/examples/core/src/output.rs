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
    let stream = device
        .build_output_stream(
            config,
            callback,
            |err| eprintln!("audio stream error: {err}"),
            None,
        )
        .expect("failed to build output stream");

    stream.play().expect("failed to start output stream");
    stream
}
