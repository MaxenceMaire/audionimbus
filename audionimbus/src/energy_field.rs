//! Types and utilities for working with energy fields.

use crate::audio_buffer::Sample;
use crate::context::Context;
use crate::error::{to_option_error, SteamAudioError};
use crate::NUM_BANDS;

/// An energy field.
///
/// Energy fields represent a histogram of sound energy arriving at a point, as a function of incident direction, frequency band, and arrival time.
///
/// Time is subdivided into “bins” of the histogram, with each bin corresponding to 10ms.
/// For each bin, incident energy is stored separately for each frequency band.
/// For a given frequency band and time bin, we store an Ambisonic representation of the variation of incident energy as a function of direction.
///
/// Energy field data is stored as a 3D array of size #channels * #bands * #bins, in row-major order.
#[derive(Debug)]
pub struct EnergyField(pub(crate) audionimbus_sys::IPLEnergyField);

impl EnergyField {
    /// Creates a new energy field.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if creation fails.
    pub fn try_new(
        context: &Context,
        energy_field_settings: &EnergyFieldSettings,
    ) -> Result<Self, SteamAudioError> {
        let mut energy_field = Self(std::ptr::null_mut());

        let status = unsafe {
            audionimbus_sys::iplEnergyFieldCreate(
                context.raw_ptr(),
                &mut audionimbus_sys::IPLEnergyFieldSettings::from(energy_field_settings),
                energy_field.raw_ptr_mut(),
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        Ok(energy_field)
    }

    /// Returns the number of channels in the energy field.
    pub fn num_channels(&self) -> u32 {
        unsafe { audionimbus_sys::iplEnergyFieldGetNumChannels(self.raw_ptr()) as u32 }
    }

    /// Returns the number of bins in the energy field.
    pub fn num_bins(&self) -> u32 {
        unsafe { audionimbus_sys::iplEnergyFieldGetNumBins(self.raw_ptr()) as u32 }
    }

    /// Returns the data stored in the energy field, in row-major order.
    pub fn data(&self) -> &[Sample] {
        let ptr = unsafe { audionimbus_sys::iplEnergyFieldGetData(self.raw_ptr()) };
        let len = self.num_channels() * NUM_BANDS * self.num_bins();
        unsafe { std::slice::from_raw_parts(ptr, len as usize) }
    }

    /// Returns the data stored in the energy field for the given channel, in row-major order.
    ///
    /// # Errors
    ///
    /// Returns [`EnergyFieldError::ChannelIndexOutOfBounds`] if `channel_index` is out of bounds.
    pub fn channel(&self, channel_index: u32) -> Result<&[Sample], EnergyFieldError> {
        let num_channels = self.num_channels();
        if channel_index >= num_channels {
            return Err(EnergyFieldError::ChannelIndexOutOfBounds {
                channel_index,
                num_channels,
            });
        }

        let ptr = unsafe {
            audionimbus_sys::iplEnergyFieldGetChannel(self.raw_ptr(), channel_index as i32)
        };
        let len = NUM_BANDS * self.num_bins();
        let data = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
        Ok(data)
    }

    /// Returns the data stored in the energy field for the given channel and band, in row-major order.
    ///
    /// # Errors
    ///
    /// Returns:
    /// - [`EnergyFieldError::ChannelIndexOutOfBounds`] if `channel_index` is out of bounds.
    /// - [`EnergyFieldError::BandIndexOutOfBounds`] if `band_index` is out of bounds.
    pub fn band(&self, channel_index: u32, band_index: u32) -> Result<&[Sample], EnergyFieldError> {
        let num_channels = self.num_channels();
        if channel_index >= num_channels {
            return Err(EnergyFieldError::ChannelIndexOutOfBounds {
                channel_index,
                num_channels,
            });
        }

        if band_index >= NUM_BANDS {
            return Err(EnergyFieldError::BandIndexOutOfBounds {
                band_index,
                max_bands: NUM_BANDS,
            });
        }

        let ptr = unsafe {
            audionimbus_sys::iplEnergyFieldGetBand(
                self.raw_ptr(),
                channel_index as i32,
                band_index as i32,
            )
        };
        let len = self.num_bins();
        let data = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
        Ok(data)
    }

    /// Resets all values stored in the energy field to zero.
    pub fn reset(&mut self) {
        unsafe { audionimbus_sys::iplEnergyFieldReset(self.raw_ptr()) }
    }

    /// Copies data from `self` into the `dst` energy field.
    ///
    /// If the source and destination energy fields have different numbers of channels, only the smaller of the two numbers of channels will be copied.
    ///
    /// If the source and destination energy fields have different numbers of bins, only the smaller of the two numbers of bins will be copied.
    pub fn copy_into(&self, dst: &mut Self) {
        unsafe { audionimbus_sys::iplEnergyFieldCopy(self.raw_ptr(), dst.raw_ptr()) }
    }

    /// Swaps the data contained in one energy field with the data contained in another energy field.
    ///
    /// The two energy fields may contain different numbers of channels or bins.
    pub fn swap(&mut self, other: &mut Self) {
        unsafe { audionimbus_sys::iplEnergyFieldSwap(self.raw_ptr(), other.raw_ptr()) }
    }

    /// Adds the values stored in the `other` energy field to those in `self`.
    ///
    /// If the energy fields have different numbers of channels, only the smallest of the three numbers of channels will be added.
    ///
    /// If the energy fields have different numbers of bins, only the smallest of the three numbers of bins will be added.
    pub fn add(&mut self, other: &Self) {
        unsafe {
            audionimbus_sys::iplEnergyFieldAdd(self.raw_ptr(), other.raw_ptr(), self.raw_ptr());
        }
    }

    /// Scales the values stored in the energy field by a scalar.
    pub fn scale(&mut self, scalar: f32) {
        unsafe { audionimbus_sys::iplEnergyFieldScale(self.raw_ptr(), scalar, self.raw_ptr()) }
    }

    /// Returns the raw FFI pointer to the underlying energy field.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub const fn raw_ptr(&self) -> audionimbus_sys::IPLEnergyField {
        self.0
    }

    /// Returns a mutable reference to the raw FFI pointer.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub const fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLEnergyField {
        &mut self.0
    }
}

