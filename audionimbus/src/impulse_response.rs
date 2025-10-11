use crate::audio_buffer::Sample;
use crate::context::Context;
use crate::error::{to_option_error, SteamAudioError};

/// An impulse response.
///
/// Impulse responses are represented in Ambisonics to allow for directional variation of propagated sound.
///
/// Impulse response data is stored as a 2D array of size #channels * #samples, in row-major order.
#[derive(Debug)]
pub struct ImpulseResponse(audionimbus_sys::IPLImpulseResponse);

impl ImpulseResponse {
    pub fn try_new(
        context: &Context,
        impulse_response_settings: &ImpulseResponseSettings,
    ) -> Result<Self, SteamAudioError> {
        let mut impulse_response = Self(std::ptr::null_mut());

        let status = unsafe {
            audionimbus_sys::iplImpulseResponseCreate(
                context.raw_ptr(),
                &mut audionimbus_sys::IPLImpulseResponseSettings::from(impulse_response_settings),
                impulse_response.raw_ptr_mut(),
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        Ok(impulse_response)
    }

    /// Returns the number of channels in the impulse response.
    pub fn num_channels(&self) -> u32 {
        unsafe { audionimbus_sys::iplImpulseResponseGetNumChannels(self.raw_ptr()) as u32 }
    }

    /// Returns the number of samples in the impulse response.
    pub fn num_samples(&self) -> u32 {
        unsafe { audionimbus_sys::iplImpulseResponseGetNumSamples(self.raw_ptr()) as u32 }
    }

    /// Returns a pointer to the data stored in the impulse response, , in row-major order.
    pub fn data(&self) -> &[Sample] {
        let ptr = unsafe { audionimbus_sys::iplImpulseResponseGetData(self.raw_ptr()) };
        let len = self.num_channels() * self.num_samples();
        unsafe { std::slice::from_raw_parts(ptr, len as usize) }
    }

    /// Returns a pointer to the data stored in the impulse response for the given channel, in row-major order.
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
            audionimbus_sys::iplImpulseResponseGetChannel(self.raw_ptr(), channel_index as i32)
        };
        let len = self.num_samples();
        unsafe { std::slice::from_raw_parts(ptr, len as usize) }
    }

    /// Resets all values stored in the impulse response to zero.
    pub fn reset(&mut self) {
        unsafe { audionimbus_sys::iplImpulseResponseReset(self.raw_ptr()) }
    }

    /// Copies data from `self` into the `dst` impulse response.
    ///
    /// If the source and destination impulse responses have different numbers of channels, only the smaller of the two numbers of channels will be copied.
    ///
    /// If the source and destination impulse responses have different numbers of samples, only the smaller of the two numbers of samples will be copied.
    pub fn copy_into(&self, dst: &mut ImpulseResponse) {
        unsafe { audionimbus_sys::iplImpulseResponseCopy(self.raw_ptr(), dst.raw_ptr()) }
    }

    /// Swaps the data contained in one impulse response with the data contained in another impulse response.
    ///
    /// The two impulse responses may contain different numbers of channels or samples.
    pub fn swap(&mut self, other: &mut ImpulseResponse) {
        unsafe { audionimbus_sys::iplImpulseResponseSwap(self.raw_ptr(), other.raw_ptr()) }
    }

    /// Adds the values stored in the `other` impulse response to those in `self`.
    ///
    /// If the impulse responses have different numbers of channels, only the smallest of the three numbers of channels will be added.
    ///
    /// If the impulse responses have different numbers of samples, only the smallest of the three numbers of samples will be added.
    pub fn add(&mut self, other: &ImpulseResponse) {
        unsafe {
            audionimbus_sys::iplImpulseResponseAdd(self.raw_ptr(), other.raw_ptr(), self.raw_ptr())
        }
    }

    /// Scales the values stored in the impulse response by a scalar.
    pub fn scale(&mut self, scalar: f32) {
        unsafe { audionimbus_sys::iplImpulseResponseScale(self.raw_ptr(), scalar, self.raw_ptr()) }
    }

    pub fn raw_ptr(&self) -> audionimbus_sys::IPLImpulseResponse {
        self.0
    }

    pub fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLImpulseResponse {
        &mut self.0
    }
}

impl Clone for ImpulseResponse {
    fn clone(&self) -> Self {
        unsafe {
            audionimbus_sys::iplImpulseResponseRetain(self.0);
        }
        Self(self.0)
    }
}

impl Drop for ImpulseResponse {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplImpulseResponseRelease(&mut self.0) }
    }
}

unsafe impl Send for ImpulseResponse {}
unsafe impl Sync for ImpulseResponse {}

/// Settings used to create an impulse response.
#[derive(Debug)]
pub struct ImpulseResponseSettings {
    /// Total duration (in seconds) of the impulse response.
    ///
    /// This determines the number of samples in each channel.
    pub duration: f32,

    /// The Ambisonic order.
    ///
    /// This determines the number of channels.
    pub order: u32,

    /// The sampling rate.
    ///
    /// This, together with the duration, determines the number of samples in each channel.
    pub sampling_rate: u32,
}

impl From<&ImpulseResponseSettings> for audionimbus_sys::IPLImpulseResponseSettings {
    fn from(settings: &ImpulseResponseSettings) -> Self {
        Self {
            duration: settings.duration,
            order: settings.order as i32,
            samplingRate: settings.sampling_rate as i32,
        }
    }
}

/// Adds the values stored in two impulse responses, and stores the result in a third impulse response.
///
/// If the impulse responses have different numbers of channels, only the smallest of the three numbers of channels will be added.
///
/// If the impulse responses have different numbers of bins, only the smallest of the three numbers of bins will be added.
pub fn add_impulse_responses(
    in1: &ImpulseResponse,
    in2: &ImpulseResponse,
    out: &mut ImpulseResponse,
) {
    unsafe { audionimbus_sys::iplImpulseResponseAdd(in1.raw_ptr(), in2.raw_ptr(), out.raw_ptr()) }
}

/// Scales the values stored in an impulse response by a scalar, and stores the result in the `out` impulse response.
///
/// If the impulse responses have different numbers of channels, only the smallest of the two numbers of channels will be scaled.
///
/// If the impulse responses have different numbers of bins, only the smallest of the two numbers of bins will be scaled.
pub fn scale_impulse_response(
    impulse_response: &ImpulseResponse,
    scalar: f32,
    out: &mut ImpulseResponse,
) {
    unsafe {
        audionimbus_sys::iplImpulseResponseScale(impulse_response.raw_ptr(), scalar, out.raw_ptr())
    }
}

/// Scales the values stored in an impulse response by a scalar, and adds the result to a second impulse response.
///
/// If the impulse responses have different numbers of channels, only the smallest of the two numbers of channels will be added.
///
/// If the impulse responses have different numbers of bins, only the smallest of the two numbers of bins will be added.
pub fn scale_accum_impulse_response(
    impulse_response: &ImpulseResponse,
    scalar: f32,
    out: &mut ImpulseResponse,
) {
    unsafe {
        audionimbus_sys::iplImpulseResponseScaleAccum(
            impulse_response.raw_ptr(),
            scalar,
            out.raw_ptr(),
        )
    }
}
