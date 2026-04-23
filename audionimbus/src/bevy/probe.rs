//! Acoustic probes for baked data and pathing.
//!
//! [`ProbeArray`] stores probe positions that can be generated from a scene or authored manually.
//! [`ProbeBatch`] stores committed probes and baked data.

use super::asset::{ProbeBatchAsset, with_serialized_object_mut};
use super::configuration::SimulationConfiguration;
use super::simulation::Simulation;
use crate::context::Context;
use crate::error::SteamAudioError;
use crate::serialized_object::SerializedObject;
use bevy::prelude::{
    Add, Changed, Component, On, Query, Reflect, ReflectComponent, Remove, ResMut,
};
use std::ops::{Deref, DerefMut};

#[cfg(doc)]
use crate::probe::{ProbeArray as RawProbeArray, ProbeBatch as RawProbeBatch};

/// Component wrapping an [AudioNimbus probe array](`crate::probe::ProbeArray`).
///
/// Probe arrays can be used to generate or store probe positions before copying them into a
/// [`ProbeBatch`].
#[derive(Component, Reflect, Clone, Debug, PartialEq, Eq)]
#[reflect(Component, opaque)]
pub struct ProbeArray(pub crate::probe::ProbeArray);

impl ProbeArray {
    /// Creates a new probe array.
    ///
    /// Mirrors [`ProbeArray::try_new`][crate::probe::ProbeArray::try_new].
    pub fn try_new(context: &Context) -> Result<Self, SteamAudioError> {
        crate::probe::ProbeArray::try_new(context).map(Self)
    }
}

impl Deref for ProbeArray {
    type Target = crate::probe::ProbeArray;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ProbeArray {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<crate::probe::ProbeArray> for ProbeArray {
    fn from(probe_array: crate::probe::ProbeArray) -> Self {
        Self(probe_array)
    }
}

/// Component wrapping an [AudioNimbus probe batch](`crate::probe::ProbeBatch`).
///
/// Adding this component to an entity registers the batch with the simulator.
/// Mutated batches are commited before the next simulation frame.
/// Removing a batch unregisters it from the simulator.
#[derive(Component, Reflect, Clone, Debug)]
#[reflect(Component, opaque)]
pub struct ProbeBatch(pub crate::probe::ProbeBatch);

impl ProbeBatch {
    /// Creates a new probe batch.
    ///
    /// Mirrors [`ProbeBatch::try_new`][crate::probe::ProbeBatch::try_new].
    pub fn try_new(context: &Context) -> Result<Self, SteamAudioError> {
        crate::probe::ProbeBatch::try_new(context).map(Self)
    }

    /// Loads a probe batch from a serialized object.
    ///
    /// Mirrors [`ProbeBatch::load`][crate::probe::ProbeBatch::load].
    pub fn load(
        context: &Context,
        serialized_object: &mut SerializedObject,
    ) -> Result<Self, SteamAudioError> {
        crate::probe::ProbeBatch::load(context, serialized_object).map(Self)
    }

    /// Loads a probe batch from a [`ProbeBatchAsset`].
    pub fn from_asset(context: &Context, asset: &ProbeBatchAsset) -> Result<Self, SteamAudioError> {
        with_serialized_object_mut(context, asset.bytes().to_vec(), |serialized_object| {
            Self::load(context, serialized_object)
        })
    }
}

impl Deref for ProbeBatch {
    type Target = crate::probe::ProbeBatch;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ProbeBatch {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<crate::probe::ProbeBatch> for ProbeBatch {
    fn from(probe_batch: crate::probe::ProbeBatch) -> Self {
        Self(probe_batch)
    }
}

/// Adds a [`ProbeBatch`] to the simulator and requests a commit when it is spawned.
pub(crate) fn on_probe_batch_added<C: SimulationConfiguration>(
    event: On<Add, ProbeBatch>,
    query: Query<&ProbeBatch>,
    mut simulation: ResMut<Simulation<C>>,
) {
    let probe_batch = query.get(event.entity).unwrap();
    simulation.0.simulator.add_probe_batch(&probe_batch.0);
    simulation.0.request_simulator_commit();
}

/// Removes a [`ProbeBatch`] from the simulator and requests a commit when it is removed.
pub(crate) fn on_probe_batch_removed<C: SimulationConfiguration>(
    event: On<Remove, ProbeBatch>,
    query: Query<&ProbeBatch>,
    mut simulation: ResMut<Simulation<C>>,
) {
    let probe_batch = query.get(event.entity).unwrap();
    simulation.0.simulator.remove_probe_batch(&probe_batch.0);
    simulation.0.request_simulator_commit();
}

/// Commits mutated probe batches and requests a simulator commit.
pub(crate) fn commit_probe_batches<C: SimulationConfiguration>(
    probe_batches: Query<&ProbeBatch, Changed<ProbeBatch>>,
    mut simulation: ResMut<Simulation<C>>,
) {
    if probe_batches.is_empty() {
        return;
    }

    for probe_batch in &probe_batches {
        probe_batch.0.commit();
    }

    simulation.0.request_simulator_commit();
}
