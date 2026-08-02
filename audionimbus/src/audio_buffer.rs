//! Types and utilities for working with audio buffers.

use crate::context::Context;
use crate::effect::ambisonics::AmbisonicsType;
use crate::ffi_wrapper::FFIWrapper;
use smallvec::SmallVec;
use std::marker::PhantomData;

/// Number of channel pointers stored without a heap allocation.
const INLINE_CHANNEL_CAPACITY: usize = 16;

/// Audio sample type.
pub type Sample = f32;

/// Channel pointers
type ChannelPointers = SmallVec<[*mut Sample; INLINE_CHANNEL_CAPACITY]>;

mod sealed {
    use super::Sample;

    /// Supplies channel pointers to the public read interface.
    pub trait ChannelPointers {
        /// Returns the channel pointers.
        fn channel_ptrs(&self) -> &[*mut Sample];
    }
}

/// Read access to borrowed audio samples.
pub trait AudioBuffer: crate::sealed::Sealed + sealed::ChannelPointers {
    /// Returns the number of channels.
    fn num_channels(&self) -> usize {
        self.channel_ptrs().len()
    }

    /// Returns the number of samples per channel.
    fn num_samples(&self) -> usize;

    /// Returns a channel by index.
    fn channel(&self, index: usize) -> Option<&[Sample]> {
        self.channel_ptrs().get(index).map(|pointer| {
            // SAFETY: Implementations guarantee that every pointer is valid for `num_samples`.
            unsafe { std::slice::from_raw_parts(*pointer, AudioBuffer::num_samples(self)) }
        })
    }

    /// Returns an iterator over channels.
    fn channels(&self) -> impl ExactSizeIterator<Item = &[Sample]> + '_ {
        let num_samples = AudioBuffer::num_samples(self);
        self.channel_ptrs().iter().map(move |pointer| {
            // SAFETY: Implementations guarantee that every pointer is valid for `num_samples`.
            unsafe { std::slice::from_raw_parts(*pointer, num_samples) }
        })
    }

    /// Interleaves the channel data into `dst`.
    ///
    /// # Errors
    ///
    /// - [`AudioBufferOperationError::InterleaveLengthOverflow`] if the total sample count exceeds
    ///   the native indexing range.
    /// - [`AudioBufferOperationError::InterleaveLengthMismatch`] if `dst` does not match the total
    ///   sample count.
    fn interleave(
        &self,
        context: &Context,
        dst: &mut [Sample],
    ) -> Result<(), AudioBufferOperationError>
    where
        Self: Sized,
    {
        let expected_len = self
            .num_channels()
            .checked_mul(self.num_samples())
            .filter(|expected_len| i32::try_from(*expected_len).is_ok())
            .ok_or(AudioBufferOperationError::InterleaveLengthOverflow {
                num_channels: self.num_channels(),
                num_samples: self.num_samples(),
            })?;
        if dst.len() != expected_len {
            return Err(AudioBufferOperationError::InterleaveLengthMismatch {
                dst_len: dst.len(),
                expected_len,
            });
        }

        let mut input = view_as_ffi(self, self.channel_ptrs(), self.num_samples());
        unsafe {
            audionimbus_sys::iplAudioBufferInterleave(
                context.raw_ptr(),
                &raw mut *input,
                dst.as_mut_ptr(),
            );
        }

        Ok(())
    }

    /// Converts Ambisonic samples from `in_type` into `out_type`.
    ///
    /// Steam Audio processes N3D natively, so conversion is best kept at integration boundaries.
    ///
    /// # Errors
    ///
    /// - [`AudioBufferOperationError::ChannelCountMismatch`] if the channel counts differ.
    /// - [`AudioBufferOperationError::SampleCountMismatch`] if the per-channel sample counts differ.
    fn convert_ambisonics_into(
        &self,
        context: &Context,
        in_type: AmbisonicsType,
        out_type: AmbisonicsType,
        out: &mut AudioBufferMut<'_>,
    ) -> Result<(), AudioBufferOperationError>
    where
        Self: Sized,
    {
        validate_matching_shape(self, out)?;

        let mut input = view_as_ffi(self, self.channel_ptrs(), self.num_samples());
        let mut output = out.as_ffi_mut();
        unsafe {
            audionimbus_sys::iplAudioBufferConvertAmbisonics(
                context.raw_ptr(),
                in_type.into(),
                out_type.into(),
                &raw mut *input,
                &raw mut *output,
            );
        }

        Ok(())
    }
}

/// An immutable view over borrowed audio samples.
///
/// The view stores channel pointers, but never owns the samples they reference.
#[derive(Clone, Debug)]
pub struct AudioBufferRef<'a> {
    /// Number of samples in each channel.
    num_samples: usize,
    /// Pointer to the first sample in each channel.
    channel_ptrs: ChannelPointers,
    /// Ties the view to the borrowed samples.
    _samples: PhantomData<&'a [Sample]>,
}

