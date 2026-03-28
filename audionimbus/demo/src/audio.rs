use crate::consts::{
    AMBISONICS_CHANNELS, AMBISONICS_ORDER, DIRECT_GAIN, IMPULSE_RESPONSE_DURATION,
    REFLECTIONS_GAIN, REVERB_GAIN,
};
use crate::dsp::ToneBurst;
use crate::output::{FRAME_SIZE, NUM_CHANNELS, SAMPLE_RATE};
use crate::simulation::AudioSetup;
use audionimbus::wiring::*;
use audionimbus::*;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

/// Builds the effect chain and starts the CPAL output stream.
pub fn spawn_audio_thread(
    setup: AudioSetup,
    source_angle: Arc<AtomicU32>,
    direct_output: SharedSimulationOutput<Vec<((), DirectEffectParams)>>,
    reflections_reverb_output: SharedSimulationOutput<ReflectionsReverbOutput<(), Convolution>>,
) -> cpal::Stream {
    let AudioSetup {
        context,
        audio_settings,
        hrtf,
    } = setup;
    let (device, stream_config) = crate::output::default_device();

    let mut direct_path = DirectPath::new(&context, &audio_settings, &hrtf);
    let mut reflection_path = ConvolutionPath::new(&context, &audio_settings, &hrtf);
    let mut reverb_path = ConvolutionPath::new(&context, &audio_settings, &hrtf);

    let mut tone_burst = ToneBurst::new(440.0, 0.3, 0.3, 0.9, SAMPLE_RATE);
    let mut dry_buffer = vec![0.0; FRAME_SIZE as usize];

    crate::output::start_output_stream(&device, &stream_config, move |output, _| {
        tone_burst.fill(&mut dry_buffer);
        let dry_audio = AudioBuffer::try_with_data(dry_buffer.as_slice()).unwrap();

        let direct_snapshot = direct_output.load();
        let reflections_snapshot = reflections_reverb_output.load();
        let direct_params = direct_snapshot
            .first()
            .map(|(_, p)| p.clone())
            .unwrap_or_default();
        let source_reflection_params = reflections_snapshot.sources.first().map(|(_, p)| p);
        let listener_reverb_params = reflections_snapshot.listener.as_ref();

        let angle = f32::from_bits(source_angle.load(Ordering::Relaxed));
        direct_path.process(
            &dry_audio,
            &direct_params,
            Direction::new(angle.cos(), 0.0, angle.sin()),
            &hrtf,
        );
        reflection_path.process(&dry_audio, source_reflection_params, &hrtf);
        reverb_path.process(&dry_audio, listener_reverb_params, &hrtf);

        for sample in direct_path.stereo_buffer.iter_mut() {
            *sample *= DIRECT_GAIN;
        }
        crate::dsp::mix_into(
            &mut direct_path.stereo_buffer,
            &reflection_path.stereo_buffer,
            REFLECTIONS_GAIN,
        );
        crate::dsp::mix_into(
            &mut direct_path.stereo_buffer,
            &reverb_path.stereo_buffer,
            REVERB_GAIN,
        );

        AudioBuffer::try_with_data_and_settings(
            &mut direct_path.stereo_buffer,
            AudioBufferSettings {
                num_channels: Some(NUM_CHANNELS),
                ..Default::default()
            },
        )
        .unwrap()
        .interleave(&context, output)
        .unwrap();
    })
}

/// Direct path.
struct DirectPath {
    direct_effect: DirectEffect,
    binaural_effect: BinauralEffect,
    mono_buffer: Vec<Sample>,
    stereo_buffer: Vec<Sample>,
}

impl DirectPath {
    fn new(context: &Context, audio_settings: &AudioSettings, hrtf: &Hrtf) -> Self {
        Self {
            direct_effect: DirectEffect::try_new(
                context,
                audio_settings,
                &DirectEffectSettings { num_channels: 1 },
            )
            .unwrap(),
            binaural_effect: BinauralEffect::try_new(
                context,
                audio_settings,
                &BinauralEffectSettings { hrtf },
            )
            .unwrap(),
            mono_buffer: vec![0.0; FRAME_SIZE as usize],
            stereo_buffer: vec![0.0; (FRAME_SIZE * NUM_CHANNELS) as usize],
        }
    }

