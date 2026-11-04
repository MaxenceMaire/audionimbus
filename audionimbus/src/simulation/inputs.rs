//! Source participation parameters and native input conversion.

#[cfg(feature = "dynamic")]
use super::SimulationFlags;
use super::{Direct, Pathing, Reflections};
use crate::baking::{BakedDataIdentifier, BakedDataVariation};
#[cfg(feature = "dynamic")]
use crate::binding::Dynamic;
use crate::binding::Static;
#[cfg(feature = "dynamic")]
use crate::effect::reflections::binding::ReflectionAlgorithm;
use crate::model::{
    air_absorption::AirAbsorptionModel, deviation::DeviationModel, directivity::Directivity,
    distance_attenuation::DistanceAttenuationModel,
};
use crate::{CoordinateSystem, ProbeBatch, Sealed};
use std::marker::PhantomData;

/// Parameter storage and conversion selected by an input binding.
pub trait InputBinding {
    /// The source participation parameters.
    type Parameters;

    /// Returns the direct simulation parameters.
    fn direct(parameters: &Self::Parameters) -> &Option<DirectSimulationParameters>;
    /// Converts the reflection simulation parameters.
    fn reflections(parameters: &Self::Parameters) -> Option<ReflectionsSimulationData>;
    /// Returns the pathing simulation parameters.
    fn pathing(parameters: &Self::Parameters) -> Option<&PathingSimulationParameters>;
}

impl<D, R, P> InputBinding for Static<(D, R, P)> {
    type Parameters = SimulationParameters<D, R, P>;

    fn direct(parameters: &Self::Parameters) -> &Option<DirectSimulationParameters> {
        &parameters.direct_simulation
    }

    fn reflections(parameters: &Self::Parameters) -> Option<ReflectionsSimulationData> {
        parameters.reflections_simulation
    }

    fn pathing(parameters: &Self::Parameters) -> Option<&PathingSimulationParameters> {
        parameters.pathing_simulation.as_ref()
    }
}

#[cfg(feature = "dynamic")]
impl InputBinding for Dynamic<()> {
    type Parameters = DynamicSimulationParameters;

    fn direct(parameters: &Self::Parameters) -> &Option<DirectSimulationParameters> {
        &parameters.direct
    }

    fn reflections(parameters: &Self::Parameters) -> Option<ReflectionsSimulationData> {
        parameters
            .reflections
            .as_ref()
            .map(|parameters| match parameters {
                ReflectionsParameters::Convolution(parameters) => parameters.clone().into_data(),
                ReflectionsParameters::Parametric(parameters) => parameters.clone().into_data(),
                ReflectionsParameters::Hybrid(parameters) => parameters.clone().into_data(),
                ReflectionsParameters::TrueAudioNext(parameters) => parameters.clone().into_data(),
            })
    }

    fn pathing(parameters: &Self::Parameters) -> Option<&PathingSimulationParameters> {
        parameters.pathing.as_ref()
    }
}

/// Simulation inputs for a source.
pub type StaticSimulationInputs<D = (), R = (), P = ()> = SimulationInputs<Static<(D, R, P)>>;

/// Source coordinates and runtime participation parameters.
#[cfg(feature = "dynamic")]
pub type DynamicSimulationInputs = SimulationInputs<Dynamic<()>>;

/// Storage shared by static and runtime source inputs.
#[derive(Clone, Default, Debug)]
pub struct SimulationInputs<B: InputBinding> {
    /// The position and orientation of this source.
    pub source: CoordinateSystem,

    /// Simulation parameters for the source.
    pub parameters: B::Parameters,
}

