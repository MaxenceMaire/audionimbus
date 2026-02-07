//! Callback definitions.

use crate::geometry::{Hit, Ray, Vector3};
use std::cell::Cell;
use std::ffi::c_void;

#[cfg(doc)]
use crate::simulation::Simulator;

/// Internal macro to generate callback wrapper types.
macro_rules! callback {
    (
        $(#[$meta:meta])*
        $vis:vis $name:ident($($arg:ident: $arg_ty:ty),* $(,)?) $(-> $ret:ty)?
    ) => {
        $(#[$meta])*
        $vis struct $name {
            callback: Box<dyn FnMut($($arg_ty),*) $(-> $ret)? + Send>,
        }

        impl $name {
            pub fn new<F>(f: F) -> Self
            where
                F: FnMut($($arg_ty),*) $(-> $ret)? + Send + 'static,
            {
                Self {
                    callback: Box::new(f),
                }
            }

            unsafe extern "C" fn trampoline(
                $($arg: <$arg_ty as $crate::callback::FfiConvert>::FfiType,)*
                user_data: *mut c_void,
            ) $(-> <$ret as $crate::callback::FfiConvert>::FfiType)? {
                let callback = &mut *(user_data as *mut Box<dyn FnMut($($arg_ty),*) $(-> $ret)? + Send>);

                $(let $arg = <$arg_ty as $crate::callback::FfiConvert>::from_ffi($arg);)*

                #[allow(unused_variables)]
                let result = callback($($arg),*);

                $(return <$ret as $crate::callback::FfiConvert>::to_ffi(result);)?
            }

            #[allow(dead_code)]
            pub(crate) fn as_raw_parts(&self) -> (
                unsafe extern "C" fn($(<$arg_ty as $crate::callback::FfiConvert>::FfiType,)* *mut c_void) $(-> <$ret as $crate::callback::FfiConvert>::FfiType)?,
                *mut c_void,
            ) {
                (
                    Self::trampoline,
                    &self.callback as *const _ as *mut c_void,
                )
            }
        }

        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct(stringify!($name))
                    .field("callback", &"<closure>")
                    .finish()
            }
        }
    };
}

pub(crate) use callback;

/// Trait for types that can be converted to/from FFI representations.
pub(crate) trait FfiConvert {
    type FfiType;

    #[allow(dead_code)]
    fn to_ffi(self) -> Self::FfiType;

    #[allow(dead_code)]
    fn from_ffi(ffi: Self::FfiType) -> Self;
}

/// Marker trait for types that pass through to FFI unchanged.
pub(crate) trait FfiPassthrough: Copy + 'static {}

impl<T: FfiPassthrough> FfiConvert for T {
    type FfiType = T;

    fn to_ffi(self) -> Self::FfiType {
        self
    }

    fn from_ffi(ffi: Self::FfiType) -> Self {
        ffi
    }
}

impl FfiPassthrough for f32 {}
impl FfiPassthrough for usize {}
impl FfiPassthrough for std::ffi::c_int {}

impl FfiConvert for Vector3 {
    type FfiType = audionimbus_sys::IPLVector3;

    fn to_ffi(self) -> Self::FfiType {
        audionimbus_sys::IPLVector3 {
            x: self.x,
            y: self.y,
            z: self.z,
        }
    }

    fn from_ffi(ffi: Self::FfiType) -> Self {
        Vector3 {
            x: ffi.x,
            y: ffi.y,
            z: ffi.z,
        }
    }
}

impl FfiConvert for bool {
    type FfiType = audionimbus_sys::IPLbool;

    fn to_ffi(self) -> Self::FfiType {
        if self {
            audionimbus_sys::IPLbool::IPL_TRUE
        } else {
            audionimbus_sys::IPLbool::IPL_FALSE
        }
    }

    fn from_ffi(ffi: Self::FfiType) -> Self {
        match ffi {
            audionimbus_sys::IPLbool::IPL_TRUE => true,
            audionimbus_sys::IPLbool::IPL_FALSE => false,
        }
    }
}

impl FfiConvert for Ray {
    type FfiType = audionimbus_sys::IPLRay;