    /// Attenuates `dry_buffer` using `params`, then renders to binaural stereo using `direction`.
    ///
    /// Output is written to `self.stereo_buffer`.
    fn process(
        &mut self,
        dry_buffer: &AudioBuffer<&[Sample]>,
        params: &DirectEffectParams,
        direction: Direction,
        hrtf: &Hrtf,
    ) {
        let mono = AudioBuffer::try_with_data(&mut self.mono_buffer).unwrap();
        self.direct_effect.apply(params, dry_buffer, &mono).unwrap();

        let stereo = AudioBuffer::try_with_data_and_settings(
            self.stereo_buffer.as_mut_slice(),
            AudioBufferSettings {
                num_channels: Some(NUM_CHANNELS),
                ..Default::default()
            },
        )
        .unwrap();
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
            .unwrap();
    }
}

/// Convolution path.
///
/// Used for both early reflections (source IR) and late reverb (listener IR).
struct ConvolutionPath {
    reflection_effect: ReflectionEffect<Convolution>,
    decode_effect: AmbisonicsDecodeEffect,
    ambisonics_buffer: Vec<Sample>,
    stereo_buffer: Vec<Sample>,
}

impl ConvolutionPath {
    fn new(context: &Context, audio_settings: &AudioSettings, hrtf: &Hrtf) -> Self {
        Self {
            reflection_effect: ReflectionEffect::<Convolution>::try_new(
                context,
                audio_settings,
                &ReflectionEffectSettings {
                    impulse_response_size: (IMPULSE_RESPONSE_DURATION * SAMPLE_RATE as f32) as u32,
                    num_channels: AMBISONICS_CHANNELS,
                },
            )
            .unwrap(),
            decode_effect: AmbisonicsDecodeEffect::try_new(
                context,
                audio_settings,
                &AmbisonicsDecodeEffectSettings {
                    speaker_layout: SpeakerLayout::Stereo,
                    hrtf,
                    max_order: AMBISONICS_ORDER,
                    rendering: Rendering::Binaural,
                },
            )
            .unwrap(),
            ambisonics_buffer: vec![0.0; (FRAME_SIZE * AMBISONICS_CHANNELS) as usize],
            stereo_buffer: vec![0.0; (FRAME_SIZE * NUM_CHANNELS) as usize],
        }
    }

    /// Convolves `dry_buffer` with the room IR in `params` and decodes to binaural stereo.
    fn process(
        &mut self,
        dry_buffer: &AudioBuffer<&[f32]>,
        params: Option<&ReflectionEffectParams<Convolution>>,
        hrtf: &Hrtf,
    ) {
        let Some(params) = params else {
            self.stereo_buffer.fill(0.0);
            return;
        };

        let ambisonics = AudioBuffer::try_with_data_and_settings(
            self.ambisonics_buffer.as_mut_slice(),
            AudioBufferSettings {
                num_channels: Some(AMBISONICS_CHANNELS),
                ..Default::default()
            },
        )
        .unwrap();
        self.reflection_effect
            .apply(params, dry_buffer, &ambisonics)
            .unwrap();

        self.decode_effect
            .apply(
                &AmbisonicsDecodeEffectParams {
                    order: AMBISONICS_ORDER,
                    hrtf,
                    orientation: CoordinateSystem::default(),
                },
                &ambisonics,
                &AudioBuffer::try_with_data_and_settings(
                    self.stereo_buffer.as_mut_slice(),
                    AudioBufferSettings {
                        num_channels: Some(NUM_CHANNELS),
                        ..Default::default()
                    },
                )
                .unwrap(),
            )
            .unwrap();
    }
}
