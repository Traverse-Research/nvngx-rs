//! API-agnostic types shared between the Vulkan and DX12 backends.
//!
//! Types that interact only with the common `NVSDK_NGX_Parameter_*` functions
//! (not the `VULKAN_` or `D3D12_` variants) live here. API-specific cleanup
//! (destroy / release) is dispatched through stored function pointers.

use std::ffi::CStr;
use std::path::Path;
use std::rc::Rc;

use nvngx_sys::{
    NVSDK_NGX_DLSSD_Create_Params, NVSDK_NGX_DLSS_Create_Params, NVSDK_NGX_DLSS_Denoise_Mode,
    NVSDK_NGX_DLSS_Depth_Type, NVSDK_NGX_DLSS_Feature_Flags, NVSDK_NGX_DLSS_Roughness_Mode,
    NVSDK_NGX_Feature, NVSDK_NGX_Logging_Level, NVSDK_NGX_PerfQuality_Value, Result,
};

// ---------------------------------------------------------------------------
// Logging
// ---------------------------------------------------------------------------

/// Controls how verbose NGX's own logging is.
#[derive(Debug, Clone, Copy)]
pub enum LoggingLevel {
    /// Disable NGX logging.
    Off,
    /// Standard information and error logging.
    On,
    /// Verbose logging (diagnostic details).
    Verbose,
}

impl From<LoggingLevel> for NVSDK_NGX_Logging_Level {
    fn from(l: LoggingLevel) -> Self {
        match l {
            LoggingLevel::Off => Self::NVSDK_NGX_LOGGING_LEVEL_OFF,
            LoggingLevel::On => Self::NVSDK_NGX_LOGGING_LEVEL_ON,
            LoggingLevel::Verbose => Self::NVSDK_NGX_LOGGING_LEVEL_VERBOSE,
        }
    }
}

/// Opt-in routing of NGX log lines through the [`log`] crate.
///
/// When supplied via [`FeatureCommonInfo::logging`], every NGX log message is
/// passed to [`log::log!`] at a level derived from the NGX level
/// ([`LoggingLevel::Verbose`] → [`log::Level::Debug`], [`LoggingLevel::On`] →
/// [`log::Level::Info`]). This avoids having to enable NGX's registry-based
/// log sinks on Windows.
#[derive(Debug, Clone, Copy)]
pub struct LoggingConfig {
    /// Minimum NGX severity to forward. NGX honours the *higher* of this value
    /// and whatever is configured via the registry.
    pub minimum_level: LoggingLevel,
    /// When [`true`], NGX won't write to its other log sinks (files, on-screen
    /// console). Log lines are delivered *only* through the [`log`] crate.
    pub disable_other_sinks: bool,
}

/// Common information provided to NGX at initialization time.
///
/// This mirrors [`nvngx_sys::NVSDK_NGX_FeatureCommonInfo`] from the NGX SDK, exposing:
/// - an optional list of additional search paths for feature DLLs
///   (e.g. `nvngx_dlss.dll`), searched in descending order of preference in
///   addition to the application folder;
/// - an optional [`LoggingConfig`] that pipes NGX logs through the [`log`] crate.
#[derive(Debug, Clone, Copy, Default)]
pub struct FeatureCommonInfo<'a> {
    /// Extra directories to search for feature DLLs, in descending order of
    /// preference. The application folder is always searched as well.
    pub search_paths: &'a [&'a Path],
    /// If [`Some`], NGX logs are routed through the [`log`] crate.
    pub logging: Option<LoggingConfig>,
}

/// Static `extern "C"` callback that pipes NGX log lines into the [`log`]
/// crate. NGX calls this from arbitrary threads; [`log`]'s macros are
/// thread-safe so no additional synchronization is needed.
pub(crate) unsafe extern "C" fn log_crate_callback(
    message: *const std::os::raw::c_char,
    level: NVSDK_NGX_Logging_Level,
    source: NVSDK_NGX_Feature,
) {
    let msg = if message.is_null() {
        std::borrow::Cow::Borrowed("<null>")
    } else {
        unsafe { CStr::from_ptr(message) }.to_string_lossy()
    };
    let log_level = match level {
        NVSDK_NGX_Logging_Level::NVSDK_NGX_LOGGING_LEVEL_VERBOSE => log::Level::Debug,
        NVSDK_NGX_Logging_Level::NVSDK_NGX_LOGGING_LEVEL_ON => log::Level::Info,
        other => {
            log::warn!(target: "nvngx", "[{source:?}] Unrecognized NGX logging level {other:?}: {}", msg.trim_end());
            return;
        }
    };
    log::log!(target: "nvngx", log_level, "[{source:?}] {}", msg.trim_end());
}

