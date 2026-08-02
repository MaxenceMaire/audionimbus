use audionimbus::*;

#[test]
fn accesses_channels() {
    let source = [1.0, 2.0, 3.0, 4.0];
    let source_buffer = AudioBufferRef::try_new(&source, 2).unwrap();

    assert_eq!(source_buffer.channel(0), Some(&source[..2]));
    assert_eq!(source_buffer.channel(1), Some(&source[2..]));
    assert_eq!(source_buffer.channel(2), None);

    let mut destination = [0.0; 4];
    let mut destination_buffer = AudioBufferMut::try_new(&mut destination, 2).unwrap();
    destination_buffer
        .channel_mut(1)
        .unwrap()
        .copy_from_slice(&[3.0, 4.0]);
    assert_eq!(destination_buffer.channel(1), Some(&[3.0, 4.0][..]));
    assert!(destination_buffer.channel_mut(2).is_none());
}

#[test]
fn mixes_mono_buffers() {
    let context = Context::default();
    let source = vec![0.1; 1024];
    let source_buffer = AudioBufferRef::try_from(&source[..]).unwrap();
    let mut mixed = vec![0.2; 1024];

    AudioBufferMut::try_from(&mut mixed[..])
        .unwrap()
        .mix(&context, &source_buffer)
        .unwrap();

    assert_eq!(mixed, vec![0.3; 1024]);
}

#[test]
fn mixes_multichannel_buffers() {
    let context = Context::default();
    let mut source = vec![0.1; 512];
    source.extend(std::iter::repeat_n(0.2, 512));
    let source_buffer = AudioBufferRef::try_new(&source, 2).unwrap();
    let mut mixed = vec![0.3; 512];
    mixed.extend(std::iter::repeat_n(0.4, 512));

    AudioBufferMut::try_new(&mut mixed, 2)
        .unwrap()
        .mix(&context, &source_buffer)
        .unwrap();

    assert_eq!(&mixed[..512], &[0.4; 512]);
    assert_eq!(&mixed[512..], &[0.6; 512]);
}

#[test]
fn rejects_mix_shape_mismatches() {
    let context = Context::default();
    let source = [0.0; 8];
    let source_buffer = AudioBufferRef::try_new(&source, 2).unwrap();

    let mut mono = [0.0; 4];
    let error = AudioBufferMut::try_from(&mut mono[..])
        .unwrap()
        .mix(&context, &source_buffer)
        .unwrap_err();
    assert_eq!(
        error,
        AudioBufferOperationError::ChannelCountMismatch {
            self_num_channels: 1,
            other_num_channels: 2,
        }
    );

    let mut short = [0.0; 4];
    let error = AudioBufferMut::try_new(&mut short, 2)
        .unwrap()
        .mix(&context, &source_buffer)
        .unwrap_err();
    assert_eq!(
        error,
        AudioBufferOperationError::SampleCountMismatch {
            self_num_samples: 2,
            other_num_samples: 4,
        }
    );
}

#[test]
fn downmixes_multichannel_buffer() {
    let context = Context::default();
    let mut source = vec![0.1; 512];
    source.extend(std::iter::repeat_n(0.2, 512));
    source.extend(std::iter::repeat_n(0.3, 512));
    source.extend(std::iter::repeat_n(0.4, 512));
    let source_buffer = AudioBufferRef::try_new(&source, 4).unwrap();
    let mut downmixed = vec![0.0; 512];

    AudioBufferMut::try_from(&mut downmixed[..])
        .unwrap()
        .downmix(&context, &source_buffer)
        .unwrap();

    assert_eq!(downmixed, vec![0.25; 512]);
}

#[test]
fn rejects_non_mono_downmix_destination() {
    let context = Context::default();
    let source = [0.0; 8];
    let source_buffer = AudioBufferRef::try_new(&source, 2).unwrap();
    let mut destination = [0.0; 8];

    let error = AudioBufferMut::try_new(&mut destination, 2)
        .unwrap()
        .downmix(&context, &source_buffer)
        .unwrap_err();

    assert_eq!(
        error,
        AudioBufferOperationError::DownmixDestinationNotMono { num_channels: 2 }
    );
}

