pub mod geometry;

pub const STEAMAUDIO_VERSION: u32 = audionimbus_sys::STEAMAUDIO_VERSION;
pub const STEAMAUDIO_VERSION_MAJOR: u32 = audionimbus_sys::STEAMAUDIO_VERSION_MAJOR;
pub const STEAMAUDIO_VERSION_MINOR: u32 = audionimbus_sys::STEAMAUDIO_VERSION_MINOR;
pub const STEAMAUDIO_VERSION_PATCH: u32 = audionimbus_sys::STEAMAUDIO_VERSION_PATCH;

/// A Head-Related Transfer Function (HRTF).
///
/// HRTFs describe how sound from different directions is perceived by a each of a listener’s ears, and are a crucial component of spatial audio.
/// Steam Audio includes a built-in HRTF, while also allowing developers and users to import their own custom HRTFs.
pub struct Hrtf(pub audionimbus_sys::IPLHRTF);

impl Hrtf {
    pub fn try_new(
        context: Context,
        audio_settings: &AudioSettings,
        hrtf_settings: &HrtfSettings,
    ) -> Result<Self, SteamAudioError> {
        let hrtf = unsafe {
            let hrtf: *mut audionimbus_sys::IPLHRTF = std::ptr::null_mut();
            let status = audionimbus_sys::iplHRTFCreate(
                *context,
                &mut audionimbus_sys::IPLAudioSettings::from(audio_settings),
                &mut audionimbus_sys::IPLHRTFSettings::from(hrtf_settings),
                hrtf,
            );

            if let Some(error) = to_option_error(status) {
                return Err(error);
            }

            *hrtf
        };

        Ok(Self(hrtf))
    }
}

impl std::ops::Deref for Hrtf {
    type Target = audionimbus_sys::IPLHRTF;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for Hrtf {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Drop for Hrtf {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplHRTFRelease(&mut self.0) }
    }
}

/// A context object, which controls low-level operations of Steam Audio.
///
/// Typically, a context is specified once during the execution of the client program, before calling any other API functions.
pub struct Context(pub audionimbus_sys::IPLContext);

impl Context {
    pub fn try_new(settings: &ContextSettings) -> Result<Self, SteamAudioError> {
        let context = unsafe {
            let context: *mut audionimbus_sys::IPLContext = std::ptr::null_mut();
            let status = audionimbus_sys::iplContextCreate(
                &mut audionimbus_sys::IPLContextSettings::from(settings),
                context,
            );

            if let Some(error) = to_option_error(status) {
                return Err(error);
            }

            *context
        };

        Ok(Self(context))
    }

    pub fn as_raw_mut(&mut self) -> *mut audionimbus_sys::IPLContext {
        &mut self.0
    }
}

impl std::ops::Deref for Context {
    type Target = audionimbus_sys::IPLContext;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for Context {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplContextRelease(&mut self.0) }
    }
}

/// Settings used to create a [`Context`].
pub struct ContextSettings {
    // TODO: add other fields from IPLContextSettings.
    /// The API version.
    ///
    /// Context creation will fail if `phonon.dll` does not implement a compatible version of the API.
    /// Typically, this should be set to [`STEAMAUDIO_VERSION`].
    pub version: u32,
}

impl Default for ContextSettings {
    fn default() -> Self {
        Self {
            version: STEAMAUDIO_VERSION,
        }
    }
}

impl From<&ContextSettings> for audionimbus_sys::IPLContextSettings {
    fn from(settings: &ContextSettings) -> Self {
        todo!()
    }
}

/// Settings used to create an [`Hrtf`].
pub struct HrtfSettings {
    /// Volume correction factor to apply to the loaded HRTF data.
    ///
    /// A value of 1.0 means the HRTF data will be used without any change.
    pub volume: f32,

    /// An optional buffer containing SOFA file data from which to load HRTF data.
    pub sofa_data: Option<Vec<u8>>,

    /// Volume normalization setting.
    pub volume_normalization: VolumeNormalization,
}

impl Default for HrtfSettings {
    fn default() -> Self {
        Self {
            volume: 1.0,
            sofa_data: None,
            volume_normalization: VolumeNormalization::None,
        }
    }
}

impl From<&HrtfSettings> for audionimbus_sys::IPLHRTFSettings {
    fn from(settings: &HrtfSettings) -> Self {
        todo!()
    }
}

/// HRTF volume normalization setting.
pub enum VolumeNormalization {
    /// No normalization.
    None,

    /// Root-mean squared normalization.
    ///
    /// Normalize HRTF volume to ensure similar volume from all directions based on root-mean-square value of each HRTF.
    RootMeanSquared,
}

/// Global settings for audio signal processing.
pub struct AudioSettings {
    /// Sampling rate, in Hz.
    pub sampling_rate: u32,

    /// Frame size, in samples.
    /// Independent of number of channels.
    pub frame_size: u32,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            sampling_rate: 48000,
            frame_size: 1024,
        }
    }
}