// ---------------------------------------------------------------------------
// Feature progress callback
// ---------------------------------------------------------------------------

pub(crate) unsafe extern "C" fn feature_progress_callback(
    progress: f32,
    _should_cancel: *mut bool,
) {
    log::debug!("Feature evalution progress={progress}.");
}

// ---------------------------------------------------------------------------
// FeatureHandle
// ---------------------------------------------------------------------------

/// Signature of the API-specific release function
/// (`NVSDK_NGX_VULKAN_ReleaseFeature` / `NVSDK_NGX_D3D12_ReleaseFeature`).
pub(crate) type ReleaseFn =
    unsafe extern "C" fn(*mut nvngx_sys::NVSDK_NGX_Handle) -> nvngx_sys::NVSDK_NGX_Result;

/// An NGX handle. Created by the API-specific `System` and released on drop
/// via the matching API-specific release function.
#[derive(Debug)]
pub struct FeatureHandle {
    pub(crate) ptr: *mut nvngx_sys::NVSDK_NGX_Handle,
    release_fn: ReleaseFn,
}

impl Default for FeatureHandle {
    fn default() -> Self {
        Self {
            ptr: std::ptr::null_mut(),
            release_fn: stub_release,
        }
    }
}

/// No-op release used only by `Default` (null handle).
unsafe extern "C" fn stub_release(
    _: *mut nvngx_sys::NVSDK_NGX_Handle,
) -> nvngx_sys::NVSDK_NGX_Result {
    nvngx_sys::NVSDK_NGX_Result::NVSDK_NGX_Result_Success
}

impl FeatureHandle {
    pub(crate) fn new(release_fn: ReleaseFn) -> Self {
        Self {
            ptr: std::ptr::null_mut(),
            release_fn,
        }
    }

    fn release(&mut self) -> Result {
        unsafe { (self.release_fn)(self.ptr) }.into()
    }
}

impl Drop for FeatureHandle {
    fn drop(&mut self) {
        if self.ptr.is_null() {
            return;
        }

        if let Err(e) = self.release() {
            log::error!("Couldn't release the feature handle: {:?}: {e}", self)
        }
    }
}

// ---------------------------------------------------------------------------
// FeatureParameterName
// ---------------------------------------------------------------------------

/// A type alias for feature parameter, like
/// [`nvngx_sys::NVSDK_NGX_Parameter_NumFrames`].
pub type FeatureParameterName = [u8];

// ---------------------------------------------------------------------------
// FeatureParameters
// ---------------------------------------------------------------------------

/// Signature of the API-specific destroy function
/// (`NVSDK_NGX_VULKAN_DestroyParameters` / `NVSDK_NGX_D3D12_DestroyParameters`).
pub(crate) type DestroyParametersFn =
    unsafe extern "C" fn(*mut nvngx_sys::NVSDK_NGX_Parameter) -> nvngx_sys::NVSDK_NGX_Result;

/// Feature parameters is a collection of parameters of a feature.
///
/// All getters, setters, and capability queries use API-agnostic
/// `NVSDK_NGX_Parameter_*` functions. Only construction and destruction are
/// API-specific and dispatched via the stored function pointer.
pub struct FeatureParameters {
    pub(crate) ptr: *mut nvngx_sys::NVSDK_NGX_Parameter,
    destroy_fn: DestroyParametersFn,
}

impl std::fmt::Debug for FeatureParameters {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        macro_rules! debug_field {
            ($s:ident, $key:path, bool) => {
                if let Ok(v) = self.get_bool($key) {
                    $s.field(stringify!($key), &v);
                }
            };
            ($s:ident, $key:path, i32) => {
                if let Ok(v) = self.get_i32($key) {
                    $s.field(stringify!($key), &v);
                }
            };
            ($s:ident, $key:path, u32) => {
                if let Ok(v) = self.get_u32($key) {
                    $s.field(stringify!($key), &v);
                }
            };
            ($s:ident, $key:path, u64) => {
                if let Ok(v) = self.get_u64($key) {
                    $s.field(stringify!($key), &v);
                }
            };
            ($s:ident, $key:path, f32) => {
                if let Ok(v) = self.get_f32($key) {
                    $s.field(stringify!($key), &v);
                }
            };
            ($s:ident, $key:path, f64) => {
                if let Ok(v) = self.get_f64($key) {
                    $s.field(stringify!($key), &v);
                }
            };
        }