impl<B: InputBinding> SimulationInputs<B> {
    /// Converts source inputs to their native representation.
    ///
    /// Only source publication retains callback owners while these native pointers are borrowed.
    pub(crate) fn to_ffi(
        &self,
    ) -> (
        audionimbus_sys::IPLSimulationInputs,
        audionimbus_sys::IPLDeviationModel,
    ) {
        let mut flags = audionimbus_sys::IPLSimulationFlags(0);

        let source = self.source.into();

        if B::direct(&self.parameters).is_some() {
            flags |= audionimbus_sys::IPLSimulationFlags::IPL_SIMULATIONFLAGS_DIRECT;
        }
        let direct_data = DirectSimulationData::from_params(B::direct(&self.parameters));

        if B::reflections(&self.parameters).is_some() {
            flags |= audionimbus_sys::IPLSimulationFlags::IPL_SIMULATIONFLAGS_REFLECTIONS;
        }
        let reflections_data = B::reflections(&self.parameters).unwrap_or_default();

        if B::pathing(&self.parameters).is_some() {
            flags |= audionimbus_sys::IPLSimulationFlags::IPL_SIMULATIONFLAGS_PATHING;
        }
        let pathing_data = PathingSimulationData::from_params(B::pathing(&self.parameters));

        let inputs = audionimbus_sys::IPLSimulationInputs {
            flags,
            directFlags: direct_data.flags,
            source,
            distanceAttenuationModel: direct_data.distance_attenuation_model,
            airAbsorptionModel: direct_data.air_absorption_model,
            directivity: direct_data.directivity,
            occlusionType: direct_data.occlusion_type,
            occlusionRadius: direct_data.occlusion_radius,
            numOcclusionSamples: direct_data.num_occlusion_samples,
            numTransmissionRays: direct_data.num_transmission_rays,
            reverbScale: reflections_data.reverb_scale,
            hybridReverbTransitionTime: reflections_data.hybrid_reverb_transition_time,
            hybridReverbOverlapPercent: reflections_data.hybrid_reverb_overlap_percent,
            baked: reflections_data.baked,
            bakedDataIdentifier: reflections_data.baked_data_identifier.into(),
            pathingProbes: pathing_data.pathing_probes,
            visRadius: pathing_data.visibility_radius,
            visThreshold: pathing_data.visibility_threshold,
            visRange: pathing_data.visibility_range,
            pathingOrder: pathing_data.pathing_order,
            enableValidation: pathing_data.enable_validation,
            findAlternatePaths: pathing_data.find_alternate_paths,
            deviationModel: std::ptr::null_mut(),
        };

        (inputs, pathing_data.deviation_model)
    }
}

/// Simulation parameters for a source.
#[derive(Clone, Default, Debug)]
pub struct SimulationParameters<D = (), R = (), P = ()> {
    /// If `Some`, enables direct simulation.
    /// This includes distance attenuation, air absorption, directivity, occlusion, and transmission.
    direct_simulation: Option<DirectSimulationParameters>,

    /// If `Some`, enables reflections simulation.
    /// This includes both real-time and baked simulation.
    reflections_simulation: Option<ReflectionsSimulationData>,

    /// If `Some`, enables pathing simulation.
    pathing_simulation: Option<PathingSimulationParameters>,

    /// The direct simulation marker.
    _direct: PhantomData<D>,
    /// The reflection simulation marker.
    _reflections: PhantomData<R>,
    /// The pathing simulation marker.
    _pathing: PhantomData<P>,
}

impl SimulationParameters {
    /// Creates new [`SimulationParameters`].
    pub fn new() -> Self {
        Self {
            direct_simulation: None,
            reflections_simulation: None,
            pathing_simulation: None,
            _direct: PhantomData,
            _reflections: PhantomData,
            _pathing: PhantomData,
        }
    }
}

impl<D, R, P> SimulationParameters<D, R, P> {
    /// Returns the direct simulation parameters for this source, if direct simulation is enabled.
    pub fn direct_simulation_parameters(&self) -> Option<&DirectSimulationParameters> {
        self.direct_simulation.as_ref()
    }

    /// Enables direct simulation with the specified parameters.
    pub fn with_direct(
        self,
        params: DirectSimulationParameters,
    ) -> SimulationParameters<Direct, R, P> {
        let Self {
            reflections_simulation,
            pathing_simulation,
            _reflections,
            _pathing,
            ..
        } = self;

        SimulationParameters {
            direct_simulation: Some(params),
            reflections_simulation,
            pathing_simulation,
            _direct: PhantomData,
            _reflections,
            _pathing,
        }
    }

