#![cfg(feature = "dynamic")]

use audionimbus::dynamic::{
    ReflectionAlgorithm, ReflectionEffect, ReflectionError, ReflectionMixer,
};
use audionimbus::{
    AudioBufferMut, AudioSettings, ChannelRequirement, Context, EffectError,
    ReflectionEffectSettings, SteamAudioError,
};

#[test]
fn processes_cpu_tails() {
    let context = Context::default();
    let audio_settings = AudioSettings::default();
    let reflection_effect_settings = ReflectionEffectSettings {
        impulse_response_size: audio_settings.sampling_rate,
        num_channels: 4,
    };

    for algorithm in [
        ReflectionAlgorithm::Convolution,
        ReflectionAlgorithm::Parametric,
        ReflectionAlgorithm::Hybrid,
    ] {
        let reflection_effect: Result<_, SteamAudioError> = ReflectionEffect::try_new(
            &context,
            &audio_settings,
            &reflection_effect_settings,
            algorithm,
        );
        let mut reflection_effect = reflection_effect.unwrap();
        assert_eq!(reflection_effect.algorithm(), algorithm);
        let cloned_effect = reflection_effect.clone();
        drop(reflection_effect);
        reflection_effect = cloned_effect;
        reflection_effect.reset();
        assert_eq!(reflection_effect.tail_size(), 0);
        let mut samples = vec![0.0; 4 * audio_settings.frame_size as usize];
        let mut output_buffer = AudioBufferMut::try_new(&mut samples, 4).unwrap();
        assert!(reflection_effect.tail(&mut output_buffer).is_ok());
    }
}

#[test]
fn rejects_unsupported_mixers() {
    let context = Context::default();
    let audio_settings = AudioSettings::default();
    let reflection_effect_settings = ReflectionEffectSettings {
        impulse_response_size: audio_settings.sampling_rate,
        num_channels: 4,
    };

    for algorithm in [ReflectionAlgorithm::Parametric, ReflectionAlgorithm::Hybrid] {
        assert_eq!(
            ReflectionMixer::try_new(
                &context,
                &audio_settings,
                &reflection_effect_settings,
                algorithm
            )
            .unwrap_err(),
            ReflectionError::MixerUnsupported
        );
        let reflection_effect = ReflectionEffect::try_new(
            &context,
            &audio_settings,
            &reflection_effect_settings,
            algorithm,
        )
        .unwrap();
        let reflection_mixer = ReflectionMixer::try_new(
            &context,
            &audio_settings,
            &reflection_effect_settings,
            ReflectionAlgorithm::Convolution,
        )
        .unwrap();
        let mut samples = vec![0.0; 4 * audio_settings.frame_size as usize];
        let mut output_buffer = AudioBufferMut::try_new(&mut samples, 4).unwrap();
        assert_eq!(
            reflection_effect.tail_into_mixer(&mut output_buffer, &reflection_mixer),
            Err(ReflectionError::MixerUnsupported)
        );
    }
}

#[test]
fn rejects_incompatible_routes() {
    let context = Context::default();
    let audio_settings = AudioSettings::default();
    let reflection_effect_settings = ReflectionEffectSettings {
        impulse_response_size: audio_settings.sampling_rate,
        num_channels: 4,
    };
    let convolution_effect = ReflectionEffect::try_new(
        &context,
        &audio_settings,
        &reflection_effect_settings,
        ReflectionAlgorithm::Convolution,
    )
    .unwrap();
    let tan_effect = ReflectionEffect::try_new(
        &context,
        &audio_settings,
        &reflection_effect_settings,
        ReflectionAlgorithm::TrueAudioNext,
    )
    .unwrap();
    let convolution_mixer = ReflectionMixer::try_new(
        &context,
        &audio_settings,
        &reflection_effect_settings,
        ReflectionAlgorithm::Convolution,
    )
    .unwrap();
    let tan_mixer = ReflectionMixer::try_new(
        &context,
        &audio_settings,
        &reflection_effect_settings,
        ReflectionAlgorithm::TrueAudioNext,
    )
    .unwrap();
    let mut samples = vec![0.0; 4 * audio_settings.frame_size as usize];
    let mut output_buffer = AudioBufferMut::try_new(&mut samples, 4).unwrap();

    assert_eq!(
        tan_effect.tail(&mut output_buffer),
        Err(ReflectionError::DirectApplyUnsupported)
    );
    assert_eq!(
        convolution_effect.tail_into_mixer(&mut output_buffer, &tan_mixer),
        Err(ReflectionError::AlgorithmMismatch {
            expected: ReflectionAlgorithm::Convolution,
            actual: ReflectionAlgorithm::TrueAudioNext
        })
    );
    assert_eq!(
        tan_effect.tail_into_mixer(&mut output_buffer, &convolution_mixer),
        Err(ReflectionError::AlgorithmMismatch {
            expected: ReflectionAlgorithm::TrueAudioNext,
            actual: ReflectionAlgorithm::Convolution
        })
    );
    assert!(
        convolution_effect
            .tail_into_mixer(&mut output_buffer, &convolution_mixer)
            .is_ok()
    );
    assert_eq!(
        tan_mixer.clone().algorithm(),
        ReflectionAlgorithm::TrueAudioNext
    );
}

#[test]
fn preserves_channel_requirements() {
    let context = Context::default();
    let audio_settings = AudioSettings::default();
    let reflection_effect_settings = ReflectionEffectSettings {
        impulse_response_size: audio_settings.sampling_rate,
        num_channels: 4,
    };
    let parametric_effect = ReflectionEffect::try_new(
        &context,
        &audio_settings,
        &reflection_effect_settings,
        ReflectionAlgorithm::Parametric,
    )
    .unwrap();
    let convolution_effect = ReflectionEffect::try_new(
        &context,
        &audio_settings,
        &reflection_effect_settings,
        ReflectionAlgorithm::Convolution,
    )
    .unwrap();
    let mut samples = vec![0.0; audio_settings.frame_size as usize];
    let mut output_buffer = AudioBufferMut::try_new(&mut samples, 1).unwrap();

    assert!(parametric_effect.tail(&mut output_buffer).is_ok());
    assert_eq!(
        convolution_effect.tail(&mut output_buffer),
        Err(ReflectionError::Effect(
            EffectError::InvalidOutputChannels {
                expected: ChannelRequirement::Exactly(4),
                actual: 1
            }
        ))
    );
}