        let mut s = f.debug_struct("FeatureParameters");
        s.field("ptr", &self.ptr);
        debug_field!(
            s,
            nvngx_sys::NVSDK_NGX_Parameter_SuperSampling_Available,
            bool
        );
        debug_field!(
            s,
            nvngx_sys::NVSDK_NGX_Parameter_SuperSamplingDenoising_Available,
            bool
        );
        debug_field!(s, nvngx_sys::NVSDK_NGX_Parameter_InPainting_Available, bool);
        debug_field!(
            s,
            nvngx_sys::NVSDK_NGX_Parameter_ImageSuperResolution_Available,
            bool
        );
        debug_field!(s, nvngx_sys::NVSDK_NGX_Parameter_SlowMotion_Available, bool);
        debug_field!(
            s,
            nvngx_sys::NVSDK_NGX_Parameter_VideoSuperResolution_Available,
            bool
        );
        debug_field!(
            s,
            nvngx_sys::NVSDK_NGX_Parameter_ImageSignalProcessing_Available,
            bool
        );
        debug_field!(
            s,
            nvngx_sys::NVSDK_NGX_Parameter_DeepResolve_Available,
            bool
        );
        debug_field!(
            s,
            nvngx_sys::NVSDK_NGX_Parameter_SuperSampling_NeedsUpdatedDriver,
            bool
        );
        debug_field!(
            s,
            nvngx_sys::NVSDK_NGX_Parameter_InPainting_NeedsUpdatedDriver,
            bool
        );
        debug_field!(
            s,
            nvngx_sys::NVSDK_NGX_Parameter_ImageSuperResolution_NeedsUpdatedDriver,
            bool
        );
        debug_field!(
            s,
            nvngx_sys::NVSDK_NGX_Parameter_SlowMotion_NeedsUpdatedDriver,
            bool
        );
        debug_field!(
            s,
            nvngx_sys::NVSDK_NGX_Parameter_VideoSuperResolution_NeedsUpdatedDriver,
            bool
        );
        debug_field!(
            s,
            nvngx_sys::NVSDK_NGX_Parameter_ImageSignalProcessing_NeedsUpdatedDriver,
            bool
        );
        debug_field!(
            s,
            nvngx_sys::NVSDK_NGX_Parameter_DeepResolve_NeedsUpdatedDriver,
            bool
        );
        debug_field!(
            s,
            nvngx_sys::NVSDK_NGX_Parameter_FrameInterpolation_NeedsUpdatedDriver,
            bool
        );
        debug_field!(s, nvngx_sys::NVSDK_NGX_Parameter_NumFrames, u32);
        debug_field!(s, nvngx_sys::NVSDK_NGX_Parameter_Scale, u32);
        debug_field!(s, nvngx_sys::NVSDK_NGX_Parameter_OptLevel, u32);
        debug_field!(s, nvngx_sys::NVSDK_NGX_Parameter_IsDevSnippetBranch, bool);
        debug_field!(
            s,
            nvngx_sys::NVSDK_NGX_Parameter_SuperSampling_ScaleFactor,
            f32
        );
        s.finish()
    }
}

impl FeatureParameters {
    /// Wraps a raw parameter pointer with the appropriate API-specific destroy function.
    ///
    /// # Safety
    ///
    /// `ptr` must be a valid, non-null pointer obtained from the NGX API.
    pub(crate) unsafe fn from_raw(
        ptr: *mut nvngx_sys::NVSDK_NGX_Parameter,
        destroy_fn: DestroyParametersFn,
    ) -> Self {
        Self { ptr, destroy_fn }
    }

    /// Sets the value for the parameter named `name` to be a
    /// type-erased (`void *`) pointer.
    pub fn set_ptr<T>(&self, name: &FeatureParameterName, ptr: *mut T) {
        unsafe {
            nvngx_sys::NVSDK_NGX_Parameter_SetVoidPointer(
                self.ptr,
                name.as_ptr().cast(),
                ptr as *mut _,
            );
        }
    }

    /// Returns a type-erased pointer associated with the provided
    /// `name`.
    pub fn get_ptr(&self, name: &FeatureParameterName) -> Result<*mut std::ffi::c_void> {
        let mut ptr = std::ptr::null_mut();
        Result::from(unsafe {
            nvngx_sys::NVSDK_NGX_Parameter_GetVoidPointer(
                self.ptr,
                name.as_ptr().cast(),
                &mut ptr as *mut _,
            )
        })
        .map(|()| ptr)
    }

    /// Sets an [`bool`] value for the parameter named `name`.
    pub fn set_bool(&self, name: &FeatureParameterName, value: bool) {
        unsafe {
            nvngx_sys::NVSDK_NGX_Parameter_SetI(
                self.ptr,
                name.as_ptr().cast(),
                if value { 1 } else { 0 },
            )
        }
    }