    fn to_ffi(self) -> Self::FfiType {
        self.into()
    }

    fn from_ffi(ffi: Self::FfiType) -> Self {
        ffi.into()
    }
}

callback! {
    /// A progress callback for long-running operations.
    ///
    /// # Callback arguments
    ///
    /// - `progress`: Fraction of the function work that has been completed, between 0.0 and 1.0.
    pub ProgressCallback(progress: f32)
}

callback! {
    /// Callback for visualizing valid path segments during the call to [`Simulator::run_pathing`].
    ///
    /// You can use this to provide the user with visual feedback, like drawing each segment of a path.
    ///
    /// # Callback arguments
    ///
    /// - `from`: position of the starting probe.
    /// - `to`: position of the ending probe.
    /// - `occluded`: occlusion status of the ray segment between `from` to `to`.
    pub PathingVisualizationCallback(
        from: Vector3,
        to: Vector3,
        occluded: bool,
    )
}

callback! {
    /// Callback for calculating how much to attenuate sound in a given frequency band based on the angle of deviation when the sound path bends around a corner as it propagated from the source to the listener.
    ///
    /// # Callback arguments
    ///
    /// - `angle`: angle (in radians) that the sound path deviates (bends) by.
    /// - `band`: index of the frequency band for which to calculate air absorption.
    ///
    /// # Returns
    ///
    /// The frequency-dependent attenuation to apply, between 0.0 and 1.0.
    /// 0.0 = sound in the frequency band is not audible; 1.0 = sound in the frequency band is not attenuated.
    pub DeviationCallback(
        angle: f32,
        band: std::ffi::c_int,
    ) -> f32
}

callback! {
    /// Callback for calculating how much attenuation should be applied to a sound based on its distance from the listener.
    ///
    /// # Callback arguments
    ///
    /// - `distance`: the distance (in meters) between the source and the listener.
    ///
    /// # Returns
    ///
    /// The distance attenuation to apply, between 0.0 and 1.0.
    /// 0.0 = the sound is not audible, 1.0 = the sound is as loud as it would be if it were emitted from the listener’s position.
    pub DistanceAttenuationCallback(distance: f32) -> f32
}

// BUG: Steam Audio has a bug where the `userData` pointer passed to directivity callbacks
// is corrupted and does not match the originally provided pointer. This causes segfaults
// when attempting to dereference the pointer to access our closure.
//
// See: https://github.com/ValveSoftware/steam-audio/issues/526
//
// This absolutely unholy hack consists in storing the callback pointer in a static cell local to
// the thread instead of relying on Steam Audio to pass it back to us. It is safe because only one
// directivity calculation can execute at a time per thread.
//
// TODO: the this workaround and use `callback!` macro once the Steam Audio bug is fixed.
thread_local! {
    static DIRECTIVITY_CALLBACK_PTR: Cell<*mut c_void> = const { Cell::new(std::ptr::null_mut()) };
}

/// Callback for calculating how much to attenuate a sound based on its directivity pattern and orientation in world space.
///
/// # Callback arguments
///
/// - `direction`: unit vector (in world space) pointing forwards from the source. This is the direction that the source is “pointing towards”.
///
/// # Returns
///
/// The directivity value to apply, between 0.0 and 1.0.
/// 0.0 = the sound is not audible, 1.0 = the sound is as loud as it would be if it had a uniform (omnidirectional) directivity pattern.
pub struct DirectivityCallback {
    callback: Box<dyn FnMut(Vector3) -> f32 + Send>,
}

impl DirectivityCallback {
    pub fn new<F>(f: F) -> Self
    where
        F: FnMut(Vector3) -> f32 + Send + 'static,
    {
        Self {
            callback: Box::new(f),
        }
    }

    // WORKAROUND: This function ignores the `_user_data` parameter (which Steam Audio
    // corrupts) and instead retrieves the callback pointer from thread-local storage.
    unsafe extern "C" fn trampoline(
        direction: audionimbus_sys::IPLVector3,
        _user_data: *mut c_void,
    ) -> f32 {
        let callback_ptr = DIRECTIVITY_CALLBACK_PTR.get();
        let callback = &mut *(callback_ptr as *mut Box<dyn FnMut(Vector3) -> f32 + Send>);
        let direction = Vector3::from_ffi(direction);
        callback(direction)
    }