    /// Enables reflections simulation with the specified parameters.
    pub fn with_reflections<Params: ReflectionsSimulationParameters>(
        self,
        params: Params,
    ) -> SimulationParameters<D, Reflections, P> {
        let Self {
            direct_simulation,
            pathing_simulation,
            _direct,
            _pathing,
            ..
        } = self;

        SimulationParameters {
            direct_simulation,
            reflections_simulation: Some(params.into_data()),
            pathing_simulation,
            _direct,
            _reflections: PhantomData,
            _pathing,
        }
    }

    /// Enables pathing simulation with the specified parameters.
    pub fn with_pathing(
        self,
        params: PathingSimulationParameters,
    ) -> SimulationParameters<D, R, Pathing> {
        let Self {
            direct_simulation,
            reflections_simulation,
            _direct,
            _reflections,
            ..
        } = self;

        SimulationParameters {
            direct_simulation,
            reflections_simulation,
            pathing_simulation: Some(params),
            _direct,
            _reflections,
            _pathing: PhantomData,
        }
    }
}

impl<R, P> SimulationParameters<Direct, R, P> {
    /// Sets the parameters to use for direct simulation.
    pub fn set_direct_simulation_parameters(&mut self, params: DirectSimulationParameters) {
        self.direct_simulation.replace(params);
    }
}

impl<D, P> SimulationParameters<D, Reflections, P> {
    /// Sets the parameters to use for reflections simulation.
    pub fn set_reflections_simulation_parameters<Params: ReflectionsSimulationParameters>(
        &mut self,
        params: Params,
    ) {
        self.reflections_simulation.replace(params.into_data());
    }
}

impl<D, R> SimulationParameters<D, R, Pathing> {
    /// Sets the parameters to use for path simulation.
    pub fn set_pathing_simulation_parameters(&mut self, params: PathingSimulationParameters) {
        self.pathing_simulation.replace(params);
    }
}

/// Optional source participation parameters for each simulation mode.
#[cfg(feature = "dynamic")]
#[derive(Clone, Default, Debug)]
pub struct DynamicSimulationParameters {
    /// The direct simulation parameters.
    pub direct: Option<DirectSimulationParameters>,
    /// The reflection simulation parameters and algorithm.
    pub reflections: Option<ReflectionsParameters>,
    /// The pathing simulation parameters and probes.
    pub pathing: Option<PathingSimulationParameters>,
}

#[cfg(feature = "dynamic")]
impl DynamicSimulationParameters {
    /// Returns the modes enabled by these parameters.
    pub fn flags(&self) -> SimulationFlags {
        let mut flags = SimulationFlags::empty();
        if self.direct.is_some() {
            flags |= SimulationFlags::DIRECT;
        }
        if self.reflections.is_some() {
            flags |= SimulationFlags::REFLECTIONS;
        }
        if self.pathing.is_some() {
            flags |= SimulationFlags::PATHING;
        }

        flags
    }
}

/// Direct simulation parameters for a source.
#[derive(Clone, Default, Debug)]
pub struct DirectSimulationParameters {
    /// If `Some`, enables distance attenuation calculations with the specified model.
    pub distance_attenuation: Option<DistanceAttenuationModel>,

    /// If `Some`, enables air absorption calculations with the specified model.
    pub air_absorption: Option<AirAbsorptionModel>,

    /// If `Some`, enables directivity calculations with the specified directivity pattern.
    pub directivity: Option<Directivity>,

    /// If `Some`, enables occlusion simulation.
    pub occlusion: Option<Occlusion>,
}

impl DirectSimulationParameters {
    /// Creates new [`DirectSimulationParameters`] with all calculations disabled by default.
    pub fn new() -> Self {
        Self {
            distance_attenuation: None,
            air_absorption: None,
            directivity: None,
            occlusion: None,
        }
    }

    /// Enables distance attenuation with the specified model.
    pub fn with_distance_attenuation(mut self, model: DistanceAttenuationModel) -> Self {
        self.distance_attenuation = Some(model);
        self
    }

    /// Enables air absorption with the specified model.
    pub fn with_air_absorption(mut self, model: AirAbsorptionModel) -> Self {
        self.air_absorption = Some(model);
        self
    }

