use audionimbus::*;

const SOFA_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../audionimbus-sys/steam-audio/core/data/hrtf/sadie_h12.sofa"
);

#[test]
fn test_load_hrtf_sofa_filename() {
    let context = Context::default();
    let audio_settings = AudioSettings::default();

    let hrtf_settings = HrtfSettings {
        volume: 1.0,
        sofa_information: Some(Sofa::Filename(SOFA_PATH.to_string())),
        volume_normalization: VolumeNormalization::None,
    };

    let result = Hrtf::try_new(&context, &audio_settings, &hrtf_settings);
    assert!(result.is_ok());

    // Test with non-existent file.
    let hrtf_settings = HrtfSettings {
        volume: 1.0,
        sofa_information: Some(Sofa::Filename("nonexistent.sofa".to_string())),
        volume_normalization: VolumeNormalization::None,
    };

    let result = Hrtf::try_new(&context, &audio_settings, &hrtf_settings);
    assert!(result.is_err());
}

#[test]
fn test_load_hrtf_sofa_buffer() {
    let context = Context::default();
    let audio_settings = AudioSettings::default();

    let buffer = std::fs::read(SOFA_PATH).expect("failed to read SOFA file");

    let hrtf_settings = HrtfSettings {
        volume: 1.0,
        sofa_information: Some(Sofa::Buffer(buffer)),
        volume_normalization: VolumeNormalization::None,
    };

    let result = Hrtf::try_new(&context, &audio_settings, &hrtf_settings);
    assert!(result.is_ok());

    // Test with empty buffer (should fail).
    let hrtf_settings = HrtfSettings {
        volume: 1.0,
        sofa_information: Some(Sofa::Buffer(vec![])),
        volume_normalization: VolumeNormalization::None,
    };

    let result = Hrtf::try_new(&context, &audio_settings, &hrtf_settings);
    assert!(result.is_err());

    // Test with invalid SOFA data.
    let hrtf_settings = HrtfSettings {
        volume: 1.0,
        sofa_information: Some(Sofa::Buffer(vec![0u8; 1024])),
        volume_normalization: VolumeNormalization::None,
    };

    let result = Hrtf::try_new(&context, &audio_settings, &hrtf_settings);
    assert!(result.is_err());
}

#[test]
fn test_load_hrtf_with_volume_normalization() {
    let context = Context::default();
    let audio_settings = AudioSettings::default();

    // Test RMS normalization.
    let hrtf_settings = HrtfSettings {
        volume: 0.5,
        sofa_information: Some(Sofa::Filename(SOFA_PATH.to_string())),
        volume_normalization: VolumeNormalization::RootMeanSquared,
    };

    let result = Hrtf::try_new(&context, &audio_settings, &hrtf_settings);
    assert!(result.is_ok());

    // Test no normalization.
    let hrtf_settings = HrtfSettings {
        volume: 1.0,
        sofa_information: Some(Sofa::Filename(SOFA_PATH.to_string())),
        volume_normalization: VolumeNormalization::None,
    };

    let result = Hrtf::try_new(&context, &audio_settings, &hrtf_settings);
    assert!(result.is_ok());
}