impl Clone for EnergyField {
    fn clone(&self) -> Self {
        unsafe {
            audionimbus_sys::iplEnergyFieldRetain(self.0);
        }
        Self(self.0)
    }
}

impl Drop for EnergyField {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplEnergyFieldRelease(&raw mut self.0) }
    }
}

unsafe impl Send for EnergyField {}
unsafe impl Sync for EnergyField {}

/// Settings used to create an [`EnergyField`].
#[derive(Debug)]
pub struct EnergyFieldSettings {
    /// Total duration (in seconds) of the energy field.
    ///
    /// This determines the number of bins in each channel and band.
    pub duration: f32,

    /// The Ambisonic order.
    ///
    /// This determines the number of channels.
    pub order: u32,
}

impl From<&EnergyFieldSettings> for audionimbus_sys::IPLEnergyFieldSettings {
    fn from(settings: &EnergyFieldSettings) -> Self {
        Self {
            duration: settings.duration,
            order: settings.order as i32,
        }
    }
}

/// [`EnergyField`] errors.
#[derive(Debug, PartialEq, Eq)]
pub enum EnergyFieldError {
    /// Channel index is out of bounds.
    ChannelIndexOutOfBounds {
        channel_index: u32,
        num_channels: u32,
    },
    /// Band index is out of bounds.
    BandIndexOutOfBounds { band_index: u32, max_bands: u32 },
}

impl std::error::Error for EnergyFieldError {}

impl std::fmt::Display for EnergyFieldError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::ChannelIndexOutOfBounds {
                channel_index,
                num_channels,
            } => write!(
                f,
                "channel index {channel_index} out of bounds (num_channels: {num_channels})"
            ),
            Self::BandIndexOutOfBounds {
                band_index,
                max_bands,
            } => write!(
                f,
                "band index {band_index} out of bounds (max_bands: {max_bands})"
            ),
        }
    }
}

