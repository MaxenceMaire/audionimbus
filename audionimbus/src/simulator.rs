use crate::context::Context;
use crate::effect::{DirectEffectParams, PathEffectParams, ReflectionEffectParams};
use crate::error::{to_option_error, SteamAudioError};
use crate::geometry;
use crate::geometry::Scene;
use crate::geometry::SceneType;
use crate::open_cl::OpenClDevice;
use crate::radeon_rays::RadeonRaysDevice;
use crate::ProbeBatch;

/// Manages direct and indirect sound propagation simulation for multiple sources.
///
/// Your application will typically create one simulator object and use it to run simulations with different source and listener parameters between consecutive simulation runs.
/// The simulator can also be reused across scene changes.
#[derive(Debug)]
pub struct Simulator(audionimbus_sys::IPLSimulator);

impl Simulator {
    pub fn try_new(
        context: &Context,
        simulation_settings: &SimulationSettings,
    ) -> Result<Self, SteamAudioError> {
        let simulator = unsafe {
            let simulator: *mut audionimbus_sys::IPLSimulator = std::ptr::null_mut();
            let status = audionimbus_sys::iplSimulatorCreate(
                context.raw_ptr(),
                &mut audionimbus_sys::IPLSimulationSettings::from(simulation_settings),
                simulator,
            );

            if let Some(error) = to_option_error(status) {
                return Err(error);
            }

            *simulator
        };

        Ok(Self(simulator))
    }

    /// Specifies the scene within which all subsequent simulations should be run.
    ///
    /// Call [`Self::commit`] after calling this function for the changes to take effect.
    ///
    /// This function cannot be called while any simulation is running.
    pub fn set_scene(&self, scene: &Scene) {
        unsafe { audionimbus_sys::iplSimulatorSetScene(self.raw_ptr(), scene.raw_ptr()) }
    }

    /// Adds a probe batch for use in subsequent simulations.
    /// Sources that require baked data can then use the data contained in the specified probe batch.
    ///
    /// Call [`Self::commit`] after calling this function for the changes to take effect.
    ///
    /// This function cannot be called while any simulation is running.
    pub fn add_probe_batch(&self) {
        todo!()
    }

    /// Removes a probe batch from use in subsequent simulations.
    /// Sources that require baked data will then stop using the data contained in the specified probe batch.
    ///
    /// Call [`Self::commit`] after calling this function for the changes to take effect.
    ///
    /// This function cannot be called while any simulation is running.
    pub fn remove_probe_batch(&self) {
        todo!()
    }

    /// Adds a source to the set of sources processed by a simulator in subsequent simulations.
    pub fn add_source(&self, source: &Source) {
        unsafe {
            audionimbus_sys::iplSourceAdd(source.raw_ptr(), self.raw_ptr());
        }
    }

    /// Removes a source from the set of sources processed by a simulator in subsequent simulations.
    pub fn remove_source(&self, source: &Source) {
        unsafe {
            audionimbus_sys::iplSourceRemove(source.raw_ptr(), self.raw_ptr());
        }
    }

    /// Commits changes to the scene or probe batches used for simulation.
    ///
    /// Call this function after calling [`Self::set_scene`], [`Self::add_probe_batch`], or [`Self::remove_probe_batch`] for the changes to take effect.
    ///
    /// This function cannot be called while any simulation is running.
    pub fn commit(&self) {
        unsafe { audionimbus_sys::iplSimulatorCommit(self.raw_ptr()) }
    }