    // WORKAROUND: This stores the callback pointer in thread-local storage rather than
    // passing it through Steam Audio's userData parameter (which is corrupted by a bug).
    pub(crate) fn as_raw_parts(
        &self,
    ) -> (
        unsafe extern "C" fn(audionimbus_sys::IPLVector3, *mut c_void) -> f32,
        *mut c_void,
    ) {
        let callback_ptr = &self.callback as *const _ as *mut c_void;
        DIRECTIVITY_CALLBACK_PTR.set(callback_ptr);
        (Self::trampoline, callback_ptr)
    }
}

impl std::fmt::Debug for DirectivityCallback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DirectivityCallback")
            .field("callback", &"<closure>")
            .finish()
    }
}

callback! {
    /// Callback for calculating how much air absorption should be applied to a sound based on its distance from the listener.
    ///
    /// # Callback arguments
    ///
    /// - `distance`: the distance (in meters) between the source and the listener.
    /// - `band`: index of the frequency band for which to calculate air absorption. 0.0 = low frequencies, 1.0 = middle frequencies, 2.0 = high frequencies.
    ///
    /// # Returns
    ///
    /// The air absorption to apply, between 0.0 and 1.0.
    /// 0.0 = sound in the frequency band `band` is not audible, 1.0 = sound in the frequency band `band` is not attenuated.
    pub AirAbsorptionCallback(distance: f32, band: i32) -> f32
}

/// Callbacks used for a custom ray tracer.
pub struct CustomRayTracingCallbacks {
    closest_hit_callback: Option<ClosestHitCallback>,
    any_hit_callback: Option<AnyHitCallback>,
    batched_closest_hit_callback: Option<BatchedClosestHitCallback>,
    batched_any_hit_callback: Option<BatchedAnyHitCallback>,
}

impl CustomRayTracingCallbacks {
    /// Creates new custom callbacks.
    pub fn new(
        closest_hit: Option<ClosestHitCallback>,
        any_hit: Option<AnyHitCallback>,
        batched_closest_hit: Option<BatchedClosestHitCallback>,
        batched_any_hit: Option<BatchedAnyHitCallback>,
    ) -> Self {
        Self {
            closest_hit_callback: closest_hit,
            any_hit_callback: any_hit,
            batched_closest_hit_callback: batched_closest_hit,
            batched_any_hit_callback: batched_any_hit,
        }
    }

    /// Returns FFI scene settings with custom callbacks and the user data box.
    /// The caller must ensure the returned Box<CustomRayTracingUserData> lives as long as the settings are in use.
    pub(crate) fn as_ffi_settings(
        &self,
    ) -> (
        audionimbus_sys::IPLSceneSettings,
        Box<CustomRayTracingUserData>,
    ) {
        let user_data = Box::new(CustomRayTracingUserData {
            closest_hit: self
                .closest_hit_callback
                .as_ref()
                .map(|cb| &cb.callback as *const _ as *mut _),
            any_hit: self
                .any_hit_callback
                .as_ref()
                .map(|cb| &cb.callback as *const _ as *mut _),
            batched_closest_hit: self
                .batched_closest_hit_callback
                .as_ref()
                .map(|cb| &cb.callback as *const _ as *mut _),
            batched_any_hit: self
                .batched_any_hit_callback
                .as_ref()
                .map(|cb| &cb.callback as *const _ as *mut _),
        });

        let user_data_ptr = &*user_data as *const _ as *mut c_void;

        let settings = audionimbus_sys::IPLSceneSettings {
            type_: audionimbus_sys::IPLSceneType::IPL_SCENETYPE_CUSTOM,
            closestHitCallback: self
                .closest_hit_callback
                .as_ref()
                .map(|_| ClosestHitCallback::trampoline as unsafe extern "C" fn(_, _, _, _, _)),
            anyHitCallback: self
                .any_hit_callback
                .as_ref()
                .map(|_| AnyHitCallback::trampoline as unsafe extern "C" fn(_, _, _, _, _)),
            batchedClosestHitCallback: self.batched_closest_hit_callback.as_ref().map(|_| {
                BatchedClosestHitCallback::trampoline as unsafe extern "C" fn(_, _, _, _, _, _)
            }),
            batchedAnyHitCallback: self.batched_any_hit_callback.as_ref().map(|_| {
                BatchedAnyHitCallback::trampoline as unsafe extern "C" fn(_, _, _, _, _, _)
            }),
            userData: user_data_ptr,
            embreeDevice: std::ptr::null_mut(),
            radeonRaysDevice: std::ptr::null_mut(),
        };

        (settings, user_data)
    }
}

