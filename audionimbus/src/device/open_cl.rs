//! OpenCL backend.

use crate::context::Context;
use crate::error::{to_option_error, SteamAudioError};

/// Application-wide state for OpenCL.
///
/// An OpenCL device must be created before using any of Steam Audio’s Radeon Rays or TrueAudio Next functionality.
#[derive(Debug)]
pub struct OpenClDevice(audionimbus_sys::IPLOpenCLDevice);

impl OpenClDevice {
    /// Creates a new OpenCL device from a device list.
    ///
    /// # Arguments
    ///
    /// - `context`: The context used to initialize AudioNimbus.
    /// - `device_list`: List of available OpenCL devices.
    /// - `index`: Index of the device to create from the list.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if device creation fails.
    pub fn try_new(
        context: &Context,
        device_list: &OpenClDeviceList,
        index: usize,
    ) -> Result<Self, SteamAudioError> {
        let mut open_cl_device = Self(std::ptr::null_mut());

        let status = unsafe {
            audionimbus_sys::iplOpenCLDeviceCreate(
                context.raw_ptr(),
                **device_list,
                index as i32,
                open_cl_device.raw_ptr_mut(),
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        Ok(open_cl_device)
    }

    /// Creates an OpenCL device from an existing OpenCL device created by your application. Steam Audio will use up to two command queues that you provide for enqueuing OpenCL computations.
    ///
    /// # Safety
    ///
    /// The caller must ensure that:
    /// - `convolution_queue` and `ir_update_queue` are valid OpenCL command queue handles.
    /// - They remain valid for the lifetime of the created device.
    ///
    /// # Arguments
    ///
    /// - `context`: the context used to initialize AudioNimbus.
    /// - `convolution_queue`: the command queue to use for enqueueing convolution work.
    /// - `ir_update_queue`: the command queue to use for enqueueing IR update work.
    ///
    /// # Errors
    ///
    /// Returns [`SteamAudioError`] if device creation fails.
    pub unsafe fn from_existing(
        context: &Context,
        convolution_queue: *mut std::ffi::c_void,
        ir_update_queue: *mut std::ffi::c_void,
    ) -> Result<Self, SteamAudioError> {
        let mut open_cl_device = Self(std::ptr::null_mut());

        let status = unsafe {
            audionimbus_sys::iplOpenCLDeviceCreateFromExisting(
                context.raw_ptr(),
                convolution_queue,
                ir_update_queue,
                open_cl_device.raw_ptr_mut(),
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        Ok(open_cl_device)
    }

    /// Returns the raw FFI pointer to the underlying OpenCL device.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub fn raw_ptr(&self) -> audionimbus_sys::IPLOpenCLDevice {
        self.0
    }

    /// Returns a mutable reference to the raw FFI pointer.
    ///
    /// This is intended for internal use and advanced scenarios.
    pub fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLOpenCLDevice {
        &mut self.0
    }
}

impl Clone for OpenClDevice {
    fn clone(&self) -> Self {
        unsafe {
            audionimbus_sys::iplOpenCLDeviceRetain(self.0);
        }
        Self(self.0)
    }
}

impl Drop for OpenClDevice {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplOpenCLDeviceRelease(&mut self.0) }
    }
}

unsafe impl Send for OpenClDevice {}
unsafe impl Sync for OpenClDevice {}

/// Provides a list of OpenCL devices available on the user’s system.
///
/// Use this to enumerate the available OpenCL devices, inspect their capabilities, and select the most suitable one for your application’s needs.
pub struct OpenClDeviceList(audionimbus_sys::IPLOpenCLDeviceList);

impl OpenClDeviceList {
    pub fn try_new(
        context: &Context,
        open_cl_device_settings: &OpenClDeviceSettings,
    ) -> Result<Self, SteamAudioError> {
        let mut open_cl_device_list: audionimbus_sys::IPLOpenCLDeviceList = std::ptr::null_mut();
        let mut settings = audionimbus_sys::IPLOpenCLDeviceSettings::from(open_cl_device_settings);

        let status = unsafe {
            audionimbus_sys::iplOpenCLDeviceListCreate(
                context.raw_ptr(),
                &mut settings,
                &mut open_cl_device_list,
            )
        };

        if let Some(error) = to_option_error(status) {
            return Err(error);
        }

        Ok(Self(open_cl_device_list))
    }

