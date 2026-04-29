//! Vulkan bindings to NGX.

use std::rc::Rc;

use ash::vk;
use nvngx_sys::{
    NVSDK_NGX_Coordinates, NVSDK_NGX_Dimensions, NVSDK_NGX_Feature, NVSDK_NGX_ImageViewInfo_VK,
    NVSDK_NGX_PerfQuality_Value, NVSDK_NGX_Resource_VK, NVSDK_NGX_Resource_VK_Type,
    NVSDK_NGX_Resource_VK__bindgen_ty_1, Result,
};

pub(crate) mod dispatch;
pub(crate) mod helpers;

pub mod feature;
pub use feature::*;
pub mod super_sampling;
pub use super_sampling::*;
pub mod ray_reconstruction;
pub use ray_reconstruction::*;

fn convert_slice_of_strings_to_cstrings(data: &[String]) -> Result<Vec<std::ffi::CString>> {
    let strings: Vec<_> = data
        .iter()
        .cloned()
        .map(std::ffi::CString::new)
        .collect::<Result<_, _>>()
        // TODO: Add NulError to our Error enum?
        .map_err(|_| "Couldn't convert the extensions to CStrings.".to_string())?;

    Ok(strings)
}

/// Vulkan extensions required for the NVIDIA NGX operation.
#[derive(Debug, Clone)]
pub struct RequiredExtensions {
    /// Vulkan device extensions required for NVIDIA NGX.
    pub device: Vec<String>,
    /// Vulkan instance extensions required for NVIDIA NGX.
    pub instance: Vec<String>,
}

impl RequiredExtensions {
    /// Returns a list of device extensions as a list of
    /// [`std::ffi::CString`].
    pub fn get_device_extensions_c_strings(&self) -> Result<Vec<std::ffi::CString>> {
        convert_slice_of_strings_to_cstrings(&self.device)
    }

    /// Returns a list of instance extensions as a list of
    /// [`std::ffi::CString`].
    pub fn get_instance_extensions_c_strings(&self) -> Result<Vec<std::ffi::CString>> {
        convert_slice_of_strings_to_cstrings(&self.instance)
    }

    /// Returns a list of required vulkan extensions for NGX to work.
    #[cfg(feature = "linked")]
    pub fn get() -> Result<Self> {
        let mut instance_extensions: *mut *const std::ffi::c_char = std::ptr::null_mut();
        let mut device_extensions: *mut *const std::ffi::c_char = std::ptr::null_mut();
        let mut instance_count = 0u32;
        let mut device_count = 0u32;
        Result::from(unsafe {
            nvngx_sys::NVSDK_NGX_VULKAN_RequiredExtensions(
                &mut instance_count,
                &mut instance_extensions,
                &mut device_count,
                &mut device_extensions,
            )
        })?;

        Self::parse_extensions(
            instance_extensions,
            instance_count,
            device_extensions,
            device_count,
        )
    }

    /// Returns a list of required vulkan extensions for NGX to work.
    #[cfg(feature = "loaded")]
    pub fn get(library: &nvngx_sys::library::Library) -> Result<Self> {
        let mut instance_extensions: *mut *const std::ffi::c_char = std::ptr::null_mut();
        let mut device_extensions: *mut *const std::ffi::c_char = std::ptr::null_mut();
        let mut instance_count = 0u32;
        let mut device_count = 0u32;
        Result::from(unsafe {
            library.NVSDK_NGX_VULKAN_RequiredExtensions(
                &mut instance_count,
                &mut instance_extensions,
                &mut device_count,
                &mut device_extensions,
            )
        })?;

        Self::parse_extensions(
            instance_extensions,
            instance_count,
            device_extensions,
            device_count,
        )
    }

