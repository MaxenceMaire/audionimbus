//! Reconstruction of impulse responses from simulation data.

use crate::context::Context;
use crate::energy_field::EnergyField;
use crate::error::{to_option_error, SteamAudioError};
use crate::impulse_response::ImpulseResponse;

/// An object that can convert energy fields to impulse responses.
///
/// Energy fields are typically much smaller in size than impulse responses, and therefore are used when storing baked reflections or reverb in a probe batch, but impulse responses are what is needed at runtime for convolution.
#[derive(Debug)]
pub struct Reconstructor {
    inner: audionimbus_sys::IPLReconstructor,

    /// Used for validation when calling [`Self::reconstruct`].
    max_duration: f32,
    max_order: u32,
}

impl Reconstructor {
    /// Creates a new reconstructor.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if creation fails.
    pub fn try_new(
        context: &Context,
        reconstructor_settings: &ReconstructorSettings,
    ) -> Result<Self, SteamAudioError> {
        let mut reconstructor = Self {
            inner: std::ptr::null_mut(),
            max_duration: reconstructor_settings.max_duration,
            max_order: reconstructor_settings.max_order,
        };

        let status = unsafe {
            audionimbus_sys::iplReconstructorCreate(
                context.raw_ptr(),
                &mut audionimbus_sys::IPLReconstructorSettings::from(reconstructor_settings),
                reconstructor.raw_ptr_mut(),
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        Ok(reconstructor)
    }

    /// Reconstructs one or more impulse responses as a single batch of work.
    pub fn reconstruct(
        &self,
        inputs: &[ReconstructorInputs],
        shared_inputs: &ReconstructorSharedInputs,
        outputs: &[ReconstructorOutputs],
    ) {
        assert!(shared_inputs.duration <= self.max_duration, "duration must be less than or equal to the max duration specified in the reconstructor's settings");
        assert!(shared_inputs.order <= self.max_order, "order must be less than or equal to the max order specified in the reconstructor's settings");
        assert_eq!(inputs.len(), outputs.len());

        let c_inputs: Vec<audionimbus_sys::IPLReconstructorInputs> = inputs
            .iter()
            .map(audionimbus_sys::IPLReconstructorInputs::from)
            .collect();

        let c_outputs: Vec<audionimbus_sys::IPLReconstructorOutputs> = outputs
            .iter()
            .map(audionimbus_sys::IPLReconstructorOutputs::from)
            .collect();

        let c_shared_inputs = audionimbus_sys::IPLReconstructorSharedInputs::from(shared_inputs);

        unsafe {
            audionimbus_sys::iplReconstructorReconstruct(
                self.raw_ptr(),
                inputs.len() as i32,
                c_inputs.as_ptr() as *mut audionimbus_sys::IPLReconstructorInputs,
                &c_shared_inputs as *const audionimbus_sys::IPLReconstructorSharedInputs
                    as *mut audionimbus_sys::IPLReconstructorSharedInputs,
                c_outputs.as_ptr() as *mut audionimbus_sys::IPLReconstructorOutputs,
            )
        }
    }

    /// Returns the raw FFI pointer to the underlying reconstructor.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub fn raw_ptr(&self) -> audionimbus_sys::IPLReconstructor {
        self.inner
    }

    /// Returns a mutable reference to the raw FFI pointer.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLReconstructor {
        &mut self.inner
    }
}

impl Clone for Reconstructor {
    fn clone(&self) -> Self {
        unsafe {
            audionimbus_sys::iplReconstructorRetain(self.inner);
        }
        Self {
            inner: self.inner,
            max_duration: self.max_duration,
            max_order: self.max_order,
        }
    }
}

impl Drop for Reconstructor {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplReconstructorRelease(&mut self.inner) }
    }
}

unsafe impl Send for Reconstructor {}
unsafe impl Sync for Reconstructor {}

/// Settings used to create a reconstructor.
#[derive(Debug)]
pub struct ReconstructorSettings {
    /// The largest possible duration (in seconds) of any impulse response that will be reconstructed using this reconstructor.
    pub max_duration: f32,

    /// The largest possible Ambisonic order of any impulse response that will be reconstructed using this reconstructor.
    pub max_order: u32,

    /// The sampling rate of impulse responses reconstructed using this reconstructor.
    pub sampling_rate: u32,
}

impl From<&ReconstructorSettings> for audionimbus_sys::IPLReconstructorSettings {
    fn from(settings: &ReconstructorSettings) -> Self {
        Self {
            maxDuration: settings.max_duration,
            maxOrder: settings.max_order as i32,
            samplingRate: settings.sampling_rate as i32,
        }
    }
}

/// Inputs common to all reconstruction operations specified in a single call to
/// [`Reconstructor::reconstruct`].
#[derive(Debug)]
pub struct ReconstructorSharedInputs {
    /// Duration of impulse responses to reconstruct.
    ///
    /// Must be less than or equal to maxDuration specified in [`ReconstructorSettings`].
    pub duration: f32,

    /// Ambisonic order of impulse responses to reconstruct.
    ///
    /// Must be less than or equal to maxOrder specified in [`ReconstructorSettings`].
    pub order: u32,
}

impl From<&ReconstructorSharedInputs> for audionimbus_sys::IPLReconstructorSharedInputs {
    fn from(reconstructor_shared_inputs: &ReconstructorSharedInputs) -> Self {
        Self {
            duration: reconstructor_shared_inputs.duration,
            order: reconstructor_shared_inputs.order as i32,
        }
    }
}

/// The inputs for a single reconstruction operation.
#[derive(Debug)]
pub struct ReconstructorInputs<'a> {
    /// The energy field from which to reconstruct an impulse response.
    pub energy_field: &'a EnergyField,
}

impl From<&ReconstructorInputs<'_>> for audionimbus_sys::IPLReconstructorInputs {
    fn from(reconstructor_inputs: &ReconstructorInputs) -> Self {
        Self {
            energyField: reconstructor_inputs.energy_field.raw_ptr(),
        }
    }
}

/// The outputs for a single reconstruction operation.
#[derive(Debug)]
pub struct ReconstructorOutputs<'a> {
    pub impulse_response: &'a mut ImpulseResponse,
}

impl From<&ReconstructorOutputs<'_>> for audionimbus_sys::IPLReconstructorOutputs {
    fn from(reconstructor_outputs: &ReconstructorOutputs) -> Self {
        Self {
            impulseResponse: reconstructor_outputs.impulse_response.raw_ptr(),
        }
    }
}