impl<'a> AudioBufferRef<'a> {
    /// Constructs a view over contiguous `samples`.
    ///
    /// Each channel occupies one contiguous region of equal length.
    ///
    /// # Errors
    ///
    /// - [`AudioBufferError::NoChannels`] if `num_channels` is zero.
    /// - [`AudioBufferError::NoSamples`] if `samples` is empty.
    /// - [`AudioBufferError::DataLengthMismatch`] if the data cannot be divided evenly.
    /// - [`AudioBufferError::TooManyChannels`] if the channel count exceeds the native limit.
    /// - [`AudioBufferError::TooManySamples`] if the per-channel count exceeds the native limit.
    pub fn try_new(samples: &'a [Sample], num_channels: usize) -> Result<Self, AudioBufferError> {
        let num_samples = validate_contiguous_layout(samples.len(), num_channels)?;
        let channel_ptrs = samples
            .chunks_exact(num_samples)
            .map(|channel| channel.as_ptr().cast_mut())
            .collect();

        Ok(Self::from_validated_parts(channel_ptrs, num_samples))
    }

    /// Constructs a view over separate channels.
    ///
    /// # Errors
    ///
    /// - [`AudioBufferError::NoChannels`] if no channels are provided.
    /// - [`AudioBufferError::NoSamples`] if the channels are empty.
    /// - [`AudioBufferError::ChannelLengthMismatch`] if channel lengths differ.
    /// - [`AudioBufferError::TooManyChannels`] if the channel count exceeds the native limit.
    /// - [`AudioBufferError::TooManySamples`] if the per-channel count exceeds the native limit.
    pub fn try_from_channels<I>(channels: I) -> Result<Self, AudioBufferError>
    where
        I: IntoIterator<Item = &'a [Sample]>,
    {
        let mut channels = channels.into_iter();
        let first = channels.next().ok_or(AudioBufferError::NoChannels)?;
        let num_samples = first.len();
        let mut channel_ptrs = ChannelPointers::new();
        channel_ptrs.push(first.as_ptr().cast_mut());

        for (channel, samples) in channels.enumerate() {
            let channel = channel + 1;
            if samples.len() != num_samples {
                return Err(AudioBufferError::ChannelLengthMismatch {
                    channel,
                    expected: num_samples,
                    actual: samples.len(),
                });
            }
            channel_ptrs.push(samples.as_ptr().cast_mut());
        }

        validate_view_dimensions(channel_ptrs.len(), num_samples)?;
        Ok(Self::from_validated_parts(channel_ptrs, num_samples))
    }

    /// Returns the number of channels.
    pub fn num_channels(&self) -> usize {
        AudioBuffer::num_channels(self)
    }

    /// Returns the number of samples per channel.
    pub fn num_samples(&self) -> usize {
        AudioBuffer::num_samples(self)
    }

    /// Returns a channel by index.
    pub fn channel(&self, index: usize) -> Option<&[Sample]> {
        AudioBuffer::channel(self, index)
    }

    /// Returns an iterator over channels.
    pub fn channels(&self) -> impl ExactSizeIterator<Item = &[Sample]> + '_ {
        AudioBuffer::channels(self)
    }

    /// Constructs a view from raw channel pointers.
    ///
    /// # Errors
    ///
    /// - [`AudioBufferError::NoChannels`] if no pointers are provided.
    /// - [`AudioBufferError::NoSamples`] if `num_samples` is zero.
    /// - [`AudioBufferError::NullChannelPointer`] if a pointer is null.
    /// - [`AudioBufferError::TooManyChannels`] if the channel count exceeds the native limit.
    /// - [`AudioBufferError::TooManySamples`] if the sample count exceeds the native limit.
    ///
    /// # Safety
    ///
    /// Every pointer must be aligned, initialized, and valid to read `num_samples` consecutive
    /// samples for the returned view's lifetime.
    /// The referenced memory must not be mutated for that lifetime except through interior
    /// mutability that upholds Rust's aliasing rules.
    pub unsafe fn try_from_raw_parts(
        channel_ptrs: &[*const Sample],
        num_samples: usize,
    ) -> Result<Self, AudioBufferError> {
        validate_view_dimensions(channel_ptrs.len(), num_samples)?;
        let channel_ptrs = channel_ptrs
            .iter()
            .enumerate()
            .map(|(channel, pointer)| {
                if pointer.is_null() {
                    Err(AudioBufferError::NullChannelPointer { channel })
                } else {
                    Ok(pointer.cast_mut())
                }
            })
            .collect::<Result<_, _>>()?;

        Ok(Self::from_validated_parts(channel_ptrs, num_samples))
    }

    fn from_validated_parts(channel_ptrs: ChannelPointers, num_samples: usize) -> Self {
        Self {
            num_samples,
            channel_ptrs,
            _samples: PhantomData,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn as_ffi(&self) -> FFIWrapper<'_, audionimbus_sys::IPLAudioBuffer, Self> {
        view_as_ffi(self, &self.channel_ptrs, self.num_samples)
    }
}