    /// Specifies simulation parameters that are not associated with any particular source.
    ///
    /// # Arguments
    ///
    /// - `flags`: the types of simulation for which to specify shared inputs. If, for example, direct and reflections simulations are being run on separate threads, you can call this function on the direct simulation thread with [`SimulationFlags::DIRECT`], and on the reflections simulation thread with [`SimulationFlags::REFLECTIONS`], without requiring any synchronization between the calls.
    /// - `shared_inputs`: the shared input parameters to set.
    pub fn set_shared_inputs(
        &self,
        simulation_flags: SimulationFlags,
        shared_inputs: &SimulationSharedInputs,
    ) {
        unsafe {
            audionimbus_sys::iplSimulatorSetSharedInputs(
                self.raw_ptr(),
                simulation_flags.into(),
                &mut audionimbus_sys::IPLSimulationSharedInputs::from(shared_inputs),
            );
        }
    }

    /// Runs a direct simulation for all sources added to the simulator.
    /// This may include distance attenuation, air absorption, directivity, occlusion, and transmission.
    ///
    /// This function should not be called from the audio processing thread if occlusion and/or transmission are enabled.
    pub fn run_direct(&self) {
        unsafe {
            audionimbus_sys::iplSimulatorRunDirect(self.raw_ptr());
        }
    }

    /// Runs a reflections simulation for all sources added to the simulator.
    ///
    /// This function can be CPU intensive, and should be called from a separate thread in order to not block either the audio processing thread or the game’s main update thread.
    pub fn run_reflections(&self) {
        unsafe {
            audionimbus_sys::iplSimulatorRunReflections(self.raw_ptr());
        }
    }

    /// Runs a pathing simulation for all sources added to the simulator.
    ///
    /// This function can be CPU intensive, and should be called from a separate thread in order to not block either the audio processing thread or the game’s main update thread.
    pub fn run_pathing(&self) {
        unsafe {
            audionimbus_sys::iplSimulatorRunPathing(self.raw_ptr());
        }
    }

    pub fn raw_ptr(&self) -> audionimbus_sys::IPLSimulator {
        self.0
    }
}

impl Drop for Simulator {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplSimulatorRelease(&mut self.0) }
    }
}

/// Settings used to create a simulator.
#[derive(Debug)]
pub struct SimulationSettings {
    /// The types of simulation that this simulator will be used for.
    pub flags: SimulationFlags,

    /// The type of scene that will be used for simulations via [`Simulator::set_scene`].
    /// The scene type cannot change during the lifetime of a simulator object.
    pub scene_type: SceneType,

    /// The type of reflections effect that will be used to render the results of reflections simulation.
    /// The reflections effect type cannot change during the lifetime of a simulator object.
    pub reflection_type: ReflectionEffect,

    /// The maximum number of point samples to consider when calculating occlusion using the volumetric occlusion algorithm.
    /// Different sources can use different numbers of samples, and the number of samples can change between simulation runs, but this is the maximum value.
    /// Increasing this value results in smoother occlusion transitions, at the cost of increased CPU usage.
    pub max_num_occlusion_samples: i32,

    /// The maximum number of rays to trace from the listener when simulating reflections.
    /// You can use different numbers of rays between simulation runs, but this is the maximum value.
    /// Increasing this value results in more accurate reflections, at the cost of increased CPU usage.
    pub max_num_rays: i32,

    /// The number of directions to sample when generating diffusely reflected rays.
    /// Increasing this value may increase the accuracy of diffuse reflections.
    pub num_diffuse_samples: i32,

    /// The maximum length (in seconds) of impulse responses generated by reflection simulations.
    /// You can change this value betweeen simulation runs, but this is the maximum value.
    /// Increasing this value results in longer, more accurate reverb tails, at the cost of increased CPU and memory usage.
    pub max_duration: f32,

    /// The maximum Ambisonic order of impulse responses generated by reflection simulations.
    /// You can change this value between simulation runs, but this is the maximum value.
    /// Increasing this value results in more accurate directional variations in the impulse responses, at the cost of increased CPU and memory usage.
    pub max_order: i32,

    /// The maximum number of sources for which reflection simulations will be run at any given time.
    pub max_num_sources: i32,

    /// The number of threads used for real-time reflection simulations.
    pub num_threads: i32,

