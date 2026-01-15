use crate::context::Context;
use crate::effect::ambisonics::AmbisonicsType;
use crate::ffi_wrapper::FFIWrapper;

pub trait ChannelPointers {
    fn as_slice(&self) -> &[*mut Sample];
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
    pub fn num_samples(&self) -> u32 {
        self.num_samples
    }

    /// Reads samples from the audio buffer and interleaves them into `dst`.
    pub fn interleave(&self, context: &Context, dst: &mut [Sample]) {
        assert_eq!(
            dst.len() as u32,
            self.num_channels() * self.num_samples(),
            "destination slice and audio buffer must have the same length"
        );

        let mut audio_buffer_ffi = self.as_ffi();

        unsafe {
            audionimbus_sys::iplAudioBufferInterleave(
                context.raw_ptr(),
                &mut *audio_buffer_ffi,
                dst.as_mut_ptr(),
            );
        }
    }

    /// Deinterleaves the `src` sample data into `Self`.
    pub fn deinterleave(&mut self, context: &Context, src: &[Sample]) {
        assert_eq!(
            src.len() as u32,
            self.num_channels() * self.num_samples(),
            "source slice and audio buffer must have the same length"
        );

        let mut audio_buffer_ffi = self.as_ffi();

        unsafe {
            audionimbus_sys::iplAudioBufferDeinterleave(
                context.raw_ptr(),
                src.as_ptr() as *mut Sample,
                &mut *audio_buffer_ffi,
            )
        };
    }

    /// Mixes `source` into `self`.
    ///
    /// Both audio buffers must have the same number of channels and samples.
    pub fn mix<T2, P2: ChannelPointers>(
        &mut self,
        context: &Context,
        source: &AudioBuffer<T2, P2>,
    ) {
        assert_eq!(
            self.num_channels(),
            source.num_channels(),
            "both audio buffers must have the same number of channels"
        );

        assert_eq!(
            self.num_samples(),
            source.num_samples(),
            "both audio buffers must have the same number of samples per channel"
        );

        unsafe {
            audionimbus_sys::iplAudioBufferMix(
                context.raw_ptr(),
                &mut *source.as_ffi(),
                &mut *self.as_ffi(),
            );
        }
    }

    /// Downmixes the multi-channel `source` audio buffer into a mono `self` audio buffer.
    ///
    /// Both audio buffers must have the same number of samples per channel.
    ///
    /// Downmixing is performed by summing up the source channels and dividing the result by the number of source channels.
    /// If this is not the desired downmixing behavior, we recommend that downmixing be performed manually.
    pub fn downmix<T2, P2: ChannelPointers>(
        &mut self,
        context: &Context,
        source: &AudioBuffer<T2, P2>,
    ) {
        assert_eq!(
            self.num_samples(),
            source.num_samples(),
            "both audio buffers must have the same number of samples per channel"
        );

        unsafe {
            audionimbus_sys::iplAudioBufferDownmix(
                context.raw_ptr(),
                &mut *source.as_ffi(),
                &mut *self.as_ffi(),
            );
        }
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
                &mut *self.as_ffi(),
                &mut *self.as_ffi(),
            )
        }
    }

    /// Converts an Ambisonic audio buffer from one Ambisonic format to another.
    ///
    /// Both audio buffers must have the same number of samples.
    ///
    /// Steam Audio’s "native" Ambisonic format is [`AmbisonicsType::N3D`], so for best performance, keep all Ambisonic data in N3D format except when exchanging data with your audio engine.
    pub fn convert_ambisonics_into<T2, P2: ChannelPointers>(
        &mut self,
        context: &Context,
        in_type: AmbisonicsType,
        out_type: AmbisonicsType,
        out: &mut AudioBuffer<T2, P2>,
    ) {
        assert_eq!(
            self.num_channels() * self.num_samples(),
            out.num_channels() * out.num_samples(),
            "both audio buffers must have the same number of samples"
        );

        unsafe {
            audionimbus_sys::iplAudioBufferConvertAmbisonics(
                context.raw_ptr(),
                in_type.into(),
                out_type.into(),
                &mut *self.as_ffi(),
                &mut *out.as_ffi(),
            )
        }
    }

    pub(crate) fn as_ffi(&self) -> FFIWrapper<'_, audionimbus_sys::IPLAudioBuffer, Self> {
        let audio_buffer = audionimbus_sys::IPLAudioBuffer {
            numChannels: self.num_channels() as i32,
            numSamples: self.num_samples() as i32,
            data: self.channel_ptrs.as_slice().as_ptr() as *mut *mut Sample,
        };

        FFIWrapper::new(audio_buffer)
    }
}

impl<T: AsRef<[Sample]>> AudioBuffer<T, Vec<*mut Sample>> {
    /// Constructs an `AudioBuffer` over `data` with one channel spanning the entire data provided.
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
                data[index..].as_ptr() as *mut Sample
            })
            .collect();

        Ok(AudioBuffer {
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
    /// - [`AudioBufferError::InvalidNumSamples`] if `num_samples` is 0 or the data length is not divisible by `num_samples`.
    /// - [`AudioBufferError::InvalidNumChannels`] if `num_channels` is 0 or the data length is not divisible by `num_channels`.
    /// - [`AudioBufferError::FrameOutOfBounds`] if the frame is out of channel bounds.
    /// - [`AudioBufferError::InvalidChannelPtrs`] if the length of `null_channel_ptrs` is not equal to `num_channels`.
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
    /// - [`AudioBufferError::EmptyData`] if the `data` slice is empty.
    /// - [`AudioBufferError::InvalidNumSamples`] if `num_samples` is 0 or the data length is not divisible by `num_samples`.
    /// - [`AudioBufferError::InvalidNumChannels`] if `num_channels` is 0 or the data length is not divisible by `num_channels`.
    /// - [`AudioBufferError::FrameOutOfBounds`] if the frame is out of channel bounds.
    /// - [`AudioBufferError::InvalidChannelPtrs`] if the length of `null_channel_ptrs` is not equal to `num_channels`.
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
                *channel = data[index as usize..].as_ptr() as *mut Sample;
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
            *ptr = channel.as_ptr() as *mut Sample;
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
#[derive(Debug)]
pub enum AudioBufferError {
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
