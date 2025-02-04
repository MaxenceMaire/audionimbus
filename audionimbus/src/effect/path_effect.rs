use super::audio_effect_state::AudioEffectState;
use super::SpeakerLayout;
use crate::audio_buffer::AudioBuffer;
use crate::audio_settings::AudioSettings;
use crate::context::Context;
use crate::error::{to_option_error, SteamAudioError};
use crate::ffi_wrapper::FFIWrapper;
use crate::geometry::{CoordinateSystem, Scene};
use crate::hrtf::Hrtf;
use crate::probe::ProbeBatch;
use crate::progress_callback::ProgressCallbackInformation;
use crate::simulator::BakedDataIdentifier;

/// Applies the result of simulating sound paths from the source to the listener.
///
/// Multiple paths that sound can take as it propagates from the source to the listener are combined into an Ambisonic sound field.
#[derive(Debug)]
pub struct PathEffect(audionimbus_sys::IPLPathEffect);

impl PathEffect {
    pub fn try_new(
        context: &Context,
        audio_settings: &AudioSettings,
        path_effect_settings: &PathEffectSettings,
    ) -> Result<Self, SteamAudioError> {
        let path_effect = unsafe {
            let path_effect: *mut audionimbus_sys::IPLPathEffect = std::ptr::null_mut();
            let status = audionimbus_sys::iplPathEffectCreate(
                context.as_raw_ptr(),
                &mut audionimbus_sys::IPLAudioSettings::from(audio_settings),
                &mut audionimbus_sys::IPLPathEffectSettings::from(path_effect_settings),
                path_effect,
            );

            if let Some(error) = to_option_error(status) {
                return Err(error);
            }

            *path_effect
        };

        Ok(Self(path_effect))
    }

    /// Applies a path effect to an audio buffer.
    ///
    /// This effect CANNOT be applied in-place.
    pub fn apply(
        &self,
        path_effect_params: &PathEffectParams,
        input_buffer: &mut AudioBuffer,
        output_buffer: &mut AudioBuffer,
    ) -> AudioEffectState {
        unsafe {
            audionimbus_sys::iplPathEffectApply(
                self.as_raw_ptr(),
                &mut *path_effect_params.as_ffi(),
                &mut *input_buffer.as_ffi(),
                &mut *output_buffer.as_ffi(),
            )
        }
        .into()
    }

    pub fn as_raw_ptr(&self) -> audionimbus_sys::IPLPathEffect {
        self.0
    }
}

impl Drop for PathEffect {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplPathEffectRelease(&mut self.0) }
    }
}

/// Settings used to create a path effect.
#[derive(Debug)]
pub struct PathEffectSettings {
    /// The maximum Ambisonics order that will be used by output audio buffers.
    pub max_order: i32,

    /// If `Some`, then this effect will render spatialized audio into the output buffer.
    ///
    /// If `None`, this effect will render un-spatialized (and un-rotated) Ambisonic audio.
    /// Setting this to `None` is mainly useful only if you plan to mix multiple Ambisonic buffers and/or apply additional processing to the Ambisonic audio before spatialization.
    /// If you plan to immediately spatialize the output of the path effect, setting this value to `Some` can result in significant performance improvements.
    pub spatialization: Option<Spatialization>,
}

impl From<&PathEffectSettings> for audionimbus_sys::IPLPathEffectSettings {
    fn from(settings: &PathEffectSettings) -> Self {
        todo!()
    }
}

#[derive(Debug)]
pub struct Spatialization {
    /// The speaker layout to use when spatializing.
    pub speaker_layout: SpeakerLayout,

    /// The HRTF to use when spatializing.
    pub hrtf: Hrtf,
}

/// Parameters for applying a path effect to an audio buffer.
#[derive(Debug)]
pub struct PathEffectParams {
    /// 3-band EQ coefficients for modeling frequency-dependent attenuation caused by paths bending around obstacles.
    pub eq_coeffs: [f32; 3],

    /// Ambisonic coefficients for modeling the directional distribution of sound reaching the listener.
    /// The coefficients are specified in world-space, and must be rotated to match the listener’s orientation separately.
    pub sh_coeffs: *mut f32,

    /// Ambisonic order of the output buffer.
    /// May be less than the maximum order specified when creating the effect, in which case higher-order [`Self::sh_coeffs`] will be ignored, and CPU usage will be reduced.
    pub order: i32,

    /// If `true`, spatialize using HRTF-based binaural rendering.
    /// Only used if [`PathEffectSettings::spatialize`] was set to `true`.
    pub binaural: bool,

    /// The HRTF to use when spatializing.
    /// Only used if [`PathEffectSettings::spatialize`] was set to `true` and [`Self::binaural`] is set to `true`.
    pub hrtf: Hrtf,

    /// The position and orientation of the listener.
    /// Only used if [`PathEffectSettings::spatialize`] was set to `true` and [`Self::binaural`] is set to `true`.
    pub listener: CoordinateSystem,
}

impl PathEffectParams {
    pub(crate) fn as_ffi(&self) -> FFIWrapper<'_, audionimbus_sys::IPLPathEffectParams, Self> {
        todo!()
    }
}

/// Bakes a single layer of pathing data in a probe batch.
///
/// Only one bake can be in progress at any point in time.
pub fn bake_path(
    context: &Context,
    path_bake_params: &PathBakeParams,
    progress_callback: Option<ProgressCallbackInformation>,
) {
    let (callback, user_data) = if let Some(callback_information) = progress_callback {
        (
            Some(callback_information.callback),
            callback_information.user_data,
        )
    } else {
        (None, std::ptr::null_mut())
    };

    unsafe {
        audionimbus_sys::iplPathBakerBake(
            context.as_raw_ptr(),
            &mut audionimbus_sys::IPLPathBakeParams::from(path_bake_params),
            callback,
            user_data,
        );
    }
}

/// Parameters used to control how pathing data is baked.
#[derive(Debug)]
pub struct PathBakeParams {
    /// The scene in which the probes exist.
    pub scene: Scene,

    /// A probe batch containing the probes for which pathing data should be baked.
    pub probe_batch: ProbeBatch,

    /// An identifier for the data layer that should be baked.
    /// The identifier determines what data is simulated and stored at each probe.
    /// If the probe batch already contains data with this identifier, it will be overwritten.
    pub identifier: BakedDataIdentifier,

    /// Number of point samples to use around each probe when testing whether one probe can see another.
    /// To determine if two probes are mutually visible, numSamples * numSamples rays are traced, from each point sample of the first probe, to every other point sample of the second probe.
    pub num_samples: i32,

    /// When testing for mutual visibility between a pair of probes, each probe is treated as a sphere of this radius (in meters), and point samples are generated within this sphere.
    pub radius: f32,

    /// When tracing rays to test for mutual visibility between a pair of probes, the fraction of rays that are unoccluded must be greater than this threshold for the pair of probes to be considered mutually visible.
    pub threshold: f32,

    /// If the distance between two probes is greater than this value, the probes are not considered mutually visible.
    /// Increasing this value can result in simpler paths, at the cost of increased bake times.
    pub visibility_range: f32,

    /// If the length of the path between two probes is greater than this value, the probes are considered to not have any path between them.
    /// Increasing this value allows sound to propagate over greater distances, at the cost of increased bake times and memory usage.
    pub path_range: f32,

    /// Number of threads to use for baking.
    pub num_threads: i32,
}

impl From<&PathBakeParams> for audionimbus_sys::IPLPathBakeParams {
    fn from(params: &PathBakeParams) -> Self {
        todo!()
    }
}