    /// If using custom ray tracer callbacks, this the number of rays that will be passed to the callbacks every time rays need to be traced.
    pub ray_batch_size: i32,

    /// The number of point samples to consider when calculating probe-to-probe visibility for pathing simulations.
    /// Baked paths may end up being occluded by dynamic objects, in which case you can configure the simulator to look for alternate paths in real time.
    /// This process will involve checking visibility between probes.
    pub num_vis_samples: i32,

    /// The sampling rate (in Hz) used for audio processing.
    pub sampling_rate: i32,

    /// The size (in samples) of the audio buffers used for audio processing.
    pub frame_size: i32,

    /// The OpenCL device being used.
    /// Only necessary if [`Self::scene_type`] is [`SceneType::RadeonRays`], or [`Self::reflection_type`] is [`ReflectionEffect::TrueAudioNext`].
    pub open_cl_device: OpenClDevice,

    /// The Radeon Rays device being used. Only necessary if [`Self::scene_type`] is [`SceneType::RadeonRays`].
    pub radeon_rays_device: RadeonRaysDevice,
}

impl From<&SimulationSettings> for audionimbus_sys::IPLSimulationSettings {
    fn from(settings: &SimulationSettings) -> Self {
        todo!()
    }
}

bitflags::bitflags! {
    /// Flags indicating which types of simulation should be enabled.
    #[derive(Debug)]
    pub struct SimulationFlags: u32 {
        /// Enable direct simulation.
        /// This includes distance attenuation, air absorption, directivity, occlusion, and transmission.
        const DIRECT = 1 << 0;

        /// Enable reflections simulation.
        /// This includes both real-time and baked simulation.
        const REFLECTIONS = 1 << 1;

        /// Enable pathing simulation.
        const PATHING = 1 << 2;
    }
}

impl From<SimulationFlags> for audionimbus_sys::IPLSimulationFlags {
    fn from(simulation_flags: SimulationFlags) -> Self {
        Self(simulation_flags.bits())
    }
}

/// Type of reflection effect algorithm to use.
#[derive(Debug)]
pub enum ReflectionEffect {
    /// Multi-channel convolution reverb.
    /// Reflections reaching the listener are encoded in an Impulse Response (IR), which is a filter that records each reflection as it arrives.
    /// This algorithm renders reflections with the most detail, but may result in significant CPU usage.
    /// Using a reflection mixer with this algorithm provides a reduction in CPU usage.
    Convolution,

    /// Parametric (or artificial) reverb, using feedback delay networks.
    /// The reflected sound field is reduced to a few numbers that describe how reflected energy decays over time.
    /// This is then used to drive an approximate model of reverberation in an indoor space.
    /// This algorithm results in lower CPU usage, but cannot render individual echoes, especially in outdoor spaces.
    /// A reflection mixer cannot be used with this algorithm.
    Parametric,

    /// A hybrid of convolution and parametric reverb.
    /// The initial portion of the IR is rendered using convolution reverb, but the later part is used to estimate a parametric reverb.
    /// The point in the IR where this transition occurs can be controlled.
    /// This algorithm allows a trade-off between rendering quality and CPU usage.
    /// An reflection mixer cannot be used with this algorithm.
    Hybrid,

    /// Multi-channel convolution reverb, using AMD TrueAudio Next for GPU acceleration.
    /// This algorithm is similar to [`Self::Convolution`], but uses the GPU instead of the CPU for processing, allowing significantly more sources to be processed.
    /// A reflection mixer must be used with this algorithm, because the GPU will process convolution reverb at a single point in your audio processing pipeline.
    TrueAudioNext,
}

/// A sound source, for the purposes of simulation.
///
/// This object is used to specify various parameters for direct and indirect sound propagation simulation, and to retrieve the simulation results.
#[derive(Debug)]
pub struct Source(audionimbus_sys::IPLSource);