    /// Returns a [`bool`] value of a parameter named `name`.
    pub fn get_bool(&self, name: &FeatureParameterName) -> Result<bool> {
        let mut value = 0i32;
        Result::from(unsafe {
            nvngx_sys::NVSDK_NGX_Parameter_GetI(
                self.ptr,
                name.as_ptr().cast(),
                &mut value as *mut _,
            )
        })
        .map(|()| value == 1)
    }

    /// Sets an [`f32`] value for the parameter named `name`.
    pub fn set_f32(&self, name: &FeatureParameterName, value: f32) {
        unsafe { nvngx_sys::NVSDK_NGX_Parameter_SetF(self.ptr, name.as_ptr().cast(), value) }
    }

    /// Returns a [`f32`] value of a parameter named `name`.
    pub fn get_f32(&self, name: &FeatureParameterName) -> Result<f32> {
        let mut value = 0f32;
        Result::from(unsafe {
            nvngx_sys::NVSDK_NGX_Parameter_GetF(
                self.ptr,
                name.as_ptr().cast(),
                &mut value as *mut _,
            )
        })
        .map(|()| value)
    }

    /// Sets an [`u32`] value for the parameter named `name`.
    pub fn set_u32(&self, name: &FeatureParameterName, value: u32) {
        unsafe { nvngx_sys::NVSDK_NGX_Parameter_SetUI(self.ptr, name.as_ptr().cast(), value) }
    }

    /// Returns a [`u32`] value of a parameter named `name`.
    pub fn get_u32(&self, name: &FeatureParameterName) -> Result<u32> {
        let mut value = 0u32;
        Result::from(unsafe {
            nvngx_sys::NVSDK_NGX_Parameter_GetUI(
                self.ptr,
                name.as_ptr().cast(),
                &mut value as *mut _,
            )
        })
        .map(|()| value)
    }

    /// Sets an [`f64`] value for the parameter named `name`.
    pub fn set_f64(&self, name: &FeatureParameterName, value: f64) {
        unsafe { nvngx_sys::NVSDK_NGX_Parameter_SetD(self.ptr, name.as_ptr().cast(), value) }
    }

    /// Returns a [`f64`] value of a parameter named `name`.
    pub fn get_f64(&self, name: &FeatureParameterName) -> Result<f64> {
        let mut value = 0f64;
        Result::from(unsafe {
            nvngx_sys::NVSDK_NGX_Parameter_GetD(
                self.ptr,
                name.as_ptr().cast(),
                &mut value as *mut _,
            )
        })
        .map(|()| value)
    }

    /// Sets an [`i32`] value for the parameter named `name`.
    pub fn set_i32(&self, name: &FeatureParameterName, value: i32) {
        unsafe { nvngx_sys::NVSDK_NGX_Parameter_SetI(self.ptr, name.as_ptr().cast(), value) }
    }

    /// Returns a [`i32`] value of a parameter named `name`.
    pub fn get_i32(&self, name: &FeatureParameterName) -> Result<i32> {
        let mut value = 0i32;
        Result::from(unsafe {
            nvngx_sys::NVSDK_NGX_Parameter_GetI(
                self.ptr,
                name.as_ptr().cast(),
                &mut value as *mut _,
            )
        })
        .map(|()| value)
    }

    /// Sets an [`u64`] value for the parameter named `name`.
    pub fn set_u64(&self, name: &FeatureParameterName, value: u64) {
        unsafe { nvngx_sys::NVSDK_NGX_Parameter_SetULL(self.ptr, name.as_ptr().cast(), value) }
    }

    /// Returns a [`u64`] value of a parameter named `name`.
    pub fn get_u64(&self, name: &FeatureParameterName) -> Result<u64> {
        let mut value = 0u64;
        Result::from(unsafe {
            nvngx_sys::NVSDK_NGX_Parameter_GetULL(
                self.ptr,
                name.as_ptr().cast(),
                &mut value as *mut _,
            )
        })
        .map(|()| value)
    }

