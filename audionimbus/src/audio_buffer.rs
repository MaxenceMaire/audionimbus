//! Types and utilities for working with audio buffers.

use crate::context::Context;
use crate::effect::ambisonics::AmbisonicsType;
use crate::ffi_wrapper::FFIWrapper;
use smallvec::SmallVec;
use std::marker::PhantomData;

/// Number of channel pointers stored without a heap allocation.
const INLINE_CHANNEL_CAPACITY: usize = 16;

/// Channel pointers
type ViewChannelPointers = SmallVec<[*mut Sample; INLINE_CHANNEL_CAPACITY]>;

mod sealed {
    use super::Sample;

    /// Supplies channel pointers to the public read interface.
    pub trait ChannelPointers {
        /// Returns the channel pointers.
        fn channel_ptrs(&self) -> &[*mut Sample];
    }
}

/// Read access to borrowed audio samples.
pub trait AudioBufferRead: crate::sealed::Sealed + sealed::ChannelPointers {
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
            unsafe { std::slice::from_raw_parts(*pointer, AudioBufferRead::num_samples(self)) }
        })
    }

    /// Returns an iterator over channels.
    fn channels(&self) -> impl ExactSizeIterator<Item = &[Sample]> + '_ {
        let num_samples = AudioBufferRead::num_samples(self);
        self.channel_ptrs().iter().map(move |pointer| {
            // SAFETY: Implementations guarantee that every pointer is valid for `num_samples`.
            unsafe { std::slice::from_raw_parts(*pointer, num_samples) }
        })
    }

    /// Interleaves the channel data into `dst`.
    ///
    /// # Errors
    ///
    /// Returns [`AudioBufferOperationError::InterleaveLengthMismatch`] if `dst` does not match the
    /// total sample count.
    fn interleave(
        &self,
        context: &Context,
        dst: &mut [Sample],
    ) -> Result<(), AudioBufferOperationError>
    where
        Self: Sized,
    {
        let expected_len = self.num_channels() * self.num_samples();
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
    channel_ptrs: ViewChannelPointers,
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
        let mut channel_ptrs = ViewChannelPointers::new();
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
        AudioBufferRead::num_channels(self)
    }

    /// Returns the number of samples per channel.
    pub fn num_samples(&self) -> usize {
        AudioBufferRead::num_samples(self)
    }

    /// Returns a channel by index.
    pub fn channel(&self, index: usize) -> Option<&[Sample]> {
        AudioBufferRead::channel(self, index)
    }

    /// Returns an iterator over channels.
    pub fn channels(&self) -> impl ExactSizeIterator<Item = &[Sample]> + '_ {
        AudioBufferRead::channels(self)
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

    fn from_validated_parts(channel_ptrs: ViewChannelPointers, num_samples: usize) -> Self {
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

impl AudioBufferRead for AudioBufferRef<'_> {
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
    channel_ptrs: ViewChannelPointers,
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
        let mut channel_ptrs = ViewChannelPointers::new();
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
        AudioBufferRead::num_channels(self)
    }

    /// Returns the number of samples per channel.
    pub fn num_samples(&self) -> usize {
        AudioBufferRead::num_samples(self)
    }

    /// Returns a channel by index.
    pub fn channel(&self, index: usize) -> Option<&[Sample]> {
        AudioBufferRead::channel(self, index)
    }

    /// Returns an iterator over channels.
    pub fn channels(&self) -> impl ExactSizeIterator<Item = &[Sample]> + '_ {
        AudioBufferRead::channels(self)
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
        source: &impl AudioBufferRead,
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
        source: &impl AudioBufferRead,
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

    fn from_validated_parts(channel_ptrs: ViewChannelPointers, num_samples: usize) -> Self {
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

impl AudioBufferRead for AudioBufferMut<'_> {
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
    first: &impl AudioBufferRead,
    second: &impl AudioBufferRead,
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

/// Trait for types that can provide access to channel pointers.
///
/// This trait abstracts over different storage backends for channel pointers,
/// allowing [`AudioBuffer`] to work with both owned (`Vec<*mut Sample>`) and
/// borrowed (`&[*mut Sample]`, `&mut [*mut Sample]`) pointer storage.
pub trait ChannelPointers {
    /// Returns an immutable slice of channel pointers.
    ///
    /// Each pointer in the slice points to the sample data for one audio channel.
    fn as_slice(&self) -> &[*mut Sample];

    /// Returns a mutable slice of channel pointers.
    ///
    /// Each pointer in the slice points to the sample data for one audio channel.
    fn as_mut_slice(&mut self) -> &mut [*mut Sample];
}

impl<T> ChannelPointers for T
where
    T: AsRef<[*mut Sample]> + AsMut<[*mut Sample]>,
{
    fn as_slice(&self) -> &[*mut Sample] {
        self.as_ref()
    }
    fn as_mut_slice(&mut self) -> &mut [*mut Sample] {
        self.as_mut()
    }
}

/// An audio buffer descriptor.
///
/// This struct does not hold the actual sample data, but instead contains pointers to samples stored elsewhere.
/// The generic parameter `T` is used to ensure that these pointers remain valid for the lifetime of the underlying data.
/// The generic parameter `P` allows for different storage backends (owned Vec or borrowed slice of
/// channel pointers).
///
/// # Examples
///
/// ```
/// use audionimbus::{AudioBuffer, AudioBufferSettings};
///
/// // Mono buffer
/// let samples = vec![0.0; 1024];
/// let buffer = AudioBuffer::try_with_data(&samples)?;
///
/// // Stereo buffer
/// let stereo_samples = vec![0.0; 2048];
/// let buffer = AudioBuffer::try_with_data_and_settings(
///     &stereo_samples,
///     AudioBufferSettings::with_num_channels(2),
/// )?;
/// # Ok::<(), audionimbus::AudioBufferError>(())
/// ```
#[derive(Debug)]
pub struct AudioBuffer<T, P: ChannelPointers = Vec<*mut Sample>> {
    /// Number of samples per channel.
    num_samples: u32,

    /// Pointers to sample data for each channel.
    channel_ptrs: P,

    /// Marker to enforce the lifetime of the channel pointers.
    _marker: std::marker::PhantomData<T>,
}

impl<T, P: ChannelPointers> AudioBuffer<T, P> {
    /// Constructs a new `AudioBuffer` from raw pointers to mutable channel samples and the number
    /// of samples.
    ///
    /// This function is designed to provide maximum flexibility for advanced users who need
    /// fine-grained control over the memory layout of audio data.
    /// However, for most use cases, the safe constructors [`Self::try_with_data`] and
    /// [`Self::try_with_data_and_settings`] should be preferred, because they enforce invariants
    /// using lifetimes.
    ///
    /// The generic parameter `T` can be used to enforce a lifetime and ensure the pointers remain
    /// valid.
    ///
    /// # Errors
    ///
    /// - [`AudioBufferError::InvalidNumChannels`] if `channel_ptrs` is empty.
    /// - [`AudioBufferError::InvalidNumSamples`] if `num_samples` is 0.
    ///
    /// # Safety
    ///
    /// - `channel_ptrs` must contain valid pointers for the duration of the `AudioBuffer`.
    /// - Each pointer in `channel_ptrs` must point to a region of memory containing at least `num_samples` valid samples.
    /// - The lifetime of the `AudioBuffer` must not exceed the lifetime of the memory referenced by `channel_ptrs`.
    ///
    /// Any violations of the above invariants will result in undefined behavior.
    pub unsafe fn try_new(channel_ptrs: P, num_samples: u32) -> Result<Self, AudioBufferError> {
        if channel_ptrs.as_slice().is_empty() {
            return Err(AudioBufferError::InvalidNumChannels { num_channels: 0 });
        }

        if num_samples == 0 {
            return Err(AudioBufferError::InvalidNumSamples { num_samples });
        }

        debug_assert!(
            channel_ptrs.as_slice().iter().all(|&ptr| !ptr.is_null()),
            "some channel pointers are null"
        );

        Ok(Self {
            num_samples,
            channel_ptrs,
            _marker: std::marker::PhantomData,
        })
    }

    /// Returns the number of channels of the audio buffer.
    pub fn num_channels(&self) -> u32 {
        self.channel_ptrs.as_slice().len() as u32
    }

    /// Returns the number of samples per channel in the audio buffer.
    pub const fn num_samples(&self) -> u32 {
        self.num_samples
    }

    /// Reads samples from the audio buffer and interleaves them into `dst`.
    ///
    /// # Errors
    ///
    /// Returns [`AudioBufferOperationError::InterleaveLengthMismatch`] if the destination slice length
    /// does not match the audio buffer's total sample count.
    pub fn interleave(
        &self,
        context: &Context,
        dst: &mut [Sample],
    ) -> Result<(), AudioBufferOperationError> {
        let expected_len = self.num_channels() as usize * self.num_samples() as usize;
        if dst.len() != expected_len {
            return Err(AudioBufferOperationError::InterleaveLengthMismatch {
                dst_len: dst.len(),
                expected_len,
            });
        }

        let mut audio_buffer_ffi = self.as_ffi();

        unsafe {
            audionimbus_sys::iplAudioBufferInterleave(
                context.raw_ptr(),
                &raw mut *audio_buffer_ffi,
                dst.as_mut_ptr(),
            );
        }

        Ok(())
    }

    /// Deinterleaves the `src` sample data into `Self`.
    ///
    /// # Errors
    ///
    /// Returns [`AudioBufferOperationError::DeinterleaveLengthMismatch`] if the source slice length
    /// does not match the audio buffer's total sample count.
    pub fn deinterleave(
        &mut self,
        context: &Context,
        src: &[Sample],
    ) -> Result<(), AudioBufferOperationError> {
        let expected_len = self.num_channels() as usize * self.num_samples() as usize;
        if src.len() != expected_len {
            return Err(AudioBufferOperationError::DeinterleaveLengthMismatch {
                src_len: src.len(),
                expected_len,
            });
        }

        let mut audio_buffer_ffi = self.as_ffi();

        unsafe {
            audionimbus_sys::iplAudioBufferDeinterleave(
                context.raw_ptr(),
                src.as_ptr().cast_mut(),
                &raw mut *audio_buffer_ffi,
            );
        };

        Ok(())
    }

    /// Mixes `source` into `self`.
    ///
    /// Both audio buffers must have the same number of channels and samples.
    ///
    /// # Errors
    ///
    /// Returns:
    /// - [`AudioBufferOperationError::ChannelCountMismatch`] if the audio buffers have different numbers of channels.
    /// - [`AudioBufferOperationError::SampleCountMismatch`] if the audio buffers have different numbers of samples per channel.
    pub fn mix<T2, P2: ChannelPointers>(
        &mut self,
        context: &Context,
        source: &AudioBuffer<T2, P2>,
    ) -> Result<(), AudioBufferOperationError> {
        let self_num_channels = self.num_channels();
        let other_num_channels = source.num_channels();
        if self_num_channels != other_num_channels {
            return Err(AudioBufferOperationError::ChannelCountMismatch {
                self_num_channels: self_num_channels as usize,
                other_num_channels: other_num_channels as usize,
            });
        }

        let self_num_samples = self.num_samples();
        let other_num_samples = source.num_samples();
        if self_num_samples != other_num_samples {
            return Err(AudioBufferOperationError::SampleCountMismatch {
                self_num_samples: self_num_samples as usize,
                other_num_samples: other_num_samples as usize,
            });
        }

        unsafe {
            audionimbus_sys::iplAudioBufferMix(
                context.raw_ptr(),
                &raw mut *source.as_ffi(),
                &raw mut *self.as_ffi(),
            );
        }

        Ok(())
    }

    /// Downmixes the multi-channel `source` audio buffer into a mono `self` audio buffer.
    ///
    /// Both audio buffers must have the same number of samples per channel.
    ///
    /// Downmixing is performed by summing up the source channels and dividing the result by the number of source channels.
    /// If this is not the desired downmixing behavior, we recommend that downmixing be performed manually.
    ///
    /// # Errors
    ///
    /// Returns [`AudioBufferOperationError::SampleCountMismatch`] if the audio buffers have different numbers of samples per channel.
    pub fn downmix<T2, P2: ChannelPointers>(
        &mut self,
        context: &Context,
        source: &AudioBuffer<T2, P2>,
    ) -> Result<(), AudioBufferOperationError> {
        let self_num_samples = self.num_samples();
        let other_num_samples = source.num_samples();
        if self_num_samples != other_num_samples {
            return Err(AudioBufferOperationError::SampleCountMismatch {
                self_num_samples: self_num_samples as usize,
                other_num_samples: other_num_samples as usize,
            });
        }

        unsafe {
            audionimbus_sys::iplAudioBufferDownmix(
                context.raw_ptr(),
                &raw mut *source.as_ffi(),
                &raw mut *self.as_ffi(),
            );
        }

        Ok(())
    }

    /// Returns an iterator over channels.
    pub fn channels(&self) -> impl Iterator<Item = &[Sample]> + '_ {
        self.channel_ptrs.as_slice().iter().map(|&ptr|
            // SAFETY: pointers are guaranteed to be valid by the lifetime.
            unsafe { std::slice::from_raw_parts(ptr, self.num_samples() as usize) })
    }

    /// Returns an iterator over mutable channels.
    pub fn channels_mut(&mut self) -> impl Iterator<Item = &mut [Sample]> + '_ {
        let num_samples = self.num_samples as usize;
        self.channel_ptrs.as_mut_slice().iter_mut().map(move |ptr|
            // SAFETY: pointers are guaranteed to be valid by the lifetime.
            unsafe { std::slice::from_raw_parts_mut(*ptr, num_samples) })
    }

    /// Converts an Ambisonic audio buffer from one Ambisonic format to another.
    ///
    /// Steam Audio’s "native" Ambisonic format is [`AmbisonicsType::N3D`], so for best performance, keep all Ambisonic data in N3D format except when exchanging data with your audio engine.
    pub fn convert_ambisonics(
        &mut self,
        context: &Context,
        in_type: AmbisonicsType,
        out_type: AmbisonicsType,
    ) {
        unsafe {
            audionimbus_sys::iplAudioBufferConvertAmbisonics(
                context.raw_ptr(),
                in_type.into(),
                out_type.into(),
                &raw mut *self.as_ffi(),
                &raw mut *self.as_ffi(),
            );
        }
    }

    /// Converts an Ambisonic audio buffer from one Ambisonic format to another.
    ///
    /// Both audio buffers must have the same number of samples.
    ///
    /// Steam Audio’s "native" Ambisonic format is [`AmbisonicsType::N3D`], so for best performance, keep all Ambisonic data in N3D format except when exchanging data with your audio engine.
    ///
    /// # Errors
    ///
    /// Returns [`AudioBufferOperationError::TotalSampleMismatch`] if the audio buffers have different total sample counts.
    pub fn convert_ambisonics_into<T2, P2: ChannelPointers>(
        &mut self,
        context: &Context,
        in_type: AmbisonicsType,
        out_type: AmbisonicsType,
        out: &mut AudioBuffer<T2, P2>,
    ) -> Result<(), AudioBufferOperationError> {
        let self_count = self.num_channels() as usize * self.num_samples() as usize;
        let other_count = out.num_channels() as usize * out.num_samples() as usize;
        if self_count != other_count {
            return Err(AudioBufferOperationError::TotalSampleMismatch {
                self_count,
                other_count,
            });
        }

        unsafe {
            audionimbus_sys::iplAudioBufferConvertAmbisonics(
                context.raw_ptr(),
                in_type.into(),
                out_type.into(),
                &raw mut *self.as_ffi(),
                &raw mut *out.as_ffi(),
            );
        }

        Ok(())
    }

    pub(crate) fn as_ffi(&self) -> FFIWrapper<'_, audionimbus_sys::IPLAudioBuffer, Self> {
        let audio_buffer = audionimbus_sys::IPLAudioBuffer {
            numChannels: self.num_channels() as i32,
            numSamples: self.num_samples() as i32,
            data: self.channel_ptrs.as_slice().as_ptr().cast_mut(),
        };

        FFIWrapper::new(audio_buffer)
    }
}

