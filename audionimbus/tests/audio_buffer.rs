use audionimbus::*;

#[test]
fn test_buffer_mix() {
    let context = Context::default();

    const FRAME_SIZE: usize = 1024;

    let source_container = vec![0.1; FRAME_SIZE];
    let source_buffer = AudioBuffer::try_with_data(&source_container).unwrap();

    let mix_container = vec![0.2; FRAME_SIZE];
    let mut mix_buffer = AudioBuffer::try_with_data(&mix_container).unwrap();

    mix_buffer.mix(&context, &source_buffer);

    assert_eq!(mix_container, vec![0.3; FRAME_SIZE]);
}

#[test]
fn test_buffer_mix_multichannel() {
    let context = Context::default();

    const FRAME_SIZE: usize = 512;
    const NUM_CHANNELS: usize = 2;

    let mut source_container = Vec::with_capacity(NUM_CHANNELS * FRAME_SIZE);
    source_container.extend(std::iter::repeat_n(0.1, FRAME_SIZE));
    source_container.extend(std::iter::repeat_n(0.2, FRAME_SIZE));
    let source_buffer = AudioBuffer::try_with_data_and_settings(
        &source_container,
        AudioBufferSettings::with_num_channels(NUM_CHANNELS as u32),
    )
    .unwrap();

    let mut mix_container = Vec::with_capacity(NUM_CHANNELS * FRAME_SIZE);
    mix_container.extend(std::iter::repeat_n(0.3, FRAME_SIZE));
    mix_container.extend(std::iter::repeat_n(0.4, FRAME_SIZE));
    let mut mix_buffer = AudioBuffer::try_with_data_and_settings(
        &mut mix_container,
        AudioBufferSettings::with_num_channels(NUM_CHANNELS as u32),
    )
    .unwrap();

    mix_buffer.mix(&context, &source_buffer);

    // First channel: 0.3 + 0.1 = 0.4
    assert_eq!(&mix_container[0..FRAME_SIZE], &vec![0.4; FRAME_SIZE][..]);
    // Second channel: 0.4 + 0.2 = 0.6
    assert_eq!(&mix_container[FRAME_SIZE..], &vec![0.6; FRAME_SIZE][..]);
}

#[test]
fn test_buffer_downmix() {
    let context = Context::default();

    const FRAME_SIZE: usize = 1024;
    const NUM_CHANNELS: usize = 2;

    let mut input_container = Vec::with_capacity(NUM_CHANNELS * FRAME_SIZE);
    input_container.extend(std::iter::repeat_n(0.1, FRAME_SIZE));
    input_container.extend(std::iter::repeat_n(0.3, FRAME_SIZE));
    let input_buffer = AudioBuffer::try_with_data_and_settings(
        &mut input_container,
        AudioBufferSettings {
            num_channels: Some(NUM_CHANNELS as u32),
            ..Default::default()
        },
    )
    .unwrap();

    let mut downmix_container = vec![0.0; FRAME_SIZE];
    let mut downmix_buffer = AudioBuffer::try_with_data(&mut downmix_container).unwrap();

    downmix_buffer.downmix(&context, &input_buffer);

    assert_eq!(downmix_container, vec![0.2; FRAME_SIZE]);
}

#[test]
fn test_buffer_downmix_multichannel() {
    let context = Context::default();

    const FRAME_SIZE: usize = 512;
    const NUM_CHANNELS: usize = 4;

    let mut input_container = Vec::with_capacity(NUM_CHANNELS * FRAME_SIZE);
    input_container.extend(std::iter::repeat_n(0.1, FRAME_SIZE));
    input_container.extend(std::iter::repeat_n(0.2, FRAME_SIZE));
    input_container.extend(std::iter::repeat_n(0.3, FRAME_SIZE));
    input_container.extend(std::iter::repeat_n(0.4, FRAME_SIZE));
    let input_buffer = AudioBuffer::try_with_data_and_settings(
        &mut input_container,
        AudioBufferSettings::with_num_channels(NUM_CHANNELS as u32),
    )
    .unwrap();

    let mut downmix_container = vec![0.0; FRAME_SIZE];
    let mut downmix_buffer = AudioBuffer::try_with_data(&mut downmix_container).unwrap();

    downmix_buffer.downmix(&context, &input_buffer);

    // Average of 0.1, 0.2, 0.3, 0.4 = 0.25
    assert_eq!(downmix_container, vec![0.25; FRAME_SIZE]);
}

