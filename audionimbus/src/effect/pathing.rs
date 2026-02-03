//! Sound propagation path effects for navigating around obstacles.

use super::audio_effect_state::AudioEffectState;
use super::{EffectError, SpeakerLayout};
use crate::audio_buffer::{AudioBuffer, Sample};
use crate::audio_settings::AudioSettings;
use crate::context::Context;
use crate::error::{to_option_error, SteamAudioError};
use crate::ffi_wrapper::FFIWrapper;
use crate::geometry::CoordinateSystem;
use crate::hrtf::Hrtf;
use crate::num_ambisonics_channels;
use crate::{ChannelPointers, ChannelRequirement};

#[cfg(doc)]
use crate::baking::PathBaker;
#[cfg(doc)]
use crate::geometry::Scene;
#[cfg(doc)]
use crate::probe::ProbeBatch;
#[cfg(doc)]
use crate::simulation::{SimulationOutputs, Simulator, Source};

/// Applies the result of simulating sound paths from the source to the listener.
///
/// Multiple paths that sound can take as it propagates from the source to the listener are combined into an Ambisonic sound field.
///
/// # Examples
///
/// Applying pathing involves:
/// 1. Baking path data between probes using [`PathBaker::bake`]
/// 2. Setting up a [`Simulator`] with a [`Scene`] and [`ProbeBatch`]
/// 3. Adding [`Source`]s with pathing enabled (don't forget to commit using [`Simulator::commit`]!)
/// 4. Running the simulation ([`Simulator::run_pathing`]) and retrieving the output for the source ([`Source::get_outputs`])
/// 5. Applying the path effect using the simulation output ([`SimulationOutputs::pathing`]) as params
///
/// ```
/// use audionimbus::*;
///
/// let context = Context::default();
///
/// const SAMPLING_RATE: u32 = 48_000;
/// const FRAME_SIZE: u32 = 1024;
/// const MAX_ORDER: u32 = 1;
///
/// let simulation_settings = SimulationSettings::new(SAMPLING_RATE, FRAME_SIZE, MAX_ORDER)
///     .with_pathing(PathingSimulationSettings {
///         num_visibility_samples: 4,
///     });
/// let mut simulator = Simulator::try_new(&context, &simulation_settings)?;
///
/// let mut scene = Scene::try_new(&context)?;
/// let vertices = vec![
///     Point::new(-50.0, 0.0, -50.0),
///     Point::new(50.0, 0.0, -50.0),
///     Point::new(50.0, 0.0, 50.0),
///     Point::new(-50.0, 0.0, 50.0),
/// ];
/// let triangles = vec![Triangle::new(0, 1, 2), Triangle::new(0, 2, 3)];
/// let materials = vec![Material::default()];
/// let material_indices = vec![0, 0];
/// let static_mesh = StaticMesh::try_new(
///     &scene,
///     &StaticMeshSettings {
///         vertices: &vertices,
///         triangles: &triangles,
///         material_indices: &material_indices,
///         materials: &materials,
///     },
/// )?;
/// scene.add_static_mesh(static_mesh);
/// scene.commit();
/// simulator.set_scene(&scene);
///
/// let identifier = BakedDataIdentifier::Pathing {
///     variation: BakedDataVariation::Dynamic,
/// };
///
/// let mut probe_array = ProbeArray::try_new(&context)?;
/// let box_transform = Matrix::new([
///     [100.0, 0.0, 0.0, 0.0],
///     [0.0, 100.0, 0.0, 0.0],
///     [0.0, 0.0, 100.0, 0.0],
///     [0.0, 0.0, 0.0, 1.0],
/// ]);
/// probe_array.generate_probes(
///     &scene,
///     &ProbeGenerationParams::UniformFloor {
///         spacing: 2.0,
///         height: 1.5,
///         transform: box_transform,
///     },
/// );
///
/// let mut probe_batch = ProbeBatch::try_new(&context)?;
/// probe_batch.add_probe_array(&probe_array);
/// probe_batch.commit();
/// simulator.add_probe_batch(&probe_batch);
///
/// let path_bake_params = PathBakeParams {
///     identifier,
///     num_samples: 1, // Trace a single ray to test if one probe can see another probe.
///     visibility_range: 50.0, // Don't check visibility between probes that are > 50m apart.
///     path_range: 100.0, // Don't store paths between probes that are > 100m apart.
///     num_threads: 8,
///     radius: 1.0,
///     threshold: 0.5,
/// };
/// PathBaker::new()
///     .bake(&context, &mut probe_batch, &scene, path_bake_params)
///     .unwrap();
///
/// let source_settings = SourceSettings {
///     flags: SimulationFlags::PATHING,
/// };
/// let mut source = Source::try_new(&simulator, &source_settings)?;
/// let simulation_inputs = SimulationInputs {
///     source: CoordinateSystem::default(),
///     direct_simulation: Some(DirectSimulationParameters {
///         distance_attenuation: Some(DistanceAttenuationModel::default()),
///         air_absorption: Some(AirAbsorptionModel::default()),
///         directivity: Some(Directivity::default()),
///         occlusion: Some(Occlusion {
///             transmission: Some(TransmissionParameters {
///                 num_transmission_rays: 1,
///             }),
///             algorithm: OcclusionAlgorithm::Raycast,
///         }),
///     }),
///     reflections_simulation: Some(ReflectionsSimulationParameters::Convolution {
///         baked_data_identifier: None,
///     }),
///     pathing_simulation: Some(PathingSimulationParameters {
///         pathing_probes: &probe_batch,
///         visibility_radius: 1.0,
///         visibility_threshold: 10.0,
///         visibility_range: 10.0,
///         pathing_order: 1,
///         enable_validation: true,
///         find_alternate_paths: true,
///         deviation: DeviationModel::default(),
///     }),
/// };
/// source.set_inputs(SimulationFlags::PATHING, simulation_inputs);
/// simulator.add_source(&source);
///
/// simulator.commit();
/// simulator.run_pathing();
///
/// let audio_settings = AudioSettings::default();
/// let path_effect_settings = PathEffectSettings {
///     max_order: MAX_ORDER,
///     spatialization: None,
/// };
/// let mut path_effect = PathEffect::try_new(&context, &audio_settings, &path_effect_settings)?;
///
/// let input = vec![0.5; FRAME_SIZE as usize];
/// let input_buffer = AudioBuffer::try_with_data(&input)?;
///
/// // Must have 4 channels (1st order Ambisonics) for this example.
/// const NUM_CHANNELS: u32 = num_ambisonics_channels(1);
/// let mut output_container = vec![0.0; (NUM_CHANNELS * input_buffer.num_samples()) as usize];
/// let output_buffer = AudioBuffer::try_with_data_and_settings(
///     &mut output_container,
///     AudioBufferSettings::with_num_channels(NUM_CHANNELS),
/// )?;
///
/// let simulation_outputs = source.get_outputs(SimulationFlags::PATHING)?;
/// let path_effect_params = simulation_outputs.pathing();
/// let _ = path_effect.apply(&path_effect_params, &input_buffer, &output_buffer);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug)]
pub struct PathEffect {
    inner: audionimbus_sys::IPLPathEffect,