impl From<&AudioSettings> for audionimbus_sys::IPLAudioSettings {
    fn from(settings: &AudioSettings) -> Self {
        todo!()
    }
}

/// Spatializes a point source using an HRTF, based on the 3D position of the source relative to the listener.
///
/// The source audio can be 1- or 2-channel; in either case all input channels are spatialized from the same position.
pub struct BinauralEffect(pub audionimbus_sys::IPLBinauralEffect);

impl BinauralEffect {
    pub fn try_new(
        context: Context,
        audio_settings: &AudioSettings,
        binaural_effect_settings: &BinauralEffectSettings,
    ) -> Result<Self, SteamAudioError> {
        let binaural_effect = unsafe {
            let binaural_effect: *mut audionimbus_sys::IPLBinauralEffect = std::ptr::null_mut();
            let status = audionimbus_sys::iplBinauralEffectCreate(
                *context,
                &mut audionimbus_sys::IPLAudioSettings::from(audio_settings),
                &mut audionimbus_sys::IPLBinauralEffectSettings::from(binaural_effect_settings),
                binaural_effect,
            );

            if let Some(error) = to_option_error(status) {
                return Err(error);
            }

            *binaural_effect
        };

        Ok(Self(binaural_effect))
    }

    pub fn apply(
        &self,
        binaural_effect_params: &BinauralEffectParams,
        input_buffer: &mut AudioBuffer,
        output_buffer: &mut AudioBuffer,
    ) -> AudioEffectState {
        unsafe {
            audionimbus_sys::iplBinauralEffectApply(
                **self,
                &mut *binaural_effect_params.as_ffi(),
                &mut *input_buffer.as_ffi(),
                &mut *output_buffer.as_ffi(),
            )
        }
        .into()
    }
}

impl std::ops::Deref for BinauralEffect {
    type Target = audionimbus_sys::IPLBinauralEffect;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for BinauralEffect {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Drop for BinauralEffect {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplBinauralEffectRelease(&mut self.0) }
    }
}

/// Settings used to create a binaural effect.
pub struct BinauralEffectSettings {
    /// The HRTF to use.
    pub hrtf: Hrtf,
}

impl From<&BinauralEffectSettings> for audionimbus_sys::IPLBinauralEffectSettings {
    fn from(settings: &BinauralEffectSettings) -> Self {
        todo!()
    }
}

/// States that an audio effect can be left in after processing a frame of audio.
pub enum AudioEffectState {
    /// One or more samples of tail remain in the effect’s internal buffers.
    TailRemaining,

    /// No tail remains in the effect’s internal buffers.
    TailComplete,
}

impl From<audionimbus_sys::IPLAudioEffectState> for AudioEffectState {
    fn from(state: audionimbus_sys::IPLAudioEffectState) -> Self {
        match state {
            audionimbus_sys::IPLAudioEffectState::IPL_AUDIOEFFECTSTATE_TAILREMAINING => {
                Self::TailRemaining
            }
            audionimbus_sys::IPLAudioEffectState::IPL_AUDIOEFFECTSTATE_TAILCOMPLETE => {
                Self::TailComplete
            }
        }
    }
}

/// Parameters for applying an Ambisonics binaural effect to an audio buffer.
pub struct BinauralEffectParams {
    /// Unit vector pointing from the listener towards the source.
    pub direction: geometry::Vector3,

    /// The interpolation technique to use.
    pub interpolation: HrtfInterpolation,

    /// Amount to blend input audio with spatialized audio.
    ///
    /// When set to 0.0, output audio is not spatialized at all and is close to input audio.
    /// If set to 1.0, output audio is fully spatialized.
    pub spatial_blend: f32,

    /// The HRTF to use.
    pub hrtf: Hrtf,

    /// Optional left- and right-ear peak delays for the HRTF used to spatialize the input audio.
    /// Can be None, in which case peak delays will not be written.
    pub peak_delays: Option<[f32; 2]>,
}

impl BinauralEffectParams {
    fn as_ffi(&self) -> FFIWrapper<'_, audionimbus_sys::IPLBinauralEffectParams, Self> {
        let peak_delays_ptr = self
            .peak_delays
            .as_ref()
            .map(|peak_delays| peak_delays.as_ptr() as *mut f32)
            .unwrap_or(std::ptr::null_mut());

        let binaural_effect_params = audionimbus_sys::IPLBinauralEffectParams {
            direction: self.direction.into(),
            interpolation: self.interpolation.into(),
            spatialBlend: self.spatial_blend,
            hrtf: *self.hrtf,
            peakDelays: peak_delays_ptr,
        };

        FFIWrapper::new(binaural_effect_params)
    }
}

/// Techniques for interpolating HRTF data.
///
/// This is used when rendering a point source whose position relative to the listener is not contained in the measured HRTF data.
#[derive(Copy, Clone)]
pub enum HrtfInterpolation {
    /// Nearest-neighbor filtering, i.e., no interpolation.
    ///
    /// Selects the measurement location that is closest to the source’s actual location.
    Nearest,

