use super::audio_effect_state::AudioEffectState;
use super::SpeakerLayout;
use crate::audio_buffer::{AudioBuffer, Sample};
use crate::audio_settings::AudioSettings;
use crate::callback::{CallbackInformation, ProgressCallback};
use crate::context::Context;
use crate::error::{to_option_error, SteamAudioError};
use crate::ffi_wrapper::FFIWrapper;
use crate::geometry::{CoordinateSystem, Scene};
use crate::hrtf::Hrtf;
use crate::probe::ProbeBatch;
use crate::simulation::BakedDataIdentifier;

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
        let mut path_effect = Self(std::ptr::null_mut());

        let status = unsafe {
            audionimbus_sys::iplPathEffectCreate(
                context.raw_ptr(),
                &mut audionimbus_sys::IPLAudioSettings::from(audio_settings),
                &mut audionimbus_sys::IPLPathEffectSettings::from(path_effect_settings),
                path_effect.raw_ptr_mut(),
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        Ok(path_effect)
    }

    /// Applies a path effect to an audio buffer.
    ///
    /// This effect CANNOT be applied in-place.
    pub fn apply<I, O>(
        &self,
        path_effect_params: &PathEffectParams,
        input_buffer: &AudioBuffer<I>,
        output_buffer: &AudioBuffer<O>,
    ) -> AudioEffectState
    where
        I: AsRef<[Sample]>,
        O: AsRef<[Sample]> + AsMut<[Sample]>,
    {
        unsafe {
            audionimbus_sys::iplPathEffectApply(
                self.raw_ptr(),
                &mut *path_effect_params.as_ffi(),
                &mut *input_buffer.as_ffi(),
                &mut *output_buffer.as_ffi(),
            )
        }
        .into()
    }

    /// Retrieves a single frame of tail samples from a path effect’s internal buffers.
    ///
    /// After the input to the path effect has stopped, this function must be called instead of [`Self::apply`] until the return value indicates that no more tail samples remain.
    ///
    /// The output audio buffer must have as many channels as needed for the Ambisonics order specified when creating the effect.
    pub fn tail<O>(&self, output_buffer: &AudioBuffer<O>) -> AudioEffectState
    where
        O: AsRef<[Sample]> + AsMut<[Sample]>,
    {
        unsafe {
            audionimbus_sys::iplPathEffectGetTail(self.raw_ptr(), &mut *output_buffer.as_ffi())
        }
        .into()
    }

    /// Returns the number of tail samples remaining in a path effect’s internal buffers.
    ///
    /// Tail samples are audio samples that should be played even after the input to the effect has stopped playing and no further input samples are available.
    pub fn tail_size(&self) -> usize {
        unsafe { audionimbus_sys::iplPathEffectGetTailSize(self.raw_ptr()) as usize }
    }

    /// Resets the internal processing state of a path effect.
    pub fn reset(&mut self) {
        unsafe { audionimbus_sys::iplPathEffectReset(self.raw_ptr()) };
    }

    pub fn raw_ptr(&self) -> audionimbus_sys::IPLPathEffect {
        self.0
    }

    pub fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLPathEffect {
        &mut self.0
    }
}

impl Clone for PathEffect {
    fn clone(&self) -> Self {
        unsafe {
            audionimbus_sys::iplPathEffectRetain(self.0);
        }
        Self(self.0)
    }
}

impl Drop for PathEffect {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplPathEffectRelease(&mut self.0) }
    }
}

unsafe impl Send for PathEffect {}
unsafe impl Sync for PathEffect {}

/// Settings used to create a path effect.
#[derive(Debug)]
pub struct PathEffectSettings<'a> {
    /// The maximum ambisonics order that will be used by output audio buffers.
    pub max_order: usize,

    /// If `Some`, then this effect will render spatialized audio into the output buffer.
    ///
    /// If `None`, this effect will render un-spatialized (and un-rotated) Ambisonic audio.
    /// Setting this to `None` is mainly useful only if you plan to mix multiple Ambisonic buffers and/or apply additional processing to the Ambisonic audio before spatialization.
    /// If you plan to immediately spatialize the output of the path effect, setting this value to `Some` can result in significant performance improvements.
    pub spatialization: Option<Spatialization<'a>>,
}

impl From<&PathEffectSettings<'_>> for audionimbus_sys::IPLPathEffectSettings {
    fn from(settings: &PathEffectSettings) -> Self {
        let (spatialize, speaker_layout, hrtf) = if let Some(Spatialization {
            speaker_layout,
            hrtf,
        }) = &settings.spatialization
        {
            (
                audionimbus_sys::IPLbool::IPL_TRUE,
                speaker_layout.clone(),
                hrtf.raw_ptr(),
            )
        } else {
            (
                audionimbus_sys::IPLbool::IPL_FALSE,
                SpeakerLayout::Mono,
                std::ptr::null_mut(),
            )
        };

        Self {
            maxOrder: settings.max_order as i32,
            spatialize,
            speakerLayout: (&speaker_layout).into(),
            hrtf,
        }
    }
}

