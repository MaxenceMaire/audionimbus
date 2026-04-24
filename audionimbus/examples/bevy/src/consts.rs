use audionimbus::bevy::*;

/// Sample rate.
pub const SAMPLE_RATE: u32 = 48_000;

/// Frame size.
pub const FRAME_SIZE: u32 = 1024;

/// Room size in meters.
pub const ROOM_SIZE: f32 = 20.0;

/// Ambisonics order.
pub const AMBISONICS_ORDER: u32 = 3;

/// Number of ambisonics channels.
pub const AMBISONICS_CHANNELS: u32 = num_ambisonics_channels(AMBISONICS_ORDER);

/// Height of the listener's ears and the orbiting source above the floor (meters).
pub const LISTENER_HEIGHT: f32 = 1.8;

/// Maximum impulse-response length in seconds.
pub const IMPULSE_RESPONSE_DURATION: f32 = 2.0;

/// Linear gain applied to the binaurally-rendered direct path.
pub const DIRECT_GAIN: f32 = 1.0;

/// Linear gain applied to early source reflections before mixing.
pub const REFLECTIONS_GAIN: f32 = 0.3;

/// Linear gain applied to the reverb before mixing.
pub const REVERB_GAIN: f32 = 0.1;

/// Frequency of the tone.
pub const TONE_FREQUENCY: f32 = 440.0;

/// Amplitude of the tone.
pub const TONE_AMPLITUDE: f32 = 1.0;

/// Duration of the tone.
pub const TONE_ON_DURATION: f32 = 0.3;

/// Duration of the pause between tones.
pub const TONE_OFF_DURATION: f32 = 0.9;