    fn parse_extensions(
        instance_extensions: *mut *const std::ffi::c_char,
        instance_count: u32,
        device_extensions: *mut *const std::ffi::c_char,
        device_count: u32,
    ) -> Result<Self> {
        let mut instance = Vec::new();
        for i in 0..instance_count {
            instance.push(unsafe {
                std::ffi::CStr::from_ptr(*instance_extensions.add(i as usize))
                    .to_str()
                    .map(|s| s.to_owned())
                    .unwrap()
            });
        }

        let mut device = Vec::new();
        for i in 0..device_count {
            device.push(unsafe {
                std::ffi::CStr::from_ptr(*device_extensions.add(i as usize))
                    .to_str()
                    .map(|s| s.to_owned())
                    .unwrap()
            });
        }

        // unsafe {
        //     libc::free(device_extensions as _);
        //     libc::free(instance_extensions as _);
        // }

        Ok(Self { device, instance })
    }
}

/// NVIDIA NGX system.
#[derive(Debug)]
pub struct System {
    device: vk::Device,
    dispatch: Rc<dispatch::Dispatch>,
}

impl System {
    /// Creates a new NVIDIA NGX system (linked).
    ///
    /// `common_info` carries optional feature-DLL search paths and a logging
    /// callback configuration; pass [`None`] to preserve the NGX defaults (no
    /// extra search paths, logging controlled by the registry on Windows).
    #[cfg(feature = "linked")]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        project_id: Option<uuid::Uuid>,
        engine_version: &str,
        application_data_path: &std::path::Path,
        entry: &ash::Entry,
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        logical_device: vk::Device,
        common_info: Option<&crate::common::FeatureCommonInfo<'_>>,
    ) -> Result<Self> {
        let dispatch = Rc::new(dispatch::Dispatch::new_linked());
        Self::new_inner(
            dispatch,
            project_id,
            engine_version,
            application_data_path,
            entry,
            instance,
            physical_device,
            logical_device,
            common_info,
        )
    }

    /// Creates a new NVIDIA NGX system (loaded).
    ///
    /// `library` is the runtime-loaded NGX library obtained via
    /// [`nvngx_sys::library::Library::new`].
    ///
    /// `common_info` carries optional feature-DLL search paths and a logging
    /// callback configuration; pass [`None`] to preserve the NGX defaults (no
    /// extra search paths, logging controlled by the registry on Windows).
    #[cfg(feature = "loaded")]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        library: nvngx_sys::library::Library,
        project_id: Option<uuid::Uuid>,
        engine_version: &str,
        application_data_path: &std::path::Path,
        entry: &ash::Entry,
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        logical_device: vk::Device,
        common_info: Option<&crate::common::FeatureCommonInfo<'_>>,
    ) -> Result<Self> {
        let dispatch = Rc::new(dispatch::Dispatch::new_loaded(library));
        Self::new_inner(
            dispatch,
            project_id,
            engine_version,
            application_data_path,
            entry,
            instance,
            physical_device,
            logical_device,
            common_info,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new_inner(
        dispatch: Rc<dispatch::Dispatch>,
        project_id: Option<uuid::Uuid>,
        engine_version: &str,
        application_data_path: &std::path::Path,
        entry: &ash::Entry,
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        logical_device: vk::Device,
        common_info: Option<&crate::common::FeatureCommonInfo<'_>>,
    ) -> Result<Self> {
        let engine_type = nvngx_sys::NVSDK_NGX_EngineType::NVSDK_NGX_ENGINE_TYPE_CUSTOM;
        let project_id =
            std::ffi::CString::new(project_id.unwrap_or_else(uuid::Uuid::new_v4).to_string())
                .unwrap();
        let engine_version = std::ffi::CString::new(engine_version).unwrap();
        let application_data_path =
            widestring::WideString::from_str(application_data_path.to_str().unwrap());

        // Build `NVSDK_NGX_FeatureCommonInfo` as plain locals scoped to the
        // FFI call below. NGX consumes the paths synchronously during
        // `NGXInitContext` (the feature DLL is loaded inline; see breda's
        // `nvngx.log`) and captures `LoggingInfo` into its own state, so the
        // buffers do not need to outlive the init call.
        let path_buffers: Vec<widestring::WideString>;
        let path_pointers: Vec<*const libc::wchar_t>;
        let c_common_info: nvngx_sys::NVSDK_NGX_FeatureCommonInfo;
        let common_info_ptr: *const nvngx_sys::NVSDK_NGX_FeatureCommonInfo = match common_info {
            None => std::ptr::null(),
            Some(info) => {
                path_buffers = info
                    .search_paths
                    .iter()
                    .map(|p| {
                        widestring::WideString::from_str(
                            p.to_str().expect("NGX search paths must be valid UTF-8"),
                        )
                    })
                    .collect();
                path_pointers = path_buffers
                    .iter()
                    .map(|s| s.as_ptr().cast::<libc::wchar_t>())
                    .collect();
                c_common_info = nvngx_sys::NVSDK_NGX_FeatureCommonInfo {
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
                            LoggingCallback: Some(crate::common::log_crate_callback),
                            MinimumLoggingLevel: cfg.minimum_level.into(),
                            DisableOtherLoggingSinks: cfg.disable_other_sinks,
                        },
                        None => nvngx_sys::NVSDK_NGX_LoggingInfo::default(),
                    },
                };
                &c_common_info
            }
        };

        Result::from(unsafe {
            dispatch.NVSDK_NGX_VULKAN_Init_with_ProjectID(
                project_id.as_ptr(),
                engine_type,
                engine_version.as_ptr(),
                application_data_path.as_ptr().cast(),
                instance.handle(),
                physical_device,
                logical_device,
                entry.static_fn().get_instance_proc_addr,
                instance.fp_v1_0().get_device_proc_addr,
                common_info_ptr,
                nvngx_sys::NVSDK_NGX_Version::NVSDK_NGX_Version_API,
            )
        })
        .map(|_| Self {
            device: logical_device,
            dispatch,
        })
    }

    fn shutdown(&self) -> Result {
        unsafe { self.dispatch.NVSDK_NGX_VULKAN_Shutdown1(self.device) }.into()
    }

    /// Get a feature parameter set populated with NGX and feature
    /// capabilities.
    pub fn get_capability_parameters(&self) -> Result<FeatureParameters> {
        FeatureParameters::get_capability_parameters(self.dispatch.clone())
    }

    /// Creates a new [`Feature`] with the logical device used to create
    /// this [`System`].
    pub fn create_feature(
        &self,
        command_buffer: vk::CommandBuffer,
        feature_type: nvngx_sys::NVSDK_NGX_Feature,
        parameters: Option<FeatureParameters>,
    ) -> Result<Feature> {
        let parameters = match parameters {
            Some(p) => p,
            None => self.get_capability_parameters()?,
        };
        Feature::new(
            self.dispatch.clone(),
            self.device,
            command_buffer,
            feature_type,
            parameters,
        )
    }

    /// Creates a [`SuperSamplingFeature`] (or "DLSS").
    pub fn create_super_sampling_feature(
        &self,
        command_buffer: vk::CommandBuffer,
        feature_parameters: FeatureParameters,
        create_parameters: SuperSamplingCreateParameters,
    ) -> Result<SuperSamplingFeature> {
        Feature::new_super_sampling(
            self.dispatch.clone(),
            self.device,
            command_buffer,
            feature_parameters,
            create_parameters,
        )
    }

    /// Creates a Frame Generation [`Feature`].
    pub fn create_frame_generation_feature(
        &self,
        command_buffer: vk::CommandBuffer,
        feature_parameters: FeatureParameters,
    ) -> Result<Feature> {
        Feature::new_frame_generation(
            self.dispatch.clone(),
            self.device,
            command_buffer,
            feature_parameters,
        )
    }

    /// Creates a [`RayReconstructionFeature`].
    pub fn create_ray_reconstruction_feature(
        &self,
        command_buffer: vk::CommandBuffer,
        feature_parameters: FeatureParameters,
        create_parameters: RayReconstructionCreateParameters,
    ) -> Result<RayReconstructionFeature> {
        Feature::new_ray_reconstruction(
            self.dispatch.clone(),
            self.device,
            command_buffer,
            feature_parameters,
            create_parameters,
        )
    }
}