impl Source {
    pub fn try_new(
        simulator: &Simulator,
        source_settings: &SourceSettings,
    ) -> Result<Self, SteamAudioError> {
        let source = unsafe {
            let source: *mut audionimbus_sys::IPLSource = std::ptr::null_mut();
            let status = audionimbus_sys::iplSourceCreate(
                simulator.raw_ptr(),
                &mut audionimbus_sys::IPLSourceSettings::from(source_settings),
                source,
            );

            if let Some(error) = to_option_error(status) {
                return Err(error);
            }

            *source
        };

        Ok(Self(source))
    }

    /// Specifies simulation parameters for a source.
    ///
    /// # Arguments
    ///
    /// - `flags`: the types of simulation for which to specify inputs. If, for example, direct and reflections simulations are being run on separate threads, you can call this function on the direct simulation thread with [`SimulationFlags::DIRECT`], and on the reflections simulation thread with [`SimulationFlags::REFLECTIONS`], without requiring any synchronization between the calls.
    /// - `inputs`: the input parameters to set.
    pub fn set_inputs(&self, simulation_flags: SimulationFlags, inputs: &SimulationInputs) {
        unsafe {
            audionimbus_sys::iplSourceSetInputs(
                self.raw_ptr(),
                simulation_flags.into(),
                &mut inputs.into(),
            );
        }
    }

    /// Retrieves simulation results for a source.
    ///
    /// # Arguments
    ///
    /// - `flags`: the types of simulation for which to retrieve results.
    pub fn get_outputs(&self, simulation_flags: SimulationFlags) -> SimulationOutputs {
        let simulation_outputs = unsafe {
            let simulation_outputs: *mut audionimbus_sys::IPLSimulationOutputs =
                std::ptr::null_mut();
            audionimbus_sys::iplSourceGetOutputs(
                self.raw_ptr(),
                simulation_flags.into(),
                simulation_outputs,
            );

            *simulation_outputs
        };

        simulation_outputs.into()
    }

    pub fn raw_ptr(&self) -> audionimbus_sys::IPLSource {
        self.0
    }
}

impl Drop for Source {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplSourceRelease(&mut self.0) }
    }
}

/// Settings used to create a source.
#[derive(Debug)]
pub struct SourceSettings {
    /// The types of simulation that may be run for this source.
    flags: SimulationFlags,
}

impl From<&SourceSettings> for audionimbus_sys::IPLSourceSettings {
    fn from(settings: &SourceSettings) -> Self {
        todo!()
    }
}

/// Simulation parameters for a source.
#[derive(Debug)]
pub struct SimulationInputs {
    /// The types of simulation to run for this source.
    pub flags: SimulationFlags,

    /// The types of direct simulation to run for this source.
    pub direct_flags: DirectSimulationFlags,

    /// The position and orientation of this source.
    pub source: geometry::CoordinateSystem,

    /// The distance attenuation model to use for this source.
    pub distance_attenuation_model: DistanceAttenuationModel,

    /// The air absorption model to use for this source.
    pub air_absorption_model: AirAbsorptionModel,

    /// The directivity pattern to use for this source.
    pub directivity: Directivity,

    /// The occlusion algorithm to use for this source.
    pub occlusion: Occlusion,

    /// If using parametric or hybrid reverb for rendering reflections, the reverb decay times for each frequency band are scaled by these values.
    /// Set to `[1.0, 1.0, 1.0]` to use the simulated values without modification.
    pub reverb_scale: [f32; 3],

    /// If using hybrid reverb for rendering reflections, this is the length (in seconds) of impulse response to use for convolution reverb.
    /// The rest of the impulse response will be used for parametric reverb estimation only.
    /// Increasing this value results in more accurate reflections, at the cost of increased CPU usage.
    pub hybrid_reverb_transition_time: f32,