/// Spatialization settings.
#[derive(Debug)]
pub struct Spatialization<'a> {
    /// The speaker layout to use when spatializing.
    pub speaker_layout: SpeakerLayout,

    /// The HRTF to use when spatializing.
    pub hrtf: &'a Hrtf,
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
    pub order: usize,

    /// If `true`, spatialize using HRTF-based binaural rendering.
    /// Only used if [`PathEffectSettings::spatialization`] is `Some`.
    pub binaural: bool,

    /// The HRTF to use when spatializing.
    /// Only used if [`PathEffectSettings::spatialization`] is `Some` and [`Self::binaural`] is set to `true`.
    pub hrtf: audionimbus_sys::IPLHRTF,

    /// The position and orientation of the listener.
    /// Only used if [`PathEffectSettings::spatialization`] is `Some` and [`Self::binaural`] is set to `true`.
    pub listener: CoordinateSystem,

    /// If `true`, the values in [`Self::eq_coeffs`] will be normalized before being used, i.e., each value in [`Self::eq_coeffs`] will be divided by the largest value in [`Self::eq_coeffs`].
    /// This can help counteract overly-aggressive filtering due to a physics-based deviation model.
    /// If `false`, the values in [`Self::eq_coeffs`] will be used as-is.
    pub normalize_eq: bool,
}

impl From<audionimbus_sys::IPLPathEffectParams> for PathEffectParams {
    fn from(params: audionimbus_sys::IPLPathEffectParams) -> Self {
        Self {
            eq_coeffs: params.eqCoeffs,
            sh_coeffs: params.shCoeffs,
            order: params.order as usize,
            binaural: params.binaural == audionimbus_sys::IPLbool::IPL_TRUE,
            hrtf: params.hrtf,
            listener: params.listener.into(),
            normalize_eq: params.normalizeEQ == audionimbus_sys::IPLbool::IPL_TRUE,
        }
    }
}

impl PathEffectParams {
    pub(crate) fn as_ffi(&self) -> FFIWrapper<'_, audionimbus_sys::IPLPathEffectParams, Self> {
        let path_effect_params = audionimbus_sys::IPLPathEffectParams {
            eqCoeffs: self.eq_coeffs,
            shCoeffs: self.sh_coeffs,
            order: self.order as i32,
            binaural: if self.binaural {
                audionimbus_sys::IPLbool::IPL_TRUE
            } else {
                audionimbus_sys::IPLbool::IPL_FALSE
            },
            hrtf: self.hrtf,
            listener: self.listener.into(),
            normalizeEQ: if self.normalize_eq {
                audionimbus_sys::IPLbool::IPL_TRUE
            } else {
                audionimbus_sys::IPLbool::IPL_FALSE
            },
        };

        FFIWrapper::new(path_effect_params)
    }
}

/// Bakes a single layer of pathing data in a probe batch.
///
/// Only one bake can be in progress at any point in time.
pub fn bake_path(
    context: &Context,
    path_bake_params: &PathBakeParams,
    progress_callback: Option<CallbackInformation<ProgressCallback>>,
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
            context.raw_ptr(),
            &mut audionimbus_sys::IPLPathBakeParams::from(path_bake_params),
            callback,
            user_data,
        );
    }
}

/// Parameters used to control how pathing data is baked.
#[derive(Debug)]
pub struct PathBakeParams<'a> {
    /// The scene in which the probes exist.
    pub scene: &'a Scene,

    /// A probe batch containing the probes for which pathing data should be baked.
    pub probe_batch: &'a ProbeBatch,

    /// An identifier for the data layer that should be baked.
    /// The identifier determines what data is simulated and stored at each probe.
    /// If the probe batch already contains data with this identifier, it will be overwritten.
    pub identifier: &'a BakedDataIdentifier,

    /// Number of point samples to use around each probe when testing whether one probe can see another.
    /// To determine if two probes are mutually visible, numSamples * numSamples rays are traced, from each point sample of the first probe, to every other point sample of the second probe.
    pub num_samples: usize,

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
    pub num_threads: usize,
}

impl From<&PathBakeParams<'_>> for audionimbus_sys::IPLPathBakeParams {
    fn from(params: &PathBakeParams) -> Self {
        Self {
            scene: params.scene.raw_ptr(),
            probeBatch: params.probe_batch.raw_ptr(),
            identifier: (*params.identifier).into(),
            numSamples: params.num_samples as i32,
            radius: params.radius,
            threshold: params.threshold,
            visRange: params.visibility_range,
            pathRange: params.path_range,
            numThreads: params.num_threads as i32,
        }
    }
}
