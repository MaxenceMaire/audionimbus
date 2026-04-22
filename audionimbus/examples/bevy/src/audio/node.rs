use super::{ConvolutionPath, DirectPath, SharedDirection};
use crate::consts::{
    DIRECT_GAIN, REFLECTIONS_GAIN, REVERB_GAIN, TONE_AMPLITUDE, TONE_FREQUENCY, TONE_OFF_DURATION,
    TONE_ON_DURATION,
};
use crate::dsp::{ToneBurst, mix_into, scale_buffer};
use audionimbus::wiring::{ReflectionsReverbOutput, SharedSimulationOutput};
use audionimbus::{
    AudioBuffer, AudioSettings, Context, Convolution, DirectEffectParams, Hrtf, HrtfSettings,
};
use bevy::prelude::*;
use bevy_seedling::firewheel::{
    StreamInfo,
    channel_config::{ChannelConfig, ChannelCount},
    event::ProcEvents,
    node::{
        AudioNode, AudioNodeInfo, AudioNodeProcessor, ConstructProcessorContext, EmptyConfig,
        ProcBuffers, ProcExtra, ProcInfo, ProcessStatus,
    },
};
use std::collections::HashMap;

#[derive(Component, Clone)]
pub struct SpatialNode {
    entity: Entity,
    context: Context,
    direct_output: SharedSimulationOutput<HashMap<Entity, DirectEffectParams>>,
    reflections_reverb_output: SharedSimulationOutput<ReflectionsReverbOutput<Entity, Convolution>>,
    pub direction: SharedDirection,
}

impl SpatialNode {
    pub fn new(
        entity: Entity,
        context: Context,
        direct_output: SharedSimulationOutput<HashMap<Entity, DirectEffectParams>>,
        reflections_reverb_output: SharedSimulationOutput<
            ReflectionsReverbOutput<Entity, Convolution>,
        >,
        direction: SharedDirection,
    ) -> Self {
        Self {
            entity,
            context,
            direct_output,
            reflections_reverb_output,
            direction,
        }
    }
}

impl AudioNode for SpatialNode {
    type Configuration = EmptyConfig;

    fn info(&self, _config: &Self::Configuration) -> AudioNodeInfo {
        AudioNodeInfo::new()
            .debug_name("audionimbus_spatial_node")
            .channel_config(ChannelConfig {
                num_inputs: ChannelCount::ZERO,
                num_outputs: ChannelCount::STEREO,
            })
    }

    fn construct_processor(
        &self,
        _config: &Self::Configuration,
        cx: ConstructProcessorContext,
    ) -> impl AudioNodeProcessor {
        SpatialProcessor::new(
            self.entity,
            self.context.clone(),
            self.direct_output.clone(),
            self.reflections_reverb_output.clone(),
            self.direction.clone(),
            cx.stream_info,
        )
    }
}

struct SpatialProcessor {
    entity: Entity,
    context: Context,
    direct_output: SharedSimulationOutput<HashMap<Entity, DirectEffectParams>>,
    reflections_reverb_output: SharedSimulationOutput<ReflectionsReverbOutput<Entity, Convolution>>,
    direction: SharedDirection,
    hrtf: Hrtf,
    direct_path: DirectPath,
    reflections_path: ConvolutionPath,
    reverb_path: ConvolutionPath,
    tone: ToneBurst,
    dry_buffer: Vec<f32>,
    mix_buffer: Vec<f32>,
    frame_size: usize,
}

impl SpatialProcessor {
    fn new(
        entity: Entity,
        context: Context,
        direct_output: SharedSimulationOutput<HashMap<Entity, DirectEffectParams>>,
        reflections_reverb_output: SharedSimulationOutput<
            ReflectionsReverbOutput<Entity, Convolution>,
        >,
        direction: SharedDirection,
        stream_info: &StreamInfo,
    ) -> Self {
        let audio_settings = AudioSettings {
            sampling_rate: stream_info.sample_rate.get(),
            frame_size: stream_info.max_block_frames.get(),
        };
        let hrtf = Hrtf::try_new(&context, &audio_settings, &HrtfSettings::default())
            .expect("failed to create HRTF");
        let frame_size = audio_settings.frame_size as usize;

        Self {
            entity,
            context: context.clone(),
            direct_output,
            reflections_reverb_output,
            direction,
            direct_path: DirectPath::new(&context, &audio_settings, hrtf.clone()),
            reflections_path: ConvolutionPath::new(&context, &audio_settings, hrtf.clone()),
            reverb_path: ConvolutionPath::new(&context, &audio_settings, hrtf.clone()),
            hrtf,
            tone: ToneBurst::new(
                TONE_FREQUENCY,
                TONE_AMPLITUDE,
                TONE_ON_DURATION,
                TONE_OFF_DURATION,
                stream_info.sample_rate.get(),
            ),
            dry_buffer: vec![0.0; frame_size],
            mix_buffer: vec![0.0; frame_size * 2],
            frame_size,
        }
    }

    fn rebuild_for_stream(&mut self, stream_info: &StreamInfo) {
        *self = Self::new(
            self.entity,
            self.context.clone(),
            self.direct_output.clone(),
            self.reflections_reverb_output.clone(),
            self.direction.clone(),
            stream_info,
        );
    }
}

impl AudioNodeProcessor for SpatialProcessor {
    fn process(
        &mut self,
        info: &ProcInfo,
        ProcBuffers { outputs, .. }: ProcBuffers,
        _events: &mut ProcEvents,
        _extra: &mut ProcExtra,
    ) -> ProcessStatus {
        let frames = info.frames;
        self.dry_buffer[..frames].fill(0.0);
        self.tone.fill(&mut self.dry_buffer[..frames]);

        if frames < self.frame_size {
            self.dry_buffer[frames..].fill(0.0);
        }

        let dry_audio = AudioBuffer::try_with_data(self.dry_buffer.as_slice()).unwrap();

        let direct_snapshot = self.direct_output.load();
        let direct_params = direct_snapshot
            .get(&self.entity)
            .cloned()
            .unwrap_or_default();

        let reflections_snapshot = self.reflections_reverb_output.load();
        let source_reflections = reflections_snapshot.sources.get(&self.entity);
        let listener_reverb = reflections_snapshot.listener.as_ref();

        let direct = self.direct_path.process(
            &dry_audio,
            &direct_params,
            self.direction.load(),
            self.hrtf.clone(),
        );
        let reflections =
            self.reflections_path
                .process(&dry_audio, source_reflections, self.hrtf.clone());
        let reverb = self
            .reverb_path
            .process(&dry_audio, listener_reverb, self.hrtf.clone());

        self.mix_buffer.copy_from_slice(direct);
        scale_buffer(&mut self.mix_buffer, DIRECT_GAIN);
        mix_into(&mut self.mix_buffer, reflections, REFLECTIONS_GAIN);
        mix_into(&mut self.mix_buffer, reverb, REVERB_GAIN);

        outputs[0].copy_from_slice(&self.mix_buffer[..frames]);
        outputs[1].copy_from_slice(&self.mix_buffer[self.frame_size..self.frame_size + frames]);

        ProcessStatus::OutputsModified
    }

    fn new_stream(
        &mut self,
        stream_info: &StreamInfo,
        _context: &mut bevy_seedling::firewheel::node::ProcStreamCtx,
    ) {
        self.rebuild_for_stream(stream_info);
    }
}