impl<'a> TryFrom<&'a [Sample]> for AudioBufferRef<'a> {
    type Error = AudioBufferError;

    fn try_from(samples: &'a [Sample]) -> Result<Self, Self::Error> {
        Self::try_new(samples, 1)
    }
}

impl crate::sealed::Sealed for AudioBufferRef<'_> {}

impl sealed::ChannelPointers for AudioBufferRef<'_> {
    fn channel_ptrs(&self) -> &[*mut Sample] {
        &self.channel_ptrs
    }
}

impl AudioBuffer for AudioBufferRef<'_> {
    fn num_samples(&self) -> usize {
        self.num_samples
    }
}

// SAFETY: The view has the same read-only access semantics as `&[Sample]`.
unsafe impl Send for AudioBufferRef<'_> {}
// SAFETY: The view has the same read-only access semantics as `&[Sample]`.
unsafe impl Sync for AudioBufferRef<'_> {}

/// A mutable view over exclusively borrowed audio samples.
///
/// The view stores channel pointers, but never owns the samples they reference.
#[derive(Debug)]
pub struct AudioBufferMut<'a> {
    /// Number of samples in each channel.
    num_samples: usize,
    /// Pointer to the first sample in each channel.
    channel_ptrs: ChannelPointers,
    /// Ties the view to the exclusively borrowed samples.
    _samples: PhantomData<&'a mut [Sample]>,
}

impl<'a> AudioBufferMut<'a> {
    /// Constructs a mutable view over contiguous `samples`.
    ///
    /// # Errors
    ///
    /// - [`AudioBufferError::NoChannels`] if `num_channels` is zero.
    /// - [`AudioBufferError::NoSamples`] if `samples` is empty.
    /// - [`AudioBufferError::DataLengthMismatch`] if the data cannot be divided evenly.
    /// - [`AudioBufferError::TooManyChannels`] if the channel count exceeds the native limit.
    /// - [`AudioBufferError::TooManySamples`] if the per-channel count exceeds the native limit.
    pub fn try_new(
        samples: &'a mut [Sample],
        num_channels: usize,
    ) -> Result<Self, AudioBufferError> {
        let num_samples = validate_contiguous_layout(samples.len(), num_channels)?;
        let channel_ptrs = samples
            .chunks_exact_mut(num_samples)
            .map(<[Sample]>::as_mut_ptr)
            .collect();

        Ok(Self::from_validated_parts(channel_ptrs, num_samples))
    }

    /// Constructs a mutable view over separate channels.
    ///
    /// Safe Rust guarantees that the mutable channel slices do not overlap.
    ///
    /// # Errors
    ///
    /// - [`AudioBufferError::NoChannels`] if no channels are provided.
    /// - [`AudioBufferError::NoSamples`] if the channels are empty.
    /// - [`AudioBufferError::ChannelLengthMismatch`] if channel lengths differ.
    /// - [`AudioBufferError::TooManyChannels`] if the channel count exceeds the native limit.
    /// - [`AudioBufferError::TooManySamples`] if the per-channel count exceeds the native limit.
    pub fn try_from_channels<I>(channels: I) -> Result<Self, AudioBufferError>
    where
        I: IntoIterator<Item = &'a mut [Sample]>,
    {
        let mut channels = channels.into_iter();
        let first = channels.next().ok_or(AudioBufferError::NoChannels)?;
        let num_samples = first.len();
        let mut channel_ptrs = ChannelPointers::new();
        channel_ptrs.push(first.as_mut_ptr());

        for (channel, samples) in channels.enumerate() {
            let channel = channel + 1;
            if samples.len() != num_samples {
                return Err(AudioBufferError::ChannelLengthMismatch {
                    channel,
                    expected: num_samples,
                    actual: samples.len(),
                });
            }
            channel_ptrs.push(samples.as_mut_ptr());
        }

        validate_view_dimensions(channel_ptrs.len(), num_samples)?;
        Ok(Self::from_validated_parts(channel_ptrs, num_samples))
    }

    /// Returns the number of channels.
    pub fn num_channels(&self) -> usize {
        AudioBuffer::num_channels(self)
    }

    /// Returns the number of samples per channel.
    pub fn num_samples(&self) -> usize {
        AudioBuffer::num_samples(self)
    }