    /// Enables directivity with the specified pattern.
    pub fn with_directivity(mut self, directivity: Directivity) -> Self {
        self.directivity = Some(directivity);
        self
    }

    /// Enables occlusion with the specified parameters.
    pub fn with_occlusion(mut self, occlusion: Occlusion) -> Self {
        self.occlusion = Some(occlusion);
        self
    }
}

/// Occlusion parameters.
#[derive(Debug, Copy, Clone)]
pub struct Occlusion {
    /// If `Some`, enables transmission simulation.
    pub transmission: Option<TransmissionParameters>,

    /// The occlusion algorithm to use.
    pub algorithm: OcclusionAlgorithm,
}

impl Occlusion {
    /// Creates a new [`Occlusion`] with transmission simulation disabled by default.
    pub fn new(algorithm: OcclusionAlgorithm) -> Self {
        Self {
            transmission: None,
            algorithm,
        }
    }

    /// Enables transmission simulation with the specified parameters.
    pub fn with_transmission(mut self, params: TransmissionParameters) -> Self {
        self.transmission = Some(params);
        self
    }
}

/// Transmission parameters.
#[derive(Debug, Copy, Clone)]
pub struct TransmissionParameters {
    /// If simulating transmission, this is the maximum number of surfaces, starting from the closest surface to the listener, whose transmission coefficients will be considered when calculating the total amount of sound transmitted.
    /// Increasing this value will result in more accurate results when multiple surfaces lie between the source and the listener, at the cost of increased CPU usage.
    pub num_transmission_rays: u32,
}

/// Trait implemented by all reflections simulation parameter structs.
#[allow(private_bounds)]
pub trait ReflectionsSimulationParameters: Sealed + IntoSimulationData {}

/// Types that can be converted into [`ReflectionsSimulationData`].
trait IntoSimulationData {
    /// Returns the intermediate representation of reflections simulation parameters.
    fn into_data(self) -> ReflectionsSimulationData;
}

/// Multi-channel convolution reverb parameters.
#[derive(Default, Clone, Debug)]
pub struct ConvolutionParameters {
    /// The optional identifier used to specify which layer of baked data to use for simulating reflections for this source.
    pub baked_data_identifier: Option<BakedDataIdentifier>,
}

impl Sealed for ConvolutionParameters {}

impl ReflectionsSimulationParameters for ConvolutionParameters {}

impl IntoSimulationData for ConvolutionParameters {
    fn into_data(self) -> ReflectionsSimulationData {
        let (baked, baked_data_identifier) = self.baked_data_identifier.map_or(
            (
                false.into(),
                BakedDataIdentifier::Reflections {
                    variation: BakedDataVariation::Reverb,
                },
            ),
            |id| (true.into(), id),
        );

        ReflectionsSimulationData {
            baked,
            baked_data_identifier,
            reverb_scale: [0.0; 3],
            hybrid_reverb_transition_time: 0.0,
            hybrid_reverb_overlap_percent: 0.0,
        }
    }
}

/// Parameters for parametric (or artificial) reverb, using feedback delay networks.
#[derive(Clone, Debug)]
pub struct ParametricParameters {
    /// The reverb decay times for each frequency band are scaled by these values.
    /// Set to `[1.0, 1.0, 1.0]` to use the simulated values without modification.
    pub reverb_scale: [f32; 3],

    /// The optional identifier used to specify which layer of baked data to use for simulating reflections for this source.
    pub baked_data_identifier: Option<BakedDataIdentifier>,
}

impl Sealed for ParametricParameters {}

impl ReflectionsSimulationParameters for ParametricParameters {}

impl IntoSimulationData for ParametricParameters {
    fn into_data(self) -> ReflectionsSimulationData {
        let (baked, baked_data_identifier) = self.baked_data_identifier.map_or(
            (
                false.into(),
                BakedDataIdentifier::Reflections {
                    variation: BakedDataVariation::Reverb,
                },
            ),
            |id| (true.into(), id),
        );

        ReflectionsSimulationData {
            baked,
            baked_data_identifier,
            reverb_scale: self.reverb_scale,
            hybrid_reverb_transition_time: 0.0,
            hybrid_reverb_overlap_percent: 0.0,
        }
    }
}

