//! Simulation mode markers and static compatibility contracts.

use crate::Sealed;

/// Marker type indicating that direct sound simulation is enabled.
#[derive(Default, Copy, Clone, Debug)]
pub struct Direct;

/// Marker type indicating that reflection simulation is enabled.
#[derive(Default, Copy, Clone, Debug)]
pub struct Reflections;

/// Marker type indicating that pathing simulation is enabled.
#[derive(Default, Copy, Clone, Debug)]
pub struct Pathing;

/// Provider of [`SimulationFlags`].
pub trait SimulationFlagsProvider {
    /// Returns the simulation flags associated with this type.
    fn flags() -> SimulationFlags;
}

impl SimulationFlagsProvider for Direct {
    fn flags() -> SimulationFlags {
        SimulationFlags::DIRECT
    }
}

impl SimulationFlagsProvider for Reflections {
    fn flags() -> SimulationFlags {
        SimulationFlags::REFLECTIONS
    }
}

impl SimulationFlagsProvider for Pathing {
    fn flags() -> SimulationFlags {
        SimulationFlags::PATHING
    }
}

impl SimulationFlagsProvider for () {
    fn flags() -> SimulationFlags {
        SimulationFlags::empty()
    }
}

bitflags::bitflags! {
    /// Flags indicating which types of simulation should be enabled.
    #[derive(Copy, Clone, Debug)]
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
        Self(simulation_flags.bits() as _)
    }
}

/// Trait used to validate simulation flags in `Source::set_inputs` and `Source::get_outputs`.
pub trait SimulationType: Sealed {
    /// The simulation modes selected by this type.
    const FLAG: SimulationFlags;
}

impl Sealed for Direct {}

impl SimulationType for Direct {
    const FLAG: SimulationFlags = SimulationFlags::DIRECT;
}

impl Sealed for Reflections {}

impl SimulationType for Reflections {
    const FLAG: SimulationFlags = SimulationFlags::REFLECTIONS;
}

impl Sealed for Pathing {}

impl SimulationType for Pathing {
    const FLAG: SimulationFlags = SimulationFlags::PATHING;
}

impl Sealed for () {}

impl SimulationType for () {
    const FLAG: SimulationFlags = SimulationFlags::empty();
}

/// Trait ensuring Source's Direct type is compatible with Simulator's Direct type.
pub trait DirectCompatible<SimD> {}

impl DirectCompatible<Direct> for Direct {}

impl DirectCompatible<Direct> for () {}

impl DirectCompatible<()> for () {}

/// Trait ensuring Source's Reflections type is compatible with Simulator's Reflections type.
pub trait ReflectionsCompatible<SimR> {}

impl ReflectionsCompatible<Reflections> for Reflections {}

impl ReflectionsCompatible<Reflections> for () {}

impl ReflectionsCompatible<()> for () {}

/// Trait ensuring Source's Pathing type is compatible with Simulator's Pathing type.
pub trait PathingCompatible<SimP> {}

impl PathingCompatible<Pathing> for Pathing {}

impl PathingCompatible<Pathing> for () {}

impl PathingCompatible<()> for () {}

/// Trait ensuring reflection effect compatibility.
///
/// If the source participates in reflections, its `RE` must match the simulator's.
pub trait ReflectionEffectCompatible<SrcR, SimRE> {}

impl<SrcRE, SimRE> ReflectionEffectCompatible<(), SimRE> for SrcRE {}

impl<SimRE> ReflectionEffectCompatible<Reflections, SimRE> for SimRE {}