impl<T: AsRef<[Sample]>> AudioBuffer<T, Vec<*mut Sample>> {
    /// Constructs an `AudioBuffer` over `data` with one channel spanning the entire data provided.
    ///
    /// # Errors
    ///
    /// - [`AudioBufferError::EmptyData`] if the `data` slice is empty.
    /// - [`AudioBufferError::InvalidNumSamples`] if `num_samples` is 0 or the data length is not divisible by `num_samples`.
    /// - [`AudioBufferError::InvalidNumChannels`] if `num_channels` is 0 or the data length is not divisible by `num_channels`.
    /// - [`AudioBufferError::FrameOutOfBounds`] if the frame is out of channel bounds.
    pub fn try_with_data(data: T) -> Result<Self, AudioBufferError> {
        Self::try_with_data_and_settings(data, AudioBufferSettings::default())
    }

    /// Constructs an `AudioBuffer` over `data` given the provided [`AudioBufferSettings`].
    ///
    /// # Errors
    ///
    /// - [`AudioBufferError::EmptyData`] if the `data` slice is empty.
    /// - [`AudioBufferError::InvalidNumSamples`] if `num_samples` is 0 or the data length is not divisible by `num_samples`.
    /// - [`AudioBufferError::InvalidNumChannels`] if `num_channels` is 0 or the data length is not divisible by `num_channels`.
    /// - [`AudioBufferError::FrameOutOfBounds`] if the frame is out of channel bounds.
    pub fn try_with_data_and_settings(
        data: T,
        settings: AudioBufferSettings,
    ) -> Result<Self, AudioBufferError> {
        let data = data.as_ref();

        if data.is_empty() {
            return Err(AudioBufferError::EmptyData);
        }

        let (num_channels, num_samples) = settings.num_channels_and_samples(data)?;
        let frame_size = settings.frame_size.unwrap_or(num_samples);
        let frame_index = settings.frame_index;

        if (frame_index + 1) * frame_size > num_samples {
            return Err(AudioBufferError::FrameOutOfBounds {
                frame_size,
                frame_index,
            });
        }

        let channel_ptrs = (0..num_channels)
            .map(|channel| {
                let index = (channel * num_samples + frame_index * frame_size) as usize;
                data[index..].as_ptr().cast_mut()
            })
            .collect();

        Ok(Self {
            num_samples: frame_size,
            channel_ptrs,
            _marker: std::marker::PhantomData,
        })
    }
}