    /// Number of output channels needed for the ambisonics order specified when creating the
    /// effect.
    num_output_channels: u32,
}

impl PathEffect {
    /// Creates a new path effect.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if effect creation fails.
    pub fn try_new(
        context: &Context,
        audio_settings: &AudioSettings,
        path_effect_settings: &PathEffectSettings,
    ) -> Result<Self, SteamAudioError> {
        let mut inner = std::ptr::null_mut();

        let status = unsafe {
            audionimbus_sys::iplPathEffectCreate(
                context.raw_ptr(),
                &mut audionimbus_sys::IPLAudioSettings::from(audio_settings),
                &mut audionimbus_sys::IPLPathEffectSettings::from(path_effect_settings),
                &raw mut inner,
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        let path_effect = Self {
            inner,
            num_output_channels: num_ambisonics_channels(path_effect_settings.max_order),
        };

        Ok(path_effect)
    }

    /// Applies a path effect to an audio buffer.
    ///
    /// This effect CANNOT be applied in-place.
    ///
    /// The input audio buffer must have one channel, and the output audio buffer must have as many
    /// channels as needed for the ambisonics order specified when creating the effect (see
    /// [`num_ambisonics_channels`]).
    ///
    /// # Errors
    ///
    /// Returns [`EffectError`] if:
    /// - The input buffer has more than one channel
    /// - The output buffer has a number of channels different from that needed for the ambisonics
    ///   order specified when creating the effect
    pub fn apply<I, O, PI: ChannelPointers, PO: ChannelPointers>(
        &mut self,
        path_effect_params: &PathEffectParams,
        input_buffer: &AudioBuffer<I, PI>,
        output_buffer: &AudioBuffer<O, PO>,
    ) -> Result<AudioEffectState, EffectError>
    where
        I: AsRef<[Sample]>,
        O: AsRef<[Sample]> + AsMut<[Sample]>,
    {
        let num_input_channels = input_buffer.num_channels();
        if num_input_channels != 1 {
            return Err(EffectError::InvalidInputChannels {
                expected: ChannelRequirement::Exactly(1),
                actual: num_input_channels,
            });
        }

        let num_output_channels = output_buffer.num_channels();
        if num_output_channels != self.num_output_channels {
            return Err(EffectError::InvalidOutputChannels {
                expected: ChannelRequirement::Exactly(self.num_output_channels),
                actual: num_output_channels,
            });
        }

        let state = unsafe {
            audionimbus_sys::iplPathEffectApply(
                self.raw_ptr(),
                &raw mut *path_effect_params.as_ffi(),
                &raw mut *input_buffer.as_ffi(),
                &raw mut *output_buffer.as_ffi(),
            )
        }
        .into();

        Ok(state)
    }

    /// Retrieves a single frame of tail samples from a path effect’s internal buffers.
    ///
    /// After the input to the path effect has stopped, this function must be called instead of [`Self::apply`] until the return value indicates that no more tail samples remain.
    ///
    /// The output audio buffer must have as many channels as needed for the Ambisonics order specified when creating the effect.
    ///
    /// # Errors
    ///
    /// Returns [`EffectError`] if the output buffer has a number of channels different from that
    /// needed for the ambisonics order specified when creating the effect.
    pub fn tail<O>(&self, output_buffer: &AudioBuffer<O>) -> Result<AudioEffectState, EffectError>
    where
        O: AsRef<[Sample]> + AsMut<[Sample]>,
    {
        let num_output_channels = output_buffer.num_channels();
        if num_output_channels != self.num_output_channels {
            return Err(EffectError::InvalidOutputChannels {
                expected: ChannelRequirement::Exactly(self.num_output_channels),
                actual: num_output_channels,
            });
        }

        let state = unsafe {
            audionimbus_sys::iplPathEffectGetTail(self.raw_ptr(), &raw mut *output_buffer.as_ffi())
        }
        .into();

        Ok(state)
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

    /// Returns the raw FFI pointer to the underlying path effect.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub const fn raw_ptr(&self) -> audionimbus_sys::IPLPathEffect {
        self.inner
    }

    /// Returns a mutable reference to the raw FFI pointer.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub const fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLPathEffect {
        &mut self.inner
    }
}

impl Clone for PathEffect {
    fn clone(&self) -> Self {
        unsafe {
            audionimbus_sys::iplPathEffectRetain(self.inner);
        }

        Self {
            inner: self.inner,
            num_output_channels: self.num_output_channels,
        }
    }
}

impl Drop for PathEffect {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplPathEffectRelease(&raw mut self.inner) }
    }
}