    /// If using hybrid reverb for rendering reflections, this is the amount of overlap between the convolution and parametric parts.
    /// To ensure smooth transitions from the early convolution part to the late parametric part, the two are cross-faded towards the end of the convolution part.
    /// For example, if [`Self::hybrid_reverb_transition_time`] is 1.0, and [`Self::hybrid_reverb_overlap_percent`] is 0.25, then the first 0.75 seconds are pure convolution, the next 0.25 seconds are a blend between convolution and parametric, and the portion of the tail beyond 1.0 second is pure parametric.
    pub hybrid_reverb_overlap_percent: f32,

    /// The optional identifier used to specify which layer of baked data to use for simulating reflections for this source.
    pub baked_data_identifier: Option<BakedDataIdentifier>,

    /// The probe batch within which to find paths from this source to the listener.
    pub pathing_probes: ProbeBatch,

    /// When testing for mutual visibility between a pair of probes, each probe is treated as a sphere of this radius (in meters), and point samples are generated within this sphere.
    pub visibility_radius: f32,

    /// When tracing rays to test for mutual visibility between a pair of probes, the fraction of rays that are unoccluded must be greater than this threshold for the pair of probes to be considered mutually visible.
    pub visibility_threshold: f32,

    /// If the distance between two probes is greater than this value, the probes are not considered mutually visible.
    /// Increasing this value can result in simpler paths, at the cost of increased CPU usage.
    pub visibility_range: f32,

    /// If simulating pathing, this is the Ambisonic order used for representing path directionality.
    /// Higher values result in more precise spatialization of paths, at the cost of increased CPU usage.
    pub pathing_order: i32,

    /// If `true`, baked paths are tested for visibility.
    /// This is useful if your scene has dynamic objects that might occlude baked paths.
    pub enable_validation: bool,

    /// If `true`, and [`Self::enable_validation`] is `true`, then if a baked path is occluded by dynamic geometry, path finding is re-run in real-time to find alternate paths that take into account the dynamic geometry.
    pub find_alternative_paths: bool,

    /// If simulating transmission, this is the maximum number of surfaces, starting from the closest surface to the listener, whose transmission coefficients will be considered when calculating the total amount of sound transmitted.
    /// Increasing this value will result in more accurate results when multiple surfaces lie between the source and the listener, at the cost of increased CPU usage.
    pub num_transmission_rays: i32,
}

impl From<&SimulationInputs> for audionimbus_sys::IPLSimulationInputs {
    fn from(simulation_inputs: &SimulationInputs) -> Self {
        todo!()
    }
}

bitflags::bitflags! {
    /// Flags indicating which types of direct simulation should be enabled for a given [`Source`].
    #[derive(Debug)]
    pub struct DirectSimulationFlags: u32 {
        /// Enable distance attenuation calculations.
        const DISTANCE_ATTENUATION = 1 << 0;

        /// Enable air absorption calculations.
        const AIR_ABSORPTION = 1 << 1;

        /// Enable directivity calculations.
        const DIRECTIVITY = 1 << 2;

        /// Enable occlusion simulation.
        const OCCLUSION = 1 << 3;

        /// Enable transmission simulation.
        /// Requires occlusion to also be enabled.
        const TRANSMISSION = 1 << 4;
    }
}

impl From<DirectSimulationFlags> for audionimbus_sys::IPLDirectSimulationFlags {
    fn from(direct_simulation_flags: DirectSimulationFlags) -> Self {
        Self(direct_simulation_flags.bits())
    }
}

/// A distance attenuation model that can be used for modeling attenuation of sound over distance.
/// Can be used with both direct and indirect sound propagation.
#[derive(Debug)]
pub enum DistanceAttenuationModel {
    /// The default distance attenuation model.
    /// This is an inverse distance falloff, with all sounds within 1 meter of the listener rendered without distance attenuation.
    Default,

    /// An inverse distance falloff.
    /// You can configure the minimum distance, within which distance attenuation is not applied.
    InverseDistance {
        /// No distance attenuation is applied to any sound whose distance from the listener is less than this value.
        min_distance: f32,
    },

