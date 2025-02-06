#[test]
fn test_initialization() {
    let context_settings = audionimbus::ContextSettings::default();

    let context_result = audionimbus::Context::try_new(&context_settings);
    assert!(context_result.is_ok());
}

#[test]
fn test_load_hrtf_default() {
    let context_settings = audionimbus::ContextSettings::default();

    let context_result = audionimbus::Context::try_new(&context_settings);
    let context = context_result.unwrap();

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
    let context_settings = audionimbus::ContextSettings::default();

    let context_result = audionimbus::Context::try_new(&context_settings);
    let context = context_result.unwrap();

    let audio_settings = audionimbus::AudioSettings::default();
    let hrtf_settings = audionimbus::HrtfSettings::default();

    let hrtf_result = audionimbus::Hrtf::try_new(&context, &audio_settings, &hrtf_settings);
    let hrtf = hrtf_result.unwrap();

    let binaural_effect_settings = audionimbus::effect::BinauralEffectSettings { hrtf: &hrtf };

    let binaural_effect_result = audionimbus::effect::BinauralEffect::try_new(
        &context,
        &audio_settings,
        &binaural_effect_settings,
    );
    assert!(binaural_effect_result.is_ok());
}
