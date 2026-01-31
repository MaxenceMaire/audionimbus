//! Ray tracing implementations.

use crate::Sealed;

/// Steam Audio’s built-in ray tracer.
///
/// Supports multi-threading. Runs on all platforms that Steam Audio supports.
#[derive(Debug)]
pub struct DefaultRayTracer;

/// The Intel Embree ray tracer.
///
/// Supports multi-threading.
/// This is a highly optimized implementation, and is likely to be faster than the default ray tracer.
/// However, Embree requires Windows, Linux, or macOS, and a 32-bit x86 or 64-bit x86_64 CPU.
#[derive(Debug)]
pub struct Embree;

/// The AMD Radeon Rays ray tracer.
///
/// This is an OpenCL implementation, and can use either the CPU or any GPU that supports OpenCL 1.2 or later.
/// If using the GPU, it is likely to be significantly faster than the default ray tracer.
/// However, with heavy real-time simulation workloads, it may impact your application’s frame rate.
/// On supported AMD GPUs, you can use the Resource Reservation feature to mitigate this issue.
#[derive(Debug)]
pub struct RadeonRays;

/// Allows you to specify callbacks to your own ray tracer.
///
/// Useful if your application already uses a high-performance ray tracer.
/// This option uses the least amount of memory at run-time, since it does not have to build any ray tracing data structures of its own.
#[derive(Debug)]
pub struct CustomRayTracer;

impl Sealed for DefaultRayTracer {}
impl Sealed for Embree {}
impl Sealed for RadeonRays {}
impl Sealed for CustomRayTracer {}

/// Ray tracer implementation. Can be:
/// - [`DefaultRayTracer`]: Steam Audio’s built-in ray tracer
/// - [`Embree`]: The Intel Embree ray tracer
/// - [`RadeonRays`]: The AMD Radeon Rays ray tracer
/// - [`CustomRayTracer`]: Allows you to specify callbacks to your own ray tracer
pub trait RayTracer: Sealed {
    /// Returns the FFI scene type for this ray tracer implementation.
    fn scene_type() -> audionimbus_sys::IPLSceneType;
}

impl RayTracer for DefaultRayTracer {
    fn scene_type() -> audionimbus_sys::IPLSceneType {
        audionimbus_sys::IPLSceneType::IPL_SCENETYPE_DEFAULT
    }
}

impl RayTracer for Embree {
    fn scene_type() -> audionimbus_sys::IPLSceneType {
        audionimbus_sys::IPLSceneType::IPL_SCENETYPE_EMBREE
    }
}

impl RayTracer for RadeonRays {
    fn scene_type() -> audionimbus_sys::IPLSceneType {
        audionimbus_sys::IPLSceneType::IPL_SCENETYPE_RADEONRAYS
    }
}

impl RayTracer for CustomRayTracer {
    fn scene_type() -> audionimbus_sys::IPLSceneType {
        audionimbus_sys::IPLSceneType::IPL_SCENETYPE_CUSTOM
    }
}
