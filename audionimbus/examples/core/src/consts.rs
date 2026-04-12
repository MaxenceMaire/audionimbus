use audionimbus::num_ambisonics_channels;

/// Ambisonics order.
pub const AMBISONICS_ORDER: u32 = 3;

/// Number of ambisonics channels.
pub const AMBISONICS_CHANNELS: u32 = num_ambisonics_channels(AMBISONICS_ORDER);

/// Height of the listener's ears and the orbiting source above the floor (meters).
pub const LISTENER_HEIGHT: f32 = 1.5;

/// Maximum impulse-response length in seconds.
pub const IMPULSE_RESPONSE_DURATION: f32 = 2.0;

/// Linear gain applied to the binaurally-rendered direct path.
pub const DIRECT_GAIN: f32 = 1.0;

/// Linear gain applied to early source reflections before mixing.
pub const REFLECTIONS_GAIN: f32 = 0.5;

/// Linear gain applied to the reverb before mixing.
pub const REVERB_GAIN: f32 = 0.1;