impl<'a, T: AsRef<[Sample]>> AudioBuffer<T, &'a mut [*mut Sample]> {
    /// Constructs an `AudioBuffer` over `data` with one channel spanning the entire data provided.
    /// The `null_channel_ptrs` argument will be filled with actual channel pointers.
    ///
    /// # Errors
    ///
    /// - [`AudioBufferError::EmptyData`] if the `data` slice is empty.
    /// - [`AudioBufferError::InvalidNumSamples`] if the number of samples is 0 or the data length is not divisible by the number of samples.
    /// - [`AudioBufferError::InvalidNumChannels`] if the number of channels is 0 or the data length is not divisible by the number of channels.
    /// - [`AudioBufferError::FrameOutOfBounds`] if the frame is out of channel bounds.
    /// - [`AudioBufferError::InvalidChannelPtrs`] if the length of `null_channel_ptrs` is not equal to the number of channels.
    pub fn try_borrowed_with_data(
        data: T,
        null_channel_ptrs: &'a mut [*mut Sample],
    ) -> Result<Self, AudioBufferError> {
        Self::try_borrowed_with_data_and_settings(
            data,
            null_channel_ptrs,
            AudioBufferSettings::default(),
        )
    }

    /// Constructs an `AudioBuffer` over `data` given the provided [`AudioBufferSettings`].
    /// The `null_channel_ptrs` argument will be filled with actual channel pointers.
    ///
    /// # Errors
    ///
    /// - [`AudioBufferError::EmptyData`] if `data` is empty.
    /// - [`AudioBufferError::InvalidNumSamples`] if the number of samples is 0 or the data length is not divisible by the number of samples.
    /// - [`AudioBufferError::InvalidNumChannels`] if the number of channels is 0 or the data length is not divisible by the number of channels.
    /// - [`AudioBufferError::FrameOutOfBounds`] if the frame is out of channel bounds.
    /// - [`AudioBufferError::InvalidChannelPtrs`] if the length of `null_channel_ptrs` is not equal to the number of channels.
    pub fn try_borrowed_with_data_and_settings(
        data: T,
        null_channel_ptrs: &'a mut [*mut Sample],
        settings: AudioBufferSettings,
    ) -> Result<Self, AudioBufferError> {
        let data = data.as_ref();

        if data.is_empty() {
            return Err(AudioBufferError::EmptyData);
        }

        let (num_channels, num_samples) = settings.num_channels_and_samples(data)?;
        let frame_size = settings.frame_size.unwrap_or(num_samples);
        let frame_index = settings.frame_index;

        if (frame_index + 1) * frame_size > num_samples {
            return Err(AudioBufferError::FrameOutOfBounds {
                frame_size,
                frame_index,
            });
        }

        if null_channel_ptrs.len() as u32 != num_channels {
            return Err(AudioBufferError::InvalidChannelPtrs {
                actual: null_channel_ptrs.len() as u32,
                expected: num_channels,
            });
        }

        null_channel_ptrs
            .iter_mut()
            .enumerate()
            .for_each(|(i, channel)| {
                let index = i as u32 * num_samples + frame_index * frame_size;
                *channel = data[index as usize..].as_ptr().cast_mut();
            });

        let channel_ptrs = null_channel_ptrs;

        Ok(AudioBuffer {
            num_samples: frame_size,
            channel_ptrs,
            _marker: std::marker::PhantomData,
        })
    }
}