unsafe impl Send for PathEffect {}
unsafe impl Sync for PathEffect {}

/// Settings used to create a path effect.
#[derive(Debug)]
pub struct PathEffectSettings<'a> {
    /// The maximum ambisonics order that will be used by output audio buffers.
    pub max_order: u32,

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
    pub sh_coeffs: ShCoeffs,

    /// Ambisonic order of the output buffer.
    /// May be less than the maximum order specified when creating the effect, in which case higher-order [`Self::sh_coeffs`] will be ignored, and CPU usage will be reduced.
    pub order: u32,

    /// If `true`, spatialize using HRTF-based binaural rendering.
    /// Only used if [`PathEffectSettings::spatialization`] is `Some`.
    pub binaural: bool,

    /// The HRTF to use when spatializing.
    /// Only used if [`PathEffectSettings::spatialization`] is `Some` and [`Self::binaural`] is set to `true`.
    pub hrtf: Hrtf,

    /// The position and orientation of the listener.
    /// Only used if [`PathEffectSettings::spatialization`] is `Some` and [`Self::binaural`] is set to `true`.
    pub listener: CoordinateSystem,

    /// If `true`, the values in [`Self::eq_coeffs`] will be normalized before being used, i.e., each value in [`Self::eq_coeffs`] will be divided by the largest value in [`Self::eq_coeffs`].
    /// This can help counteract overly-aggressive filtering due to a physics-based deviation model.
    /// If `false`, the values in [`Self::eq_coeffs`] will be used as-is.
    pub normalize_eq: bool,
}

/// The spherical harmonic coefficients used in [`PathEffectParams`].
/// Do not access these pointers after applying the effect.
#[derive(Debug)]
pub struct ShCoeffs(pub *mut f32);

unsafe impl Send for ShCoeffs {}