    /// Returns [`Ok`] if the parameters claim to support the
    /// super sampling feature ([`nvngx_sys::NVSDK_NGX_Parameter_SuperSampling_Available`]).
    pub fn supports_super_sampling(&self) -> Result<()> {
        if self.get_bool(nvngx_sys::NVSDK_NGX_Parameter_SuperSampling_NeedsUpdatedDriver)? {
            let major =
                self.get_u32(nvngx_sys::NVSDK_NGX_Parameter_SuperSampling_MinDriverVersionMajor)?;
            let minor =
                self.get_u32(nvngx_sys::NVSDK_NGX_Parameter_SuperSampling_MinDriverVersionMinor)?;
            return Err(nvngx_sys::Error::Other(format!("The SuperSampling feature requires a driver update. The driver version required should be higher or equal to {major}.{minor}")));
        }
        match self.get_bool(nvngx_sys::NVSDK_NGX_Parameter_SuperSampling_Available) {
            Ok(true) => Ok(()),
            Ok(false) => Err(nvngx_sys::Error::Other(
                "The SuperSampling feature isn't supported on this platform.".to_string(),
            )),
            Err(e) => Err(e),
        }
    }

    /// Returns [`Ok`] if the parameters claim to support the
    /// ray reconstruction feature ([`nvngx_sys::NVSDK_NGX_Feature::NVSDK_NGX_Feature_RayReconstruction`]).
    pub fn supports_ray_reconstruction(&self) -> Result<()> {
        if self
            .get_bool(nvngx_sys::NVSDK_NGX_Parameter_SuperSamplingDenoising_NeedsUpdatedDriver)?
        {
            let major = self.get_u32(
                nvngx_sys::NVSDK_NGX_Parameter_SuperSamplingDenoising_MinDriverVersionMajor,
            )?;
            let minor = self.get_u32(
                nvngx_sys::NVSDK_NGX_Parameter_SuperSamplingDenoising_MinDriverVersionMinor,
            )?;
            return Err(nvngx_sys::Error::Other(format!("The Ray Reconstruction feature requires a driver update. The driver version required should be higher or equal to {major}.{minor}")));
        }
        match self.get_bool(nvngx_sys::NVSDK_NGX_Parameter_SuperSamplingDenoising_Available) {
            Ok(true) => Ok(()),
            Ok(false) => Err(nvngx_sys::Error::Other(
                "The Ray Reconstruction feature isn't supported on this platform.".to_string(),
            )),
            Err(e) => Err(e),
        }
    }

    /// Returns [`true`] if the SuperSampling feature is initialised
    /// correctly.
    pub fn is_super_sampling_initialised(&self) -> bool {
        self.get_bool(nvngx_sys::NVSDK_NGX_Parameter_SuperSampling_FeatureInitResult)
            .unwrap_or(false)
    }

    /// Returns [`true`] if the Ray Reconstruction feature is initialised
    /// correctly.
    pub fn is_ray_reconstruction_initialised(&self) -> bool {
        self.get_bool(nvngx_sys::NVSDK_NGX_Parameter_SuperSamplingDenoising_FeatureInitResult)
            .unwrap_or(false)
    }

    /// Deallocates the feature parameter set.
    fn release(&self) -> Result {
        unsafe { (self.destroy_fn)(self.ptr) }.into()
    }
}

impl Drop for FeatureParameters {
    fn drop(&mut self) {
        if self.ptr.is_null() {
            return;
        }

        if let Err(e) = self.release() {
            log::error!(
                "Couldn't release the feature parameter set: {:?}: {e}",
                self
            )
        }
    }
}

// ---------------------------------------------------------------------------
// Feature
// ---------------------------------------------------------------------------

/// Describes a single NGX feature. API-agnostic — only holds handles,
/// parameters, and query methods. Construction and evaluation are provided
/// by the API-specific modules ([`crate::vk`] / `crate::dx`).
#[derive(Debug)]
pub struct Feature {
    /// The feature handle.
    pub handle: Rc<FeatureHandle>,
    /// The type of the feature.
    pub feature_type: NVSDK_NGX_Feature,
    /// The parameters of the feature.
    pub parameters: Rc<FeatureParameters>,
}

impl Feature {
    /// Returns the [`FeatureParameters`] associated with this feature.
    pub fn get_parameters(&self) -> &FeatureParameters {
        &self.parameters
    }

    /// Returns the [`FeatureParameters`] associated with this feature.
    pub fn get_parameters_mut(&mut self) -> &mut FeatureParameters {
        Rc::get_mut(&mut self.parameters).unwrap()
    }

    /// Returns the type of this feature.
    pub fn get_feature_type(&self) -> NVSDK_NGX_Feature {
        self.feature_type
    }

    /// Returns [`true`] if this is a super sampling (DLSS) feature.
    pub fn is_super_sampling(&self) -> bool {
        self.feature_type == NVSDK_NGX_Feature::NVSDK_NGX_Feature_SuperSampling
    }

    /// Returns [`true`] if this is a Frame Generation feature.
    pub fn is_frame_generation(&self) -> bool {
        self.feature_type == NVSDK_NGX_Feature::NVSDK_NGX_Feature_FrameGeneration
    }