type ClosestHitFn = dyn FnMut(Ray, f32, f32) -> Option<Hit> + Send;
type AnyHitFn = dyn FnMut(Ray, f32, f32) -> bool + Send;
type BatchedClosestHitFn = dyn FnMut(&[Ray], &[f32], &[f32]) -> Vec<Option<Hit>> + Send;
type BatchedAnyHitFn = dyn FnMut(&[Ray], &[f32], &[f32]) -> Vec<bool> + Send;

/// Internal struct holding pointers to all callback closures.
#[derive(Debug)]
pub(crate) struct CustomRayTracingUserData {
    closest_hit: Option<*mut Box<ClosestHitFn>>,
    any_hit: Option<*mut Box<AnyHitFn>>,
    batched_closest_hit: Option<*mut Box<BatchedClosestHitFn>>,
    batched_any_hit: Option<*mut Box<BatchedAnyHitFn>>,
}

/// Callback for calculating the closest hit along a ray.
///
/// Strictly speaking, the intersection is calculated with a ray interval (equivalent to a line segment).
/// Any ray interval may have multiple points of intersection with scene geometry; this function must return information about the point of intersection that is closest to the ray’s origin.
///
/// # Callback arguments
///
/// - `ray`: The ray to trace.
/// - `min_mistance`: The minimum distance from the origin at which an intersection may occur for it to be considered.
///   This function must not return any intersections closer to the origin than this value.
/// - `max_distance`: The maximum distance from the origin at which an intersection may occur for it to be considered.
///   This function must not return any intersections farther from the origin than this value.
///   If this value is less than or equal to `min_distance`, the function should ignore the ray, and return immediately.
///
/// # Returns
///
/// Information describing the ray’s intersection with geometry, if any.
pub struct ClosestHitCallback {
    callback: Box<ClosestHitFn>,
}

impl ClosestHitCallback {
    /// Creates a new [`ClosestHitCallback`].
    pub fn new<F>(f: F) -> Self
    where
        F: FnMut(Ray, f32, f32) -> Option<Hit> + Send + 'static,
    {
        Self {
            callback: Box::new(f),
        }
    }

    unsafe extern "C" fn trampoline(
        ray: *const audionimbus_sys::IPLRay,
        min_distance: f32,
        max_distance: f32,
        hit: *mut audionimbus_sys::IPLHit,
        user_data: *mut c_void,
    ) {
        let all_callbacks = &*(user_data as *const CustomRayTracingUserData);
        if let Some(callback_ptr) = all_callbacks.closest_hit {
            let callback = &mut *callback_ptr;
            let ray = if ray.is_null() {
                Ray::default()
            } else {
                Ray::from(*ray)
            };
            let result = callback(ray, min_distance, max_distance);

            if let Some(hit_result) = result {
                if !hit.is_null() {
                    *hit = audionimbus_sys::IPLHit {
                        distance: hit_result.distance,
                        triangleIndex: hit_result.triangle_index.map(|i| i as i32).unwrap_or(-1),
                        objectIndex: hit_result.object_index.map(|i| i as i32).unwrap_or(-1),
                        materialIndex: hit_result.material_index.map(|i| i as i32).unwrap_or(-1),
                        normal: hit_result.normal.into(),
                        material: std::ptr::null_mut(),
                    };
                }
            }
        }
    }
}

