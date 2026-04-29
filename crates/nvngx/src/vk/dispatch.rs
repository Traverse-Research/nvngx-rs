//! Internal dispatch layer that abstracts over compile-time linking (`linked`)
//! and runtime loading (`loaded`).

use ash::vk;

/// Generates wrapper methods on [`Dispatch`] that forward to either the
/// `nvngx_sys` free function (linked) or the `Library` method (loaded).
macro_rules! dispatch_fns {
    // Void functions (no return value).
    (void fn $name:ident ( $($arg:ident : $ty:ty),* $(,)? )) => {
        #[allow(non_snake_case, clippy::too_many_arguments)]
        pub(crate) unsafe fn $name(&self, $($arg: $ty),*) {
            #[cfg(feature = "linked")]
            {
                nvngx_sys::$name($($arg),*)
            }
            #[cfg(all(feature = "loaded", not(feature = "linked")))]
            {
                self.library.$name($($arg),*)
            }
        }
    };
    // Functions returning NVSDK_NGX_Result.
    (fn $name:ident ( $($arg:ident : $ty:ty),* $(,)? ) -> $ret:ty) => {
        #[allow(non_snake_case, clippy::too_many_arguments)]
        pub(crate) unsafe fn $name(&self, $($arg: $ty),*) -> $ret {
            #[cfg(feature = "linked")]
            {
                nvngx_sys::$name($($arg),*)
            }
            #[cfg(all(feature = "loaded", not(feature = "linked")))]
            {
                self.library.$name($($arg),*)
            }
        }
    };
}

/// Internal FFI dispatch table.
///
/// When the `linked` feature is active this is a zero-sized type — every call
/// goes through the link-time symbol. With the `loaded` feature it wraps an
/// [`nvngx_sys::library::Library`] whose function pointers were resolved at
/// runtime via `libloading`.
pub(crate) struct Dispatch {
    #[cfg(all(feature = "loaded", not(feature = "linked")))]
    library: nvngx_sys::library::Library,
}

impl std::fmt::Debug for Dispatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Dispatch").finish_non_exhaustive()
    }
}

#[allow(dead_code)]
impl Dispatch {
    /// Create a dispatch table for the `linked` feature.
    #[cfg(feature = "linked")]
    pub(crate) fn new_linked() -> Self {
        Self {}
    }

    /// Create a dispatch table for the `loaded` feature.
    #[cfg(all(feature = "loaded", not(feature = "linked")))]
    pub(crate) fn new_loaded(library: nvngx_sys::library::Library) -> Self {
        Self { library }
    }

    // ── Parameter setters (void) ────────────────────────────────────

    dispatch_fns!(void fn NVSDK_NGX_Parameter_SetULL(
        InParameter: *mut nvngx_sys::NVSDK_NGX_Parameter,
        InName: *const std::os::raw::c_char,
        InValue: std::os::raw::c_ulonglong,
    ));

    dispatch_fns!(void fn NVSDK_NGX_Parameter_SetF(
        InParameter: *mut nvngx_sys::NVSDK_NGX_Parameter,
        InName: *const std::os::raw::c_char,
        InValue: f32,
    ));

    dispatch_fns!(void fn NVSDK_NGX_Parameter_SetD(
        InParameter: *mut nvngx_sys::NVSDK_NGX_Parameter,
        InName: *const std::os::raw::c_char,
        InValue: f64,
    ));

    dispatch_fns!(void fn NVSDK_NGX_Parameter_SetUI(
        InParameter: *mut nvngx_sys::NVSDK_NGX_Parameter,
        InName: *const std::os::raw::c_char,
        InValue: std::os::raw::c_uint,
    ));

    dispatch_fns!(void fn NVSDK_NGX_Parameter_SetI(
        InParameter: *mut nvngx_sys::NVSDK_NGX_Parameter,
        InName: *const std::os::raw::c_char,
        InValue: std::os::raw::c_int,
    ));

    dispatch_fns!(void fn NVSDK_NGX_Parameter_SetVoidPointer(
        InParameter: *mut nvngx_sys::NVSDK_NGX_Parameter,
        InName: *const std::os::raw::c_char,
        InValue: *mut std::os::raw::c_void,
    ));

    // ── Parameter getters (return Result) ───────────────────────────

    dispatch_fns!(fn NVSDK_NGX_Parameter_GetULL(
        InParameter: *mut nvngx_sys::NVSDK_NGX_Parameter,
        InName: *const std::os::raw::c_char,
        OutValue: *mut std::os::raw::c_ulonglong,
    ) -> nvngx_sys::NVSDK_NGX_Result);

    dispatch_fns!(fn NVSDK_NGX_Parameter_GetF(
        InParameter: *mut nvngx_sys::NVSDK_NGX_Parameter,
        InName: *const std::os::raw::c_char,
        OutValue: *mut f32,
    ) -> nvngx_sys::NVSDK_NGX_Result);

    dispatch_fns!(fn NVSDK_NGX_Parameter_GetD(
        InParameter: *mut nvngx_sys::NVSDK_NGX_Parameter,
        InName: *const std::os::raw::c_char,
        OutValue: *mut f64,
    ) -> nvngx_sys::NVSDK_NGX_Result);