impl Drop for System {
    fn drop(&mut self) {
        if let Err(e) = self.shutdown() {
            log::error!("Couldn't shutdown the NGX system {self:?}: {e}");
        }
    }
}

/// A mode that a vulkan resource might have.
#[derive(Default, Debug, Copy, Clone)]
pub enum VkResourceMode {
    /// Indicates that the resource can only be read.
    #[default]
    Readable,
    /// Indicates that the resource can be written to.
    Writable,
}

/// A struct, objects of which should be hold by
/// [`SuperSamplingEvaluationParameters`] for feature evaluation.
#[derive(Debug, Default, Copy, Clone)]
pub struct VkBufferResourceDescription {
    /// The buffer!
    pub buffer: vk::Buffer,
    /// The size of the buffer in bytes.
    pub size_in_bytes: usize,
    /// The mode this resource has.
    pub mode: VkResourceMode,
}

/// A struct, objects of which should be hold by
/// [`SuperSamplingEvaluationParameters`] for feature evaluation.
#[derive(Debug, Default, Copy, Clone)]
pub struct VkImageResourceDescription {
    /// The image view.
    pub image_view: vk::ImageView,
    /// The image.
    pub image: vk::Image,
    /// The subresource range.
    pub subresource_range: vk::ImageSubresourceRange,
    /// The format.
    pub format: vk::Format,
    /// The width of the image.
    pub width: u32,
    /// The height of the image.
    pub height: u32,
    /// The mode this resource has.
    pub mode: VkResourceMode,
}