impl<'a> AudioBuffer<(), &'a mut [*mut Sample]> {
    /// Constructs an `AudioBuffer` from channel data `channels` and null channel pointers to be
    /// initialized.
    /// The `null_channel_ptrs` argument will be filled with actual channel pointers.
    ///
    /// # Errors
    ///
    /// - [`AudioBufferError::InvalidNumSamples`] if `channels` is empty.
    /// - [`AudioBufferError::InvalidNumChannels`] if channels contain no samples.
    /// - [`AudioBufferError::InvalidChannelPtrs`] if the length of `null_channel_ptrs` is not equal to the length of `channels`.
    pub fn try_from_slices(
        channels: &[&'a [Sample]],
        null_channel_ptrs: &'a mut [*mut Sample],
    ) -> Result<Self, AudioBufferError> {
        if channels.is_empty() {
            return Err(AudioBufferError::InvalidNumChannels { num_channels: 0 });
        }

        let num_samples = channels[0].len();
        if num_samples == 0 {
            return Err(AudioBufferError::InvalidNumSamples { num_samples: 0 });
        }

        if null_channel_ptrs.len() != channels.len() {
            return Err(AudioBufferError::InvalidChannelPtrs {
                actual: null_channel_ptrs.len() as u32,
                expected: channels.len() as u32,
            });
        }

        for (ptr, channel) in null_channel_ptrs.iter_mut().zip(channels.iter()) {
            *ptr = channel.as_ptr().cast_mut();
        }

        Ok(AudioBuffer {
            num_samples: num_samples as u32,
            channel_ptrs: null_channel_ptrs,
            _marker: std::marker::PhantomData,
        })
    }
}

/// An audio sample.
pub type Sample = f32;

/// Settings used to construct an [`AudioBuffer`].
#[derive(Default, Copy, Clone, Debug)]
pub struct AudioBufferSettings {
    /// The number of channels.
    ///
    /// If `None`, the number of channels is:
    /// - 1 if [`Self::num_samples`] is `None`.
    /// - The length of the data divided by the number of samples per channel if [`Self::num_samples`] is `Some`.
    pub num_channels: Option<u32>,

    /// The number of samples per channel.
    ///
    /// If `None`, the number of samples per channel is:
    /// - The length of the data if [`Self::num_channels`] is `None`.
    /// - The length of the data divided by the number of channels if [`Self::num_channels`] is `Some`.
    pub num_samples: Option<u32>,

    /// The size of a frame.
    ///
    /// If `None`, the frame size is the number of samples per channel.
    pub frame_size: Option<u32>,

    /// Zero-based index of the frame.
    pub frame_index: u32,
}

impl AudioBufferSettings {
    /// Creates a new [`AudioBufferSettings`] with the specified number of channels.
    /// The number of samples per channel will be inferred.
    pub fn with_num_channels(num_channels: u32) -> Self {
        Self {
            num_channels: Some(num_channels),
            ..Default::default()
        }
    }

    /// Creates a new [`AudioBufferSettings`] with the specified number of samples per channel.
    /// The number of channels will be inferred.
    pub fn with_num_samples(num_samples: u32) -> Self {
        Self {
            num_samples: Some(num_samples),
            ..Default::default()
        }
    }