    /// Returns a channel by index.
    pub fn channel(&self, index: usize) -> Option<&[Sample]> {
        AudioBuffer::channel(self, index)
    }

    /// Returns an iterator over channels.
    pub fn channels(&self) -> impl ExactSizeIterator<Item = &[Sample]> + '_ {
        AudioBuffer::channels(self)
    }

    /// Returns a mutable channel by index.
    pub fn channel_mut(&mut self, index: usize) -> Option<&mut [Sample]> {
        let pointer = self.channel_ptrs.get_mut(index)?;
        // SAFETY: Construction guarantees exclusive, disjoint channel regions.
        Some(unsafe { std::slice::from_raw_parts_mut(*pointer, self.num_samples) })
    }

    /// Deinterleaves `src` into the channel data.
    ///
    /// # Errors
    ///
    /// Returns [`AudioBufferOperationError::DeinterleaveLengthMismatch`] if `src` does not match
    /// the total sample count.
    pub fn deinterleave(
        &mut self,
        context: &Context,
        src: &[Sample],
    ) -> Result<(), AudioBufferOperationError> {
        let expected_len = self.num_channels() * self.num_samples();
        if src.len() != expected_len {
            return Err(AudioBufferOperationError::DeinterleaveLengthMismatch {
                src_len: src.len(),
                expected_len,
            });
        }

        let mut output = self.as_ffi_mut();
        unsafe {
            audionimbus_sys::iplAudioBufferDeinterleave(
                context.raw_ptr(),
                src.as_ptr().cast_mut(),
                &raw mut *output,
            );
        }

        Ok(())
    }

    /// Mixes `source` into this buffer.
    ///
    /// # Errors
    ///
    /// - [`AudioBufferOperationError::ChannelCountMismatch`] if the channel counts differ.
    /// - [`AudioBufferOperationError::SampleCountMismatch`] if the per-channel sample counts differ.
    pub fn mix(
        &mut self,
        context: &Context,
        source: &impl AudioBuffer,
    ) -> Result<(), AudioBufferOperationError> {
        validate_matching_shape(self, source)?;

        let mut input = view_as_ffi(source, source.channel_ptrs(), source.num_samples());
        let mut output = self.as_ffi_mut();
        unsafe {
            audionimbus_sys::iplAudioBufferMix(
                context.raw_ptr(),
                &raw mut *input,
                &raw mut *output,
            );
        }

        Ok(())
    }

    /// Downmixes `source` into this mono buffer.
    ///
    /// Steam Audio writes the arithmetic mean of the source channels. Mix manually when the
    /// channels require different weights.
    ///
    /// # Errors
    ///
    /// - [`AudioBufferOperationError::DownmixDestinationNotMono`] if this buffer is not mono.
    /// - [`AudioBufferOperationError::SampleCountMismatch`] if the per-channel sample counts differ.
    pub fn downmix(
        &mut self,
        context: &Context,
        source: &impl AudioBuffer,
    ) -> Result<(), AudioBufferOperationError> {
        if self.num_channels() != 1 {
            return Err(AudioBufferOperationError::DownmixDestinationNotMono {
                num_channels: self.num_channels(),
            });
        }
        if self.num_samples() != source.num_samples() {
            return Err(AudioBufferOperationError::SampleCountMismatch {
                self_num_samples: self.num_samples(),
                other_num_samples: source.num_samples(),
            });
        }

        let mut input = view_as_ffi(source, source.channel_ptrs(), source.num_samples());
        let mut output = self.as_ffi_mut();
        unsafe {
            audionimbus_sys::iplAudioBufferDownmix(
                context.raw_ptr(),
                &raw mut *input,
                &raw mut *output,
            );
        }

        Ok(())
    }

    /// Converts Ambisonic samples from `in_type` into `out_type` in place.
    ///
    /// Steam Audio processes N3D natively, so conversion is best kept at integration boundaries.
    pub fn convert_ambisonics(
        &mut self,
        context: &Context,
        in_type: AmbisonicsType,
        out_type: AmbisonicsType,
    ) {
        let mut buffer = self.as_ffi_mut();
        unsafe {
            audionimbus_sys::iplAudioBufferConvertAmbisonics(
                context.raw_ptr(),
                in_type.into(),
                out_type.into(),
                &raw mut *buffer,
                &raw mut *buffer,
            );
        }
    }

    /// Constructs a mutable view from raw channel pointers.
    ///
    /// # Errors
    ///
    /// - [`AudioBufferError::NoChannels`] if no pointers are provided.
    /// - [`AudioBufferError::NoSamples`] if `num_samples` is zero.
    /// - [`AudioBufferError::NullChannelPointer`] if a pointer is null.
    /// - [`AudioBufferError::TooManyChannels`] if the channel count exceeds the native limit.
    /// - [`AudioBufferError::TooManySamples`] if the sample count exceeds the native limit.
    ///
    /// # Safety
    ///
    /// Every pointer must be aligned, initialized, and valid to read and write `num_samples`
    /// consecutive samples for the returned view's lifetime.
    /// Channel sample regions must be pairwise disjoint, and no other pointer may access them
    /// during that lifetime.
    pub unsafe fn try_from_raw_parts(
        channel_ptrs: &[*mut Sample],
        num_samples: usize,
    ) -> Result<Self, AudioBufferError> {
        validate_view_dimensions(channel_ptrs.len(), num_samples)?;
        let channel_ptrs = channel_ptrs
            .iter()
            .copied()
            .enumerate()
            .map(|(channel, pointer)| {
                if pointer.is_null() {
                    Err(AudioBufferError::NullChannelPointer { channel })
                } else {
                    Ok(pointer)
                }
            })
            .collect::<Result<_, _>>()?;

        Ok(Self::from_validated_parts(channel_ptrs, num_samples))
    }

    /// Returns an iterator over mutable channels.
    pub fn channels_mut(&mut self) -> impl ExactSizeIterator<Item = &mut [Sample]> + '_ {
        let num_samples = self.num_samples;
        self.channel_ptrs.iter_mut().map(move |pointer| {
            // SAFETY: Construction guarantees exclusive, disjoint channel regions.
            unsafe { std::slice::from_raw_parts_mut(*pointer, num_samples) }
        })
    }

    fn from_validated_parts(channel_ptrs: ChannelPointers, num_samples: usize) -> Self {
        Self {
            num_samples,
            channel_ptrs,
            _samples: PhantomData,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn as_ffi(&self) -> FFIWrapper<'_, audionimbus_sys::IPLAudioBuffer, Self> {
        view_as_ffi(self, &self.channel_ptrs, self.num_samples)
    }

    #[allow(dead_code)]
    pub(crate) fn as_ffi_mut(&mut self) -> FFIWrapper<'_, audionimbus_sys::IPLAudioBuffer, Self> {
        view_as_ffi(self, &self.channel_ptrs, self.num_samples)
    }
}

