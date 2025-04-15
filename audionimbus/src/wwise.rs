use crate::context::Context;
use crate::hrtf::Hrtf;
use crate::simulation::{SimulationSettings, Source};

#[derive(Debug, Copy, Clone)]
/// Settings used for initializing the Steam Audio Wwise integration.
pub struct WwiseSettings {
    /// Scaling factor to apply when converting from game engine units to Steam Audio units (which are in meters).
    pub meters_per_unit: f32,
}

/// Initializes the Wwise integration.
///
/// This function must be called before creating any Steam Audio DSP effects.
pub fn initialize(context: &Context, settings: Option<WwiseSettings>) {
    let mut ffi_settings = settings.map(|s| audionimbus_sys::IPLWwiseSettings {
        metersPerUnit: s.meters_per_unit,
    });

    let ipl_settings = ffi_settings
        .as_mut()
        .map(|s| s as *mut _)
        .unwrap_or(std::ptr::null_mut());

    unsafe { audionimbus_sys::wwise::iplWwiseInitialize(context.raw_ptr(), ipl_settings) }
}

/// Shuts down the Wwise integration.
///
/// This function must be called after all Steam Audio DSP effects have been destroyed.
pub fn terminate() {
    unsafe { audionimbus_sys::wwise::iplWwiseTerminate() }
}

/// Specifies the simulation settings used by the game engine for simulating direct and/or indirect sound propagation.
///
/// This function must be called once during initialization, after [`initialize`].
pub fn set_simulation_settings(simulation_settings: SimulationSettings) {
    unsafe {
        audionimbus_sys::wwise::iplWwiseSetSimulationSettings(
            audionimbus_sys::IPLSimulationSettings::from(simulation_settings),
        )
    }
}

/// Specifies the HRTF to use for spatialization in subsequent audio frames.
///
/// This function must be called once during initialization, after [`initialize`].
/// It should also be called whenever the game engine needs to change the HRTF.
pub fn set_hrtf(hrtf: &Hrtf) {
    unsafe { audionimbus_sys::wwise::iplWwiseSetHRTF(hrtf.raw_ptr()) }
}

/// Wwise game object ID.
pub type WwiseGameObjectId = u64;

/// Specifies the [`Source`] used by the game engine for simulating occlusion, reflections, etc. for the given Wwise game object (identified by its AkGameObjectID).
pub fn add_source(game_object_id: WwiseGameObjectId, source: &Source) {
    unsafe { audionimbus_sys::wwise::iplWwiseAddSource(game_object_id, source.raw_ptr()) }
}

/// Remove any [`Source`] associated the given Wwise game object ID.
///
/// This should be called when the game engine no longer needs to render occlusion, reflections, etc. for the given game object.
pub fn remove_source(game_object_id: WwiseGameObjectId) {
    unsafe { audionimbus_sys::wwise::iplWwiseRemoveSource(game_object_id) }
}

/// Specifies the [`Source`] used by the game engine for simulating reverb.
///
/// Typically, listener-centric reverb is simulated by creating a [`Source`] with the same position as the listener, and simulating reflections.
/// To render this simulated reverb, call this function and pass it the [`Source`] used.
pub fn set_reverb_source(source: &Source) {
    unsafe { audionimbus_sys::wwise::iplWwiseSetReverbSource(source.raw_ptr()) }
}

/// Returns the version of the Wwise integration being used.
pub fn version() -> WwiseIntegrationVersion {
    use std::os::raw::c_uint;

    let mut major: c_uint = 0;
    let mut minor: c_uint = 0;
    let mut patch: c_uint = 0;

    unsafe {
        audionimbus_sys::wwise::iplWwiseGetVersion(&mut major, &mut minor, &mut patch);
    }

    WwiseIntegrationVersion {
        major: major as usize,
        minor: minor as usize,
        patch: patch as usize,
    }
}

/// The version of the Wwise integration.
#[derive(Copy, Clone, Debug)]
pub struct WwiseIntegrationVersion {
    pub major: usize,
    pub minor: usize,
    pub patch: usize,
}
