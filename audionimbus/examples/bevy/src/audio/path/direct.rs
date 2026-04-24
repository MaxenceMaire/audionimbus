use audionimbus::bevy::*;
use bevy::prelude::*;

/// Direct path.
pub struct DirectPath {
    direct_effect: DirectEffect,
    binaural_effect: BinauralEffect,
    mono_buffer: Vec<Sample>,
    stereo_buffer: Vec<Sample>,
}

impl DirectPath {
    pub fn new(context: &Context, audio_settings: &AudioSettings, hrtf: Hrtf) -> Self {
        let frame_size = audio_settings.frame_size as usize;

        let direct_effect = DirectEffect::try_new(
            context,
            audio_settings,
            &DirectEffectSettings { num_channels: 1 },
        )
        .expect("failed to create direct effect");

        let binaural_effect =
            BinauralEffect::try_new(context, audio_settings, &BinauralEffectSettings { hrtf })
                .expect("failed to create binaural effect");

        Self {
            direct_effect,
            binaural_effect,
            mono_buffer: vec![0.0; frame_size],
            stereo_buffer: vec![0.0; frame_size * 2],
        }
    }

    /// Attenuates `dry_buffer` using `params`, then renders to binaural stereo using `direction`.
    ///
    /// Output is written to `self.stereo_buffer`.
    pub fn process(
        &mut self,
        dry_audio: &AudioBuffer<&[Sample]>,
        params: &DirectEffectParams,
        direction: Direction,
        hrtf: Hrtf,
    ) -> &[Sample] {
        let mono = AudioBuffer::try_with_data(self.mono_buffer.as_mut_slice())
            .expect("failed to build mono direct buffer");
        self.direct_effect
            .apply(params, dry_audio, &mono)
            .expect("failed to apply direct effect");

        let stereo = AudioBuffer::try_with_data_and_settings(
            self.stereo_buffer.as_mut_slice(),
            AudioBufferSettings::with_num_channels(2),
        )
        .expect("failed to build stereo direct buffer");

        self.binaural_effect
            .apply(
                &BinauralEffectParams {
                    direction,
                    interpolation: HrtfInterpolation::Bilinear,
                    spatial_blend: 1.0,
                    hrtf,
                    peak_delays: None,
                },
                &mono,
                &stereo,
            )
            .expect("failed to apply binaural effect");

        &self.stereo_buffer
    }
}