impl<'a> TryFrom<&'a mut [Sample]> for AudioBufferMut<'a> {
    type Error = AudioBufferError;

    fn try_from(samples: &'a mut [Sample]) -> Result<Self, Self::Error> {
        Self::try_new(samples, 1)
    }
}

impl crate::sealed::Sealed for AudioBufferMut<'_> {}

impl sealed::ChannelPointers for AudioBufferMut<'_> {
    fn channel_ptrs(&self) -> &[*mut Sample] {
        &self.channel_ptrs
    }
}

impl AudioBuffer for AudioBufferMut<'_> {
    fn num_samples(&self) -> usize {
        self.num_samples
    }
}

// SAFETY: The view has the same exclusive access semantics as `&mut [Sample]`.
unsafe impl Send for AudioBufferMut<'_> {}
// SAFETY: Shared access cannot mutate samples; writable operations require `&mut self`.
unsafe impl Sync for AudioBufferMut<'_> {}

/// Validates contiguous channel data and returns the sample count per channel.
///
/// # Errors
///
/// - [`AudioBufferError::NoChannels`] if `num_channels` is zero.
/// - [`AudioBufferError::NoSamples`] if `len` is zero.
/// - [`AudioBufferError::DataLengthMismatch`] if the data cannot be divided evenly.
/// - [`AudioBufferError::TooManyChannels`] if the channel count exceeds the native limit.
/// - [`AudioBufferError::TooManySamples`] if the per-channel count exceeds the native limit.
fn validate_contiguous_layout(len: usize, num_channels: usize) -> Result<usize, AudioBufferError> {
    if num_channels == 0 {
        return Err(AudioBufferError::NoChannels);
    }
    if len == 0 {
        return Err(AudioBufferError::NoSamples);
    }
    if !len.is_multiple_of(num_channels) {
        return Err(AudioBufferError::DataLengthMismatch { len, num_channels });
    }

    let num_samples = len / num_channels;
    validate_view_dimensions(num_channels, num_samples)?;
    Ok(num_samples)
}

/// Validates channel and sample counts for the native descriptor.
///
/// # Errors
///
/// - [`AudioBufferError::NoChannels`] if `num_channels` is zero.
/// - [`AudioBufferError::NoSamples`] if `num_samples` is zero.
/// - [`AudioBufferError::TooManyChannels`] if the channel count exceeds the native limit.
/// - [`AudioBufferError::TooManySamples`] if the sample count exceeds the native limit.
fn validate_view_dimensions(
    num_channels: usize,
    num_samples: usize,
) -> Result<(), AudioBufferError> {
    if num_channels == 0 {
        return Err(AudioBufferError::NoChannels);
    }
    if num_samples == 0 {
        return Err(AudioBufferError::NoSamples);
    }
    if i32::try_from(num_channels).is_err() {
        return Err(AudioBufferError::TooManyChannels { num_channels });
    }
    if i32::try_from(num_samples).is_err() {
        return Err(AudioBufferError::TooManySamples { num_samples });
    }
    Ok(())
}

