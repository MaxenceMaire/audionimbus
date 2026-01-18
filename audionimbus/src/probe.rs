//! Sound probe generation and storage.

use crate::context::Context;
use crate::energy_field::EnergyField;
use crate::error::{to_option_error, SteamAudioError};
use crate::geometry::{Matrix, Scene, Sphere};
use crate::serialized_object::SerializedObject;
use crate::simulation::BakedDataIdentifier;

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

    /// Returns the number of probes in the probe array.
    pub fn num_probes(&self) -> usize {
        unsafe { audionimbus_sys::iplProbeArrayGetNumProbes(self.raw_ptr()) as usize }
    }

    /// Returns the probe at a given index in the probe array.
    pub fn probe(&self, index: usize) -> Sphere {
        assert!(index < self.num_probes(), "probe index out of bounds");

        let ipl_sphere =
            unsafe { audionimbus_sys::iplProbeArrayGetProbe(self.raw_ptr(), index as i32) };

        Sphere::from(ipl_sphere)
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

    /// Returns the number of probes in the probe batch.
    pub fn num_probes(&self) -> usize {
        unsafe { audionimbus_sys::iplProbeBatchGetNumProbes(self.raw_ptr()) as usize }
    }

    /// Returns the size (in bytes) of a specific baked data layer in the probe batch.
    pub fn data_size(&self, identifier: BakedDataIdentifier) -> usize {
        let mut ffi_identifier: audionimbus_sys::IPLBakedDataIdentifier = identifier.into();

        unsafe {
            audionimbus_sys::iplProbeBatchGetDataSize(self.raw_ptr(), &mut ffi_identifier as *mut _)
                as usize
        }
    }

    pub fn remove_data(&mut self, identifier: BakedDataIdentifier) {
        let mut ffi_identifier: audionimbus_sys::IPLBakedDataIdentifier = identifier.into();

        unsafe {
            audionimbus_sys::iplProbeBatchRemoveData(self.raw_ptr(), &mut ffi_identifier as *mut _)
        }
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

    /// Removes a probe from the batch.
    pub fn remove_probe(&mut self, probe_index: usize) {
        assert!(probe_index < self.num_probes(), "probe index out of bounds");

        unsafe {
            audionimbus_sys::iplProbeBatchRemoveProbe(self.raw_ptr(), probe_index as i32);
        }
    }

    /// Adds every probe in an array to a batch.
    /// The new probes will be added, in order, at the end of the batch.
    pub fn add_probe_array(&mut self, probe_array: &ProbeArray) {
        unsafe {
            audionimbus_sys::iplProbeBatchAddProbeArray(self.raw_ptr(), probe_array.raw_ptr());
        }
    }

    /// Retrieves a single array of parametric reverb times in a specific baked data layer of a specific probe in the probe batch.
    pub fn reverb(&self, identifier: BakedDataIdentifier, probe_index: usize) -> [f32; 3] {
        assert!(probe_index < self.num_probes(), "probe index out of bounds");

        let mut ffi_identifier: audionimbus_sys::IPLBakedDataIdentifier = identifier.into();
        let mut reverb_times: [f32; 3] = [0.0; 3];

        unsafe {
            audionimbus_sys::iplProbeBatchGetReverb(
                self.raw_ptr(),
                &mut ffi_identifier as *mut _,
                probe_index as i32,
                reverb_times.as_mut_ptr(),
            );
        }

        reverb_times
    }

    /// Retrieves a single energy field in a specific baked data layer of a specific probe in the probe batch.
    pub fn energy_field(&self, identifier: BakedDataIdentifier, probe_index: usize) -> EnergyField {
        assert!(probe_index < self.num_probes(), "probe index out of bounds");

        let mut ffi_identifier: audionimbus_sys::IPLBakedDataIdentifier = identifier.into();
        let energy_field = EnergyField(std::ptr::null_mut());

        unsafe {
            audionimbus_sys::iplProbeBatchGetEnergyField(
                self.raw_ptr(),
                &mut ffi_identifier as *mut _,
                probe_index as i32,
                energy_field.raw_ptr(),
            );
        }

        energy_field
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Material, Point, SceneSettings, StaticMesh, StaticMeshSettings, Triangle};

    mod probe_array {
        use super::*;

        #[test]
        fn test_try_new() {
            let context = Context::default();
            let probe_array = ProbeArray::try_new(&context);
            assert!(probe_array.is_ok());
        }

        #[test]
        fn test_generation_centroid() {
            let context = Context::default();
            let scene_settings = SceneSettings::default();
            let scene = Scene::try_new(&context, &scene_settings).expect("failed to create scene");
            let mut probe_array = ProbeArray::try_new(&context).unwrap();

            let transform = Matrix::new([
                [10.0, 0.0, 0.0, 0.0],
                [0.0, 10.0, 0.0, 0.0],
                [0.0, 0.0, 10.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ]);

            let params = ProbeGenerationParams::Centroid { transform };
            probe_array.generate_probes(&scene, &params);

            assert_eq!(probe_array.num_probes(), 1);
        }

        #[test]
        fn test_generation_uniform_floor() {
            let context = Context::default();
            let scene_settings = SceneSettings::default();
            let mut scene =
                Scene::try_new(&context, &scene_settings).expect("failed to create scene");

            // Add a floor mesh.
            let vertices = vec![
                Point::new(-50.0, 0.0, -50.0),
                Point::new(50.0, 0.0, -50.0),
                Point::new(50.0, 0.0, 50.0),
                Point::new(-50.0, 0.0, 50.0),
            ];
            let triangles = vec![Triangle::new(0, 1, 2), Triangle::new(0, 2, 3)];
            let material_indices = vec![0, 0];
            let materials = vec![Material::default()];

            let static_mesh_settings = StaticMeshSettings {
                vertices: &vertices,
                triangles: &triangles,
                material_indices: &material_indices,
                materials: &materials,
            };
            let static_mesh = StaticMesh::try_new(&scene, &static_mesh_settings).unwrap();
            scene.add_static_mesh(static_mesh);
            scene.commit();

            let mut probe_array = ProbeArray::try_new(&context).unwrap();
            let transform = Matrix::new([
                [100.0, 0.0, 0.0, 0.0],
                [0.0, 100.0, 0.0, 0.0],
                [0.0, 0.0, 100.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ]);

            let params = ProbeGenerationParams::UniformFloor {
                spacing: 10.0,
                height: 1.5,
                transform,
            };

            probe_array.generate_probes(&scene, &params);
            assert_eq!(probe_array.num_probes(), 121);
        }
    }

    mod probe_batch {
        use super::*;

        #[test]
        fn test_try_new() {
            let context = Context::default();
            let probe_batch = ProbeBatch::try_new(&context);
            assert!(probe_batch.is_ok());
        }

        #[test]
        fn test_add_remove() {
            let context = Context::default();
            let mut probe_batch = ProbeBatch::try_new(&context).unwrap();

            let probe = Sphere {
                center: Point::new(0.0, 0.0, 0.0),
                radius: 1.0,
            };

            probe_batch.add_probe(&probe);
            probe_batch.commit();
            assert_eq!(probe_batch.num_probes(), 1);

            probe_batch.remove_probe(0);
            probe_batch.commit();
            assert_eq!(probe_batch.num_probes(), 0);
        }

        #[test]
        fn test_add_array() {
            let context = Context::default();
            let scene_settings = SceneSettings::default();
            let scene = Scene::try_new(&context, &scene_settings).expect("failed to create scene");

            let mut probe_array = ProbeArray::try_new(&context).unwrap();
            let transform = Matrix::new([
                [10.0, 0.0, 0.0, 0.0],
                [0.0, 10.0, 0.0, 0.0],
                [0.0, 0.0, 10.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ]);
            let params = ProbeGenerationParams::Centroid { transform };
            probe_array.generate_probes(&scene, &params);

            let mut probe_batch = ProbeBatch::try_new(&context).unwrap();
            probe_batch.add_probe_array(&probe_array);
            probe_batch.commit();

            assert_eq!(probe_batch.num_probes(), probe_array.num_probes());
        }

        #[test]
        #[should_panic(expected = "probe index out of bounds")]
        fn test_remove_out_of_bounds() {
            let context = Context::default();
            let mut probe_batch = ProbeBatch::try_new(&context).unwrap();
            probe_batch.remove_probe(0); // No probes exist.
        }
    }
}