    /// Returns the number of devices in the OpenCL device list.
    pub fn num_devices(&self) -> usize {
        unsafe { audionimbus_sys::iplOpenCLDeviceListGetNumDevices(self.raw_ptr()) as usize }
    }

    /// Retrieves information about a specific device in an OpenCL device list.
    ///
    /// # Errors
    ///
    /// Returns [`OpenClDeviceListError::DeviceIndexOutOfBounds`] if `device_index` is out of bounds.
    pub fn device_descriptor(
        &self,
        device_index: usize,
    ) -> Result<OpenClDeviceDescriptor, OpenClDeviceListError> {
        let num_devices = self.num_devices();
        if device_index >= num_devices {
            return Err(OpenClDeviceListError::DeviceIndexOutOfBounds {
                device_index,
                num_devices,
            });
        }

        let mut device_descriptor =
            std::mem::MaybeUninit::<audionimbus_sys::IPLOpenCLDeviceDesc>::uninit();

        unsafe {
            audionimbus_sys::iplOpenCLDeviceListGetDeviceDesc(
                self.raw_ptr(),
                device_index as i32,
                device_descriptor.as_mut_ptr(),
            );

            let device_descriptor = device_descriptor.assume_init();
            Ok(OpenClDeviceDescriptor::try_from(&device_descriptor).expect("invalid C string"))
        }
    }

    pub fn raw_ptr(&self) -> audionimbus_sys::IPLOpenCLDeviceList {
        self.0
    }

    pub fn raw_ptr_mut(&mut self) -> &mut audionimbus_sys::IPLOpenCLDeviceList {
        &mut self.0
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

impl Clone for OpenClDeviceList {
    fn clone(&self) -> Self {
        unsafe {
            audionimbus_sys::iplOpenCLDeviceListRetain(self.0);
        }
        Self(self.0)
    }
}

impl Drop for OpenClDeviceList {
    fn drop(&mut self) {
        unsafe { audionimbus_sys::iplOpenCLDeviceListRelease(&mut self.0) }
    }
}

unsafe impl Send for OpenClDeviceList {}
unsafe impl Sync for OpenClDeviceList {}

/// [`OpenClDeviceList`] errors.
#[derive(Debug, PartialEq)]
pub enum OpenClDeviceListError {
    /// Device index is out of bounds.
    DeviceIndexOutOfBounds {
        device_index: usize,
        num_devices: usize,
    },
}

impl std::error::Error for OpenClDeviceListError {}

impl std::fmt::Display for OpenClDeviceListError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::DeviceIndexOutOfBounds {
                device_index,
                num_devices,
            } => write!(
                f,
                "device index {device_index} out of bounds (num_devices: {num_devices})"
            ),
        }
    }
}

/// Specifies requirements that an OpenCL device must meet in order to be considered when listing OpenCL devices.
#[derive(Debug)]
pub struct OpenClDeviceSettings {
    pub device_type: OpenClDeviceType,

    /// The number of GPU compute units (CUs) that should be reserved for use by Steam Audio.
    ///
    /// If set to a non-zero value, then a GPU will be included in the device list only if it can reserve at least this many CUs.
    /// Set to 0 to indicate that Steam Audio can use the entire GPU, in which case all available GPUs will be considered.
    pub num_compute_units_to_reserve: i32,

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
    pub fraction_of_compute_units_for_impulse_response_update: f32,

    /// If `true`, then the GPU device must support TrueAudio Next.
    ///
    /// It is not necessary to set this to `true` if `num_compute_units_to_reserve` or `fraction_of_compute_units_for_impulse_response_update` are set to non-zero values.
    pub requires_true_audio_next: bool,
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
#[derive(Copy, Clone, Debug, PartialEq)]
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

impl From<audionimbus_sys::IPLOpenCLDeviceType> for OpenClDeviceType {
    fn from(device_type: audionimbus_sys::IPLOpenCLDeviceType) -> Self {
        match device_type {
            audionimbus_sys::IPLOpenCLDeviceType::IPL_OPENCLDEVICETYPE_ANY => OpenClDeviceType::Any,
            audionimbus_sys::IPLOpenCLDeviceType::IPL_OPENCLDEVICETYPE_CPU => OpenClDeviceType::Cpu,
            audionimbus_sys::IPLOpenCLDeviceType::IPL_OPENCLDEVICETYPE_GPU => OpenClDeviceType::Gpu,
        }
    }
}

/// Describes the properties of an OpenCL device.
/// This information can be used to select the most suitable device for your application.
#[derive(Debug, PartialEq)]
pub struct OpenClDeviceDescriptor {
    /// The OpenCL platform id.
    pub platform: *mut std::ffi::c_void,

