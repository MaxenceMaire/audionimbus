mod audio_buffer;
pub use audio_buffer::{AudioBuffer, Channel, DeinterleavedChannelSamples, Sample};

mod audio_settings;
pub use audio_settings::AudioSettings;

mod context;
pub use context::{Context, ContextFlags, ContextSettings, SimdLevel};

pub mod effect;
pub use effect::*;

mod error;
pub use error::SteamAudioError;

mod ffi_wrapper;

pub mod geometry;
pub use geometry::*;

mod hrtf;
pub use hrtf::{Hrtf, HrtfInterpolation, HrtfSettings, Sofa, VolumeNormalization};

mod version;
pub use version::{
    STEAMAUDIO_VERSION, STEAMAUDIO_VERSION_MAJOR, STEAMAUDIO_VERSION_MINOR,
    STEAMAUDIO_VERSION_PATCH,
};

mod open_cl;
pub use open_cl::{OpenClDevice, OpenClDeviceList, OpenClDeviceSettings, OpenClDeviceType};

mod radeon_rays;
pub use radeon_rays::RadeonRaysDevice;

mod embree;
pub use embree::EmbreeDevice;

mod true_audio_next;
pub use true_audio_next::TrueAudioNextDevice;

mod serialized_object;
pub use serialized_object::SerializedObject;

mod simulator;
pub use simulator::{
    BakedDataIdentifier, BakedDataVariation, DirectSimulationFlags, Occlusion,
    PathingVisualizationCallback, SimulationFlags, SimulationInputs, SimulationOutputs,
    SimulationSettings, SimulationSharedInputs, Simulator, Source, SourceSettings,
};

mod distance_attenuation;
pub use distance_attenuation::{distance_attenuation, DistanceAttenuationModel};

mod air_absorption;
pub use air_absorption::{air_absorption, AirAbsorptionModel};

mod directivity;
pub use directivity::{directivity_attenuation, Directivity};

mod probe;
pub use probe::{ProbeArray, ProbeBatch, ProbeGenerationParams};

mod progress_callback;
pub use progress_callback::ProgressCallbackInformation;