#[test]
fn rejects_downmix_sample_count_mismatch() {
    let context = Context::default();
    let source = [0.0; 8];
    let source_buffer = AudioBufferRef::try_new(&source, 2).unwrap();
    let mut destination = [0.0; 3];

    let error = AudioBufferMut::try_from(&mut destination[..])
        .unwrap()
        .downmix(&context, &source_buffer)
        .unwrap_err();

    assert_eq!(
        error,
        AudioBufferOperationError::SampleCountMismatch {
            self_num_samples: 3,
            other_num_samples: 4,
        }
    );
}

#[test]
fn interleaves_and_deinterleaves() {
    let context = Context::default();
    let mut deinterleaved = (0..256).map(|sample| sample as f32).collect::<Vec<_>>();
    deinterleaved.extend((0..256).map(|sample| (sample + 1000) as f32));
    let buffer = AudioBufferRef::try_new(&deinterleaved, 2).unwrap();
    let mut interleaved = vec![0.0; 512];

    buffer.interleave(&context, &mut interleaved).unwrap();

    for sample in 0..256 {
        assert_eq!(interleaved[sample * 2], sample as f32);
        assert_eq!(interleaved[sample * 2 + 1], (sample + 1000) as f32);
    }

    let mut round_trip = vec![0.0; 512];
    AudioBufferMut::try_new(&mut round_trip, 2)
        .unwrap()
        .deinterleave(&context, &interleaved)
        .unwrap();
    assert_eq!(round_trip, deinterleaved);
}

#[test]
fn rejects_interleave_length_mismatches() {
    let context = Context::default();
    let source = [0.0; 8];
    let buffer = AudioBufferRef::try_new(&source, 2).unwrap();
    let mut interleaved = [0.0; 7];

    assert_eq!(
        buffer.interleave(&context, &mut interleaved),
        Err(AudioBufferOperationError::InterleaveLengthMismatch {
            dst_len: 7,
            expected_len: 8,
        })
    );

    let mut destination = [0.0; 8];
    let mut buffer = AudioBufferMut::try_new(&mut destination, 2).unwrap();
    assert_eq!(
        buffer.deinterleave(&context, &[0.0; 7]),
        Err(AudioBufferOperationError::DeinterleaveLengthMismatch {
            src_len: 7,
            expected_len: 8,
        })
    );
}

#[test]
fn converts_ambisonics_in_place() {
    let context = Context::default();
    let mut samples = vec![0.5; 4 * 256];
    let mut buffer = AudioBufferMut::try_new(&mut samples, 4).unwrap();

    buffer.convert_ambisonics(&context, AmbisonicsType::N3D, AmbisonicsType::SN3D);
    buffer.convert_ambisonics(&context, AmbisonicsType::SN3D, AmbisonicsType::N3D);
    drop(buffer);

    for sample in samples {
        assert!((sample - 0.5).abs() < 0.01);
    }
}

#[test]
fn converts_ambisonics_into_mutable_view() {
    let context = Context::default();
    let source = vec![0.7; 4 * 256];
    let source_buffer = AudioBufferRef::try_new(&source, 4).unwrap();
    let mut converted = vec![0.0; 4 * 256];

    source_buffer
        .convert_ambisonics_into(
            &context,
            AmbisonicsType::N3D,
            AmbisonicsType::SN3D,
            &mut AudioBufferMut::try_new(&mut converted, 4).unwrap(),
        )
        .unwrap();

    assert_ne!(converted[0], 0.0);
}

#[test]
fn rejects_ambisonics_shape_mismatches_separately() {
    let context = Context::default();
    let source = vec![0.0; 4 * 256];
    let source_buffer = AudioBufferRef::try_new(&source, 4).unwrap();

    let mut wrong_channels = vec![0.0; 2 * 512];
    let error = source_buffer
        .convert_ambisonics_into(
            &context,
            AmbisonicsType::N3D,
            AmbisonicsType::SN3D,
            &mut AudioBufferMut::try_new(&mut wrong_channels, 2).unwrap(),
        )
        .unwrap_err();
    assert_eq!(
        error,
        AudioBufferOperationError::ChannelCountMismatch {
            self_num_channels: 4,
            other_num_channels: 2,
        }
    );

    let mut wrong_samples = vec![0.0; 4 * 128];
    let error = source_buffer
        .convert_ambisonics_into(
            &context,
            AmbisonicsType::N3D,
            AmbisonicsType::SN3D,
            &mut AudioBufferMut::try_new(&mut wrong_samples, 4).unwrap(),
        )
        .unwrap_err();
    assert_eq!(
        error,
        AudioBufferOperationError::SampleCountMismatch {
            self_num_samples: 256,
            other_num_samples: 128,
        }
    );
}
