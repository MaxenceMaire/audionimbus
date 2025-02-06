const AUDIO_BUFFER_SIZE_WAVE_440HZ_1S: &[u8] = include_bytes!("sine_wave_440Hz_1s.raw");

#[test]
fn test_initialization() {
    let context_settings = audionimbus::ContextSettings::default();

    let context_result = audionimbus::Context::try_new(&context_settings);
    assert!(context_result.is_ok());
}

#[test]
fn test_load_hrtf_default() {
    let context_settings = audionimbus::ContextSettings::default();

    let context = audionimbus::Context::try_new(&context_settings).unwrap();

    let audio_settings = audionimbus::AudioSettings::default();
    let hrtf_settings = audionimbus::HrtfSettings::default();

    let hrtf_result = audionimbus::Hrtf::try_new(&context, &audio_settings, &hrtf_settings);
    assert!(hrtf_result.is_ok());
}

// TODO: implement test.
#[test]
fn test_load_hrtf_sofa_filename() {}

// TODO: implement test.
#[test]
fn test_load_hrtf_sofa_buffer() {}

#[test]
fn test_binaural_effect() {
    let mut input_buffer =
        audionimbus::AudioBuffer::from(raw_to_deinterleaved(AUDIO_BUFFER_SIZE_WAVE_440HZ_1S, 1));

    let mut output_buffer =
        audionimbus::AudioBuffer::with_num_channels_and_num_samples(2, input_buffer.num_samples);

    let context_settings = audionimbus::ContextSettings::default();

    let context_result = audionimbus::Context::try_new(&context_settings);
    let context = context_result.unwrap();

    let audio_settings = audionimbus::AudioSettings {
        frame_size: input_buffer.data.len(),
        ..Default::default()
    };
    let hrtf_settings = audionimbus::HrtfSettings::default();

    let hrtf = audionimbus::Hrtf::try_new(&context, &audio_settings, &hrtf_settings).unwrap();

    let binaural_effect_settings = audionimbus::effect::BinauralEffectSettings { hrtf: &hrtf };

    let binaural_effect = audionimbus::effect::BinauralEffect::try_new(
        &context,
        &audio_settings,
        &binaural_effect_settings,
    )
    .unwrap();

    let binaural_effect_params = audionimbus::effect::BinauralEffectParams {
        direction: audionimbus::geometry::Direction::new(1.0, 1.0, 1.0),
        interpolation: audionimbus::HrtfInterpolation::Nearest,
        spatial_blend: 1.0,
        hrtf: &hrtf,
        peak_delays: None,
    };

    binaural_effect.apply(
        &binaural_effect_params,
        &mut input_buffer,
        &mut output_buffer,
    );

    let _ = output_buffer.interleave(&context);
}

fn raw_to_deinterleaved(
    raw_data: &[u8],
    num_channels: usize,
) -> audionimbus::DeinterleavedChannelSamples {
    let num_samples = raw_data.len() / 4; // Each sample is 4 bytes (32-bit float).

    let mut channels: audionimbus::DeinterleavedChannelSamples =
        vec![Vec::with_capacity(num_samples / num_channels); num_channels];

    for i in 0..num_samples {
        let sample_bytes = &raw_data[i * 4..(i + 1) * 4];

        let sample = f32::from_le_bytes(sample_bytes.try_into().unwrap());

        // Determine which channel the sample belongs to and add it.
        let channel_index = i % num_channels;
        channels[channel_index].push(sample);
    }

    channels
}