/// Creates a native descriptor tied to `owner`.
pub(crate) fn read_as_ffi<Buffer>(
    buffer: &Buffer,
) -> FFIWrapper<'_, audionimbus_sys::IPLAudioBuffer, Buffer>
where
    Buffer: AudioBuffer,
{
    view_as_ffi(buffer, buffer.channel_ptrs(), buffer.num_samples())
}

/// Creates a native descriptor tied to `owner`.
#[allow(dead_code)]
fn view_as_ffi<'a, Owner>(
    _owner: &'a Owner,
    channel_ptrs: &[*mut Sample],
    num_samples: usize,
) -> FFIWrapper<'a, audionimbus_sys::IPLAudioBuffer, Owner> {
    let audio_buffer = audionimbus_sys::IPLAudioBuffer {
        numChannels: channel_ptrs.len() as i32,
        numSamples: num_samples as i32,
        data: channel_ptrs.as_ptr().cast_mut(),
    };

    FFIWrapper::new(audio_buffer)
}

/// Validates that two views have the same channel layout.
fn validate_matching_shape(
    first: &impl AudioBuffer,
    second: &impl AudioBuffer,
) -> Result<(), AudioBufferOperationError> {
    if first.num_channels() != second.num_channels() {
        return Err(AudioBufferOperationError::ChannelCountMismatch {
            self_num_channels: first.num_channels(),
            other_num_channels: second.num_channels(),
        });
    }
    if first.num_samples() != second.num_samples() {
        return Err(AudioBufferOperationError::SampleCountMismatch {
            self_num_samples: first.num_samples(),
            other_num_samples: second.num_samples(),
        });
    }
    Ok(())
}

/// [`AudioBuffer`] construction errors.
#[derive(Debug, PartialEq, Eq)]
pub enum AudioBufferError {
    /// No channels were provided.
    NoChannels,

    /// No samples were provided per channel.
    NoSamples,

    /// Contiguous data cannot be divided evenly into the requested channels.
    DataLengthMismatch { len: usize, num_channels: usize },

    /// A channel has a different length than the first channel.
    ChannelLengthMismatch {
        channel: usize,
        expected: usize,
        actual: usize,
    },

    /// A raw channel pointer is null.
    NullChannelPointer { channel: usize },

    /// The channel count exceeds Steam Audio's native integer range.
    TooManyChannels { num_channels: usize },

    /// The per-channel sample count exceeds Steam Audio's native integer range.
    TooManySamples { num_samples: usize },
}

impl std::error::Error for AudioBufferError {}

impl std::fmt::Display for AudioBufferError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match &self {
            Self::NoChannels => write!(f, "audio buffer has no channels"),
            Self::NoSamples => write!(f, "audio buffer channels have no samples"),
            Self::DataLengthMismatch { len, num_channels } => write!(
                f,
                "sample data length {len} is not divisible by {num_channels} channels"
            ),
            Self::ChannelLengthMismatch {
                channel,
                expected,
                actual,
            } => write!(
                f,
                "channel {channel} has {actual} samples, expected {expected}"
            ),
            Self::NullChannelPointer { channel } => {
                write!(f, "channel {channel} has a null sample pointer")
            }
            Self::TooManyChannels { num_channels } => write!(
                f,
                "channel count {num_channels} exceeds the native integer range"
            ),
            Self::TooManySamples { num_samples } => write!(
                f,
                "sample count {num_samples} exceeds the native integer range"
            ),
        }
    }
}

/// Errors produced by audio buffer operations.
#[derive(Debug, PartialEq, Eq)]
pub enum AudioBufferOperationError {
    /// The total interleaved sample count exceeds the native indexing range.
    InterleaveLengthOverflow {
        num_channels: usize,
        num_samples: usize,
    },

    /// Destination slice length does not match audio buffer length.
    InterleaveLengthMismatch { dst_len: usize, expected_len: usize },

    /// Source slice length does not match audio buffer length.
    DeinterleaveLengthMismatch { src_len: usize, expected_len: usize },

    /// Audio buffers have mismatched number of channels.
    ChannelCountMismatch {
        self_num_channels: usize,
        other_num_channels: usize,
    },

    /// Audio buffers have mismatched number of samples.
    SampleCountMismatch {
        self_num_samples: usize,
        other_num_samples: usize,
    },

