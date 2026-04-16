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
// FeatureParameterName + debug macro
// ---------------------------------------------------------------------------

/// A type alias for feature parameter, like
/// [`nvngx_sys::NVSDK_NGX_Parameter_NumFrames`].
pub type FeatureParameterName = [u8];

/// Inserts a parameter into the debug map.
#[macro_export]
macro_rules! insert_parameter_debug {
    ($map:ident, $parameters:ident, ($key:path, bool),) => {
        if let Ok(value) = $parameters.get_bool($key) {
            $map.insert(
                stringify!($key).to_owned(),
                format!("{:?}", value)
                );
        }
    };
    ($map:ident, $parameters:ident, ($key:path, i32),) => {
        if let Ok(value) = $parameters.get_i32($key) {
            $map.insert(
                stringify!($key).to_owned(),
                format!("{:?}", value),
            );
        }
    };
    ($map:ident, $parameters:ident, ($key:path, u32),) => {
        if let Ok(value) = $parameters.get_u32($key) {
            $map.insert(
                stringify!($key).to_owned(),
                format!("{:?}", value),
            );
        }
    };
    ($map:ident, $parameters:ident, ($key:path, f32),) => {
        if let Ok(value) = $parameters.get_f32($key) {
            $map.insert(
                stringify!($key).to_owned(),
                format!("{:?}", value),
            );
        }
    };
    ($map:ident, $parameters:ident, ($key:path, u64),) => {
        if let Ok(value) = $parameters.get_u64($key) {
            $map.insert(
                stringify!($key).to_owned(),
                format!("{:?}", value),
            );
        }
    };
    ($map:ident, $parameters:ident, ($key:path, f64),) => {
        if let Ok(value) = $parameters.get_f64($key) {
            $map.insert(
                stringify!($key).to_owned(),
                format!("{:?}", value),
            );
        }
    };
    ($map:ident, $parameters:ident, ($key:path, $typ:ident), $(($next_key:path, $next_type:ident)),+,) => {
        $crate::insert_parameter_debug!($map, $parameters, ($key, $typ),);
        $crate::insert_parameter_debug!($map, $parameters, $(($next_key, $next_type)),+,);
    };
}

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
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        #[repr(transparent)]
        struct FeatureParametersDebugPrinter<'a>(&'a FeatureParameters);

        impl<'a> std::fmt::Debug for FeatureParametersDebugPrinter<'a> {
            fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
                use std::collections::HashMap;

                let mut fmt = fmt.debug_struct("FeatureParameters");
                fmt.field("pointer_address", &self.0.ptr);

                let populate_map = || -> HashMap<String, String> {
                    let mut map = HashMap::new();
                    let parameters = self.0;

                    insert_parameter_debug!(
                        map,
                        parameters,
                        (nvngx_sys::NVSDK_NGX_Parameter_SuperSampling_Available, bool),
                        (
                            nvngx_sys::NVSDK_NGX_Parameter_SuperSamplingDenoising_Available,
                            bool
                        ),
                        (nvngx_sys::NVSDK_NGX_Parameter_InPainting_Available, bool),
                        (
                            nvngx_sys::NVSDK_NGX_Parameter_ImageSuperResolution_Available,
                            bool
                        ),
                        (nvngx_sys::NVSDK_NGX_Parameter_SlowMotion_Available, bool),
                        (
                            nvngx_sys::NVSDK_NGX_Parameter_VideoSuperResolution_Available,
                            bool
                        ),
                        (
                            nvngx_sys::NVSDK_NGX_Parameter_ImageSignalProcessing_Available,
                            bool
                        ),
                        (nvngx_sys::NVSDK_NGX_Parameter_DeepResolve_Available, bool),
                        (
                            nvngx_sys::NVSDK_NGX_Parameter_SuperSampling_NeedsUpdatedDriver,
                            bool
                        ),
                        (
                            nvngx_sys::NVSDK_NGX_Parameter_InPainting_NeedsUpdatedDriver,
                            bool
                        ),
                        (
                            nvngx_sys::NVSDK_NGX_Parameter_ImageSuperResolution_NeedsUpdatedDriver,
                            bool
                        ),
                        (
                            nvngx_sys::NVSDK_NGX_Parameter_SlowMotion_NeedsUpdatedDriver,
                            bool
                        ),
                        (
                            nvngx_sys::NVSDK_NGX_Parameter_VideoSuperResolution_NeedsUpdatedDriver,
                            bool
                        ),
                        (
                            nvngx_sys::NVSDK_NGX_Parameter_ImageSignalProcessing_NeedsUpdatedDriver,
                            bool
                        ),
                        (
                            nvngx_sys::NVSDK_NGX_Parameter_DeepResolve_NeedsUpdatedDriver,
                            bool
                        ),
                        (
                            nvngx_sys::NVSDK_NGX_Parameter_FrameInterpolation_NeedsUpdatedDriver,
                            bool
                        ),
                        (nvngx_sys::NVSDK_NGX_Parameter_NumFrames, u32),
                        (nvngx_sys::NVSDK_NGX_Parameter_Scale, u32),
                        (nvngx_sys::NVSDK_NGX_Parameter_OptLevel, u32),
                        (nvngx_sys::NVSDK_NGX_Parameter_IsDevSnippetBranch, bool),
                        (
                            nvngx_sys::NVSDK_NGX_Parameter_SuperSampling_ScaleFactor,
                            f32
                        ),
                    );
                    map
                };
                let map = populate_map();
                fmt.field("parameters", &map).finish()
            }
        }

        let debug = FeatureParametersDebugPrinter(self);
        fmt.debug_tuple("FeatureParameters").field(&debug).finish()
    }
}

impl FeatureParameters {
    /// Wraps a raw parameter pointer with the appropriate API-specific destroy function.
    pub(crate) fn from_raw(
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
        .map(|_| ptr)
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
        .map(|_| value == 1)
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
        .map(|_| value)
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
        .map(|_| value)
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
        .map(|_| value)
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
        .map(|_| value)
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
        .map(|_| value)
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
    pub feature_type: nvngx_sys::NVSDK_NGX_Feature,
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