    /// Bilinear filtering.
    ///
    /// Incurs a relatively high CPU overhead as compared to nearest-neighbor filtering, so use this for sounds where it has a significant benefit.
    /// Typically, bilinear filtering is most useful for wide-band noise-like sounds, such as radio static, mechanical noise, fire, etc.
    Bilinear,
}

impl From<HrtfInterpolation> for audionimbus_sys::IPLHRTFInterpolation {
    fn from(hrtf_interpolation: HrtfInterpolation) -> Self {
        match hrtf_interpolation {
            HrtfInterpolation::Nearest => {
                audionimbus_sys::IPLHRTFInterpolation::IPL_HRTFINTERPOLATION_NEAREST
            }
            HrtfInterpolation::Bilinear => {
                audionimbus_sys::IPLHRTFInterpolation::IPL_HRTFINTERPOLATION_BILINEAR
            }
        }
    }
}

/// A generic wrapper that ties the lifetime of an FFI type (`T`) to the lifetime of a struct (`Owner`).
pub struct FFIWrapper<'a, T, Owner> {
    pub ffi_object: T,
    _marker: std::marker::PhantomData<&'a Owner>,
}

impl<T, Owner> FFIWrapper<'_, T, Owner> {
    pub fn new(ffi_object: T) -> Self {
        FFIWrapper {
            ffi_object,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<T, Owner> std::ops::Deref for FFIWrapper<'_, T, Owner> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.ffi_object
    }
}

impl<T, Owner> std::ops::DerefMut for FFIWrapper<'_, T, Owner> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ffi_object
    }
}

/// An audio buffer.
///
/// All audio buffers passed to Steam Audio must be deinterleaved.
pub struct AudioBuffer {
    pub num_channels: usize,
    pub num_samples: usize,
    pub data: Vec<f32>,
    channel_ptrs: Vec<*mut f32>,
}

impl AudioBuffer {
    pub fn with_num_channels_and_num_samples(num_channels: usize, num_samples: usize) -> Self {
        let mut data: Vec<f32> = vec![0.0; num_channels * num_samples];

        let mut channel_ptrs: Vec<*mut f32> = Vec::with_capacity(num_channels);
        for i in 0..num_channels {
            let channel_ptr = data.as_mut_ptr().wrapping_add(i * num_samples);
            channel_ptrs.push(channel_ptr);
        }

        Self {
            num_channels,
            num_samples,
            data,
            channel_ptrs,
        }
    }

    fn as_ffi(&mut self) -> FFIWrapper<'_, audionimbus_sys::IPLAudioBuffer, Self> {
        let audio_buffer = audionimbus_sys::IPLAudioBuffer {
            numChannels: self.num_channels as i32,
            numSamples: self.num_samples as i32,
            data: self.channel_ptrs.as_mut_ptr(),
        };

        FFIWrapper::new(audio_buffer)
    }
}

impl From<Vec<Vec<f32>>> for AudioBuffer {
    fn from(channels: Vec<Vec<f32>>) -> Self {
        let num_channels = channels.len();
        let num_samples = channels.first().map_or(0, |channel| channel.len());
        let mut data: Vec<f32> = channels.into_iter().flatten().collect();

        let mut channel_ptrs: Vec<*mut f32> = Vec::with_capacity(num_channels);
        for i in 0..num_channels {
            let channel_ptr = data.as_mut_ptr().wrapping_add(i * num_samples);
            channel_ptrs.push(channel_ptr);
        }

        Self {
            num_channels,
            num_samples,
            data,
            channel_ptrs,
        }
    }
}

/// A Steam Audio error.
#[derive(Debug)]
pub enum SteamAudioError {
    /// An unspecified error occurred.
    Unspecified,

    /// The system ran out of memory.
    OutOfMemory,

    /// An error occurred while initializing an external dependency.
    Initialization,
}

impl std::error::Error for SteamAudioError {}

impl std::fmt::Display for SteamAudioError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match &self {
            Self::Unspecified => write!(f, "unspecified error",),
            Self::OutOfMemory => write!(f, "out of memory"),
            Self::Initialization => write!(f, "error while initializing an external dependency"),
        }
    }
}

fn to_option_error(status: audionimbus_sys::IPLerror) -> Option<SteamAudioError> {
    match status {
        audionimbus_sys::IPLerror::IPL_STATUS_SUCCESS => None,
        audionimbus_sys::IPLerror::IPL_STATUS_FAILURE => Some(SteamAudioError::Unspecified),
        audionimbus_sys::IPLerror::IPL_STATUS_OUTOFMEMORY => Some(SteamAudioError::OutOfMemory),
        audionimbus_sys::IPLerror::IPL_STATUS_INITIALIZATION => {
            Some(SteamAudioError::Initialization)
        }
    }
}