    /// A downmix destination has more than one channel.
    DownmixDestinationNotMono { num_channels: usize },
}

impl std::error::Error for AudioBufferOperationError {}

impl std::fmt::Display for AudioBufferOperationError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::InterleaveLengthOverflow {
                num_channels,
                num_samples,
            } => write!(
                f,
                "interleaved length for {num_channels} channels with {num_samples} samples exceeds the native integer range"
            ),
            Self::InterleaveLengthMismatch {
                dst_len,
                expected_len,
            } => write!(
                f,
                "destination slice length {dst_len} does not match expected length {expected_len}"
            ),
            Self::DeinterleaveLengthMismatch {
                src_len,
                expected_len,
            } => write!(
                f,
                "source slice length {src_len} does not match expected length {expected_len}"
            ),
            Self::ChannelCountMismatch {
                self_num_channels,
                other_num_channels,
            } => write!(
                f,
                "channel count mismatch: buffer has {self_num_channels} channels, other has {other_num_channels}"
            ),
            Self::SampleCountMismatch {
                self_num_samples,
                other_num_samples,
            } => write!(
                f,
                "sample count mismatch: buffer has {self_num_samples} samples, other has {other_num_samples}"
            ),
            Self::DownmixDestinationNotMono { num_channels } => write!(
                f,
                "downmix destination must be mono, but has {num_channels} channels"
            ),
        }
    }
}

/// Returns the number of channels required for a given ambisonics order.
///
/// The channel count is given by:
///
/// ```text
/// (order + 1)^2
/// ```
///
/// # Examples
///
/// ```
/// # use audionimbus::*;
/// const FOA: u32 = num_ambisonics_channels(1);
/// assert_eq!(FOA, 4);
///
/// const HOA3: u32 = num_ambisonics_channels(3);
/// assert_eq!(HOA3, 16);
/// ```
pub const fn num_ambisonics_channels(order: u32) -> u32 {
    (order + 1) * (order + 1)
}

/// Describes the channel count requirement for an audio buffer.
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum ChannelRequirement {
    /// The buffer must have exactly this many channels.
    Exactly(u32),

    /// The buffer must have at least this many channels.
    AtLeast(u32),

    /// The buffer must have a channel count within the given inclusive range.
    Range { min: u32, max: u32 },
}

impl ChannelRequirement {
    /// Returns whether a number of channels satisfies this requirement.
    pub fn is_satisfied_by(&self, actual: u32) -> bool {
        match *self {
            Self::Exactly(num_channels) => actual == num_channels,
            Self::AtLeast(num_channels) => actual >= num_channels,
            Self::Range { min, max } => (min..=max).contains(&actual),
        }
    }
}

