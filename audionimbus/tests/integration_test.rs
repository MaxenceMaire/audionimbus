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
    let context = audionimbus::Context::try_new(&context_settings).unwrap();

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

#[test]
fn test_ambisonics_encode_effect() {
    let mut input_buffer =
        audionimbus::AudioBuffer::from(raw_to_deinterleaved(AUDIO_BUFFER_SIZE_WAVE_440HZ_1S, 1));

    let mut output_buffer =
        audionimbus::AudioBuffer::with_num_channels_and_num_samples(2, input_buffer.num_samples);

    let context_settings = audionimbus::ContextSettings::default();
    let context = audionimbus::Context::try_new(&context_settings).unwrap();

    let audio_settings = audionimbus::AudioSettings {
        frame_size: input_buffer.data.len(),
        ..Default::default()
    };

    let ambisonics_encode_effect_settings =
        audionimbus::effect::AmbisonicsEncodeEffectSettings { max_order: 0 };

    let ambisonics_encode_effect = audionimbus::effect::AmbisonicsEncodeEffect::try_new(
        &context,
        &audio_settings,
        &ambisonics_encode_effect_settings,
    )
    .unwrap();

    let ambisonics_encode_effect_params = audionimbus::effect::AmbisonicsEncodeEffectParams {
        direction: audionimbus::geometry::Direction::new(1.0, 1.0, 1.0),
        order: 0,
    };

    ambisonics_encode_effect.apply(
        &ambisonics_encode_effect_params,
        &mut input_buffer,
        &mut output_buffer,
    );

    let _ = output_buffer.interleave(&context);
}

#[test]
fn test_ambisonics_decode_effect() {
    let mut input_buffer =
        audionimbus::AudioBuffer::from(raw_to_deinterleaved(AUDIO_BUFFER_SIZE_WAVE_440HZ_1S, 1));

    let mut output_buffer =
        audionimbus::AudioBuffer::with_num_channels_and_num_samples(2, input_buffer.num_samples);

    let context_settings = audionimbus::ContextSettings::default();
    let context = audionimbus::Context::try_new(&context_settings).unwrap();

    let audio_settings = audionimbus::AudioSettings {
        frame_size: input_buffer.data.len(),
        ..Default::default()
    };

    let hrtf_settings = audionimbus::HrtfSettings::default();

    let hrtf = audionimbus::Hrtf::try_new(&context, &audio_settings, &hrtf_settings).unwrap();

    let ambisonics_decode_effect_settings = audionimbus::effect::AmbisonicsDecodeEffectSettings {
        speaker_layout: audionimbus::SpeakerLayout::Mono,
        hrtf: &hrtf,
        max_order: 0,
    };

    let ambisonics_decode_effect = audionimbus::effect::AmbisonicsDecodeEffect::try_new(
        &context,
        &audio_settings,
        &ambisonics_decode_effect_settings,
    )
    .unwrap();

    let ambisonics_decode_effect_params = audionimbus::effect::AmbisonicsDecodeEffectParams {
        order: 0,
        hrtf: &hrtf,
        orientation: audionimbus::geometry::CoordinateSystem::default(),
        binaural: true,
    };

    ambisonics_decode_effect.apply(
        &ambisonics_decode_effect_params,
        &mut input_buffer,
        &mut output_buffer,
    );

    let _ = output_buffer.interleave(&context);
}

#[test]
fn test_direct_effect() {
    let mut input_buffer =
        audionimbus::AudioBuffer::from(raw_to_deinterleaved(AUDIO_BUFFER_SIZE_WAVE_440HZ_1S, 1));

    let mut output_buffer =
        audionimbus::AudioBuffer::with_num_channels_and_num_samples(2, input_buffer.num_samples);

    let context_settings = audionimbus::ContextSettings::default();
    let context = audionimbus::Context::try_new(&context_settings).unwrap();

    let audio_settings = audionimbus::AudioSettings {
        frame_size: input_buffer.data.len(),
        ..Default::default()
    };

    let direct_effect_settings = audionimbus::effect::DirectEffectSettings { num_channels: 1 };

    let direct_effect = audionimbus::effect::DirectEffect::try_new(
        &context,
        &audio_settings,
        &direct_effect_settings,
    )
    .unwrap();

    let direct_effect_params = audionimbus::effect::DirectEffectParams {
        distance_attenuation: Some(0.6),
        air_absorption: Some(audionimbus::Equalizer([0.9, 0.7, 0.5])),
        directivity: Some(0.7),
        occlusion: Some(0.4),
        transmission: Some(audionimbus::Transmission::FrequencyIndependent(
            audionimbus::Equalizer([0.3, 0.2, 0.1]),
        )),
    };

    direct_effect.apply(&direct_effect_params, &mut input_buffer, &mut output_buffer);

    let _ = output_buffer.interleave(&context);
}

#[test]
fn test_distance_attenuation() {
    let context_settings = audionimbus::ContextSettings::default();
    let context = audionimbus::Context::try_new(&context_settings).unwrap();

    let source = audionimbus::Point::new(1.0, 1.0, 1.0);
    let listener = audionimbus::Point::new(0.0, 0.0, 0.0);

    let distance_attenuation_model = audionimbus::DistanceAttenuationModel::default();

    let distance_attenuation = audionimbus::calculate_distance_attenuation(
        &context,
        &source,
        &listener,
        &distance_attenuation_model,
    );

    assert_eq!(distance_attenuation, 0.57735026);
}

#[test]
fn test_air_absorption() {
    let context_settings = audionimbus::ContextSettings::default();
    let context = audionimbus::Context::try_new(&context_settings).unwrap();

    let source = audionimbus::Point::new(1.0, 1.0, 1.0);
    let listener = audionimbus::Point::new(0.0, 0.0, 0.0);

    let air_absorption_model = audionimbus::AirAbsorptionModel::default();

    let air_absorption =
        audionimbus::calculate_air_absorption(&context, &source, &listener, &air_absorption_model);

    assert_eq!(air_absorption, [0.99965364, 0.9970598, 0.96896833]);
}

#[test]
fn test_directivity_attenuation() {
    let context_settings = audionimbus::ContextSettings::default();
    let context = audionimbus::Context::try_new(&context_settings).unwrap();

    let source = audionimbus::CoordinateSystem::default();
    let listener = audionimbus::Point::new(0.0, 0.0, 0.0);

    let directivity = audionimbus::Directivity::default();

    let directivity_attenuation =
        audionimbus::calculate_directivity_attenuation(&context, &source, &listener, &directivity);

    assert_eq!(directivity_attenuation, 0.70710677);
}

#[test]
fn test_scene() {
    let context_settings = audionimbus::ContextSettings::default();
    let context = audionimbus::Context::try_new(&context_settings).unwrap();

    let scene_settings = audionimbus::SceneSettings::default();

    let scene_result = audionimbus::Scene::try_new(&context, &scene_settings);

    assert!(scene_result.is_ok());
}
