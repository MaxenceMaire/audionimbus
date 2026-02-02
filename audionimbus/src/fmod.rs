use crate::context::Context;
use crate::hrtf::Hrtf;
use crate::ray_tracing::RayTracer;
use crate::simulation::{SimulationSettings, Source};

/// Initializes the FMOD Studio integration.
///
/// This function must be called before creating any Steam Audio DSP effects.
pub fn initialize(context: &Context) {
    unsafe { audionimbus_sys::fmod::iplFMODInitialize(context.raw_ptr()) }
}

/// Shuts down the FMOD Studio integration.
///
/// This function must be called after all Steam Audio DSP effects have been destroyed.
pub fn terminate() {
    unsafe { audionimbus_sys::fmod::iplFMODTerminate() }
}

/// Specifies the simulation settings used by the game engine for simulating direct and/or indirect sound propagation.
///
/// This function must be called once during initialization, after [`initialize`].
pub fn set_simulation_settings<T: RayTracer>(simulation_settings: &SimulationSettings<'static, T>) {
    unsafe { audionimbus_sys::fmod::iplFMODSetSimulationSettings(simulation_settings.to_ffi()) }
}

/// Specifies the HRTF to use for spatialization in subsequent audio frames.
///
/// This function must be called once during initialization, after [`initialize`].
/// It should also be called whenever the game engine needs to change the HRTF.
pub fn set_hrtf(hrtf: &Hrtf) {
    unsafe { audionimbus_sys::fmod::iplFMODSetHRTF(hrtf.raw_ptr()) }
}

/// Enables or disables HRTF.
pub fn set_hrtf_disabled(disabled: bool) {
    unsafe { audionimbus_sys::fmod::iplFMODSetHRTFDisabled(disabled) }
}

/// A handle to a [`Source`] that can be used in C# scripts.
pub type SourceHandle = i32;

/// Registers a source for use by Steam Audio DSP effects in the audio thread, and returns the corresponding handle.
pub fn add_source(source: &Source) -> SourceHandle {
    unsafe { audionimbus_sys::fmod::iplFMODAddSource(source.raw_ptr()) }
}

/// Unregisters a [`Source`] associated with the given handle, so the Steam Audio DSP effects can no longer use it.
pub fn remove_source(handle: SourceHandle) {
    unsafe { audionimbus_sys::fmod::iplFMODRemoveSource(handle as audionimbus_sys::IPLint32) }
}

/// Specifies the [`Source`] used by the game engine for simulating reverb.
///
/// Typically, listener-centric reverb is simulated by creating a [`Source`] with the same position as the listener, and simulating reflections.
/// To render this simulated reverb, call this function and pass it the [`Source`] used.
pub fn set_reverb_source(source: &Source) {
    unsafe { audionimbus_sys::fmod::iplFMODSetReverbSource(source.raw_ptr()) }
}

/// Returns the version of the FMOD Studio integration being used.
pub fn version() -> FmodStudioIntegrationVersion {
    use std::os::raw::c_uint;

    let mut major: c_uint = 0;
    let mut minor: c_uint = 0;
    let mut patch: c_uint = 0;

    unsafe {
        audionimbus_sys::fmod::iplFMODGetVersion(&mut major, &mut minor, &mut patch);
    }

    FmodStudioIntegrationVersion {
        major: major as usize,
        minor: minor as usize,
        patch: patch as usize,
    }
}

/// The version of the FMOD Studio integration.
#[derive(Copy, Clone, Debug)]
pub struct FmodStudioIntegrationVersion {
    pub major: usize,
    pub minor: usize,
    pub patch: usize,
}
