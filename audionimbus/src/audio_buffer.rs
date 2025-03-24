use crate::context::Context;
use crate::ffi_wrapper::FFIWrapper;

/// An audio buffer descriptor.
///
/// This struct does not hold the actual sample data, but instead contains pointers to samples stored elsewhere.
/// The generic parameter `T` is used to ensure that these pointers remain valid for the lifetime of the underlying data.
#[derive(Debug)]
pub struct AudioBuffer<T> {
    /// Number of samples per channel.
    num_samples: usize,

    /// Pointers to sample data for each channel.
    channel_ptrs: Vec<*mut Sample>,

    /// Marker to enforce the lifetime of the channel pointers.
    _marker: std::marker::PhantomData<T>,
}

impl<T: AsRef<[Sample]>> AudioBuffer<T> {
    /// Constructs a new `AudioBuffer` from raw pointers to mutable channel samples and the number
    /// of samples.
    ///
    /// This function is designed to provide maximum flexibility for advanced users who need
    /// fine-grained control over the memory layout of audio data.
    /// However, for most use cases, the safe constructors [`Self::try_with_data_and_settings`] and
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
    pub unsafe fn try_new(
        channel_ptrs: Vec<*mut Sample>,
        num_samples: usize,
    ) -> Result<Self, AudioBufferError> {
        if channel_ptrs.is_empty() {
            return Err(AudioBufferError::InvalidNumChannels {
                num_channels: channel_ptrs.len(),
            });
        }

        if num_samples == 0 {
            return Err(AudioBufferError::InvalidNumSamples { num_samples });
        }

        debug_assert!(
            channel_ptrs.iter().all(|&ptr| !ptr.is_null()),
            "some channel pointers are null"
        );

        Ok(Self {
            num_samples,
            channel_ptrs,
            _marker: std::marker::PhantomData,
        })
    }

    /// Returns the number of channels of the audio buffer.
    pub fn num_channels(&self) -> usize {
        self.channel_ptrs.len()
    }

    /// Returns the number of samples per channel in the audio buffer.
    pub fn num_samples(&self) -> usize {
        self.num_samples
    }

    /// Reads samples from the audio buffer and interleaves them into `dst`.
    pub fn interleave(&self, context: &Context, dst: &mut [Sample]) {
        assert_eq!(
            dst.len(),
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
            src.len(),
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

    /// Mixes `other` into `self`.
    ///
    /// Both audio buffers must have the same number of channels and samples.
    pub fn mix(&mut self, context: &Context, other: &AudioBuffer<T>) {
        assert_eq!(
            self.num_channels(),
            other.num_channels(),
            "both audio buffers must have the same number of channels"
        );

        assert_eq!(
            self.num_samples(),
            other.num_samples(),
            "both audio buffers must have the same number of samples per channel"
        );

        unsafe {
            audionimbus_sys::iplAudioBufferMix(
                context.raw_ptr(),
                &mut *self.as_ffi(),
                &mut *other.as_ffi(),
            );
        }
    }

    /// Downmixes the multi-channel `self` audio buffer into a mono `output` audio buffer.
    ///
    /// Both audio buffers must have the same number of samples per channel.
    ///
    /// Downmixing is performed by summing up the source channels and dividing the result by the number of source channels.
    /// If this is not the desired downmixing behavior, we recommend that downmixing be performed manually.
    pub fn downmix(&mut self, context: &Context, output: &mut AudioBuffer<T>) {
        assert_eq!(
            self.num_samples(),
            output.num_samples(),
            "both audio buffers must have the same number of samples per channel"
        );

        unsafe {
            audionimbus_sys::iplAudioBufferDownmix(
                context.raw_ptr(),
                &mut *self.as_ffi(),
                &mut *output.as_ffi(),
            );
        }
    }

    /// Constructs an `AudioBuffer` over `data` with one channel spanning the entire data provided.
    pub fn try_with_data(data: T) -> Result<Self, AudioBufferError> {
        Self::try_with_data_and_settings(data, &AudioBufferSettings::default())
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
        settings: &AudioBufferSettings,
    ) -> Result<Self, AudioBufferError> {
        try_new_audio_buffer_with_data_and_settings(data.as_ref(), settings)
    }

    /// Returns an iterator over channels.
    pub fn channels(&self) -> impl Iterator<Item = &[Sample]> {
        self.channel_ptrs.iter().map(|&ptr|
                // SAFETY: pointers are guaranteed to be valid by the lifetime.
                unsafe { std::slice::from_raw_parts(ptr, self.num_samples) })
    }

    pub(crate) fn as_ffi(&self) -> FFIWrapper<'_, audionimbus_sys::IPLAudioBuffer, Self> {
        let audio_buffer = audionimbus_sys::IPLAudioBuffer {
            numChannels: self.num_channels() as i32,
            numSamples: self.num_samples() as i32,
            data: self.channel_ptrs.as_ptr() as *mut *mut Sample,
        };

        FFIWrapper::new(audio_buffer)
    }
}

/// An audio sample.
pub type Sample = f32;

/// Constructs an `AudioBuffer` over `data` given the provided [`AudioBufferSettings`].
///
/// # Errors
///
/// - [`AudioBufferError::EmptyData`] if the `data` slice is empty.
/// - [`AudioBufferError::InvalidNumSamples`] if `num_samples` is 0 or the data length is not divisible by `num_samples`.
/// - [`AudioBufferError::InvalidNumChannels`] if `num_channels` is 0 or the data length is not divisible by `num_channels`.
/// - [`AudioBufferError::FrameOutOfBounds`] if the frame is out of channel bounds.
fn try_new_audio_buffer_with_data_and_settings<T>(
    data: &[Sample],
    settings: &AudioBufferSettings,
) -> Result<AudioBuffer<T>, AudioBufferError> {
    if data.is_empty() {
        return Err(AudioBufferError::EmptyData);
    }

    let (num_channels, num_samples) = match (settings.num_channels, settings.num_samples) {
        (None, None) => (1, data.len()),
        (Some(num_channels), Some(num_samples)) => {
            if num_channels == 0 {
                return Err(AudioBufferError::InvalidNumChannels { num_channels });
            }

            if num_samples == 0 || num_channels * num_samples != data.len() {
                return Err(AudioBufferError::InvalidNumSamples { num_samples });
            }

            (num_channels, num_samples)
        }
        (Some(num_channels), None) => {
            if num_channels == 0 || data.len() % num_channels != 0 {
                return Err(AudioBufferError::InvalidNumChannels { num_channels });
            }

            let num_samples = data.len() / num_channels;

            (num_channels, num_samples)
        }
        (None, Some(num_samples)) => {
            if num_samples == 0 || data.len() % num_samples != 0 {
                return Err(AudioBufferError::InvalidNumSamples { num_samples });
            }

            let num_channels = data.len() / num_samples;

            (num_channels, num_samples)
        }
    };

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
            let index = channel * num_samples + frame_index * frame_size;
            data[index..].as_ptr() as *mut Sample
        })
        .collect();

    Ok(AudioBuffer {
        num_samples: frame_size,
        channel_ptrs,
        _marker: std::marker::PhantomData,
    })
}