    /// Creates a new [`AudioBufferSettings`] with the specified number of samples per channel and
    /// channels.
    pub fn with_num_channels_and_num_samples(num_channels: u32, num_samples: u32) -> Self {
        Self {
            num_channels: Some(num_channels),
            num_samples: Some(num_samples),
            ..Default::default()
        }
    }

    /// Returns the number of channels and the number of samples derived from these
    /// [`AudioBufferSettings`].
    ///
    /// # Errors
    ///
    /// - [`AudioBufferError::InvalidNumSamples`] if [`Self::num_samples`] is 0 or the data length is not divisible by [`Self::num_samples`].
    /// - [`AudioBufferError::InvalidNumChannels`] if [`Self::num_channels`] is 0 or the data length is not divisible by [`Self::num_channels`].
    pub fn num_channels_and_samples<T: AsRef<[Sample]>>(
        &self,
        data: T,
    ) -> Result<(u32, u32), AudioBufferError> {
        let data = data.as_ref();

        let (num_channels, num_samples) = match (self.num_channels, self.num_samples) {
            (None, None) => (1, data.len() as u32),
            (Some(num_channels), Some(num_samples)) => {
                if num_channels == 0 {
                    return Err(AudioBufferError::InvalidNumChannels { num_channels });
                }

                if num_samples == 0 || num_channels * num_samples != data.len() as u32 {
                    return Err(AudioBufferError::InvalidNumSamples { num_samples });
                }

                (num_channels, num_samples)
            }
            (Some(num_channels), None) => {
                if num_channels == 0 || !(data.len() as u32).is_multiple_of(num_channels) {
                    return Err(AudioBufferError::InvalidNumChannels { num_channels });
                }

                let num_samples = data.len() as u32 / num_channels;

                (num_channels, num_samples)
            }
            (None, Some(num_samples)) => {
                if num_samples == 0 || !(data.len() as u32).is_multiple_of(num_samples) {
                    return Err(AudioBufferError::InvalidNumSamples { num_samples });
                }

                let num_channels = data.len() as u32 / num_samples;

                (num_channels, num_samples)
            }
        };

        Ok((num_channels, num_samples))
    }
}

/// Allocates a vector of mutable pointers to later store channel pointers of an audio buffer.
///
/// # Errors
///
/// - [`AudioBufferError::InvalidNumSamples`] if `num_samples` in `settings` is 0 or the data length is not divisible by `num_samples` in `settings`.
/// - [`AudioBufferError::InvalidNumChannels`] if `num_channels` in `settings` is 0 or the data length is not divisible by `num_channels` in `settings`.
pub fn allocate_channel_ptrs<T: AsRef<[Sample]>>(
    data: T,
    settings: AudioBufferSettings,
) -> Result<Vec<*mut Sample>, AudioBufferError> {
    let (num_channels, _) = settings.num_channels_and_samples(data)?;
    let channel_ptrs = vec![std::ptr::null_mut(); num_channels as usize];
    Ok(channel_ptrs)
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

    /// Error when trying to construct an [`AudioBuffer`] with empty data.
    EmptyData,

    /// Error when trying to construct an [`AudioBuffer`] with an invalid number of samples per
    /// channel.
    InvalidNumSamples { num_samples: u32 },

    /// Error when trying to construct an [`AudioBuffer`] with an invalid number of channels.
    InvalidNumChannels { num_channels: u32 },

    /// Error when trying to construct an [`AudioBuffer`] with an invalid length of channel pointers.
    InvalidChannelPtrs { actual: u32, expected: u32 },

    /// Error when trying to construct an [`AudioBuffer`] with a frame out of channel bounds.
    FrameOutOfBounds { frame_size: u32, frame_index: u32 },
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
            Self::EmptyData => write!(f, "empty audio buffer data",),
            Self::InvalidNumSamples { num_samples } => {
                write!(f, "invalid number of samples per channel: {num_samples}")
            }
            Self::InvalidNumChannels { num_channels } => {
                write!(f, "invalid number of channels: {num_channels}")
            }
            Self::InvalidChannelPtrs { actual, expected } => {
                write!(
                    f,
                    "invalid length of channel pointers: expected {expected}, got {actual}"
                )
            }
            Self::FrameOutOfBounds {
                frame_size,
                frame_index,
            } => {
                write!(
                    f,
                    "frame with index {frame_index} of size {frame_size} out of channel bounds"
                )
            }
        }
    }
}

/// Errors produced by audio buffer operations.
#[derive(Debug, PartialEq, Eq)]
pub enum AudioBufferOperationError {
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

    /// Audio buffers have mismatched total sample count for conversion.
    TotalSampleMismatch {
        self_count: usize,
        other_count: usize,
    },
}

impl std::error::Error for AudioBufferOperationError {}

impl std::fmt::Display for AudioBufferOperationError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
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
            Self::TotalSampleMismatch {
                self_count,
                other_count,
            } => write!(
                f,
                "total sample count mismatch: buffer has {self_count} samples, other has {other_count}"
            ),
        }
    }
}

