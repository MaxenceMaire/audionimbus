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
    /// # Panics
    ///
    /// Panics if `channel_index` is out of bounds.
    pub fn channel(&self, channel_index: u32) -> &[Sample] {
        assert!(
            channel_index < self.num_channels(),
            "channel index out of bounds",
        );

        let ptr = unsafe {
            audionimbus_sys::iplEnergyFieldGetChannel(self.raw_ptr(), channel_index as i32)
        };
        let len = NUM_BANDS * self.num_bins();
        unsafe { std::slice::from_raw_parts(ptr, len as usize) }
    }

    /// Returns the data stored in the energy field for the given channel and band, in row-major order.
    ///
    /// # Panics
    ///
    /// Panics if `channel_index` or `band_index` are out of bounds.
    pub fn band(&self, channel_index: u32, band_index: u32) -> &[Sample] {
        assert!(
            channel_index < self.num_channels(),
            "channel index out of bounds",
        );

        assert!(band_index < NUM_BANDS, "band index out of bounds",);

        let ptr = unsafe {
            audionimbus_sys::iplEnergyFieldGetBand(
                self.raw_ptr(),
                channel_index as i32,
                band_index as i32,
            )
        };
        let len = self.num_bins();
        unsafe { std::slice::from_raw_parts(ptr, len as usize) }
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
    pub fn copy_into(&self, dst: &mut EnergyField) {
        unsafe { audionimbus_sys::iplEnergyFieldCopy(self.raw_ptr(), dst.raw_ptr()) }
    }

    /// Swaps the data contained in one energy field with the data contained in another energy field.
    ///
    /// The two energy fields may contain different numbers of channels or bins.
    pub fn swap(&mut self, other: &mut EnergyField) {
        unsafe { audionimbus_sys::iplEnergyFieldSwap(self.raw_ptr(), other.raw_ptr()) }
    }

    /// Adds the values stored in the `other` energy field to those in `self`.
    ///
    /// If the energy fields have different numbers of channels, only the smallest of the three numbers of channels will be added.
    ///
    /// If the energy fields have different numbers of bins, only the smallest of the three numbers of bins will be added.
    pub fn add(&mut self, other: &EnergyField) {
        unsafe {
            audionimbus_sys::iplEnergyFieldAdd(self.raw_ptr(), other.raw_ptr(), self.raw_ptr())
        }
    }

    /// Scales the values stored in the energy field by a scalar.
    pub fn scale(&mut self, scalar: f32) {
        unsafe { audionimbus_sys::iplEnergyFieldScale(self.raw_ptr(), scalar, self.raw_ptr()) }
    }

    pub fn raw_ptr(&self) -> audionimbus_sys::IPLEnergyField {
        self.0
    }

    pub fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLEnergyField {
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
        unsafe { audionimbus_sys::iplEnergyFieldRelease(&mut self.0) }
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
    pub order: usize,
}

impl From<&EnergyFieldSettings> for audionimbus_sys::IPLEnergyFieldSettings {
    fn from(settings: &EnergyFieldSettings) -> Self {
        Self {
            duration: settings.duration,
            order: settings.order as i32,
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
        audionimbus_sys::iplEnergyFieldScaleAccum(energy_field.raw_ptr(), scalar, out.raw_ptr())
    }
}
