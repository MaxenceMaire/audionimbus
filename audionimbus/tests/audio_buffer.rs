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
    // TODO: implement test.
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
    // TODO: implement test.
}
