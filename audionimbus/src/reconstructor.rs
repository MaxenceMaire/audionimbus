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

    /// The largest possible duration (in seconds) of any impulse response that will be reconstructed.
    /// Used for validation when calling [`Self::reconstruct`].
    max_duration: f32,

    /// The largest possible Ambisonic order of any impulse response that will be reconstructed.
    /// Used for validation when calling [`Self::reconstruct`].
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
    ///
    /// # Errors
    ///
    /// Returns:
    /// - [`ReconstructorError::DurationExceedsMax`] if `shared_inputs.duration` exceeds the max duration.
    /// - [`ReconstructorError::OrderExceedsMax`] if `shared_inputs.order` exceeds the max order.
    /// - [`ReconstructorError::InputOutputLengthMismatch`] if `inputs` and `outputs` have different lengths.
    pub fn reconstruct(
        &self,
        inputs: &[ReconstructorInputs],
        shared_inputs: &ReconstructorSharedInputs,
        outputs: &[ReconstructorOutputs],
    ) -> Result<(), ReconstructorError> {
        if shared_inputs.duration > self.max_duration {
            return Err(ReconstructorError::DurationExceedsMax {
                duration: shared_inputs.duration,
                max_duration: self.max_duration,
            });
        }

        if shared_inputs.order > self.max_order {
            return Err(ReconstructorError::OrderExceedsMax {
                order: shared_inputs.order,
                max_order: self.max_order,
            });
        }

        if inputs.len() != outputs.len() {
            return Err(ReconstructorError::InputOutputLengthMismatch {
                inputs_len: inputs.len(),
                outputs_len: outputs.len(),
            });
        }

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

        Ok(())
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

/// [`Reconstructor`] errors.
#[derive(Debug, PartialEq)]
pub enum ReconstructorError {
    /// Duration exceeds the maximum duration specified in the reconstructor's settings.
    DurationExceedsMax { duration: f32, max_duration: f32 },
    /// Order exceeds the maximum order specified in the reconstructor's settings.
    OrderExceedsMax { order: u32, max_order: u32 },
    /// Input and output arrays have mismatched lengths.
    InputOutputLengthMismatch {
        inputs_len: usize,
        outputs_len: usize,
    },
}

impl std::error::Error for ReconstructorError {}

impl std::fmt::Display for ReconstructorError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::DurationExceedsMax {
                duration,
                max_duration,
            } => write!(
                f,
                "duration {duration} exceeds max duration {max_duration}"
            ),
            Self::OrderExceedsMax { order, max_order } => {
                write!(f, "order {order} exceeds max order {max_order}")
            }
            Self::InputOutputLengthMismatch {
                inputs_len,
                outputs_len,
            } => write!(
                f,
                "inputs and outputs length mismatch: inputs_len={inputs_len}, outputs_len={outputs_len}"
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::*;

    mod reconstructor {
        use super::*;

        const SAMPLING_RATE: u32 = 48_000;
        const MAX_DURATION: f32 = 2.0;
        const MAX_ORDER: u32 = 2;
        const VALID_DURATION: f32 = 1.0;
        const VALID_ORDER: u32 = 1;

        #[test]
        fn test_valid() {
            let context = Context::default();

            let reconstructor_settings = ReconstructorSettings {
                max_duration: MAX_DURATION,
                max_order: MAX_ORDER,
                sampling_rate: SAMPLING_RATE,
            };

            let reconstructor = Reconstructor::try_new(&context, &reconstructor_settings).unwrap();

            let shared_inputs = ReconstructorSharedInputs {
                duration: VALID_DURATION,
                order: VALID_ORDER,
            };

            let energy_field = EnergyField::try_new(
                &context,
                &EnergyFieldSettings {
                    duration: VALID_DURATION,
                    order: VALID_ORDER,
                },
            )
            .unwrap();
            let inputs = vec![ReconstructorInputs {
                energy_field: &energy_field,
            }];

            let mut impulse_response = ImpulseResponse::try_new(
                &context,
                &ImpulseResponseSettings {
                    duration: VALID_DURATION,
                    order: VALID_ORDER,
                    sampling_rate: SAMPLING_RATE,
                },
            )
            .unwrap();
            let outputs = vec![ReconstructorOutputs {
                impulse_response: &mut impulse_response,
            }];

            let result = reconstructor.reconstruct(&inputs, &shared_inputs, &outputs);

            assert!(result.is_ok());
        }

        #[test]
        fn test_duration_exceeds_max() {
            let context = Context::default();

            let reconstructor_settings = ReconstructorSettings {
                max_duration: MAX_DURATION,
                max_order: MAX_ORDER,
                sampling_rate: SAMPLING_RATE,
            };

            let reconstructor = Reconstructor::try_new(&context, &reconstructor_settings).unwrap();

            let invalid_duration = MAX_DURATION + 1.0;

            let shared_inputs = ReconstructorSharedInputs {
                duration: invalid_duration,
                order: VALID_ORDER,
            };

            let energy_field = EnergyField::try_new(
                &context,
                &EnergyFieldSettings {
                    duration: VALID_DURATION,
                    order: VALID_ORDER,
                },
            )
            .unwrap();
            let inputs = vec![ReconstructorInputs {
                energy_field: &energy_field,
            }];

            let mut impulse_response = ImpulseResponse::try_new(
                &context,
                &ImpulseResponseSettings {
                    duration: VALID_DURATION,
                    order: VALID_ORDER,
                    sampling_rate: SAMPLING_RATE,
                },
            )
            .unwrap();
            let outputs = vec![ReconstructorOutputs {
                impulse_response: &mut impulse_response,
            }];

            assert_eq!(
                reconstructor.reconstruct(&inputs, &shared_inputs, &outputs),
                Err(ReconstructorError::DurationExceedsMax {
                    duration: invalid_duration,
                    max_duration: MAX_DURATION,
                }),
            );
        }

        #[test]
        fn test_order_exceeds_max() {
            let context = Context::default();

            let reconstructor_settings = ReconstructorSettings {
                max_duration: MAX_DURATION,
                max_order: MAX_ORDER,
                sampling_rate: SAMPLING_RATE,
            };

            let reconstructor = Reconstructor::try_new(&context, &reconstructor_settings).unwrap();

            let invalid_order = MAX_ORDER + 1;

            let shared_inputs = ReconstructorSharedInputs {
                duration: MAX_DURATION,
                order: invalid_order,
            };

            let energy_field = EnergyField::try_new(
                &context,
                &EnergyFieldSettings {
                    duration: VALID_DURATION,
                    order: VALID_ORDER,
                },
            )
            .unwrap();
            let inputs = vec![ReconstructorInputs {
                energy_field: &energy_field,
            }];

            let mut impulse_response = ImpulseResponse::try_new(
                &context,
                &ImpulseResponseSettings {
                    duration: VALID_DURATION,
                    order: VALID_ORDER,
                    sampling_rate: SAMPLING_RATE,
                },
            )
            .unwrap();
            let outputs = vec![ReconstructorOutputs {
                impulse_response: &mut impulse_response,
            }];

            assert_eq!(
                reconstructor.reconstruct(&inputs, &shared_inputs, &outputs),
                Err(ReconstructorError::OrderExceedsMax {
                    order: invalid_order,
                    max_order: MAX_ORDER,
                }),
            );
        }

        #[test]
        fn test_input_output_length_mismatch() {
            let context = Context::default();

            let reconstructor_settings = ReconstructorSettings {
                max_duration: MAX_DURATION,
                max_order: MAX_ORDER,
                sampling_rate: SAMPLING_RATE,
            };

            let reconstructor = Reconstructor::try_new(&context, &reconstructor_settings).unwrap();

            let shared_inputs = ReconstructorSharedInputs {
                duration: VALID_DURATION,
                order: VALID_ORDER,
            };

            let energy_field = EnergyField::try_new(
                &context,
                &EnergyFieldSettings {
                    duration: VALID_DURATION,
                    order: VALID_ORDER,
                },
            )
            .unwrap();
            let inputs = vec![
                ReconstructorInputs {
                    energy_field: &energy_field,
                },
                ReconstructorInputs {
                    energy_field: &energy_field,
                },
            ];

            let mut impulse_response = ImpulseResponse::try_new(
                &context,
                &ImpulseResponseSettings {
                    duration: VALID_DURATION,
                    order: VALID_ORDER,
                    sampling_rate: SAMPLING_RATE,
                },
            )
            .unwrap();
            let outputs = vec![ReconstructorOutputs {
                impulse_response: &mut impulse_response,
            }];

            assert_eq!(
                reconstructor.reconstruct(&inputs, &shared_inputs, &outputs),
                Err(ReconstructorError::InputOutputLengthMismatch {
                    inputs_len: 2,
                    outputs_len: 1,
                }),
            );
        }
    }
}
