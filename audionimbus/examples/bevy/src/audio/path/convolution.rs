use crate::consts::{AMBISONICS_CHANNELS, AMBISONICS_ORDER, IMPULSE_RESPONSE_DURATION};
use audionimbus::{
    AmbisonicsDecodeEffect, AmbisonicsDecodeEffectParams, AmbisonicsDecodeEffectSettings,
    AudioBuffer, AudioBufferSettings, AudioSettings, Context, Convolution, CoordinateSystem, Hrtf,
    ReflectionEffect, ReflectionEffectParams, ReflectionEffectSettings, Rendering, Sample,
    SpeakerLayout,
};
use bevy::prelude::*;

/// Convolution path.
///
/// Used for both early reflections (source IR) and late reverb (listener IR).
pub struct ConvolutionPath {
    reflection_effect: ReflectionEffect<Convolution>,
    decode_effect: AmbisonicsDecodeEffect,
    ambisonics_buffer: Vec<Sample>,
    stereo_buffer: Vec<Sample>,
}

impl ConvolutionPath {
    pub fn new(context: &Context, audio_settings: &AudioSettings, hrtf: Hrtf) -> Self {
        let frame_size = audio_settings.frame_size as usize;

        let reflection_effect = ReflectionEffect::<Convolution>::try_new(
            context,
            audio_settings,
            &ReflectionEffectSettings {
                impulse_response_size: (IMPULSE_RESPONSE_DURATION
                    * audio_settings.sampling_rate as f32)
                    as u32,
                num_channels: AMBISONICS_CHANNELS,
            },
        )
        .expect("failed to create reflection effect");

        let decode_effect = AmbisonicsDecodeEffect::try_new(
            context,
            audio_settings,
            &AmbisonicsDecodeEffectSettings {
                speaker_layout: SpeakerLayout::Stereo,
                hrtf,
                max_order: AMBISONICS_ORDER,
                rendering: Rendering::Binaural,
            },
        )
        .expect("failed to create ambisonics decode effect");

        Self {
            reflection_effect,
            decode_effect,
            ambisonics_buffer: vec![0.0; frame_size * AMBISONICS_CHANNELS as usize],
            stereo_buffer: vec![0.0; frame_size * 2],
        }
    }

    /// Convolves `dry_buffer` with the room IR in `params` and decodes to binaural stereo.
    pub fn process(
        &mut self,
        dry_audio: &AudioBuffer<&[Sample]>,
        params: Option<&ReflectionEffectParams<Convolution>>,
        hrtf: Hrtf,
    ) -> &[Sample] {
        let Some(params) = params else {
            self.stereo_buffer.fill(0.0);
            return &[];
        };

        let ambisonics = AudioBuffer::try_with_data_and_settings(
            self.ambisonics_buffer.as_mut_slice(),
            AudioBufferSettings::with_num_channels(AMBISONICS_CHANNELS),
        )
        .expect("failed to build ambisonics buffer");
        self.reflection_effect
            .apply(params, dry_audio, &ambisonics)
            .expect("failed to apply reflection effect");

        let stereo = AudioBuffer::try_with_data_and_settings(
            self.stereo_buffer.as_mut_slice(),
            AudioBufferSettings::with_num_channels(2),
        )
        .expect("failed to build stereo reflection buffer");
        self.decode_effect
            .apply(
                &AmbisonicsDecodeEffectParams {
                    order: AMBISONICS_ORDER,
                    hrtf,
                    orientation: CoordinateSystem::default(),
                },
                &ambisonics,
                &stereo,
            )
            .expect("failed to decode ambisonics reflections");

        &self.stereo_buffer
    }
}