impl Default for ParametricParameters {
    fn default() -> Self {
        Self {
            reverb_scale: [1.0, 1.0, 1.0],
            baked_data_identifier: None,
        }
    }
}

/// Parameters for a hybrid of convolution and parametric reverb.
#[derive(Clone, Debug)]
pub struct HybridParameters {
    /// The reverb decay times for each frequency band are scaled by these values.
    /// Set to `[1.0, 1.0, 1.0]` to use the simulated values without modification.
    pub reverb_scale: [f32; 3],

    /// This is the length (in seconds) of impulse response to use for convolution reverb.
    /// The rest of the impulse response will be used for parametric reverb estimation only.
    /// Increasing this value results in more accurate reflections, at the cost of increased CPU usage.
    pub hybrid_reverb_transition_time: f32,

    /// This is the amount of overlap between the convolution and parametric parts.
    /// To ensure smooth transitions from the early convolution part to the late parametric part, the two are cross-faded towards the end of the convolution part.
    /// For example, if `hybrid_reverb_transition_time` is 1.0, and `hybrid_reverb_overlap_percent` is 0.25, then the first 0.75 seconds are pure convolution, the next 0.25 seconds are a blend between convolution and parametric, and the portion of the tail beyond 1.0 second is pure parametric.
    pub hybrid_reverb_overlap_percent: f32,

    /// The optional identifier used to specify which layer of baked data to use for simulating reflections for this source.
    pub baked_data_identifier: Option<BakedDataIdentifier>,
}

impl Sealed for HybridParameters {}

impl ReflectionsSimulationParameters for HybridParameters {}

impl IntoSimulationData for HybridParameters {
    fn into_data(self) -> ReflectionsSimulationData {
        let (baked, baked_data_identifier) = self.baked_data_identifier.map_or(
            (
                false.into(),
                BakedDataIdentifier::Reflections {
                    variation: BakedDataVariation::Reverb,
                },
            ),
            |id| (true.into(), id),
        );

        ReflectionsSimulationData {
            baked,
            baked_data_identifier,
            reverb_scale: self.reverb_scale,
            hybrid_reverb_transition_time: self.hybrid_reverb_transition_time,
            hybrid_reverb_overlap_percent: self.hybrid_reverb_overlap_percent,
        }
    }
}

impl Default for HybridParameters {
    fn default() -> Self {
        Self {
            reverb_scale: [1.0, 1.0, 1.0],
            hybrid_reverb_transition_time: 1.0,
            hybrid_reverb_overlap_percent: 0.25,
            baked_data_identifier: None,
        }
    }
}

/// Multi-channel convolution reverb parameters, using AMD TrueAudio Next for GPU acceleration.
#[derive(Default, Clone, Debug)]
pub struct TrueAudioNextParameters {
    /// The optional identifier used to specify which layer of baked data to use for simulating reflections for this source.
    pub baked_data_identifier: Option<BakedDataIdentifier>,
}

impl Sealed for TrueAudioNextParameters {}

impl ReflectionsSimulationParameters for TrueAudioNextParameters {}

impl IntoSimulationData for TrueAudioNextParameters {
    fn into_data(self) -> ReflectionsSimulationData {
        let (baked, baked_data_identifier) = self.baked_data_identifier.map_or(
            (
                false.into(),
                BakedDataIdentifier::Reflections {
                    variation: BakedDataVariation::Reverb,
                },
            ),
            |id| (true.into(), id),
        );

        ReflectionsSimulationData {
            baked,
            baked_data_identifier,
            reverb_scale: [0.0; 3],
            hybrid_reverb_transition_time: 0.0,
            hybrid_reverb_overlap_percent: 0.0,
        }
    }
}