impl VkImageResourceDescription {
    /// Sets the [`mode`](Self::mode) to [`VkResourceMode::Writable`].
    pub fn set_writable(&mut self) {
        self.mode = VkResourceMode::Writable;
    }
}

impl From<VkImageResourceDescription> for NVSDK_NGX_Resource_VK {
    fn from(value: VkImageResourceDescription) -> Self {
        let vk_image_subresource_range = vk::ImageSubresourceRange {
            aspect_mask: value.subresource_range.aspect_mask,
            base_mip_level: value.subresource_range.base_mip_level,
            base_array_layer: value.subresource_range.base_array_layer,
            level_count: value.subresource_range.level_count,
            layer_count: value.subresource_range.layer_count,
        };

        let image_view_info = NVSDK_NGX_ImageViewInfo_VK {
            ImageView: value.image_view,
            Image: value.image,
            SubresourceRange: vk_image_subresource_range,
            Format: value.format,
            Width: value.width,
            Height: value.height,
        };

        // Cannot use a Rust `union` constructor because bindgen doesn't know
        // our `Vk*` types anymore and wraps them in __BindgenUnionField:
        // https://github.com/rust-lang/rust-bindgen/issues/2187#issuecomment-3048892937
        let mut image_resource = NVSDK_NGX_Resource_VK__bindgen_ty_1::default();
        unsafe { *image_resource.ImageViewInfo.as_mut() = image_view_info }

        Self {
            Resource: image_resource,
            Type: NVSDK_NGX_Resource_VK_Type::NVSDK_NGX_RESOURCE_VK_TYPE_VK_IMAGEVIEW,
            ReadWrite: matches!(value.mode, VkResourceMode::Writable),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn features() {
        // TODO: initialise vulkan and be able to do this.
    }

    #[cfg(feature = "linked")]
    #[test]
    fn get_required_extensions() {
        assert!(super::RequiredExtensions::get().is_ok());
    }
}