/// Settings used to construct an [`AudioBuffer`].
#[derive(Default, Debug)]
pub struct AudioBufferSettings {
    /// The number of channels.
    ///
    /// If `None`, the number of channels is:
    /// - 1 if [`Self::num_samples`] is `None`.
    /// - The length of the data divided by the number of samples per channel if [`Self::num_samples`] is `Some`.
    pub num_channels: Option<usize>,

    /// The number of samples per channel.
    ///
    /// If `None`, the number of samples per channel is:
    /// - The length of the data if [`Self::num_channels`] is `None`.
    /// - The length of the data divided by the number of channels if [`Self::num_channels`] is `Some`.
    pub num_samples: Option<usize>,

    /// The size of a frame.
    ///
    /// If `None`, the frame size is the number of samples per channel.
    pub frame_size: Option<usize>,

    /// Zero-based index of the frame.
    pub frame_index: usize,
}

/// [`AudioBuffer`] construction errors.
#[derive(Debug)]
pub enum AudioBufferError {
    /// Error when trying to construct an [`AudioBuffer`] with empty data.
    EmptyData,

    /// Error when trying to construct an [`AudioBuffer`] with an invalid number of samples per
    /// channel.
    InvalidNumSamples { num_samples: usize },

    /// Error when trying to construct an [`AudioBuffer`] with an invalid number of channels.
    InvalidNumChannels { num_channels: usize },

    /// Error when trying to construct an [`AudioBuffer`] with a frame out of channel bounds.
    FrameOutOfBounds {
        frame_size: usize,
        frame_index: usize,
    },
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

    #[cfg(test)]
    mod try_new_audio_buffer_with_data_and_settings {
        use super::*;

        #[test]
        fn test_valid_default_settings() {
            let data: Vec<Sample> = vec![0.0; 10];
            let settings = AudioBufferSettings::default();

            let result = try_new_audio_buffer_with_data_and_settings::<()>(&data, &settings);
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

            let result = try_new_audio_buffer_with_data_and_settings::<()>(&data, &settings);
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

            let result = try_new_audio_buffer_with_data_and_settings::<()>(&data, &settings);
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

            let result = try_new_audio_buffer_with_data_and_settings::<()>(&data, &settings);
            assert!(result.is_ok());
        }

        #[test]
        fn test_empty_data() {
            let data: Vec<Sample> = vec![];
            let settings = AudioBufferSettings::default();

            let result = try_new_audio_buffer_with_data_and_settings::<()>(&data, &settings);
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

            let result = try_new_audio_buffer_with_data_and_settings::<()>(&data, &settings);
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

            let result = try_new_audio_buffer_with_data_and_settings::<()>(&data, &settings);
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

            let result = try_new_audio_buffer_with_data_and_settings::<()>(&data, &settings);
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

            let result = try_new_audio_buffer_with_data_and_settings::<()>(&data, &settings);
            assert!(matches!(
                result,
                Err(AudioBufferError::FrameOutOfBounds {
                    frame_size: 3,
                    frame_index: 1
                })
            ));
        }
    }
}