    /// Returns [`true`] if this is a ray reconstruction feature.
    pub fn is_ray_reconstruction(&self) -> bool {
        self.feature_type == NVSDK_NGX_Feature::NVSDK_NGX_Feature_RayReconstruction
    }
}

// ---------------------------------------------------------------------------
// FeatureRequirement
// ---------------------------------------------------------------------------

/// Describes a set of NGX feature requirements.
#[repr(transparent)]
#[derive(Debug)]
pub struct FeatureRequirement(pub(crate) nvngx_sys::NVSDK_NGX_FeatureRequirement);

impl FeatureRequirement {
    pub(crate) fn from_raw(raw: nvngx_sys::NVSDK_NGX_FeatureRequirement) -> Self {
        Self(raw)
    }

    /// Whether the feature is supported on this hardware/driver/OS combination.
    ///
    /// Per `nvsdk_ngx_vk.h`: *"FeatureSupported: Bitfield of reasons why the
    /// feature is unsupported, as specified in NVSDK_NGX_Feature_Support_Result.
    /// 0 if the feature is supported"*.
    pub fn is_supported(&self) -> bool {
        self.0.FeatureSupported
            == nvngx_sys::NVSDK_NGX_Feature_Support_Result::NVSDK_NGX_FeatureSupportResult_Supported
    }

    /// Bitfield of reasons the feature is unsupported. Returns the
    /// [`nvngx_sys::NVSDK_NGX_Feature_Support_Result::NVSDK_NGX_FeatureSupportResult_Supported`]
    /// sentinel (`0`) when the feature *is* supported.
    pub fn unsupported_reason(&self) -> nvngx_sys::NVSDK_NGX_Feature_Support_Result {
        self.0.FeatureSupported
    }

    /// Minimum required hardware architecture, as an `NV_GPU_ARCHITECTURE_ID`
    /// value defined in the NvAPI GPU Framework.
    pub fn min_hw_architecture(&self) -> u32 {
        self.0.MinHWArchitecture
    }

    /// Minimum required OS version, decoded from the fixed-size
    /// `MinOSVersion[255]` C string. Returns [`Err`] if the buffer isn't
    /// NUL-terminated or doesn't contain valid UTF-8.
    pub fn min_os_version(&self) -> Result<&str> {
        // Reinterpret the C `[c_char; 255]` (signed on most platforms) as
        // `[u8; 255]` for `CStr::from_bytes_until_nul`. The cast is sound
        // because `c_char` and `u8` share size and alignment.
        let bytes: &[u8] = unsafe {
            std::slice::from_raw_parts(
                self.0.MinOSVersion.as_ptr().cast::<u8>(),
                self.0.MinOSVersion.len(),
            )
        };
        let cstr = std::ffi::CStr::from_bytes_until_nul(bytes).map_err(|e| {
            nvngx_sys::Error::Other(format!("MinOSVersion not NUL-terminated: {e}"))
        })?;
        cstr.to_str()
            .map_err(|e| nvngx_sys::Error::Other(format!("MinOSVersion not valid UTF-8: {e}")))
    }

    /// Returns [`Ok`] if the feature is supported, otherwise [`Err`] with a
    /// description of the [`Self::unsupported_reason()`] bitfield.
    pub fn check_supported(&self) -> Result<()> {
        if self.is_supported() {
            return Ok(());
        }
        let min_os_version = self.min_os_version().unwrap_or("<invalid>");
        Err(nvngx_sys::Error::Other(format!(
            "NGX feature unsupported: reason bitfield = {:?}, min HW arch = 0x{:x}, min OS version = {min_os_version:?}",
            self.unsupported_reason(),
            self.min_hw_architecture(),
        )))
    }
}

/// Owns the temporary buffers backing a [`nvngx_sys::NVSDK_NGX_FeatureCommonInfo`]
/// passed to NGX FFI calls.
///
/// All buffers ([`widestring::WideString`] heap allocations and the [`Vec`]
/// of pointers into them) are heap-allocated, so moving the
/// `CommonInfoStorage` value does not invalidate the pointers stored inside
/// `c_common_info`. Bind to a local before calling [`Self::as_ref()`]; the
/// returned reference is valid for the lifetime of the borrow.
///
/// Wrap in [`Option`] (typically `common_info.map(CommonInfoStorage::new)`)
/// when the surrounding API allows omitting the common info — there's no
/// "absent" state on the storage itself.
///
/// NGX consumes the path list synchronously and captures `LoggingInfo` into
/// its own state during the FFI call, so the storage only needs to outlive
/// that single call.
pub(crate) struct CommonInfoStorage {
    // The `c_common_info.PathListInfo.Path` field points into
    // `path_pointers`'s heap allocation, and each entry of `path_pointers`
    // points into the corresponding `path_buffers` heap allocation. The two
    // `Vec`s are kept alive purely so those pointers stay valid.
    _path_buffers: Vec<widestring::WideString>,
    _path_pointers: Vec<*const libc::wchar_t>,
    c_common_info: nvngx_sys::NVSDK_NGX_FeatureCommonInfo,
}