impl std::fmt::Display for ChannelRequirement {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Exactly(num_channels) => {
                write!(f, "exactly {num_channels}")
            }
            Self::AtLeast(num_channels) => {
                write!(f, "at least {num_channels}")
            }
            Self::Range { min, max } => {
                write!(f, "between {min} and {max} (inclusive)")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod audio_buffer_ref {
        use super::*;

        mod try_new {
            use super::*;

            #[test]
            fn immutable() {
                let samples = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
                let buffer = AudioBufferRef::try_new(&samples, 2).unwrap();

                assert_eq!(buffer.num_channels(), 2);
                assert_eq!(buffer.num_samples(), 3);
                assert_eq!(buffer.channel(0), Some(&samples[..3]));
                assert_eq!(buffer.channel(1), Some(&samples[3..]));
                assert_eq!(buffer.clone().channels().count(), 2);
            }

            #[test]
            fn invalid() {
                assert_eq!(
                    AudioBufferRef::try_new(&[0.0; 4], 0).unwrap_err(),
                    AudioBufferError::NoChannels
                );
                assert_eq!(
                    AudioBufferRef::try_new(&[], 1).unwrap_err(),
                    AudioBufferError::NoSamples
                );
                assert_eq!(
                    AudioBufferRef::try_new(&[0.0; 5], 2).unwrap_err(),
                    AudioBufferError::DataLengthMismatch {
                        len: 5,
                        num_channels: 2,
                    }
                );
            }

            #[test]
            fn storage() {
                let samples = [0.0; INLINE_CHANNEL_CAPACITY];
                let buffer = AudioBufferRef::try_new(&samples, INLINE_CHANNEL_CAPACITY).unwrap();

                assert!(!buffer.channel_ptrs.spilled());

                let samples = [0.0; INLINE_CHANNEL_CAPACITY + 1];
                let buffer =
                    AudioBufferRef::try_new(&samples, INLINE_CHANNEL_CAPACITY + 1).unwrap();

                assert!(buffer.channel_ptrs.spilled());
            }
        }

        mod try_from {
            use super::*;

            #[test]
            fn mono() {
                let samples = [1.0, 2.0];
                let buffer = AudioBufferRef::try_from(&samples[..]).unwrap();

                assert_eq!(buffer.num_channels(), 1);
                assert_eq!(buffer.channel(0), Some(&samples[..]));
            }
        }

        mod try_from_channels {
            use super::*;

            #[test]
            fn unequal() {
                let first = [0.0; 2];
                let second = [0.0; 3];

                assert_eq!(
                    AudioBufferRef::try_from_channels([&first[..], &second[..]]).unwrap_err(),
                    AudioBufferError::ChannelLengthMismatch {
                        channel: 1,
                        expected: 2,
                        actual: 3,
                    }
                );
            }
        }

        mod try_from_raw_parts {
            use super::*;

            #[test]
            fn invalid() {
                let pointers = [std::ptr::null()];
                let result = unsafe { AudioBufferRef::try_from_raw_parts(&pointers, 1) };

                assert_eq!(
                    result.unwrap_err(),
                    AudioBufferError::NullChannelPointer { channel: 0 }
                );

                let sample = 0.0;
                let pointers = [&raw const sample];
                let num_samples = usize::try_from(i32::MAX).unwrap() + 1;
                let result = unsafe { AudioBufferRef::try_from_raw_parts(&pointers, num_samples) };

                assert_eq!(
                    result.unwrap_err(),
                    AudioBufferError::TooManySamples { num_samples }
                );
            }
        }

        mod as_ffi {
            use super::*;

            #[test]
            fn descriptors() {
                let samples = [0.0; 8];
                let buffer = AudioBufferRef::try_new(&samples, 2).unwrap();
                let ffi = buffer.as_ffi();

                assert_eq!(ffi.numChannels, 2);
                assert_eq!(ffi.numSamples, 4);
                assert!(!ffi.data.is_null());
            }
        }
    }

    mod audio_buffer_mut {
        use super::*;

        mod try_new {
            use super::*;

            #[test]
            fn valid() {
                let mut samples = [1.0, 2.0, 3.0, 4.0];
                let mut buffer = AudioBufferMut::try_new(&mut samples, 2).unwrap();

                for channel in buffer.channels_mut() {
                    channel.fill(0.5);
                }
                drop(buffer);

                assert_eq!(samples, [0.5; 4]);
            }
        }

        mod try_from {
            use super::*;

            #[test]
            fn mono() {
                let mut samples = [1.0, 2.0];
                let mut buffer = AudioBufferMut::try_from(&mut samples[..]).unwrap();
                buffer.channels_mut().next().unwrap().fill(0.5);
                drop(buffer);

                assert_eq!(samples, [0.5; 2]);
            }
        }

        mod try_from_channels {
            use super::*;

            #[test]
            fn valid() {
                let mut samples = [0.0; 6];
                let (left, right) = samples.split_at_mut(3);
                let mut buffer = AudioBufferMut::try_from_channels([left, right]).unwrap();

                let mut channels = buffer.channels_mut();
                channels.next().unwrap().fill(1.0);
                channels.next().unwrap().fill(2.0);
                drop(channels);
                drop(buffer);

                assert_eq!(samples, [1.0, 1.0, 1.0, 2.0, 2.0, 2.0]);
            }
        }

        mod try_from_raw_parts {
            use super::*;

            #[test]
            fn null_pointer() {
                let pointers = [std::ptr::null_mut()];
                let result = unsafe { AudioBufferMut::try_from_raw_parts(&pointers, 1) };

                assert_eq!(
                    result.unwrap_err(),
                    AudioBufferError::NullChannelPointer { channel: 0 }
                );
            }
        }

        mod as_ffi {
            use super::*;

            #[test]
            fn descriptors() {
                let mut samples = [0.0; 8];
                let mut buffer = AudioBufferMut::try_new(&mut samples, 2).unwrap();

                assert_eq!(buffer.as_ffi().numChannels, 2);
                assert_eq!(buffer.as_ffi_mut().numSamples, 4);
            }
        }
    }

    mod channel_requirement {
        use super::*;

        #[test]
        fn test_is_satisfied_by() {
            assert!(ChannelRequirement::Exactly(2).is_satisfied_by(2));
            assert!(!ChannelRequirement::Exactly(2).is_satisfied_by(1));
            assert!(ChannelRequirement::AtLeast(2).is_satisfied_by(3));
            assert!(!ChannelRequirement::AtLeast(2).is_satisfied_by(1));
            assert!(ChannelRequirement::Range { min: 1, max: 4 }.is_satisfied_by(3));
            assert!(!ChannelRequirement::Range { min: 1, max: 4 }.is_satisfied_by(5));
        }
    }
}