impl ShCoeffs {
    pub const fn raw_ptr(&self) -> *mut f32 {
        self.0
    }
}
impl From<audionimbus_sys::IPLPathEffectParams> for PathEffectParams {
    fn from(params: audionimbus_sys::IPLPathEffectParams) -> Self {
        Self {
            eq_coeffs: params.eqCoeffs,
            sh_coeffs: ShCoeffs(params.shCoeffs),
            order: params.order as u32,
            binaural: params.binaural == audionimbus_sys::IPLbool::IPL_TRUE,
            hrtf: params.hrtf.into(),
            listener: params.listener.into(),
            normalize_eq: params.normalizeEQ == audionimbus_sys::IPLbool::IPL_TRUE,
        }
    }
}

impl PathEffectParams {
    pub(crate) fn as_ffi(&self) -> FFIWrapper<'_, audionimbus_sys::IPLPathEffectParams, Self> {
        let path_effect_params = audionimbus_sys::IPLPathEffectParams {
            eqCoeffs: self.eq_coeffs,
            shCoeffs: self.sh_coeffs.raw_ptr(),
            order: self.order as i32,
            binaural: if self.binaural {
                audionimbus_sys::IPLbool::IPL_TRUE
            } else {
                audionimbus_sys::IPLbool::IPL_FALSE
            },
            hrtf: self.hrtf.raw_ptr(),
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

#[cfg(test)]
mod tests {
    use crate::*;

    mod apply {
        use super::*;

        #[test]
        fn test_valid() {
            let context = Context::default();

            const FRAME_SIZE: u32 = 1024;
            const MAX_ORDER: u32 = 1;
            const NUM_SH_COEFFS: usize = (MAX_ORDER as usize + 1).pow(2);

            let audio_settings = AudioSettings::default();
            let path_effect_settings = PathEffectSettings {
                max_order: MAX_ORDER,
                spatialization: None,
            };
            let mut path_effect =
                PathEffect::try_new(&context, &audio_settings, &path_effect_settings).unwrap();

            let input_container = vec![0.5; FRAME_SIZE as usize];
            let input_buffer = AudioBuffer::try_with_data(&input_container).unwrap();

            // Must have 4 channels (1st order Ambisonics) for this example.
            let mut output_container = vec![0.0; 4 * input_buffer.num_samples() as usize];
            let output_buffer = AudioBuffer::try_with_data_and_settings(
                &mut output_container,
                AudioBufferSettings::with_num_channels(4),
            )
            .unwrap();

            let hrtf_settings = HrtfSettings::default();
            let hrtf = Hrtf::try_new(&context, &audio_settings, &hrtf_settings).unwrap();

            let mut sh_storage = vec![0.0f32; NUM_SH_COEFFS];
            sh_storage[0] = 1.0;

            let sh_coeffs = ShCoeffs(sh_storage.as_mut_ptr());

            let path_effect_params = PathEffectParams {
                eq_coeffs: [1.0, 1.0, 1.0],
                sh_coeffs,
                order: MAX_ORDER,
                binaural: false,
                hrtf,
                listener: CoordinateSystem::default(),
                normalize_eq: false,
            };

            assert!(path_effect
                .apply(&path_effect_params, &input_buffer, &output_buffer)
                .is_ok());
        }

        #[test]
        fn test_invalid_input_num_channels() {
            let context = Context::default();

            const FRAME_SIZE: u32 = 1024;
            const MAX_ORDER: u32 = 1;
            const NUM_SH_COEFFS: usize = (MAX_ORDER as usize + 1).pow(2);

            let audio_settings = AudioSettings::default();
            let path_effect_settings = PathEffectSettings {
                max_order: MAX_ORDER,
                spatialization: None,
            };
            let mut path_effect =
                PathEffect::try_new(&context, &audio_settings, &path_effect_settings).unwrap();

            let input_container = vec![0.5; 2 * FRAME_SIZE as usize];
            let input_buffer = AudioBuffer::try_with_data_and_settings(
                &input_container,
                AudioBufferSettings::with_num_channels(2),
            )
            .unwrap();

            // Must have 4 channels (1st order Ambisonics) for this example.
            let mut output_container = vec![0.0; 4 * input_buffer.num_samples() as usize];
            let output_buffer = AudioBuffer::try_with_data_and_settings(
                &mut output_container,
                AudioBufferSettings::with_num_channels(4),
            )
            .unwrap();

            let hrtf_settings = HrtfSettings::default();
            let hrtf = Hrtf::try_new(&context, &audio_settings, &hrtf_settings).unwrap();

            let mut sh_storage = vec![0.0f32; NUM_SH_COEFFS];
            sh_storage[0] = 1.0;

            let sh_coeffs = ShCoeffs(sh_storage.as_mut_ptr());

            let path_effect_params = PathEffectParams {
                eq_coeffs: [1.0, 1.0, 1.0],
                sh_coeffs,
                order: MAX_ORDER,
                binaural: false,
                hrtf,
                listener: CoordinateSystem::default(),
                normalize_eq: false,
            };

            assert_eq!(
                path_effect.apply(&path_effect_params, &input_buffer, &output_buffer),
                Err(EffectError::InvalidInputChannels {
                    expected: ChannelRequirement::Exactly(1),
                    actual: 2
                })
            );
        }

        #[test]
        fn test_invalid_output_num_channels() {
            let context = Context::default();

            const FRAME_SIZE: u32 = 1024;
            const MAX_ORDER: u32 = 1;
            const NUM_SH_COEFFS: usize = (MAX_ORDER as usize + 1).pow(2);

            let audio_settings = AudioSettings::default();
            let path_effect_settings = PathEffectSettings {
                max_order: MAX_ORDER,
                spatialization: None,
            };
            let mut path_effect =
                PathEffect::try_new(&context, &audio_settings, &path_effect_settings).unwrap();

            let input_container = vec![0.5; FRAME_SIZE as usize];
            let input_buffer = AudioBuffer::try_with_data(&input_container).unwrap();

            let mut output_container = vec![0.0; 2 * input_buffer.num_samples() as usize];
            let output_buffer = AudioBuffer::try_with_data_and_settings(
                &mut output_container,
                AudioBufferSettings::with_num_channels(2),
            )
            .unwrap();

            let hrtf_settings = HrtfSettings::default();
            let hrtf = Hrtf::try_new(&context, &audio_settings, &hrtf_settings).unwrap();

            let mut sh_storage = vec![0.0f32; NUM_SH_COEFFS];
            sh_storage[0] = 1.0;

            let sh_coeffs = ShCoeffs(sh_storage.as_mut_ptr());

            let path_effect_params = PathEffectParams {
                eq_coeffs: [1.0, 1.0, 1.0],
                sh_coeffs,
                order: MAX_ORDER,
                binaural: false,
                hrtf,
                listener: CoordinateSystem::default(),
                normalize_eq: false,
            };

            assert_eq!(
                path_effect.apply(&path_effect_params, &input_buffer, &output_buffer),
                Err(EffectError::InvalidOutputChannels {
                    expected: ChannelRequirement::Exactly(4),
                    actual: 2
                })
            );
        }
    }

    mod tail {
        use super::*;

        #[test]
        fn test_valid() {
            let context = Context::default();

            const FRAME_SIZE: usize = 1024;
            const MAX_ORDER: u32 = 1;

            let audio_settings = AudioSettings::default();
            let path_effect_settings = PathEffectSettings {
                max_order: MAX_ORDER,
                spatialization: None,
            };
            let path_effect =
                PathEffect::try_new(&context, &audio_settings, &path_effect_settings).unwrap();

            // Must have 4 channels (1st order Ambisonics) for this example.
            let mut output_container = vec![0.0; 4 * FRAME_SIZE];
            let output_buffer = AudioBuffer::try_with_data_and_settings(
                &mut output_container,
                AudioBufferSettings::with_num_channels(4),
            )
            .unwrap();

            assert!(path_effect.tail(&output_buffer).is_ok());
        }

        #[test]
        fn test_invalid_output_num_channels() {
            let context = Context::default();

            const FRAME_SIZE: usize = 1024;
            const MAX_ORDER: u32 = 1;

            let audio_settings = AudioSettings::default();
            let path_effect_settings = PathEffectSettings {
                max_order: MAX_ORDER,
                spatialization: None,
            };
            let path_effect =
                PathEffect::try_new(&context, &audio_settings, &path_effect_settings).unwrap();

            // Must have 4 channels (1st order Ambisonics) for this example.
            let mut output_container = vec![0.0; 2 * FRAME_SIZE];
            let output_buffer = AudioBuffer::try_with_data_and_settings(
                &mut output_container,
                AudioBufferSettings::with_num_channels(2),
            )
            .unwrap();

            assert_eq!(
                path_effect.tail(&output_buffer),
                Err(EffectError::InvalidOutputChannels {
                    expected: ChannelRequirement::Exactly(4),
                    actual: 2
                })
            );
        }
    }
}