/// Source parameters for a runtime-selected reflection algorithm.
#[cfg(feature = "dynamic")]
#[derive(Clone, Debug)]
pub enum ReflectionsParameters {
    /// Multi-channel convolution parameters.
    Convolution(ConvolutionParameters),
    /// Parametric reverb parameters.
    Parametric(ParametricParameters),
    /// Combined convolution and parametric parameters.
    Hybrid(HybridParameters),
    /// GPU convolution parameters.
    TrueAudioNext(TrueAudioNextParameters),
}

#[cfg(feature = "dynamic")]
impl ReflectionsParameters {
    /// Returns the selected algorithm.
    pub const fn algorithm(&self) -> ReflectionAlgorithm {
        match self {
            Self::Convolution(_) => ReflectionAlgorithm::Convolution,
            Self::Parametric(_) => ReflectionAlgorithm::Parametric,
            Self::Hybrid(_) => ReflectionAlgorithm::Hybrid,
            Self::TrueAudioNext(_) => ReflectionAlgorithm::TrueAudioNext,
        }
    }
}

#[cfg(feature = "dynamic")]
impl From<ConvolutionParameters> for ReflectionsParameters {
    fn from(convolution_parameters: ConvolutionParameters) -> Self {
        Self::Convolution(convolution_parameters)
    }
}

#[cfg(feature = "dynamic")]
impl From<ParametricParameters> for ReflectionsParameters {
    fn from(parametric_parameters: ParametricParameters) -> Self {
        Self::Parametric(parametric_parameters)
    }
}

#[cfg(feature = "dynamic")]
impl From<HybridParameters> for ReflectionsParameters {
    fn from(hybrid_parameters: HybridParameters) -> Self {
        Self::Hybrid(hybrid_parameters)
    }
}

#[cfg(feature = "dynamic")]
impl From<TrueAudioNextParameters> for ReflectionsParameters {
    fn from(true_audio_next_parameters: TrueAudioNextParameters) -> Self {
        Self::TrueAudioNext(true_audio_next_parameters)
    }
}

/// Pathing simulation parameters for a source.
#[derive(Clone, Debug)]
pub struct PathingSimulationParameters {
    /// The probe batch within which to find paths from this source to the listener.
    pub pathing_probes: ProbeBatch,

    /// When testing for mutual visibility between a pair of probes, each probe is treated as a sphere of this radius (in meters), and point samples are generated within this sphere.
    pub visibility_radius: f32,

    /// When tracing rays to test for mutual visibility between a pair of probes, the fraction of rays that are unoccluded must be greater than this threshold for the pair of probes to be considered mutually visible.
    pub visibility_threshold: f32,

    /// If the distance between two probes is greater than this value, the probes are not considered mutually visible.
    /// Increasing this value can result in simpler paths, at the cost of increased CPU usage.
    pub visibility_range: f32,

    /// The Ambisonic order used for representing path directionality.
    /// Higher values result in more precise spatialization of paths, at the cost of increased CPU usage.
    pub pathing_order: u32,

    /// If `true`, baked paths are tested for visibility.
    /// This is useful if your scene has dynamic objects that might occlude baked paths.
    pub enable_validation: bool,

    /// If `true`, and [`Self::enable_validation`] is `true`, then if a baked path is occluded by dynamic geometry, path finding is re-run in real-time to find alternate paths that take into account the dynamic geometry.
    pub find_alternate_paths: bool,

    /// The deviation model to use for this source.
    pub deviation: DeviationModel,
}

/// Intermediate representation of direct simulation parameters for FFI conversion.
struct DirectSimulationData {
    /// The selected native direct simulation modes.
    flags: audionimbus_sys::IPLDirectSimulationFlags,
    /// The native distance attenuation model.
    distance_attenuation_model: audionimbus_sys::IPLDistanceAttenuationModel,
    /// The native air absorption model.
    air_absorption_model: audionimbus_sys::IPLAirAbsorptionModel,
    /// The native directivity model.
    directivity: audionimbus_sys::IPLDirectivity,
    /// The native occlusion algorithm.
    occlusion_type: audionimbus_sys::IPLOcclusionType,
    /// The radius of volumetric occlusion samples.
    occlusion_radius: f32,
    /// The number of volumetric occlusion samples.
    num_occlusion_samples: i32,
    /// The number of transmission rays.
    num_transmission_rays: i32,
}

