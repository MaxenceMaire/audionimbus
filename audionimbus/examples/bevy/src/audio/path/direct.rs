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
        dry_audio: &impl AudioBufferRead,
        params: &DirectEffectParams,
        direction: Direction,
        hrtf: Hrtf,
    ) -> &[Sample] {
        let mut mono = AudioBufferMut::try_from(self.mono_buffer.as_mut_slice())
            .expect("failed to build mono direct buffer");
        self.direct_effect
            .apply(params, dry_audio, &mut mono)
            .expect("failed to apply direct effect");

        let mut stereo = AudioBufferMut::try_new(self.stereo_buffer.as_mut_slice(), 2)
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
                &mut stereo,
            )
            .expect("failed to apply binaural effect");

        &self.stereo_buffer
    }
}
