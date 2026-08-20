use audionimbus::bevy::*;
use bevy::camera_controller::free_camera::FreeCameraPlugin;
use bevy::prelude::*;
use bevy_seedling::platform::{
    AudioStreamConfig,
    cpal::{CpalConfig, CpalOutputConfig},
};
use bevy_seedling::prelude::{RegisterNode, SeedlingPlugins};

mod audio;
mod consts;
mod dsp;
mod scene;

use audio::SpatialNode;
use consts::{AMBISONICS_ORDER, FRAME_SIZE, IMPULSE_RESPONSE_DURATION, SAMPLE_RATE};
use scene::ScenePlugin;

fn main() {
    let audio_settings = AudioSettings {
        sampling_rate: SAMPLE_RATE,
        frame_size: FRAME_SIZE,
    };
    let simulation_settings = SimulationSettings::new(&audio_settings)
        .with_direct(DirectSimulationSettings {
            max_num_occlusion_samples: 32,
        })
        .with_reflections(ConvolutionSettings {
            max_num_rays: 16_384,
            num_diffuse_samples: 128,
            max_duration: IMPULSE_RESPONSE_DURATION,
            max_num_sources: 2,
            num_threads: 2,
            max_order: AMBISONICS_ORDER,
        });

    let stream_config = AudioStreamConfig(CpalConfig {
        output: CpalOutputConfig {
            desired_sample_rate: Some(SAMPLE_RATE),
            desired_block_frames: Some(FRAME_SIZE),
            ..Default::default()
        },
        input: None,
    });

    let mut app = App::new();

    app.insert_resource(stream_config);
    app.add_plugins((
        DefaultPlugins,
        FreeCameraPlugin,
        SeedlingPlugins,
        SpatialAudioPlugin::new(simulation_settings),
        SpatialAudioDebugPlugin::default(),
        ScenePlugin,
    ));

    app.register_simple_node::<SpatialNode>();

    app.run();
}