    dispatch_fns!(fn NVSDK_NGX_Parameter_GetUI(
        InParameter: *mut nvngx_sys::NVSDK_NGX_Parameter,
        InName: *const std::os::raw::c_char,
        OutValue: *mut std::os::raw::c_uint,
    ) -> nvngx_sys::NVSDK_NGX_Result);

    dispatch_fns!(fn NVSDK_NGX_Parameter_GetI(
        InParameter: *mut nvngx_sys::NVSDK_NGX_Parameter,
        InName: *const std::os::raw::c_char,
        OutValue: *mut std::os::raw::c_int,
    ) -> nvngx_sys::NVSDK_NGX_Result);

    dispatch_fns!(fn NVSDK_NGX_Parameter_GetVoidPointer(
        InParameter: *mut nvngx_sys::NVSDK_NGX_Parameter,
        InName: *const std::os::raw::c_char,
        OutValue: *mut *mut std::os::raw::c_void,
    ) -> nvngx_sys::NVSDK_NGX_Result);

    // ── Vulkan operations ───────────────────────────────────────────

    dispatch_fns!(fn NVSDK_NGX_VULKAN_RequiredExtensions(
        OutInstanceExtCount: *mut std::os::raw::c_uint,
        OutInstanceExts: *mut *mut *const std::os::raw::c_char,
        OutDeviceExtCount: *mut std::os::raw::c_uint,
        OutDeviceExts: *mut *mut *const std::os::raw::c_char,
    ) -> nvngx_sys::NVSDK_NGX_Result);

    dispatch_fns!(fn NVSDK_NGX_VULKAN_Init_with_ProjectID(
        InProjectId: *const std::os::raw::c_char,
        InEngineType: nvngx_sys::NVSDK_NGX_EngineType,
        InEngineVersion: *const std::os::raw::c_char,
        InApplicationDataPath: *const libc::wchar_t,
        InInstance: vk::Instance,
        InPD: vk::PhysicalDevice,
        InDevice: vk::Device,
        InGIPA: vk::PFN_vkGetInstanceProcAddr,
        InGDPA: vk::PFN_vkGetDeviceProcAddr,
        InFeatureInfo: *const nvngx_sys::NVSDK_NGX_FeatureCommonInfo,
        InSDKVersion: nvngx_sys::NVSDK_NGX_Version,
    ) -> nvngx_sys::NVSDK_NGX_Result);

    dispatch_fns!(fn NVSDK_NGX_VULKAN_Shutdown1(
        InDevice: vk::Device,
    ) -> nvngx_sys::NVSDK_NGX_Result);

    dispatch_fns!(fn NVSDK_NGX_VULKAN_AllocateParameters(
        OutParameters: *mut *mut nvngx_sys::NVSDK_NGX_Parameter,
    ) -> nvngx_sys::NVSDK_NGX_Result);

    dispatch_fns!(fn NVSDK_NGX_VULKAN_GetCapabilityParameters(
        OutParameters: *mut *mut nvngx_sys::NVSDK_NGX_Parameter,
    ) -> nvngx_sys::NVSDK_NGX_Result);

    dispatch_fns!(fn NVSDK_NGX_VULKAN_DestroyParameters(
        InParameters: *mut nvngx_sys::NVSDK_NGX_Parameter,
    ) -> nvngx_sys::NVSDK_NGX_Result);

    dispatch_fns!(fn NVSDK_NGX_VULKAN_GetScratchBufferSize(
        InFeatureId: nvngx_sys::NVSDK_NGX_Feature,
        InParameters: *const nvngx_sys::NVSDK_NGX_Parameter,
        OutSizeInBytes: *mut usize,
    ) -> nvngx_sys::NVSDK_NGX_Result);

    dispatch_fns!(fn NVSDK_NGX_VULKAN_CreateFeature(
        InCmdBuffer: vk::CommandBuffer,
        InFeatureID: nvngx_sys::NVSDK_NGX_Feature,
        InParameters: *mut nvngx_sys::NVSDK_NGX_Parameter,
        OutHandle: *mut *mut nvngx_sys::NVSDK_NGX_Handle,
    ) -> nvngx_sys::NVSDK_NGX_Result);

    dispatch_fns!(fn NVSDK_NGX_VULKAN_CreateFeature1(
        InDevice: vk::Device,
        InCmdList: vk::CommandBuffer,
        InFeatureID: nvngx_sys::NVSDK_NGX_Feature,
        InParameters: *mut nvngx_sys::NVSDK_NGX_Parameter,
        OutHandle: *mut *mut nvngx_sys::NVSDK_NGX_Handle,
    ) -> nvngx_sys::NVSDK_NGX_Result);

    dispatch_fns!(fn NVSDK_NGX_VULKAN_ReleaseFeature(
        InHandle: *mut nvngx_sys::NVSDK_NGX_Handle,
    ) -> nvngx_sys::NVSDK_NGX_Result);

    dispatch_fns!(fn NVSDK_NGX_VULKAN_EvaluateFeature_C(
        InCmdList: vk::CommandBuffer,
        InFeatureHandle: *const nvngx_sys::NVSDK_NGX_Handle,
        InParameters: *const nvngx_sys::NVSDK_NGX_Parameter,
        InCallback: nvngx_sys::PFN_NVSDK_NGX_ProgressCallback_C,
    ) -> nvngx_sys::NVSDK_NGX_Result);
}
