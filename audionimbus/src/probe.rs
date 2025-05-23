use crate::context::Context;
use crate::error::{to_option_error, SteamAudioError};
use crate::geometry::{Matrix, Scene, Sphere};
use crate::serialized_object::SerializedObject;

/// An array of sound probes.
///
/// Each probe has a position and a radius of influence.
#[derive(Debug)]
pub struct ProbeArray(audionimbus_sys::IPLProbeArray);

impl ProbeArray {
    pub fn try_new(context: &Context) -> Result<Self, SteamAudioError> {
        let mut probe_array = Self(std::ptr::null_mut());

        let status = unsafe {
            audionimbus_sys::iplProbeArrayCreate(context.raw_ptr(), probe_array.raw_ptr_mut())
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        Ok(probe_array)
    }

    /// Generates probes and adds them to the probe array.
    pub fn generate_probes(&mut self, scene: &Scene, probe_params: &ProbeGenerationParams) {
        unsafe {
            audionimbus_sys::iplProbeArrayGenerateProbes(
                self.raw_ptr(),
                scene.raw_ptr(),
                &mut audionimbus_sys::IPLProbeGenerationParams::from(*probe_params),
            );
        }
    }

    pub fn raw_ptr(&self) -> audionimbus_sys::IPLProbeArray {
        self.0
    }

    pub fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLProbeArray {
        &mut self.0
    }
}

impl Clone for ProbeArray {
    fn clone(&self) -> Self {
        unsafe {
            audionimbus_sys::iplProbeArrayRetain(self.0);
        }
        Self(self.0)
    }
}

impl Drop for ProbeArray {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplProbeArrayRelease(&mut self.0) }
    }
}

unsafe impl Send for ProbeArray {}
unsafe impl Sync for ProbeArray {}

/// Settings used to generate probes.
#[derive(Copy, Clone, Debug)]
pub enum ProbeGenerationParams {
    /// Generates a single probe at the center of the specified box.
    Centroid {
        /// A transformation matrix that transforms an axis-aligned unit cube, with minimum and maximum vertices at (0.0, 0.0, 0.0) and (1.0, 1.0, 1.0), into a parallelopiped volume.
        /// Probes will be generated within this volume.
        transform: Matrix<f32, 4, 4>,
    },

    /// Generates probes that are uniformly-spaced, at a fixed height above solid geometry.
    /// A probe will never be generated above another probe unless there is a solid object between them.
    /// The goal is to model floors or terrain, and generate probes that are a fixed height above the floor or terrain, and uniformly-spaced along the horizontal plane.
    /// This algorithm is not suitable for scenarios where the listener may fly into a region with no probes; if this happens, the listener will not be influenced by any of the baked data.
    UniformFloor {
        /// Spacing (in meters) between two neighboring probes.
        spacing: f32,

        /// Height (in meters) above the floor at which probes will be generated.
        height: f32,

        /// A transformation matrix that transforms an axis-aligned unit cube, with minimum and maximum vertices at (0.0, 0.0, 0.0) and (1.0, 1.0, 1.0), into a parallelopiped volume.
        /// Probes will be generated within this volume.
        transform: Matrix<f32, 4, 4>,
    },
}

impl From<ProbeGenerationParams> for audionimbus_sys::IPLProbeGenerationParams {
    fn from(probe_generation_params: ProbeGenerationParams) -> Self {
        let (type_, spacing, height, transform) = match probe_generation_params {
            ProbeGenerationParams::Centroid { transform } => (
                audionimbus_sys::IPLProbeGenerationType::IPL_PROBEGENERATIONTYPE_CENTROID,
                f32::default(),
                f32::default(),
                transform,
            ),
            ProbeGenerationParams::UniformFloor {
                spacing,
                height,
                transform,
            } => (
                audionimbus_sys::IPLProbeGenerationType::IPL_PROBEGENERATIONTYPE_UNIFORMFLOOR,
                spacing,
                height,
                transform,
            ),
        };

        Self {
            type_,
            spacing,
            height,
            transform: transform.into(),
        }
    }
}

/// A batch of sound probes, along with associated data.
///
/// The associated data may include reverb, reflections from a static source position, pathing, and more.
/// This data is loaded and unloaded as a unit, either from disk or over the network.
#[derive(Debug)]
pub struct ProbeBatch(audionimbus_sys::IPLProbeBatch);

impl ProbeBatch {
    pub fn try_new(context: &Context) -> Result<Self, SteamAudioError> {
        let mut probe_batch = Self(std::ptr::null_mut());

        let status = unsafe {
            audionimbus_sys::iplProbeBatchCreate(context.raw_ptr(), probe_batch.raw_ptr_mut())
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        Ok(probe_batch)
    }

    /// Adds a probe to a batch.
    /// The new probe will be added as the last probe in the batch.
    pub fn add_probe(&mut self, probe: &Sphere) {
        unsafe {
            audionimbus_sys::iplProbeBatchAddProbe(
                self.raw_ptr(),
                audionimbus_sys::IPLSphere::from(*probe),
            );
        }
    }

    /// Adds every probe in an array to a batch.
    /// The new probes will be added, in order, at the end of the batch.
    pub fn add_probe_array(&mut self, probe_array: &ProbeArray) {
        unsafe {
            audionimbus_sys::iplProbeBatchAddProbeArray(self.raw_ptr(), probe_array.raw_ptr());
        }
    }

    /// Commits all changes made to a probe batch since this function was last called (or since the probe batch was first created, if this function was never called).
    /// This function must be called after adding, removing, or updating any probes in the batch, for the changes to take effect.
    pub fn commit(&self) {
        unsafe { audionimbus_sys::iplProbeBatchCommit(self.raw_ptr()) }
    }

    /// Saves a probe batch to a serialized object.
    /// Typically, the serialized object will then be saved to disk.
    pub fn save(&self, serialized_object: &mut SerializedObject) {
        unsafe {
            audionimbus_sys::iplProbeBatchSave(self.raw_ptr(), serialized_object.raw_ptr());
        }
    }

    /// Loads a probe batch from a serialized object.
    /// Typically, the serialized object will be created from a byte array loaded from disk or over the network.
    pub fn load(
        context: &Context,
        serialized_object: &mut SerializedObject,
    ) -> Result<Self, SteamAudioError> {
        let mut probe_batch = Self(std::ptr::null_mut());

        let status = unsafe {
            audionimbus_sys::iplProbeBatchLoad(
                context.raw_ptr(),
                serialized_object.raw_ptr(),
                probe_batch.raw_ptr_mut(),
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        Ok(probe_batch)
    }

    pub fn raw_ptr(&self) -> audionimbus_sys::IPLProbeBatch {
        self.0
    }

    pub fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLProbeBatch {
        &mut self.0
    }
}

impl Clone for ProbeBatch {
    fn clone(&self) -> Self {
        unsafe {
            audionimbus_sys::iplProbeBatchRetain(self.0);
        }
        Self(self.0)
    }
}

impl Drop for ProbeBatch {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplProbeBatchRelease(&mut self.0) }
    }
}

unsafe impl Send for ProbeBatch {}
unsafe impl Sync for ProbeBatch {}