impl DirectSimulationData {
    /// Converts optional direct simulation parameters into concrete FFI-compatible data.
    fn from_params(params: &Option<DirectSimulationParameters>) -> Self {
        let Some(params) = params else {
            return Self::default();
        };

        let mut flags = audionimbus_sys::IPLDirectSimulationFlags(0);

        let distance_attenuation_model =
            Self::process_distance_attenuation(&params.distance_attenuation, &mut flags);

        let air_absorption_model = Self::process_air_absorption(&params.air_absorption, &mut flags);

        let directivity = Self::process_directivity(&params.directivity, &mut flags);

        let (occlusion_type, occlusion_radius, num_occlusion_samples, num_transmission_rays) =
            Self::process_occlusion(params.occlusion, &mut flags);

        Self {
            flags,
            distance_attenuation_model,
            air_absorption_model,
            directivity,
            occlusion_type,
            occlusion_radius,
            num_occlusion_samples,
            num_transmission_rays,
        }
    }

    /// Processes optional distance attenuation settings and updates flags accordingly.
    fn process_distance_attenuation(
        distance_attenuation: &Option<DistanceAttenuationModel>,
        flags: &mut audionimbus_sys::IPLDirectSimulationFlags,
    ) -> audionimbus_sys::IPLDistanceAttenuationModel {
        if let Some(model) = distance_attenuation {
            *flags |= audionimbus_sys::IPLDirectSimulationFlags::IPL_DIRECTSIMULATIONFLAGS_DISTANCEATTENUATION;
            model.into()
        } else {
            (&DistanceAttenuationModel::default()).into()
        }
    }

    /// Processes optional air absorption settings and updates flags accordingly.
    fn process_air_absorption(
        air_absorption: &Option<AirAbsorptionModel>,
        flags: &mut audionimbus_sys::IPLDirectSimulationFlags,
    ) -> audionimbus_sys::IPLAirAbsorptionModel {
        if let Some(model) = air_absorption {
            *flags |=
                audionimbus_sys::IPLDirectSimulationFlags::IPL_DIRECTSIMULATIONFLAGS_AIRABSORPTION;
            model.into()
        } else {
            (&AirAbsorptionModel::default()).into()
        }
    }

    /// Processes optional directivity settings and updates flags accordingly.
    fn process_directivity(
        directivity: &Option<Directivity>,
        flags: &mut audionimbus_sys::IPLDirectSimulationFlags,
    ) -> audionimbus_sys::IPLDirectivity {
        if let Some(directivity) = directivity {
            *flags |=
                audionimbus_sys::IPLDirectSimulationFlags::IPL_DIRECTSIMULATIONFLAGS_DIRECTIVITY;
            directivity.into()
        } else {
            (&Directivity::default()).into()
        }
    }

    /// Processes optional occlusion settings and updates flags accordingly.
    fn process_occlusion(
        occlusion: Option<Occlusion>,
        flags: &mut audionimbus_sys::IPLDirectSimulationFlags,
    ) -> (audionimbus_sys::IPLOcclusionType, f32, i32, i32) {
        let Some(occlusion) = occlusion else {
            return (
                audionimbus_sys::IPLOcclusionType::IPL_OCCLUSIONTYPE_RAYCAST,
                0.0,
                0,
                0,
            );
        };

        *flags |= audionimbus_sys::IPLDirectSimulationFlags::IPL_DIRECTSIMULATIONFLAGS_OCCLUSION;

        let (occlusion_type, occlusion_radius, num_occlusion_samples) = match occlusion.algorithm {
            OcclusionAlgorithm::Raycast => (
                audionimbus_sys::IPLOcclusionType::IPL_OCCLUSIONTYPE_RAYCAST,
                0.0,
                0,
            ),
            OcclusionAlgorithm::Volumetric {
                radius,
                num_occlusion_samples,
            } => (
                audionimbus_sys::IPLOcclusionType::IPL_OCCLUSIONTYPE_VOLUMETRIC,
                radius,
                num_occlusion_samples,
            ),
        };

        let num_transmission_rays = if let Some(transmission) = occlusion.transmission {
            *flags |=
                audionimbus_sys::IPLDirectSimulationFlags::IPL_DIRECTSIMULATIONFLAGS_TRANSMISSION;
            transmission.num_transmission_rays
        } else {
            0
        };

        (
            occlusion_type,
            occlusion_radius,
            num_occlusion_samples as i32,
            num_transmission_rays as i32,
        )
    }
}