impl std::fmt::Debug for ClosestHitCallback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClosestHitCallback")
            .field("callback", &"<closure>")
            .finish()
    }
}

/// Callback for calculating whether a ray hits any geometry.
///
/// Strictly speaking, the intersection is calculated with a ray interval (equivalent to a line segment).
///
/// # Callback arguments
///
/// - `ray`: The ray to trace.
/// - `min_distance`: The minimum distance from the origin at which an intersection may occur for it to be considered.
///   This function must not return any intersections closer to the origin than this value.
/// - `max_distance`: The maximum distance from the origin at which an intersection may occur for it to be considered.
///   This function must not return any intersections farther from the origin than this value.
///   If this value is less than or equal to `min_distance`, the function should ignore the ray and return immediately with `true`.
///
/// # Returns
///
/// A boolean indicating whether the ray intersects any geometry.
///
/// `false` indicates no intersection, `true` indicates that an intersection exists.
pub struct AnyHitCallback {
    callback: Box<AnyHitFn>,
}

impl AnyHitCallback {
    /// Creates a new [`AnyHitCallback`].
    pub fn new<F>(f: F) -> Self
    where
        F: FnMut(Ray, f32, f32) -> bool + Send + 'static,
    {
        Self {
            callback: Box::new(f),
        }
    }

    unsafe extern "C" fn trampoline(
        ray: *const audionimbus_sys::IPLRay,
        min_distance: f32,
        max_distance: f32,
        occluded: *mut u8,
        user_data: *mut c_void,
    ) {
        let all_callbacks = &*(user_data as *const CustomRayTracingUserData);
        if let Some(callback_ptr) = all_callbacks.any_hit {
            let callback = &mut *callback_ptr;
            let ray = if ray.is_null() {
                Ray::default()
            } else {
                Ray::from(*ray)
            };
            let result = callback(ray, min_distance, max_distance);

            if !occluded.is_null() {
                *occluded = if result { 1 } else { 0 };
            }
        }
    }
}

impl std::fmt::Debug for AnyHitCallback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnyHitCallback")
            .field("callback", &"<closure>")
            .finish()
    }
}

/// Callback for calculating the closest hit along a batch of rays.
///
/// Strictly speaking, the intersection is calculated with a ray interval (equivalent to a line segment).
/// Any ray interval may have multiple points of intersection with scene geometry; this function must return information about the point of intersection that is closest to the ray’s origin.
///
/// # Callback arguments
///
/// - `numRays`: The number of rays to trace.
/// - `rays`: Array containing the rays.
/// - `min_distances`: Array containing, for each ray, the minimum distance from the origin at which an intersection may occur for it to be considered.
/// - `max_distances`: Array containing, for each ray, the maximum distance from the origin at which an intersection may occur for it to be considered.
///   If, for some ray with index `i`, `max_distances[i]` is less than `min_distances[i]`, the function should ignore the ray.
///
/// # Returns
///
/// Information describing each ray’s intersection with geometry, if any.
pub struct BatchedClosestHitCallback {
    callback: Box<BatchedClosestHitFn>,
}

impl BatchedClosestHitCallback {
    /// Creates a new [`BatchedClosestHitCallback`].
    pub fn new<F>(f: F) -> Self
    where
        F: FnMut(&[Ray], &[f32], &[f32]) -> Vec<Option<Hit>> + Send + 'static,
    {
        Self {
            callback: Box::new(f),
        }
    }