    /// The OpenCL platform name.
    pub platform_name: String,

    /// The OpenCL platform vendor's name.
    pub platform_vendor: String,

    /// The OpenCL platform version.
    pub platform_version: String,

    /// The OpenCL device id.
    pub device: *mut std::ffi::c_void,

    /// The OpenCL device name.
    pub device_name: String,

    /// The OpenCL device vendor's name.
    pub device_vendor: String,

    /// The OpenCL device version.
    pub device_version: String,

    /// The type of OpenCL device.
    pub device_type: OpenClDeviceType,

    /// The number of CUs reserved for convolution.
    /// May be 0 if CU reservation is not supported.
    pub num_convolution_cus: i32,

    /// The number of CUs reserved for IR update.
    /// May be 0 if CU reservation is not supported.
    pub num_ir_update_cus: i32,

    /// The CU reservation granularity.
    /// CUs can only be reserved on this device in multiples of this number.
    pub granularity: i32,

    /// A relative performance score of a single CU of this device.
    /// Only applicable to supported AMD GPUs.
    pub perf_score: f32,
}

/// Helper function to safely convert a C string pointer to a Rust String
///
/// # Safety
/// The caller must ensure that the pointer is valid and points to a null-terminated C string.
unsafe fn cstr_to_string(ptr: *const std::ffi::c_char) -> Result<String, std::str::Utf8Error> {
    if ptr.is_null() {
        return Ok(String::new());
    }

    let c_str = std::ffi::CStr::from_ptr(ptr);
    c_str.to_str().map(|s| s.to_string())
}

impl TryFrom<&audionimbus_sys::IPLOpenCLDeviceDesc> for OpenClDeviceDescriptor {
    type Error = std::str::Utf8Error;

    fn try_from(
        ipl_descriptor: &audionimbus_sys::IPLOpenCLDeviceDesc,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            platform: ipl_descriptor.platform,
            platform_name: unsafe { cstr_to_string(ipl_descriptor.platformName)? },
            platform_vendor: unsafe { cstr_to_string(ipl_descriptor.platformVendor)? },
            platform_version: unsafe { cstr_to_string(ipl_descriptor.platformVersion)? },
            device: ipl_descriptor.device,
            device_name: unsafe { cstr_to_string(ipl_descriptor.deviceName)? },
            device_vendor: unsafe { cstr_to_string(ipl_descriptor.deviceVendor)? },
            device_version: unsafe { cstr_to_string(ipl_descriptor.deviceVersion)? },
            device_type: ipl_descriptor.type_.into(),
            num_convolution_cus: ipl_descriptor.numConvolutionCUs,
            num_ir_update_cus: ipl_descriptor.numIRUpdateCUs,
            granularity: ipl_descriptor.granularity,
            perf_score: ipl_descriptor.perfScore,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;

    mod open_cl_device {
        use super::*;

        mod device_descriptor {
            use super::*;

            #[test]
            fn test_valid() {
                let context = Context::default();

                let settings = OpenClDeviceSettings::default();
                let Ok(device_list) = OpenClDeviceList::try_new(&context, &settings) else {
                    // OpenCL not available
                    return;
                };

                let num_devices = device_list.num_devices();

                if num_devices > 0 {
                    assert!(device_list.device_descriptor(0).is_ok());
                    assert!(device_list.device_descriptor(num_devices - 1).is_ok());
                }
            }

            #[test]
            fn test_index_out_of_bounds() {
                let context = Context::default();

                let settings = OpenClDeviceSettings::default();
                let Ok(device_list) = OpenClDeviceList::try_new(&context, &settings) else {
                    // OpenCL not available
                    return;
                };

                let num_devices = device_list.num_devices();

                assert_eq!(
                    device_list.device_descriptor(num_devices + 5),
                    Err(OpenClDeviceListError::DeviceIndexOutOfBounds {
                        device_index: num_devices + 5,
                        num_devices,
                    }),
                );
            }
        }
    }
}