impl CommonInfoStorage {
    pub(crate) fn new(info: &FeatureCommonInfo<'_>) -> Self {
        let path_buffers: Vec<widestring::WideString> = info
            .search_paths
            .iter()
            .map(|p| {
                widestring::WideString::from_str(
                    p.to_str().expect("NGX search paths must be valid UTF-8"),
                )
            })
            .collect();
        let path_pointers: Vec<*const libc::wchar_t> = path_buffers
            .iter()
            .map(|s| s.as_ptr().cast::<libc::wchar_t>())
            .collect();
        let c_common_info = nvngx_sys::NVSDK_NGX_FeatureCommonInfo {
            PathListInfo: nvngx_sys::NVSDK_NGX_PathListInfo {
                Path: if path_pointers.is_empty() {
                    std::ptr::null()
                } else {
                    path_pointers.as_ptr()
                },
                Length: path_pointers.len() as u32,
            },
            InternalData: std::ptr::null_mut(),
            LoggingInfo: match info.logging {
                Some(cfg) => nvngx_sys::NVSDK_NGX_LoggingInfo {
                    LoggingCallback: Some(log_crate_callback),
                    MinimumLoggingLevel: cfg.minimum_level.into(),
                    DisableOtherLoggingSinks: cfg.disable_other_sinks,
                },
                None => nvngx_sys::NVSDK_NGX_LoggingInfo::default(),
            },
        };
        Self {
            _path_buffers: path_buffers,
            _path_pointers: path_pointers,
            c_common_info,
        }
    }
}

impl AsRef<nvngx_sys::NVSDK_NGX_FeatureCommonInfo> for CommonInfoStorage {
    fn as_ref(&self) -> &nvngx_sys::NVSDK_NGX_FeatureCommonInfo {
        &self.c_common_info
    }
}

// ---------------------------------------------------------------------------
// SuperSamplingOptimalSettings
// ---------------------------------------------------------------------------

/// Optimal settings for DLSS based on the desired quality level and resolution.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct SuperSamplingOptimalSettings {
    /// The render width which the renderer must render to before upscaling.
    pub render_width: u32,
    /// The render height which the renderer must render to before upscaling.
    pub render_height: u32,
    /// The target width desired, to which the SuperSampling feature will upscale to.
    pub target_width: u32,
    /// The target height desired, to which the SuperSampling feature will upscale to.
    pub target_height: u32,
    /// The requested quality level.
    pub desired_quality_level: NVSDK_NGX_PerfQuality_Value,
    /// Dynamic minimum render width.
    pub dynamic_min_render_width: u32,
    /// Dynamic maximum render width.
    pub dynamic_max_render_width: u32,
    /// Dynamic minimum render height.
    pub dynamic_min_render_height: u32,
    /// Dynamic maximum render height.
    pub dynamic_max_render_height: u32,
}

impl SuperSamplingOptimalSettings {
    /// Returns a set of optimal settings for the desired parameter
    /// set, render dimensions and quality level.
    pub fn get_optimal_settings(
        parameters: &FeatureParameters,
        target_width: u32,
        target_height: u32,
        desired_quality_level: NVSDK_NGX_PerfQuality_Value,
    ) -> Result<Self> {
        let mut settings = Self {
            render_width: 0,
            render_height: 0,
            target_width,
            target_height,
            desired_quality_level,
            dynamic_min_render_width: 0,
            dynamic_max_render_width: 0,
            dynamic_min_render_height: 0,
            dynamic_max_render_height: 0,
        };
        let mut sharpness = 0.0f32;
        Result::from(unsafe {
            nvngx_sys::helpers::dlss_get_optimal_settings(
                parameters.ptr,
                settings.target_width,
                settings.target_height,
                settings.desired_quality_level,
                &mut settings.render_width as *mut _,
                &mut settings.render_height as *mut _,
                &mut settings.dynamic_max_render_width as *mut _,
                &mut settings.dynamic_max_render_height as *mut _,
                &mut settings.dynamic_min_render_width as *mut _,
                &mut settings.dynamic_min_render_height as *mut _,
                &mut sharpness as *mut _,
            )
        })?;

        if settings.render_height == 0 || settings.render_width == 0 {
            return Err(nvngx_sys::Error::Other(format!(
                "The requested quality level isn't supported: {desired_quality_level:?}"
            )));
        }

        Ok(settings)
    }
}