    unsafe extern "C" fn trampoline(
        num_rays: i32,
        rays: *const audionimbus_sys::IPLRay,
        min_distances: *const f32,
        max_distances: *const f32,
        hits: *mut audionimbus_sys::IPLHit,
        user_data: *mut c_void,
    ) {
        let all_callbacks = &*(user_data as *const CustomRayTracingUserData);
        if let Some(callback_ptr) = all_callbacks.batched_closest_hit {
            let callback = &mut *callback_ptr;

            if num_rays <= 0
                || rays.is_null()
                || min_distances.is_null()
                || max_distances.is_null()
                || hits.is_null()
            {
                return;
            }

            let num_rays = num_rays as usize;
            let rays_slice = std::slice::from_raw_parts(rays, num_rays)
                .iter()
                .map(|&r| Ray::from(r))
                .collect::<Vec<_>>();
            let min_distances_slice = std::slice::from_raw_parts(min_distances, num_rays);
            let max_distances_slice = std::slice::from_raw_parts(max_distances, num_rays);

            let results = callback(&rays_slice, min_distances_slice, max_distances_slice);

            for (i, result) in results.iter().enumerate().take(num_rays) {
                if let Some(hit_result) = result {
                    *hits.add(i) = audionimbus_sys::IPLHit {
                        distance: hit_result.distance,
                        triangleIndex: hit_result
                            .triangle_index
                            .map(|idx| idx as i32)
                            .unwrap_or(-1),
                        objectIndex: hit_result.object_index.map(|idx| idx as i32).unwrap_or(-1),
                        materialIndex: hit_result
                            .material_index
                            .map(|idx| idx as i32)
                            .unwrap_or(-1),
                        normal: hit_result.normal.into(),
                        material: std::ptr::null_mut(),
                    };
                }
            }
        }
    }
}

impl std::fmt::Debug for BatchedClosestHitCallback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BatchedClosestHitCallback")
            .field("callback", &"<closure>")
            .finish()
    }
}

/// Callback for calculating for each ray in a batch of rays, whether the ray hits any geometry.
///
/// Strictly speaking, the intersection is calculated with a ray interval (equivalent to a line segment).
///
/// # Callback arguments
///
/// - `num_rays`: The number of rays to trace.
/// - `rays`: Array containing the rays.
/// - `min_distances`: Array containing, for each ray, the minimum distance from the origin at which an intersection may occur for it to be considered.
/// - `max_distances`: Array containing, for each ray, the maximum distance from the origin at which an intersection may occur for it to be considered.
///   If, for some ray with index `i`, `max_distances[i]` is less than `min_distances[i]`, the function should ignore the ray and set the result to `true`.
///
/// # Returns
///
/// Array of integers indicating, for each ray, whether the ray intersects any geometry.
/// `false` indicates no intersection, `true` indicates that an intersection exists.
pub struct BatchedAnyHitCallback {
    callback: Box<BatchedAnyHitFn>,
}

impl BatchedAnyHitCallback {
    /// Creates a new [`BatchedAnyHitCallback`].
    pub fn new<F>(f: F) -> Self
    where
        F: FnMut(&[Ray], &[f32], &[f32]) -> Vec<bool> + Send + 'static,
    {
        Self {
            callback: Box::new(f),
        }
    }

    unsafe extern "C" fn trampoline(
        num_rays: i32,
        rays: *const audionimbus_sys::IPLRay,
        min_distances: *const f32,
        max_distances: *const f32,
        occluded: *mut u8,
        user_data: *mut c_void,
    ) {
        let all_callbacks = &*(user_data as *const CustomRayTracingUserData);
        if let Some(callback_ptr) = all_callbacks.batched_any_hit {
            let callback = &mut *callback_ptr;

            if num_rays <= 0
                || rays.is_null()
                || min_distances.is_null()
                || max_distances.is_null()
                || occluded.is_null()
            {
                return;
            }

            let num_rays = num_rays as usize;
            let rays_slice = std::slice::from_raw_parts(rays, num_rays)
                .iter()
                .map(|&r| Ray::from(r))
                .collect::<Vec<_>>();
            let min_distances_slice = std::slice::from_raw_parts(min_distances, num_rays);
            let max_distances_slice = std::slice::from_raw_parts(max_distances, num_rays);

            let results = callback(&rays_slice, min_distances_slice, max_distances_slice);

            for (i, &result) in results.iter().enumerate().take(num_rays) {
                *occluded.add(i) = if result { 1 } else { 0 };
            }
        }
    }
}

impl std::fmt::Debug for BatchedAnyHitCallback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BatchedAnyHitCallback")
            .field("callback", &"<closure>")
            .finish()
    }
}