    /// An arbitrary distance falloff function, defined by a callback function.
    Callback {
        /// Callback for calculating how much attenuation should be applied to a sound based on its distance from the listener.
        ///
        /// # Arguments
        ///
        /// - `distance`: the distance (in meters) between the source and the listener.
        /// - `user_data`: pointer to the arbitrary data specified.
        ///
        /// # Returns
        ///
        /// The distance attenuation to apply, between 0.0 and 1.0.
        /// 0.0 = the sound is not audible, 1.0 = the sound is as loud as it would be if it were emitted from the listener’s position.
        callback: unsafe extern "C" fn(distance: f32, user_data: *mut std::ffi::c_void) -> f32,

        /// Pointer to arbitrary data that will be provided to the callback function whenever it is called. May be `NULL`.
        user_data: *mut std::ffi::c_void,

        /// Set to `true` to indicate that the distance attenuation model defined by the callback function has changed since the last time simulation was run.
        /// For example, the callback may be evaluating a curve defined in a GUI.
        /// If the user is editing the curve in real-time, set this to `true` whenever the curve changes, so Steam Audio can update simulation results to match.
        dirty: bool,
    },
}

impl From<&DistanceAttenuationModel> for audionimbus_sys::IPLDistanceAttenuationModel {
    fn from(distance_attenuation_model: &DistanceAttenuationModel) -> Self {
        todo!()
    }
}

/// An air absorption model that can be used for modeling frequency-dependent attenuation of sound over distance.
#[derive(Debug)]
pub enum AirAbsorptionModel {
    /// The default air absorption model.
    /// This is an exponential falloff, with decay rates derived from physical properties of air.
    Default,

    /// An exponential falloff.
    /// You can configure the decay rates for each frequency band.
    Exponential {
        /// The exponential falloff coefficients to use.
        coefficients: [f32; 3],
    },

    /// An arbitrary air absorption model, defined by a callback function.
    Callback {
        /// Callback for calculating how much air absorption should be applied to a sound based on its distance from the listener.
        ///
        /// # Arguments
        ///
        /// - `distance`: the distance (in meters) between the source and the listener.
        /// - `band`: index of the frequency band for which to calculate air absorption. 0.0 = low frequencies, 1.0 = middle frequencies, 2.0 = high frequencies.
        /// - `user_data`: pointer to the arbitrary data specified.
        ///
        /// # Returns
        ///
        /// The air absorption to apply, between 0.0 and 1.0.
        /// 0.0 = sound in the frequency band `band` is not audible, 1.0 = sound in the frequency band `band` is not attenuated.
        callback:
            unsafe extern "C" fn(distance: f32, band: i32, user_data: *mut std::ffi::c_void) -> f32,

        /// Pointer to arbitrary data that will be provided to the callback function whenever it is called. May be `NULL`.
        user_data: *mut std::ffi::c_void,

        /// Set to `true` to indicate that the air absorption model defined by the callback function has changed since the last time simulation was run.
        /// For example, the callback may be evaluating a set of curves defined in a GUI.
        /// If the user is editing the curves in real-time, set this to `true` whenever the curves change, so Steam Audio can update simulation results to match.
        dirty: bool,
    },
}

impl From<&AirAbsorptionModel> for audionimbus_sys::IPLAirAbsorptionModel {
    fn from(air_absorption_model: &AirAbsorptionModel) -> Self {
        todo!()
    }
}

/// A directivity pattern that can be used to model changes in sound intensity as a function of the source’s orientation.
/// Can be used with both direct and indirect sound propagation.
#[derive(Debug)]
pub enum Directivity {
    /// The default directivity model is a weighted dipole.
    /// This is a linear blend between an omnidirectional source (which emits sound with equal intensity in all directions), and a dipole oriented along the z-axis in the source’s coordinate system (which focuses sound along the +z and -z axes).
    Default {
        /// How much of the dipole to blend into the directivity pattern.
        /// 0.0 = pure omnidirectional, 1.0 = pure dipole.
        /// 0.5 results in a cardioid directivity pattern.
        dipole_weight: f32,

        /// How “sharp” the dipole is.
        /// Higher values result in sound being focused within a narrower range of directions.
        dipole_power: f32,
    },

