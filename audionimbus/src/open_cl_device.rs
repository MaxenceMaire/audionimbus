use crate::context::Context;
use crate::error::{to_option_error, SteamAudioError};

#[derive(Debug)]
pub struct OpenClDevice(pub audionimbus_sys::IPLOpenCLDevice);

impl OpenClDevice {
    pub fn new(
        context: &Context,
        device_list: OpenClDeviceList,
        index: i32,
    ) -> Result<Self, SteamAudioError> {
        let open_cl_device = unsafe {
            let open_cl_device: *mut audionimbus_sys::IPLOpenCLDevice = std::ptr::null_mut();
            let status = audionimbus_sys::iplOpenCLDeviceCreate(
                context.as_raw_ptr(),
                *device_list,
                index,
                open_cl_device,
            );

            if let Some(error) = to_option_error(status) {
                return Err(error);
            }

            *open_cl_device
        };

        Ok(Self(open_cl_device))
    }
}

impl std::ops::Deref for OpenClDevice {
    type Target = audionimbus_sys::IPLOpenCLDevice;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for OpenClDevice {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Drop for OpenClDevice {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplOpenCLDeviceRelease(&mut self.0) }
    }
}

pub struct OpenClDeviceList(pub audionimbus_sys::IPLOpenCLDeviceList);

impl OpenClDeviceList {
    pub fn new(
        context: &Context,
        open_cl_device_settings: &OpenClDeviceSettings,
    ) -> Result<Self, SteamAudioError> {
        let open_cl_device_list = unsafe {
            let open_cl_device_list: *mut audionimbus_sys::IPLOpenCLDeviceList =
                std::ptr::null_mut();
            let status = audionimbus_sys::iplOpenCLDeviceListCreate(
                context.as_raw_ptr(),
                &mut audionimbus_sys::IPLOpenCLDeviceSettings::from(open_cl_device_settings),
                open_cl_device_list,
            );

            if let Some(error) = to_option_error(status) {
                return Err(error);
            }

            *open_cl_device_list
        };

        Ok(Self(open_cl_device_list))
    }
}

impl std::ops::Deref for OpenClDeviceList {
    type Target = audionimbus_sys::IPLOpenCLDeviceList;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for OpenClDeviceList {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Drop for OpenClDeviceList {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplOpenCLDeviceListRelease(&mut self.0) }
    }
}

/// Specifies requirements that an OpenCL device must meet in order to be considered when listing OpenCL devices.
#[derive(Debug)]
pub struct OpenClDeviceSettings {
    device_type: OpenClDeviceType,

    /// The number of GPU compute units (CUs) that should be reserved for use by Steam Audio.
    ///
    /// If set to a non-zero value, then a GPU will be included in the device list only if it can reserve at least this many CUs.
    /// Set to 0 to indicate that Steam Audio can use the entire GPU, in which case all available GPUs will be considered.
    num_compute_units_to_reserve: i32,

    /// The fraction of reserved CUs that should be used for impulse response (IR) update.
    ///
    /// IR update includes: a) ray tracing using Radeon Rays to simulate sound propagation, and/or b) pre-transformation of IRs for convolution using TrueAudio Next.
    /// Steam Audio will only list GPU devices that are able to subdivide the reserved CUs as per this value.
    /// The value must be between 0.0 and 1.0.
    ///
    /// For example, if `num_compute_units_to_reserve` is 8, and `fraction_of_compute_units_for_impulse_response_update` is 0.5, then 4 CUs will be used for IR update and 4 CUs will be used for convolution.
    /// Below are typical scenarios:
    /// - Using only TrueAudio Next. Set `fraction_of_compute_units_for_impulse_response_update` to 0.5. This ensures that reserved CUs are available for IR update as well as convolution.
    /// - Using TrueAudio Next and Radeon Rays for real-time simulation and rendering. Choosing `fraction_of_compute_units_for_impulse_response_update` may require some experimentation to utilize reserved CUs optimally. You can start by setting `fraction_of_compute_units_for_impulse_response_update` to 0.5. However, if IR calculation has high latency with these settings, increase `fraction_of_compute_units_for_impulse_response_update` to use more CUs for ray tracing.
    /// - Using only Radeon Rays. Set `fraction_of_compute_units_for_impulse_response_update` to 1.0, to make sure all the reserved CUs are used for ray tracing. If using Steam Audio for preprocessing (e.g. baking reverb), then consider setting `fraction_of_compute_units_for_impulse_response_update` to 0.0 to use the entire GPU for accelerated ray tracing.
    ///
    /// Ignored if `num_compute_units_to_reserve` is 0.
    fraction_of_compute_units_for_impulse_response_update: f32,

    /// If `true`, then the GPU device must support TrueAudio Next.
    ///
    /// It is not necessary to set this to `true` if `num_compute_units_to_reserve` or `fraction_of_compute_units_for_impulse_response_update` are set to non-zero values.
    requires_true_audio_next: bool,
}

impl Default for OpenClDeviceSettings {
    fn default() -> Self {
        Self {
            device_type: OpenClDeviceType::Cpu,
            num_compute_units_to_reserve: 0,
            fraction_of_compute_units_for_impulse_response_update: 0.0,
            requires_true_audio_next: false,
        }
    }
}

impl From<&OpenClDeviceSettings> for audionimbus_sys::IPLOpenCLDeviceSettings {
    fn from(settings: &OpenClDeviceSettings) -> Self {
        audionimbus_sys::IPLOpenCLDeviceSettings {
            type_: settings.device_type.into(),
            numCUsToReserve: settings.num_compute_units_to_reserve,
            fractionCUsForIRUpdate: settings.fraction_of_compute_units_for_impulse_response_update,
            requiresTAN: if settings.requires_true_audio_next {
                audionimbus_sys::IPLbool::IPL_TRUE
            } else {
                audionimbus_sys::IPLbool::IPL_FALSE
            },
        }
    }
}

/// The type of device.
#[derive(Copy, Clone, Debug)]
pub enum OpenClDeviceType {
    /// List both CPU and GPU devices.
    Any,

    /// Only list CPU devices.
    Cpu,

    /// Only list GPU devices.
    Gpu,
}

impl From<OpenClDeviceType> for audionimbus_sys::IPLOpenCLDeviceType {
    fn from(device_type: OpenClDeviceType) -> Self {
        match device_type {
            OpenClDeviceType::Any => audionimbus_sys::IPLOpenCLDeviceType::IPL_OPENCLDEVICETYPE_ANY,
            OpenClDeviceType::Cpu => audionimbus_sys::IPLOpenCLDeviceType::IPL_OPENCLDEVICETYPE_CPU,
            OpenClDeviceType::Gpu => audionimbus_sys::IPLOpenCLDeviceType::IPL_OPENCLDEVICETYPE_GPU,
        }
    }
}