#[test]
fn test_buffer_interleave_deinterleave() {
    let context = Context::default();

    const FRAME_SIZE: usize = 256;
    const NUM_CHANNELS: usize = 2;

    let mut deinterleaved = Vec::with_capacity(NUM_CHANNELS * FRAME_SIZE);
    deinterleaved.extend((0..FRAME_SIZE).map(|i| i as f32)); // Channel 0
    deinterleaved.extend((0..FRAME_SIZE).map(|i| (i + 1000) as f32)); // Channel 1

    let buffer = AudioBuffer::try_with_data_and_settings(
        &deinterleaved,
        AudioBufferSettings::with_num_channels(NUM_CHANNELS as u32),
    )
    .unwrap();

    // Interleave
    let mut interleaved = vec![0.0; NUM_CHANNELS * FRAME_SIZE];
    buffer.interleave(&context, &mut interleaved);

    // Verify interleaving: [ch0[0], ch1[0], ch0[1], ch1[1], ...]
    for i in 0..FRAME_SIZE {
        assert_eq!(interleaved[i * 2], i as f32);
        assert_eq!(interleaved[i * 2 + 1], (i + 1000) as f32);
    }

    // Deinterleave back
    let mut deinterleaved_back = vec![0.0; NUM_CHANNELS * FRAME_SIZE];
    let mut buffer_back = AudioBuffer::try_with_data_and_settings(
        &mut deinterleaved_back,
        AudioBufferSettings::with_num_channels(NUM_CHANNELS as u32),
    )
    .unwrap();

    buffer_back.deinterleave(&context, &interleaved);

    assert_eq!(deinterleaved_back, deinterleaved);
}

#[test]
fn test_convert_ambisonics() {
    let context = Context::default();

    const FRAME_SIZE: usize = 256;
    const NUM_CHANNELS: usize = 4; // First-order Ambisonics

    let mut n3d_data = vec![0.5; NUM_CHANNELS * FRAME_SIZE];
    let mut n3d_buffer = AudioBuffer::try_with_data_and_settings(
        &mut n3d_data,
        AudioBufferSettings::with_num_channels(NUM_CHANNELS as u32),
    )
    .unwrap();

    n3d_buffer.convert_ambisonics(&context, AmbisonicsType::N3D, AmbisonicsType::SN3D);

    n3d_buffer.convert_ambisonics(&context, AmbisonicsType::SN3D, AmbisonicsType::N3D);

    // After round-trip conversion, values should be approximately the same.
    for &value in &n3d_data {
        assert!(
            (value - 0.5).abs() < 0.01,
            "Value {} too far from 0.5",
            value
        );
    }
}

#[test]
fn test_convert_ambisonics_into() {
    let context = Context::default();

    const FRAME_SIZE: usize = 256;
    const NUM_CHANNELS: usize = 4;

    let mut n3d_data = vec![0.7; NUM_CHANNELS * FRAME_SIZE];
    let mut n3d_buffer = AudioBuffer::try_with_data_and_settings(
        &mut n3d_data,
        AudioBufferSettings::with_num_channels(NUM_CHANNELS as u32),
    )
    .unwrap();

    let mut sn3d_data = vec![0.0; NUM_CHANNELS * FRAME_SIZE];
    let mut sn3d_buffer = AudioBuffer::try_with_data_and_settings(
        &mut sn3d_data,
        AudioBufferSettings::with_num_channels(NUM_CHANNELS as u32),
    )
    .unwrap();

    n3d_buffer.convert_ambisonics_into(
        &context,
        AmbisonicsType::N3D,
        AmbisonicsType::SN3D,
        &mut sn3d_buffer,
    );

    // Output should be written into sn3d_buffer.
    assert_ne!(sn3d_data[0], 0.0);
}