/// Adds the values stored in two energy fields, and stores the result in a third energy field.
///
/// If the energy fields have different numbers of channels, only the smallest of the three numbers of channels will be added.
///
/// If the energy fields have different numbers of bins, only the smallest of the three numbers of bins will be added.
pub fn add_energy_fields(in1: &EnergyField, in2: &EnergyField, out: &mut EnergyField) {
    unsafe { audionimbus_sys::iplEnergyFieldAdd(in1.raw_ptr(), in2.raw_ptr(), out.raw_ptr()) }
}

/// Scales the values stored in an energy field by a scalar, and stores the result in the `out` energy field.
///
/// If the energy fields have different numbers of channels, only the smallest of the two numbers of channels will be scaled.
///
/// If the energy fields have different numbers of bins, only the smallest of the two numbers of bins will be scaled.
pub fn scale_energy_field(energy_field: &EnergyField, scalar: f32, out: &mut EnergyField) {
    unsafe { audionimbus_sys::iplEnergyFieldScale(energy_field.raw_ptr(), scalar, out.raw_ptr()) }
}

/// Scales the values stored in an energy field by a scalar, and adds the result to a second energy field.
///
/// If the energy fields have different numbers of channels, only the smallest of the two numbers of channels will be added.
///
/// If the energy fields have different numbers of bins, only the smallest of the two numbers of bins will be added.
pub fn scale_accum_energy_field(energy_field: &EnergyField, scalar: f32, out: &mut EnergyField) {
    unsafe {
        audionimbus_sys::iplEnergyFieldScaleAccum(energy_field.raw_ptr(), scalar, out.raw_ptr());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;

    mod energy_field {
        use super::*;

        mod channel {
            use super::*;

            #[test]
            fn test_valid() {
                let context = Context::default();

                let settings = EnergyFieldSettings {
                    duration: 1.0,
                    order: 1,
                };
                let energy_field = EnergyField::try_new(&context, &settings).unwrap();

                // Access valid channel
                let result = energy_field.channel(2);
                assert!(result.is_ok());

                // Access last valid channel
                let result = energy_field.channel(3);
                assert!(result.is_ok());
            }

            #[test]
            fn test_index_out_of_bounds() {
                let context = Context::default();

                let energy_field = EnergyField::try_new(
                    &context,
                    &EnergyFieldSettings {
                        duration: 1.0,
                        order: 1,
                    },
                )
                .unwrap();

                assert_eq!(
                    energy_field.channel(5),
                    Err(EnergyFieldError::ChannelIndexOutOfBounds {
                        channel_index: 5,
                        num_channels: 4,
                    }),
                );
            }
        }

        mod band {
            use super::*;

            #[test]
            fn test_valid() {
                let context = Context::default();

                let settings = EnergyFieldSettings {
                    duration: 1.0,
                    order: 1,
                };

                let energy_field = EnergyField::try_new(&context, &settings).unwrap();

                assert!(energy_field.band(0, 0).is_ok());
                assert!(energy_field.band(3, NUM_BANDS - 1).is_ok());
            }

            #[test]
            fn test_band_index_out_of_bounds() {
                let context = Context::default();

                let settings = EnergyFieldSettings {
                    duration: 1.0,
                    order: 1,
                };

                let energy_field = EnergyField::try_new(&context, &settings).unwrap();

                assert_eq!(
                    energy_field.band(0, 5),
                    Err(EnergyFieldError::BandIndexOutOfBounds {
                        band_index: 5,
                        max_bands: 3,
                    }),
                );
            }

            #[test]
            fn test_band_channel_index_out_of_bounds() {
                let context = Context::default();

                let settings = EnergyFieldSettings {
                    duration: 1.0,
                    order: 1,
                };

                let energy_field = EnergyField::try_new(&context, &settings).unwrap();

                assert_eq!(
                    energy_field.band(10, 0),
                    Err(EnergyFieldError::ChannelIndexOutOfBounds {
                        channel_index: 10,
                        num_channels: 4,
                    }),
                );
            }
        }
    }
}