    /// A callback function to implement any other arbitrary directivity pattern.
    Callback {
        /// Callback for calculating how much to attenuate a sound based on its directivity pattern and orientation in world space.
        ///
        /// # Arguments
        ///
        /// - `direction`: unit vector (in world space) pointing forwards from the source. This is the direction that the source is “pointing towards”.
        /// - `user_data`: pointer to the arbitrary data specified.
        ///
        /// # Returns
        ///
        /// The directivity value to apply, between 0.0 and 1.0.
        /// 0.0 = the sound is not audible, 1.0 = the sound is as loud as it would be if it had a uniform (omnidirectional) directivity pattern.
        callback: unsafe extern "C" fn(
            direction: audionimbus_sys::IPLVector3,
            user_data: *mut std::ffi::c_void,
        ) -> f32,

        /// Pointer to arbitrary data that will be provided to the callback function whenever it is called. May be `NULL`.
        user_data: *mut std::ffi::c_void,
    },
}

impl From<&Directivity> for audionimbus_sys::IPLDirectivity {
    fn from(directivity: &Directivity) -> Self {
        todo!()
    }
}

/// The different algorithms for simulating occlusion.
#[derive(Debug)]
pub enum Occlusion {
    /// Raycast occlusion.
    /// A single ray is traced from the listener to the source.
    /// If the ray hits a solid object before it reaches the source, the source is considered occluded.
    Raycast,

    /// A volumetric occlusion algorithm that can model partial occlusion.
    /// The source is modeled as a sphere with a configurable radius.
    /// Multiple points are sampled within the volume of this sphere.
    /// Rays are then traced from each sample point to both the source and the listener.
    /// A sample point is considered occluded if either of these two rays is occluded.
    /// The occlusion value for the source is calculated as the fraction of sample points that are unoccluded.
    /// This algorithm allows for smoother transitions in and out of occlusion.
    Volumetric {
        /// The radius of the sphere the source is modeled as.
        radius: f32,

        /// The number of point samples to consider when tracing rays.
        /// This value can change between simulation runs.
        num_occlusion_samples: i32,
    },
}

impl From<&Occlusion> for audionimbus_sys::IPLOcclusionType {
    fn from(occlusion: &Occlusion) -> Self {
        todo!()
    }
}

/// Identifies a “layer” of data stored in a probe batch.
/// Each probe batch may store multiple layers of data, such as reverb, static source reflections, or pathing.
/// Each layer can be accessed using an identifier.
#[derive(Debug)]
pub enum BakedDataIdentifier {
    /// Reflections.
    /// The source and listener positions used to compute the reflections data stored at each probe depends on the \c IPLBakedDataVariation selected.
    Reflections {
        /// The way in which source and listener positions depend on probe position.
        variation: BakedDataVariation,
    },

    /// Pathing.
    /// The probe batch stores data about the shortest paths between any pair of probes in the batch.
    Pathing,
}

impl From<&BakedDataIdentifier> for audionimbus_sys::IPLBakedDataIdentifier {
    fn from(baked_data_identifier: &BakedDataIdentifier) -> Self {
        todo!()
    }
}

/// The different ways in which the source and listener positions used to generate baked data can vary as a function of probe position.
#[derive(Debug)]
pub enum BakedDataVariation {
    /// At each probe, baked data is calculated with both the source and the listener at the probe position.
    /// This is useful for modeling traditional reverbs, which depend only on the listener’s position (or only on the source’s position).
    Reverb,