// ---------------------------------------------------------------------------
// SuperSamplingCreateParameters
// ---------------------------------------------------------------------------

/// Create parameters for a super sampling (DLSS) feature.
#[repr(transparent)]
#[derive(Debug)]
pub struct SuperSamplingCreateParameters(pub(crate) NVSDK_NGX_DLSS_Create_Params);

impl SuperSamplingCreateParameters {
    /// Creates a new set of create parameters for the SuperSampling feature.
    pub fn new(
        render_width: u32,
        render_height: u32,
        target_width: u32,
        target_height: u32,
        quality_value: Option<NVSDK_NGX_PerfQuality_Value>,
        flags: Option<NVSDK_NGX_DLSS_Feature_Flags>,
    ) -> Self {
        let mut params: NVSDK_NGX_DLSS_Create_Params = unsafe { std::mem::zeroed() };
        params.Feature.InWidth = render_width;
        params.Feature.InHeight = render_height;
        params.Feature.InTargetWidth = target_width;
        params.Feature.InTargetHeight = target_height;
        if let Some(quality_value) = quality_value {
            params.Feature.InPerfQualityValue = quality_value;
        }
        params.InFeatureCreateFlags = flags.map(|f| f.0).unwrap_or(0);
        Self(params)
    }
}

impl From<SuperSamplingOptimalSettings> for SuperSamplingCreateParameters {
    fn from(value: SuperSamplingOptimalSettings) -> Self {
        Self::new(
            value.render_width,
            value.render_height,
            value.target_width,
            value.target_height,
            Some(value.desired_quality_level),
            Some(
                NVSDK_NGX_DLSS_Feature_Flags::NVSDK_NGX_DLSS_Feature_Flags_AutoExposure
                    | NVSDK_NGX_DLSS_Feature_Flags::NVSDK_NGX_DLSS_Feature_Flags_MVLowRes,
            ),
        )
    }
}

// ---------------------------------------------------------------------------
// RayReconstructionCreateParameters
// ---------------------------------------------------------------------------

/// Create parameters for a ray reconstruction (DLSS-RR) feature.
#[repr(transparent)]
#[derive(Debug)]
pub struct RayReconstructionCreateParameters(pub(crate) NVSDK_NGX_DLSSD_Create_Params);

impl RayReconstructionCreateParameters {
    /// Creates a new set of create parameters for the Ray Reconstruction feature.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        render_width: u32,
        render_height: u32,
        target_width: u32,
        target_height: u32,
        quality_value: Option<NVSDK_NGX_PerfQuality_Value>,
        denoise_mode: Option<NVSDK_NGX_DLSS_Denoise_Mode>,
        roughness_mode: Option<NVSDK_NGX_DLSS_Roughness_Mode>,
        depth_type: Option<NVSDK_NGX_DLSS_Depth_Type>,
    ) -> Self {
        Self(NVSDK_NGX_DLSSD_Create_Params {
            InWidth: render_width,
            InHeight: render_height,
            InTargetWidth: target_width,
            InTargetHeight: target_height,
            InPerfQualityValue: quality_value
                .unwrap_or(NVSDK_NGX_PerfQuality_Value::NVSDK_NGX_PerfQuality_Value_MaxPerf),
            InDenoiseMode: denoise_mode
                .unwrap_or(NVSDK_NGX_DLSS_Denoise_Mode::NVSDK_NGX_DLSS_Denoise_Mode_DLUnified),
            InRoughnessMode: roughness_mode
                .unwrap_or(NVSDK_NGX_DLSS_Roughness_Mode::NVSDK_NGX_DLSS_Roughness_Mode_Unpacked),
            InUseHWDepth: depth_type
                .unwrap_or(NVSDK_NGX_DLSS_Depth_Type::NVSDK_NGX_DLSS_Depth_Type_Linear),
            InFeatureCreateFlags: 0,
            InEnableOutputSubrects: false,
        })
    }
}

impl From<SuperSamplingOptimalSettings> for RayReconstructionCreateParameters {
    fn from(value: SuperSamplingOptimalSettings) -> Self {
        Self::new(
            value.render_width,
            value.render_height,
            value.target_width,
            value.target_height,
            Some(value.desired_quality_level),
            None,
            None,
            None,
        )
    }
}