impl Default for DirectSimulationData {
    fn default() -> Self {
        Self {
            flags: audionimbus_sys::IPLDirectSimulationFlags(0),
            distance_attenuation_model: (&DistanceAttenuationModel::default()).into(),
            air_absorption_model: (&AirAbsorptionModel::default()).into(),
            directivity: (&Directivity::default()).into(),
            occlusion_type: audionimbus_sys::IPLOcclusionType::IPL_OCCLUSIONTYPE_RAYCAST,
            occlusion_radius: 0.0,
            num_occlusion_samples: 0,
            num_transmission_rays: 0,
        }
    }
}

/// Intermediate representation of reflections simulation parameters for FFI conversion.
#[derive(Copy, Clone, Debug)]
pub struct ReflectionsSimulationData {
    /// The baked reflection selection.
    baked: audionimbus_sys::IPLbool,
    /// The selected baked reflection layer.
    baked_data_identifier: BakedDataIdentifier,
    /// The scale for each reverb frequency band.
    reverb_scale: [f32; 3],
    /// The transition time from convolution to parametric reverb.
    hybrid_reverb_transition_time: f32,
    /// The overlap between convolution and parametric reverb.
    hybrid_reverb_overlap_percent: f32,
}

impl Default for ReflectionsSimulationData {
    fn default() -> Self {
        Self {
            baked: false.into(),
            baked_data_identifier: BakedDataIdentifier::Reflections {
                variation: BakedDataVariation::Reverb,
            },
            reverb_scale: [0.0; 3],
            hybrid_reverb_transition_time: 0.0,
            hybrid_reverb_overlap_percent: 0.0,
        }
    }
}

/// Intermediate representation of pathing simulation parameters for FFI conversion.
struct PathingSimulationData {
    /// The native probe batch used for pathing.
    pathing_probes: audionimbus_sys::IPLProbeBatch,
    /// The radius used for visibility tests.
    visibility_radius: f32,
    /// The visibility fraction required between probes.
    visibility_threshold: f32,
    /// The maximum distance between mutually visible probes.
    visibility_range: f32,
    /// The Ambisonic order of the pathing output.
    pathing_order: i32,
    /// The baked path validation flag.
    enable_validation: audionimbus_sys::IPLbool,
    /// The alternate path search flag.
    find_alternate_paths: audionimbus_sys::IPLbool,
    /// The native deviation model.
    deviation_model: audionimbus_sys::IPLDeviationModel,
}

impl PathingSimulationData {
    /// Converts optional pathing simulation parameters into concrete FFI-compatible data.
    fn from_params(params: Option<&PathingSimulationParameters>) -> Self {
        let Some(params) = params else {
            return Self::default();
        };

        Self {
            pathing_probes: params.pathing_probes.raw_ptr(),
            visibility_radius: params.visibility_radius,
            visibility_threshold: params.visibility_threshold,
            visibility_range: params.visibility_range,
            pathing_order: params.pathing_order as i32,
            enable_validation: params.enable_validation.into(),
            find_alternate_paths: params.find_alternate_paths.into(),
            deviation_model: (&params.deviation).into(),
        }
    }
}

impl Default for PathingSimulationData {
    fn default() -> Self {
        Self {
            pathing_probes: std::ptr::null_mut(),
            visibility_radius: 0.0,
            visibility_threshold: 0.0,
            visibility_range: 0.0,
            pathing_order: 0,
            enable_validation: false.into(),
            find_alternate_paths: false.into(),
            deviation_model: (&DeviationModel::Default).into(),
        }
    }
}

/// The different algorithms for simulating occlusion.
#[derive(Copy, Clone, Debug)]
pub enum OcclusionAlgorithm {
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
        num_occlusion_samples: u32,
    },
}