    /// At each probe, baked data is calculated with the source at some fixed position (specified separately), and the listener at the probe position.
    /// This is used for modeling reflections from a static source to any point within the probe batch.
    StaticSource {
        /// The static source used to generate baked data.
        /// Baked data is only stored for probes that lie within the radius of this sphere.
        endpoint_influence: geometry::Sphere,
    },

    /// At each probe, baked data is calculated with the source at the probe position, and the listener at some fixed position (specified separately).
    /// This is used for modeling reflections from a moving source to a static listener.
    StaticListener {
        /// The static listener used to generate baked data.
        /// Baked data is only stored for probes that lie within the radius of this sphere.
        endpoint_influence: geometry::Sphere,
    },

    /// Baked data is calculated for each pair of probes.
    /// For example, this is used for calculating paths between every pair of probes in a batch.
    Dynamic,
}

/// Simulation parameters that are not specific to any source.
#[derive(Debug)]
pub struct SimulationSharedInputs {
    /// The position and orientation of the listener.
    pub listener: geometry::CoordinateSystem,

    /// The number of rays to trace from the listener.
    /// Increasing this value results in more accurate reflections, at the cost of increased CPU usage.
    pub num_rays: i32,

    /// The number of times each ray traced from the listener is reflected when it encounters a solid object.
    /// Increasing this value results in longer, more accurate reverb tails, at the cost of increased CPU usage during simulation.
    pub num_bounces: i32,

    /// The duration (in seconds) of the impulse responses generated when simulating reflections.
    /// Increasing this value results in longer, more accurate reverb tails, at the cost of increased CPU usage during audio processing.
    pub duration: f32,

    /// The Ambisonic order of the impulse responses generated when simulating reflections.
    /// Increasing this value results in more accurate directional variation of reflected sound, at the cost of increased CPU usage during audio processing.
    pub order: i32,

    /// When calculating how much sound energy reaches a surface directly from a source, any source that is closer than [`Self::irradiance_min_distance`] to the surface is assumed to be at a distance of [`Self::irradiance_min_distance`], for the purposes of energy calculations.
    pub irradiance_min_distance: f32,

    /// Optional callback for visualizing valid path segments during call to [`Simulator::run_pathing`].
    pub pathing_visualization_callback: Option<PathingVisualizationCallback>,
}

impl From<&SimulationSharedInputs> for audionimbus_sys::IPLSimulationSharedInputs {
    fn from(simulation_shared_inputs: &SimulationSharedInputs) -> Self {
        todo!()
    }
}

#[derive(Debug)]
pub struct PathingVisualizationCallback {
    /// Callback for visualizing valid path segments during call to [`Simulator::run_pathing`].
    ///
    /// You can use this to provide the user with visual feedback, like drawing each segment of a path.
    ///
    /// # Arguments
    ///
    /// - `from`: position of starting probe.
    /// - `to`: position of ending probe.
    /// - `occluded`: occlusion status of ray segment between `from` to `to`.
    /// - `user_data`: pointer to arbitrary user-specified data provided when calling the function that will call this callback.
    pub callback: unsafe extern "C" fn(
        from: audionimbus_sys::IPLVector3,
        to: audionimbus_sys::IPLVector3,
        occluded: bool,
        userData: *mut std::ffi::c_void,
    ),

    /// Pointer to arbitrary user-specified data provided when calling the callback.
    pub user_data: *mut std::ffi::c_void,
}

/// Simulation results for a source.
#[derive(Debug)]
pub struct SimulationOutputs {
    /// Direct path simulation results.
    direct: DirectEffectParams,

    /// Reflection simulation results.
    reflections: ReflectionEffectParams,

    /// Pathing simulation results.
    pathing: PathEffectParams,
}

impl From<audionimbus_sys::IPLSimulationOutputs> for SimulationOutputs {
    fn from(simulation_outputs: audionimbus_sys::IPLSimulationOutputs) -> Self {
        todo!()
    }
}