/// Returns the number of channels required for a given ambisonics order.
///
/// The channel count is given by:
///
/// ```text
/// (order + 1)²
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

    mod try_new {
        use super::*;

        #[test]
        fn test_valid() {
            let mut data = vec![0.5f32; 2048];
            let (left, right) = data.split_at_mut(1024);
            let mut channel_ptrs: Vec<*mut f32> = vec![left.as_mut_ptr(), right.as_mut_ptr()];
            let result =
                unsafe { AudioBuffer::<f32, &mut Vec<*mut f32>>::try_new(&mut channel_ptrs, 1024) };
            assert!(result.is_ok());
        }

        #[test]
        fn test_invalid_num_samples() {
            let channel_ptrs = vec![std::ptr::null_mut(); 2];
            let result = unsafe { AudioBuffer::<(), _>::try_new(channel_ptrs, 0) };
            assert!(matches!(
                result,
                Err(AudioBufferError::InvalidNumSamples { num_samples: 0 }),
            ));
        }

        #[test]
        fn test_invalid_num_channels() {
            let channel_ptrs: Vec<*mut f32> = vec![];
            let result = unsafe { AudioBuffer::<(), _>::try_new(channel_ptrs, 1024) };
            assert!(matches!(
                result,
                Err(AudioBufferError::InvalidNumChannels { num_channels: 0 }),
            ));
        }
    }

    mod try_with_data {
        use super::*;

        #[test]
        fn test_valid() {
            let empty_data: Vec<f32> = vec![0.5; 1024];
            assert!(AudioBuffer::try_with_data(&empty_data).is_ok());
        }

        #[test]
        fn test_empty_data() {
            let empty_data: Vec<f32> = vec![];
            assert!(matches!(
                AudioBuffer::try_with_data(&empty_data),
                Err(AudioBufferError::EmptyData),
            ));
        }
    }

    mod try_with_data_and_settings {
        use super::*;

        #[test]
        fn test_valid_default_settings() {
            let data: Vec<Sample> = vec![0.0; 10];
            let settings = AudioBufferSettings::default();

            let result = AudioBuffer::try_with_data_and_settings(&data, settings);
            assert!(result.is_ok());
        }

        #[test]
        fn test_valid_settings() {
            let data: Vec<Sample> = vec![0.0; 6];
            let settings = AudioBufferSettings {
                num_channels: Some(2),
                num_samples: Some(3),
                ..Default::default()
            };

            let result = AudioBuffer::try_with_data_and_settings(&data, settings);
            assert!(result.is_ok());
        }

        #[test]
        fn test_valid_settings_with_frame_size() {
            let data: Vec<Sample> = vec![0.0; 10];
            let settings = AudioBufferSettings {
                num_channels: Some(2),
                num_samples: Some(5),
                frame_size: Some(3),
                frame_index: 0,
            };

            let result = AudioBuffer::try_with_data_and_settings(&data, settings);
            assert!(result.is_ok());
        }

        #[test]
        fn test_valid_multiple_channels_and_samples() {
            let data: Vec<Sample> = vec![0.0; 12];
            let settings = AudioBufferSettings {
                num_channels: Some(3),
                num_samples: Some(4),
                ..Default::default()
            };

            let result = AudioBuffer::try_with_data_and_settings(&data, settings);
            assert!(result.is_ok());
        }

        #[test]
        fn test_empty_data() {
            let data: Vec<Sample> = vec![];
            let settings = AudioBufferSettings::default();

            let result = AudioBuffer::try_with_data_and_settings(&data, settings);
            assert!(matches!(result, Err(AudioBufferError::EmptyData)));
        }

        #[test]
        fn test_invalid_num_channels_zero() {
            let data: Vec<Sample> = vec![0.0; 10];
            let settings = AudioBufferSettings {
                num_channels: Some(0),
                num_samples: Some(5),
                frame_size: None,
                frame_index: 0,
            };

            let result = AudioBuffer::try_with_data_and_settings(&data, settings);
            assert!(matches!(
                result,
                Err(AudioBufferError::InvalidNumChannels { num_channels: 0 })
            ));
        }

        #[test]
        fn test_invalid_num_samples_zero() {
            let data: Vec<Sample> = vec![0.0; 10];
            let settings = AudioBufferSettings {
                num_channels: Some(2),
                num_samples: Some(0),
                frame_size: None,
                frame_index: 0,
            };

            let result = AudioBuffer::try_with_data_and_settings(&data, settings);
            assert!(matches!(
                result,
                Err(AudioBufferError::InvalidNumSamples { num_samples: 0 })
            ));
        }

        #[test]
        fn test_invalid_num_samples_not_divisible() {
            let data: Vec<Sample> = vec![0.0; 10];
            let settings = AudioBufferSettings {
                num_channels: Some(3),
                num_samples: Some(3),
                frame_size: None,
                frame_index: 0,
            };

            let result = AudioBuffer::try_with_data_and_settings(&data, settings);
            assert!(matches!(
                result,
                Err(AudioBufferError::InvalidNumSamples { num_samples: 3 })
            ));
        }

        #[test]
        fn test_frame_out_of_bounds() {
            let data: Vec<Sample> = vec![0.0; 10];
            let settings = AudioBufferSettings {
                num_channels: Some(2),
                num_samples: Some(5),
                frame_size: Some(3),
                frame_index: 1,
            };

            let result = AudioBuffer::try_with_data_and_settings(&data, settings);
            assert!(matches!(
                result,
                Err(AudioBufferError::FrameOutOfBounds {
                    frame_size: 3,
                    frame_index: 1
                })
            ));
        }
    }

    mod try_new_borrowed {
        use super::*;

        #[test]
        fn test_valid_construction() {
            let mut channel1 = vec![1.0, 2.0, 3.0];
            let mut channel2 = vec![4.0, 5.0, 6.0];
            let mut ptrs = vec![channel1.as_mut_ptr(), channel2.as_mut_ptr()];

            let buffer = unsafe { AudioBuffer::<&[Sample], _>::try_new(&mut ptrs, 3) }.unwrap();
            assert_eq!(buffer.num_channels(), 2);
            assert_eq!(buffer.num_samples(), 3);

            let channels: Vec<&[Sample]> = buffer.channels().collect();
            assert_eq!(channels[0], &[1.0, 2.0, 3.0]);
            assert_eq!(channels[1], &[4.0, 5.0, 6.0]);
        }

        #[test]
        fn test_empty_channel_ptrs() {
            let mut ptrs: Vec<*mut Sample> = vec![];

            let result = unsafe { AudioBuffer::<&[Sample], _>::try_new(&mut ptrs, 100) };

            assert!(matches!(
                result,
                Err(AudioBufferError::InvalidNumChannels { num_channels: 0 })
            ));
        }

        #[test]
        fn test_zero_num_samples() {
            let mut data = vec![1.0, 2.0, 3.0];
            let mut ptrs = vec![data.as_mut_ptr()];

            let result = unsafe { AudioBuffer::<&[Sample], _>::try_new(&mut ptrs, 0) };

            assert!(matches!(
                result,
                Err(AudioBufferError::InvalidNumSamples { num_samples: 0 })
            ));
        }
    }

    mod try_borrowed_with_data_and_settings {
        use super::*;

        #[test]
        fn test_valid_construction() {
            let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
            let settings = AudioBufferSettings::with_num_channels(2);
            let mut channel_ptrs = allocate_channel_ptrs(&data, settings).unwrap();

            let buffer = AudioBuffer::try_borrowed_with_data_and_settings(
                &data,
                &mut channel_ptrs,
                settings,
            )
            .unwrap();
            assert_eq!(buffer.num_channels(), 2);
            assert_eq!(buffer.num_samples(), 3);

            let channels: Vec<&[Sample]> = buffer.channels().collect();
            assert_eq!(channels[0], &[1.0, 2.0, 3.0]);
            assert_eq!(channels[1], &[4.0, 5.0, 6.0]);
        }

        #[test]
        fn test_invalid_channel_ptrs() {
            let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
            let settings = AudioBufferSettings::with_num_channels(2);
            let mut channel_ptrs = [std::ptr::null_mut(); 3];

            let result = AudioBuffer::try_borrowed_with_data_and_settings(
                &data,
                &mut channel_ptrs,
                settings,
            );

            assert!(matches!(
                result,
                Err(AudioBufferError::InvalidChannelPtrs {
                    actual: 3,
                    expected: 2
                })
            ));
        }
    }

    mod try_from_slices {
        use super::*;

        #[test]
        fn test_valid_construction() {
            let channel_0 = vec![1.0, 2.0, 3.0, 4.0];
            let channel_1 = vec![5.0, 6.0, 7.0, 8.0];

            let channels: &[&[Sample]] = &[&channel_0, &channel_1];
            let mut channel_ptrs = vec![std::ptr::null_mut(); 2];

            let audio_buffer = AudioBuffer::try_from_slices(channels, &mut channel_ptrs).unwrap();

            assert_eq!(audio_buffer.num_channels(), 2);
            assert_eq!(audio_buffer.num_samples(), 4);

            let mut iter = audio_buffer.channels();
            assert_eq!(iter.next().unwrap(), &[1.0, 2.0, 3.0, 4.0]);
            assert_eq!(iter.next().unwrap(), &[5.0, 6.0, 7.0, 8.0]);
            assert!(iter.next().is_none());
        }

        #[test]
        fn test_empty_channels() {
            let empty_channels: &[&[Sample]] = &[];
            let mut channel_ptrs = vec![];
            let result = AudioBuffer::try_from_slices(empty_channels, &mut channel_ptrs);
            assert!(matches!(
                result,
                Err(AudioBufferError::InvalidNumChannels { num_channels: 0 })
            ));
        }

        #[test]
        fn test_mismatched_channel_ptrs_length() {
            let channel_0 = vec![1.0, 2.0, 3.0];
            let channel_1 = vec![4.0, 5.0, 6.0];
            let channels: &[&[Sample]] = &[&channel_0, &channel_1];

            let mut channel_ptrs_wrong_size = vec![std::ptr::null_mut(); 1];
            let result = AudioBuffer::try_from_slices(channels, &mut channel_ptrs_wrong_size);
            assert!(matches!(
                result,
                Err(AudioBufferError::InvalidChannelPtrs {
                    actual: 1,
                    expected: 2
                })
            ));
        }

        #[test]
        fn test_empty_channel_data() {
            let empty_channel = vec![];
            let channels_with_empty: &[&[Sample]] = &[&empty_channel];
            let mut channel_ptrs = vec![std::ptr::null_mut(); 1];
            let result = AudioBuffer::try_from_slices(channels_with_empty, &mut channel_ptrs);
            assert!(matches!(
                result,
                Err(AudioBufferError::InvalidNumSamples { num_samples: 0 })
            ));
        }
    }

    mod channels_iteration {
        use super::*;

        #[test]
        fn test_channels_iter() {
            let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
            let buffer = AudioBuffer::try_with_data_and_settings(
                &data,
                AudioBufferSettings::with_num_channels(2),
            )
            .unwrap();

            let channels: Vec<&[Sample]> = buffer.channels().collect();
            assert_eq!(channels.len(), 2);
            assert_eq!(channels[0], &[1.0, 2.0, 3.0]);
            assert_eq!(channels[1], &[4.0, 5.0, 6.0]);
        }

        #[test]
        fn test_channels_mut_iter() {
            let mut data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
            let mut buffer = AudioBuffer::try_with_data_and_settings(
                &mut data,
                AudioBufferSettings::with_num_channels(2),
            )
            .unwrap();

            for channel in buffer.channels_mut() {
                for sample in channel.iter_mut() {
                    *sample *= 2.0;
                }
            }

            assert_eq!(data, vec![2.0, 4.0, 6.0, 8.0, 10.0, 12.0]);
        }
    }

    mod audio_buffer_settings {
        use super::*;

        #[test]
        fn test_with_num_channels() {
            let settings = AudioBufferSettings::with_num_channels(4);
            assert_eq!(settings.num_channels, Some(4));
            assert_eq!(settings.num_samples, None);
        }

        #[test]
        fn test_with_num_samples() {
            let settings = AudioBufferSettings::with_num_samples(1024);
            assert_eq!(settings.num_channels, None);
            assert_eq!(settings.num_samples, Some(1024));
        }

        #[test]
        fn test_with_num_channels_and_num_samples() {
            let settings = AudioBufferSettings::with_num_channels_and_num_samples(2, 512);
            assert_eq!(settings.num_channels, Some(2));
            assert_eq!(settings.num_samples, Some(512));
        }

        #[test]
        fn test_num_channels_and_samples_inference() {
            let data = vec![0.0; 12];

            // Infer both from data length
            let settings = AudioBufferSettings::default();
            let (channels, samples) = settings.num_channels_and_samples(&data).unwrap();
            assert_eq!(channels, 1);
            assert_eq!(samples, 12);

            // Infer samples from channels
            let settings = AudioBufferSettings::with_num_channels(3);
            let (channels, samples) = settings.num_channels_and_samples(&data).unwrap();
            assert_eq!(channels, 3);
            assert_eq!(samples, 4);

            // Infer channels from samples
            let settings = AudioBufferSettings::with_num_samples(4);
            let (channels, samples) = settings.num_channels_and_samples(&data).unwrap();
            assert_eq!(channels, 3);
            assert_eq!(samples, 4);
        }
    }

    mod allocate_channel_ptrs {
        use super::*;

        #[test]
        fn test_valid() {
            let data = vec![0.0; 12];
            let settings = AudioBufferSettings::with_num_channels(3);
            let ptrs = allocate_channel_ptrs(&data, settings).unwrap();

            assert_eq!(ptrs.len(), 3);
            assert!(ptrs.iter().all(|&ptr| ptr.is_null()));
        }

        #[test]
        fn test_invalid() {
            let data = vec![0.0; 10];
            let settings = AudioBufferSettings {
                num_channels: Some(3),
                num_samples: Some(3),
                ..Default::default()
            };

            let result = allocate_channel_ptrs(&data, settings);
            assert!(result.is_err());
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

    mod mix {
        use super::*;

        #[test]
        fn test_valid() {
            let context = Context::default();

            let source = vec![0.5; 100];
            let source_buffer = AudioBuffer::try_with_data(&source).unwrap();

            let mut mix = vec![0.5; 100];
            let mut mix_buffer = AudioBuffer::try_with_data(&mut mix).unwrap();

            assert!(mix_buffer.mix(&context, &source_buffer).is_ok());
        }

        #[test]
        fn test_mismatched_channels() {
            let context = Context::default();

            let source = vec![0.5; 100];
            let source_buffer = AudioBuffer::try_with_data(&source).unwrap();

            let mut mix = vec![0.5; 200];
            let mut mix_buffer = AudioBuffer::try_with_data_and_settings(
                &mut mix,
                AudioBufferSettings::with_num_channels(2),
            )
            .unwrap();

            assert_eq!(
                mix_buffer.mix(&context, &source_buffer),
                Err(AudioBufferOperationError::ChannelCountMismatch {
                    self_num_channels: 2,
                    other_num_channels: 1
                }),
            );
        }

        #[test]
        fn test_sample_count_mismatch() {
            let context = Context::default();

            let source = vec![0.0; 512];
            let source_buffer = AudioBuffer::try_with_data(&source).unwrap();

            let mix = vec![0.0; 1024];
            let mut mix_buffer = AudioBuffer::try_with_data(&mix).unwrap();

            assert_eq!(
                mix_buffer.mix(&context, &source_buffer),
                Err(AudioBufferOperationError::SampleCountMismatch {
                    self_num_samples: 1024,
                    other_num_samples: 512,
                }),
            );
        }
    }

    mod downmix {
        use super::*;

        #[test]
        fn test_valid() {
            let context = Context::default();

            let input = vec![0.5; 200];
            let input_buffer = AudioBuffer::try_with_data(&input).unwrap();

            let mut output = vec![0.5; 200];
            let mut output_buffer = AudioBuffer::try_with_data(&mut output).unwrap();

            assert!(output_buffer.downmix(&context, &input_buffer).is_ok());
        }

        #[test]
        fn test_mismatched_samples() {
            let context = Context::default();

            let input = vec![0.5; 200];
            let input_buffer = AudioBuffer::try_with_data_and_settings(
                &input,
                AudioBufferSettings::with_num_channels(2),
            )
            .unwrap();

            let mut output = vec![0.5; 50];
            let mut output_buffer = AudioBuffer::try_with_data(&mut output).unwrap();

            assert_eq!(
                output_buffer.downmix(&context, &input_buffer),
                Err(AudioBufferOperationError::SampleCountMismatch {
                    self_num_samples: 50,
                    other_num_samples: 100
                }),
            );
        }
    }

    mod interleave {
        use super::*;

        #[test]
        fn test_valid() {
            let context = Context::default();
            let samples = vec![0.0; 1024];
            let buffer = AudioBuffer::try_with_data(&samples).unwrap();

            let mut dst = vec![0.0; 1024];
            assert!(buffer.interleave(&context, &mut dst).is_ok());
        }

        #[test]
        fn test_length_mismatch() {
            let context = Context::default();
            let samples = vec![0.0; 1024];
            let buffer = AudioBuffer::try_with_data(&samples).unwrap();

            let mut dst = vec![0.0; 512];
            assert_eq!(
                buffer.interleave(&context, &mut dst),
                Err(AudioBufferOperationError::InterleaveLengthMismatch {
                    dst_len: 512,
                    expected_len: 1024,
                }),
            );
        }
    }

    mod deinterleave {
        use super::*;

        #[test]
        fn test_valid() {
            let context = Context::default();
            let samples = vec![0.0; 1024];
            let mut buffer = AudioBuffer::try_with_data(&samples).unwrap();

            let src = vec![0.0; 1024];
            assert!(buffer.deinterleave(&context, &src).is_ok());
        }

        #[test]
        fn test_length_mismatch() {
            let context = Context::default();
            let samples = vec![0.0; 1024];
            let mut buffer = AudioBuffer::try_with_data(&samples).unwrap();

            let src = vec![0.0; 2048];
            assert_eq!(
                buffer.deinterleave(&context, &src),
                Err(AudioBufferOperationError::DeinterleaveLengthMismatch {
                    src_len: 2048,
                    expected_len: 1024,
                }),
            );
        }
    }

    mod convert_ambisonics {
        use super::*;

        #[test]
        fn test_valid() {
            let context = Context::default();

            let samples1 = vec![0.0; 1024];
            let mut buffer1 = AudioBuffer::try_with_data_and_settings(
                &samples1,
                AudioBufferSettings::with_num_channels(4),
            )
            .unwrap();

            let samples2 = vec![0.0; 1024];
            let mut buffer2 = AudioBuffer::try_with_data_and_settings(
                &samples2,
                AudioBufferSettings::with_num_channels(4),
            )
            .unwrap();

            assert!(
                buffer1
                    .convert_ambisonics_into(
                        &context,
                        AmbisonicsType::N3D,
                        AmbisonicsType::FuMa,
                        &mut buffer2,
                    )
                    .is_ok()
            );
        }

        #[test]
        fn test_total_sample_mismatch() {
            let context = Context::default();

            let samples1 = vec![0.0; 1024];
            let mut buffer1 = AudioBuffer::try_with_data_and_settings(
                &samples1,
                AudioBufferSettings::with_num_channels(4),
            )
            .unwrap();

            let samples2 = vec![0.0; 512];
            let mut buffer2 = AudioBuffer::try_with_data_and_settings(
                &samples2,
                AudioBufferSettings::with_num_channels(4),
            )
            .unwrap();

            assert_eq!(
                buffer1.convert_ambisonics_into(
                    &context,
                    AmbisonicsType::N3D,
                    AmbisonicsType::FuMa,
                    &mut buffer2,
                ),
                Err(AudioBufferOperationError::TotalSampleMismatch {
                    self_count: 1024,
                    other_count: 512,
                }),
            );
        }
    }
}
