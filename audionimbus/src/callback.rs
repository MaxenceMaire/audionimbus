//! Callback definitions.

use crate::geometry::{Hit, Ray, Vector3};
use std::cell::Cell;
use std::ffi::c_void;
use std::sync::Arc;

#[cfg(doc)]
use crate::simulation::Simulator;

/// Internal macro to generate callback wrapper types.
macro_rules! callback {
    (
        $(#[$meta:meta])*
        $vis:vis $name:ident($($arg:ident: $arg_ty:ty),* $(,)?) $(-> $ret:ty)?
    ) => {
        $(#[$meta])*
        #[derive(Clone)]
        $vis struct $name {
            callback: Arc<dyn Fn($($arg_ty),*) $(-> $ret)? + Send + Sync>,
        }

        impl $name {
            pub fn new<F>(f: F) -> Self
            where
                F: Fn($($arg_ty),*) $(-> $ret)? + Send + Sync + 'static,
            {
                Self {
                    callback: Arc::new(f),
                }
            }

            unsafe extern "C" fn trampoline(
                $($arg: <$arg_ty as $crate::callback::FfiConvert>::FfiType,)*
                user_data: *mut c_void,
            ) $(-> <$ret as $crate::callback::FfiConvert>::FfiType)? {
                // SAFETY: `user_data` was set in `as_raw_parts()`.
                // The pointer is non-null and correctly aligned.
                // The pointee remains valid for the duration of this call.
                let callback = unsafe { &*(user_data as *const Arc<dyn Fn($($arg_ty),*) $(-> $ret)? + Send + Sync>) };

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
#[derive(Clone)]
pub struct DirectivityCallback {
    callback: Arc<dyn Fn(Vector3) -> f32 + Send + Sync>,
}

impl DirectivityCallback {
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(Vector3) -> f32 + Send + Sync + 'static,
    {
        Self {
            callback: Arc::new(f),
        }
    }

    // WORKAROUND: This function ignores the `_user_data` parameter (which Steam Audio
    // corrupts) and instead retrieves the callback pointer from thread-local storage.
    unsafe extern "C" fn trampoline(
        direction: audionimbus_sys::IPLVector3,
        _user_data: *mut c_void,
    ) -> f32 {
        let callback_ptr = DIRECTIVITY_CALLBACK_PTR.get();

        // SAFETY: `callback_ptr` was stored in `as_raw_parts()`.
        // Storing it in thread-local storage ensures exclusive access per thread.
        // Only one directivity calculation can run at a time per thread.
        // The pointee remains valid for the duration of this call.
        let callback =
            unsafe { &*(callback_ptr as *const Arc<dyn Fn(Vector3) -> f32 + Send + Sync>) };

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
#[derive(Clone)]
pub struct CustomRayTracingCallbacks {
    /// Callback for calculating the closest hit along a ray.
    closest_hit_callback: ClosestHitCallback,

    /// Callback for calculating whether a ray hits any geometry.
    any_hit_callback: AnyHitCallback,

    /// Callback for calculating the closest hit along a batch of rays.
    batched_closest_hit_callback: BatchedClosestHitCallback,

    /// Callback for calculating for each ray in a batch of rays, whether the ray hits any geometry.
    batched_any_hit_callback: BatchedAnyHitCallback,
}

impl CustomRayTracingCallbacks {
    /// Creates new custom callbacks.
    ///
    /// # Arguments
    ///
    /// - `closest_hit_callback`: Callback for calculating the closest hit along a ray
    /// - `any_hit_callback`: Callback for calculating whether a ray hits any geometry
    /// - `batched_closest_hit_callback`: Callback for calculating the closest hit along a batch of rays
    /// - `batched_any_hit_callback`: Callback for calculating for each ray in a batch of rays, whether the ray hits any geometry
    pub fn new(
        closest_hit: ClosestHitCallback,
        any_hit: AnyHitCallback,
        batched_closest_hit: BatchedClosestHitCallback,
        batched_any_hit: BatchedAnyHitCallback,
    ) -> Self {
        Self {
            closest_hit_callback: closest_hit,
            any_hit_callback: any_hit,
            batched_closest_hit_callback: batched_closest_hit,
            batched_any_hit_callback: batched_any_hit,
        }
    }

    /// Returns FFI scene settings with custom callbacks and the user data box.
    /// The returned `Arc<CustomRayTracingUserData>` must be kept alive for as long as the scene is
    /// in use.
    pub(crate) fn as_ffi_settings(
        &self,
    ) -> (
        audionimbus_sys::IPLSceneSettings,
        Arc<CustomRayTracingUserData>,
    ) {
        let user_data = Arc::new(CustomRayTracingUserData {
            closest_hit: Arc::clone(&self.closest_hit_callback.callback),
            any_hit: Arc::clone(&self.any_hit_callback.callback),
            batched_closest_hit: Arc::clone(&self.batched_closest_hit_callback.callback),
            batched_any_hit: Arc::clone(&self.batched_any_hit_callback.callback),
        });

        let user_data_ptr = &*user_data as *const _ as *mut c_void;

        let settings = audionimbus_sys::IPLSceneSettings {
            type_: audionimbus_sys::IPLSceneType::IPL_SCENETYPE_CUSTOM,
            closestHitCallback: Some(
                ClosestHitCallback::trampoline as unsafe extern "C" fn(_, _, _, _, _),
            ),
            anyHitCallback: Some(AnyHitCallback::trampoline as unsafe extern "C" fn(_, _, _, _, _)),
            batchedClosestHitCallback: Some(
                BatchedClosestHitCallback::trampoline as unsafe extern "C" fn(_, _, _, _, _, _),
            ),
            batchedAnyHitCallback: Some(
                BatchedAnyHitCallback::trampoline as unsafe extern "C" fn(_, _, _, _, _, _),
            ),
            userData: user_data_ptr,
            embreeDevice: std::ptr::null_mut(),
            radeonRaysDevice: std::ptr::null_mut(),
        };

        (settings, user_data)
    }
}

type ClosestHitFn = dyn Fn(Ray, f32, f32) -> Option<Hit> + Send + Sync;
type AnyHitFn = dyn Fn(Ray, f32, f32) -> bool + Send + Sync;
type BatchedClosestHitFn = dyn Fn(&[Ray], &[f32], &[f32]) -> Vec<Option<Hit>> + Send + Sync;
type BatchedAnyHitFn = dyn Fn(&[Ray], &[f32], &[f32]) -> Vec<bool> + Send + Sync;

/// Internal struct holding pointers to all callback closures.
pub(crate) struct CustomRayTracingUserData {
    closest_hit: Arc<ClosestHitFn>,
    any_hit: Arc<AnyHitFn>,
    batched_closest_hit: Arc<BatchedClosestHitFn>,
    batched_any_hit: Arc<BatchedAnyHitFn>,
}

impl std::fmt::Debug for CustomRayTracingUserData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CustomRayTracingUserData")
            .field("closest_hit", &"<callback>")
            .field("any_hit", &"<callback>")
            .field("batched_closest_hit", &"<callback>")
            .field("batched_any_hit", &"<callback>")
            .finish()
    }
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
#[derive(Clone)]
pub struct ClosestHitCallback {
    callback: Arc<ClosestHitFn>,
}

impl ClosestHitCallback {
    /// Creates a new [`ClosestHitCallback`].
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(Ray, f32, f32) -> Option<Hit> + Send + Sync + 'static,
    {
        Self {
            callback: Arc::new(f),
        }
    }

    unsafe extern "C" fn trampoline(
        ray: *const audionimbus_sys::IPLRay,
        min_distance: f32,
        max_distance: f32,
        hit: *mut audionimbus_sys::IPLHit,
        user_data: *mut c_void,
    ) {
        // SAFETY: `user_data` was set in `CustomRayTracingCallbacks::as_ffi_settings()`.
        // The pointer is non-null and correctly aligned.
        // The caller holds the returned `Arc<CustomRayTracingUserData>` and is responsible for
        // keeping it alive for as long as the scene is in use.
        let all_callbacks = unsafe { &*(user_data as *const CustomRayTracingUserData) };
        let callback = &all_callbacks.closest_hit;
        let ray = if ray.is_null() {
            Ray::default()
        } else {
            // SAFETY: `ray` is non-null and is a valid pointer.
            Ray::from(unsafe { *ray })
        };
        let result = callback(ray, min_distance, max_distance);

        if let Some(hit_result) = result
            && !hit.is_null()
        {
            // SAFETY: `hit` is non-null and points to a valid `IPLHit` output slot.
            unsafe {
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
#[derive(Clone)]
pub struct AnyHitCallback {
    callback: Arc<AnyHitFn>,
}

impl AnyHitCallback {
    /// Creates a new [`AnyHitCallback`].
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(Ray, f32, f32) -> bool + Send + Sync + 'static,
    {
        Self {
            callback: Arc::new(f),
        }
    }

    unsafe extern "C" fn trampoline(
        ray: *const audionimbus_sys::IPLRay,
        min_distance: f32,
        max_distance: f32,
        occluded: *mut u8,
        user_data: *mut c_void,
    ) {
        // SAFETY: `user_data` was set in `CustomRayTracingCallbacks::as_ffi_settings()`.
        // The pointer is non-null and correctly aligned.
        // The caller holds the returned `Arc<CustomRayTracingUserData>` and is responsible for
        // keeping it alive for as long as the scene is in use.
        let all_callbacks = unsafe { &*(user_data as *const CustomRayTracingUserData) };
        let callback = &all_callbacks.any_hit;
        let ray = if ray.is_null() {
            Ray::default()
        } else {
            // SAFETY: `ray` is non-null and is a valid pointer.
            Ray::from(unsafe { *ray })
        };
        let result = callback(ray, min_distance, max_distance);

        if !occluded.is_null() {
            // SAFETY: `occluded` is non-null and points to a valid `u8` output slot.
            unsafe {
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
#[derive(Clone)]
pub struct BatchedClosestHitCallback {
    callback: Arc<BatchedClosestHitFn>,
}

impl BatchedClosestHitCallback {
    /// Creates a new [`BatchedClosestHitCallback`].
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(&[Ray], &[f32], &[f32]) -> Vec<Option<Hit>> + Send + Sync + 'static,
    {
        Self {
            callback: Arc::new(f),
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
        // SAFETY: `user_data` was set in `CustomRayTracingCallbacks::as_ffi_settings()`.
        // The pointer is non-null and correctly aligned.
        // The caller holds the returned `Arc<CustomRayTracingUserData>` and is responsible for
        // keeping it alive for as long as the scene is in use.
        let all_callbacks = unsafe { &*(user_data as *const CustomRayTracingUserData) };
        let callback = &all_callbacks.batched_closest_hit;

        if num_rays <= 0
            || rays.is_null()
            || min_distances.is_null()
            || max_distances.is_null()
            || hits.is_null()
        {
            return;
        }

        let num_rays = num_rays as usize;
        // SAFETY: All three pointers are non-null and `num_rays > 0`.
        // Steam Audio guarantees each points to a contiguous, valid array of exactly `num_rays`
        // elements for the duration of this call.
        let rays_slice = unsafe { std::slice::from_raw_parts(rays, num_rays) }
            .iter()
            .map(|&r| Ray::from(r))
            .collect::<Vec<_>>();
        let min_distances_slice = unsafe { std::slice::from_raw_parts(min_distances, num_rays) };
        let max_distances_slice = unsafe { std::slice::from_raw_parts(max_distances, num_rays) };

        let results = callback(&rays_slice, min_distances_slice, max_distances_slice);

        for (i, result) in results.iter().enumerate().take(num_rays) {
            if let Some(hit_result) = result {
                // SAFETY: `hits` is non-null and points to a contiguous array of `num_rays`
                // `IPLHit` elements.
                // `i < num_rays` (enforced by `.take(num_rays)`), so `hits.add(i)` is within
                // bounds.
                unsafe {
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
#[derive(Clone)]
pub struct BatchedAnyHitCallback {
    callback: Arc<BatchedAnyHitFn>,
}

impl BatchedAnyHitCallback {
    /// Creates a new [`BatchedAnyHitCallback`].
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(&[Ray], &[f32], &[f32]) -> Vec<bool> + Send + Sync + 'static,
    {
        Self {
            callback: Arc::new(f),
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
        // SAFETY: `user_data` was set in `CustomRayTracingCallbacks::as_ffi_settings()`.
        // The pointer is non-null and correctly aligned.
        // The caller holds the returned `Arc<CustomRayTracingUserData>` and is responsible for
        // keeping it alive for as long as the scene is in use.
        let all_callbacks = unsafe { &*(user_data as *const CustomRayTracingUserData) };
        let callback = &all_callbacks.batched_any_hit;

        if num_rays <= 0
            || rays.is_null()
            || min_distances.is_null()
            || max_distances.is_null()
            || occluded.is_null()
        {
            return;
        }

        let num_rays = num_rays as usize;
        // SAFETY: All three pointers are non-null and `num_rays > 0`.
        // Steam Audio guarantees each points to a contiguous, valid array of exactly `num_rays`
        // elements for the duration of this call.
        let rays_slice = unsafe { std::slice::from_raw_parts(rays, num_rays) }
            .iter()
            .map(|&r| Ray::from(r))
            .collect::<Vec<_>>();
        let min_distances_slice = unsafe { std::slice::from_raw_parts(min_distances, num_rays) };
        let max_distances_slice = unsafe { std::slice::from_raw_parts(max_distances, num_rays) };

        let results = callback(&rays_slice, min_distances_slice, max_distances_slice);

        for (i, &result) in results.iter().enumerate().take(num_rays) {
            // SAFETY: `occluded` is non-null and points to a contiguous array of `num_rays` `u8`
            // elements.
            // `i < num_rays` (enforced by `.take(num_rays)`), so `occluded.add(i)` is within
            // bounds.
            unsafe {
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

/// Log level of messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum LogLevel {
    Info,
    Warning,
    Error,
    Debug,
}

impl From<audionimbus_sys::IPLLogLevel> for LogLevel {
    fn from(level: audionimbus_sys::IPLLogLevel) -> Self {
        match level {
            audionimbus_sys::IPLLogLevel::IPL_LOGLEVEL_INFO => LogLevel::Info,
            audionimbus_sys::IPLLogLevel::IPL_LOGLEVEL_WARNING => LogLevel::Warning,
            audionimbus_sys::IPLLogLevel::IPL_LOGLEVEL_ERROR => LogLevel::Error,
            audionimbus_sys::IPLLogLevel::IPL_LOGLEVEL_DEBUG => LogLevel::Debug,
        }
    }
}

/// Creates a log callback.
///
/// This macro generates a C-compatible callback function that converts C types to idiomatic Rust
/// types before invoking the closure.
///
/// # Parameters
///
/// The callback receives:
/// - `level`: [`LogLevel`] - The log level
/// - `message`: [`&str`] - The log message
///
/// # Usage
///
/// ## Named callback
///
/// Creates a named function that can be reused:
///
/// ```
/// # use audionimbus::{log_callback, Context, ContextSettings, LogLevel};
/// log_callback!(my_logger, |level, message| {
///     println!("{level:?}: {message}");
/// });
///
/// let settings = ContextSettings::new().with_log_callback(my_logger);
/// let context = Context::try_new(&settings)?;
/// # Ok::<(), audionimbus::SteamAudioError>(())
/// ```
///
/// ## Anonymous callback
///
/// Creates an inline callback without a name:
///
/// ```
/// # use audionimbus::{log_callback, Context, ContextSettings};
/// let settings = ContextSettings::new().with_log_callback(log_callback!(|level, message| {
///     println!("{level:?}: {message}");
/// }));
/// let context = Context::try_new(&settings)?;
/// # Ok::<(), audionimbus::SteamAudioError>(())
/// ```
#[macro_export]
macro_rules! log_callback {
    ($name:ident, $closure:expr) => {
        $crate::log_callback!(@impl $name, $closure);
    };

    ($closure:expr) => {{
        $crate::log_callback!(@impl __log_callback_impl, $closure);
        __log_callback_impl
    }};

    (@impl $name:ident, $closure:expr) => {
        unsafe extern "C" fn $name(
            level: audionimbus_sys::IPLLogLevel,
            message: *const ::std::os::raw::c_char,
        ) {
            let rust_level = $crate::callback::LogLevel::from(level);
            let rust_message = if !message.is_null() {
                ::std::ffi::CStr::from_ptr(message)
                    .to_str()
                    .unwrap_or("<invalid UTF-8>")
            } else {
                "<null>"
            };

            let closure: fn($crate::callback::LogLevel, &str) = $closure;
            closure(rust_level, rust_message);
        }
    };
}

/// Creates a memory allocation callback.
///
/// This macro generates a C-compatible callback function to use a custom memory allocator.
///
/// # Parameters
///
/// The callback receives:
/// - `size`: [`usize`] - The number of bytes to allocate
/// - `alignment`: [`usize`] - The required alignment in bytes (must be a power of 2)
///
/// # Returns
///
/// [`*mut`] [`c_void`] - Pointer to the allocated memory, or null pointer on failure
///
/// # Usage
///
/// ## Named callback
///
/// ```
/// # use audionimbus::{allocate_callback, free_callback, Context, ContextSettings};
/// # free_callback!(my_free, |ptr| {
/// #   // ...
/// # });
/// use std::alloc::{alloc, Layout};
/// use std::ffi::c_void;
///
/// allocate_callback!(my_allocator, |size, alignment| {
///     unsafe {
///         let layout = Layout::from_size_align_unchecked(size, alignment);
///         alloc(layout) as *mut c_void
///     }
/// });
///
/// let settings = ContextSettings::new()
///     .with_allocate_callback(my_allocator)
///     .with_free_callback(my_free);
/// let context = Context::try_new(&settings)?;
/// # Ok::<(), audionimbus::SteamAudioError>(())
/// ```
///
/// ## Anonymous callback
///
/// ```
/// # use audionimbus::{allocate_callback, free_callback, Context, ContextSettings};
/// # free_callback!(my_free, |ptr| {
/// #   // ...
/// # });
/// use std::alloc::{alloc, Layout};
/// use std::ffi::c_void;
///
/// let settings = ContextSettings::new()
///     .with_allocate_callback(allocate_callback!(|size, alignment| {
///         unsafe {
///             let layout = Layout::from_size_align_unchecked(size, alignment);
///             alloc(layout) as *mut c_void
///         }
///     }))
///     .with_free_callback(my_free);
/// let context = Context::try_new(&settings)?;
/// # Ok::<(), audionimbus::SteamAudioError>(())
/// ```
///
/// # Safety
///
/// The returned pointer must:
/// - Point to valid memory of at least `size` bytes
/// - Be aligned to `alignment` bytes
/// - Remain valid until freed by the corresponding [`free_callback`](crate::free_callback)
#[macro_export]
macro_rules! allocate_callback {
    ($name:ident, $closure:expr) => {
        $crate::allocate_callback!(@impl $name, $closure);
    };

    ($closure:expr) => {{
        $crate::allocate_callback!(@impl __allocate_callback_impl, $closure);
        __allocate_callback_impl
    }};

    (@impl $name:ident, $closure:expr) => {
        unsafe extern "C" fn $name(
            size: usize,
            alignment: usize,
        ) -> *mut ::std::ffi::c_void {
            let closure: fn(usize, usize) -> *mut ::std::ffi::c_void = $closure;
            closure(size, alignment)
        }
    };
}

/// Creates a memory deallocation callback.
///
/// This macro generates a C-compatible callback function used to free memory previously allocated
/// by [`allocate_callback`](crate::allocate_callback).
///
/// # Parameters
///
/// The callback receives:
/// - `ptr`: [`*mut`] [`c_void`] - Pointer to memory previously allocated by the allocate callback
///
/// # Usage
///
/// ## Named callback
///
/// ```
/// # use audionimbus::{free_callback, allocate_callback, Context, ContextSettings};
/// # use std::alloc::{alloc, Layout};
/// # use std::ffi::c_void;
/// # allocate_callback!(my_allocator, |size, alignment| {
/// #     unsafe {
/// #         let layout = Layout::from_size_align_unchecked(size, alignment);
/// #         alloc(layout) as *mut c_void
/// #     }
/// # });
/// free_callback!(my_free, |ptr| {
///     // ...
/// });
///
/// let settings = ContextSettings::new()
///     .with_allocate_callback(my_allocator)
///     .with_free_callback(my_free);
/// let context = Context::try_new(&settings)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// ## Anonymous callback
///
/// ```
/// # use audionimbus::{free_callback, allocate_callback, Context, ContextSettings};
/// # use std::alloc::{alloc, Layout};
/// # use std::ffi::c_void;
/// # allocate_callback!(my_allocator, |size, alignment| {
/// #     unsafe {
/// #         let layout = Layout::from_size_align_unchecked(size, alignment);
/// #         alloc(layout) as *mut c_void
/// #     }
/// # });
/// let settings = ContextSettings::new()
///     .with_allocate_callback(my_allocator)
///     .with_free_callback(free_callback!(|ptr| {
///         // ...
///     }));
/// let context = Context::try_new(&settings)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[macro_export]
macro_rules! free_callback {
    ($name:ident, $closure:expr) => {
        $crate::free_callback!(@impl $name, $closure);
    };

    ($closure:expr) => {{
        $crate::free_callback!(@impl __free_callback_impl, $closure);
        __free_callback_impl
    }};

    (@impl $name:ident, $closure:expr) => {
        unsafe extern "C" fn $name(ptr: *mut ::std::ffi::c_void) {
            let closure: fn(*mut ::std::ffi::c_void) = $closure;
            closure(ptr);
        }
    };
}
