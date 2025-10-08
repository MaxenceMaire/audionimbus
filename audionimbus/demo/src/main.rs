use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

fn main() {
    // Initialize CPAL.
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .expect("no output device available");

    let frame_size: usize = 1024;
    let sample_rate: usize = 48000;
    let num_channels: usize = 2;

    let config = cpal::StreamConfig {
        buffer_size: cpal::BufferSize::Fixed(frame_size as u32),
        sample_rate: cpal::SampleRate(sample_rate as u32),
        channels: num_channels as u16,
    };

    // Initialize the audio context.
    let context = audionimbus::Context::try_new(&audionimbus::ContextSettings::default()).unwrap();

    let audio_settings = audionimbus::AudioSettings {
        sampling_rate: sample_rate,
        frame_size,
    };

    // Set up HRTF for binaural rendering.
    let hrtf = audionimbus::Hrtf::try_new(
        &context,
        &audio_settings,
        &audionimbus::HrtfSettings::default(),
    )
    .unwrap();

    // Create a binaural effect.
    let binaural_effect = audionimbus::BinauralEffect::try_new(
        &context,
        &audio_settings,
        &audionimbus::BinauralEffectSettings { hrtf: &hrtf },
    )
    .unwrap();

    // Parameters of the generated sine wave.
    let frequency = 440.0; // Frequency of the sine wave, in Hz.
    let amplitude = 0.5; // Amplitude of the sine wave.
    let mut phase: f32 = 0.0; // Phase of the sine wave.
    let phase_increment = 2.0 * std::f32::consts::PI * frequency / sample_rate as f32;
    let delta_time = frame_size as f32 / sample_rate as f32; // Duration of a frame, in seconds.
    let speed = 5.0; // Speed of the sound source, in m/s.
    let distance_traveled = speed * delta_time; // Distance traveled over a frame.
    let radius = 1.0; // Radius of the sound source's circular path, in meters.
    let mut angle = 0.0; // Direction angle of the sound source.

    let stream = device
        .build_output_stream(
            &config,
            move |output: &mut [audionimbus::Sample], _: &cpal::OutputCallbackInfo| {
                // Generate the sine wave for this frame.
                let sine_wave: Vec<audionimbus::Sample> = (0..frame_size)
                    .map(|_| {
                        let sample = amplitude * phase.sin();
                        phase = (phase + phase_increment) % (2.0 * std::f32::consts::PI);
                        sample
                    })
                    .collect();

                let input_buffer = audionimbus::AudioBuffer::try_with_data(&sine_wave).unwrap();

                // Container the effect will write processed samples into.
                let mut staging_container = vec![0.0; frame_size * num_channels];
                let staging_buffer = audionimbus::AudioBuffer::try_with_data_and_settings(
                    &mut staging_container,
                    audionimbus::AudioBufferSettings {
                        num_channels: Some(num_channels),
                        ..Default::default()
                    },
                )
                .unwrap();

                // Update the direction of the sound source.
                angle = (angle + distance_traveled / radius).rem_euclid(std::f32::consts::TAU);
                let x = angle.cos() * radius;
                let z = angle.sin() * radius;
                let direction = audionimbus::Direction::new(x, 0.0, z);

                let binaural_effect_params = audionimbus::BinauralEffectParams {
                    direction,
                    interpolation: audionimbus::HrtfInterpolation::Nearest,
                    spatial_blend: 1.0,
                    hrtf: &hrtf,
                    peak_delays: None,
                };
                let _effect_state =
                    binaural_effect.apply(&binaural_effect_params, &input_buffer, &staging_buffer);

                // Samples are currently deinterleaved (i.e., [L0, L1, ..., R0, R1, ...]), but CPAL
                // requires an interleaved format (i.e., [L0, R0, L1, R1, ...]) as output.
                staging_buffer.interleave(&context, output);
            },
            move |err| eprintln!("an error occurred on the output audio stream: {}", err),
            None,
        )
        .unwrap();

    stream.play().unwrap();

    // Keep the main thread alive.
    std::thread::park();
}
