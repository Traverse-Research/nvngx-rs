//! Vulkan bindings to NGX.

use ash::vk;
use nvngx_sys::vk::{
    NVSDK_NGX_ImageViewInfo_VK, NVSDK_NGX_Resource_VK, NVSDK_NGX_Resource_VK_Type,
    NVSDK_NGX_Resource_VK__bindgen_ty_1,
};
use nvngx_sys::{NVSDK_NGX_Coordinates, NVSDK_NGX_Dimensions, NVSDK_NGX_Feature, Result};

// Bring common types into scope so submodules (feature, super_sampling,
// ray_reconstruction) can access them via `use super::*`.
use crate::common::{
    Feature, FeatureHandle, FeatureParameters, FeatureRequirement,
    RayReconstructionCreateParameters, SuperSamplingCreateParameters,
};

mod feature;
pub mod super_sampling;
pub use super_sampling::*;
pub mod ray_reconstruction;
pub use ray_reconstruction::*;

/// Vulkan extensions required for the NVIDIA NGX operation.
#[derive(Debug, Clone)]
pub struct RequiredExtensions {
    /// Vulkan device extensions required for NVIDIA NGX.
    pub device: Vec<std::ffi::CString>,
    /// Vulkan instance extensions required for NVIDIA NGX.
    pub instance: Vec<std::ffi::CString>,
}

impl RequiredExtensions {
    /// Returns a list of required vulkan extensions for NGX to work.
    pub fn get() -> Result<Self> {
        let mut instance_extensions = std::ptr::null_mut();
        let mut device_extensions = std::ptr::null_mut();
        let mut instance_count = 0u32;
        let mut device_count = 0u32;
        Result::from(unsafe {
            nvngx_sys::vk::NVSDK_NGX_VULKAN_RequiredExtensions(
                &mut instance_count,
                &mut instance_extensions,
                &mut device_count,
                &mut device_extensions,
            )
        })?;

        let instance = (0..instance_count)
            .map(|i| unsafe {
                std::ffi::CStr::from_ptr(*instance_extensions.add(i as usize)).to_owned()
            })
            .collect();

        let device = (0..device_count)
            .map(|i| unsafe {
                std::ffi::CStr::from_ptr(*device_extensions.add(i as usize)).to_owned()
            })
            .collect();

        Ok(Self { device, instance })
    }
}

/// Identifies system requirements to support a given NGX feature.
///
/// Per `nvsdk_ngx_vk.h`: *"NVSDK_NGX_Init does NOT need to be called before
/// calling this function. Applications may wish to use this function to
/// determine whether a desired feature is supported before initializing the
/// complete SDK."* This means a caller can probe DLSS / Ray Reconstruction
/// support without loading `nvngx_dlss.dll` or any other feature DLL — the
/// DLL load only happens later in [`System::new()`].
///
/// `feature_id`, `project_id`, `engine_version`, `application_data_path`, and
/// `common_info` should match the values you intend to pass to
/// [`System::new()`] later (NGX uses them to locate feature DLLs and select
/// the right binary).
pub fn get_feature_requirements(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    feature_id: NVSDK_NGX_Feature,
    project_id: Option<uuid::Uuid>,
    engine_version: &str,
    application_data_path: &std::path::Path,
    common_info: Option<&crate::common::FeatureCommonInfo<'_>>,
) -> Result<FeatureRequirement> {
    let project_id =
        std::ffi::CString::new(project_id.unwrap_or_else(uuid::Uuid::new_v4).to_string()).unwrap();
    let engine_version = std::ffi::CString::new(engine_version).unwrap();
    let application_data_path =
        widestring::WideString::from_str(application_data_path.to_str().unwrap());
    let common_info_storage = common_info.map(crate::common::CommonInfoStorage::new);

    let identifier = nvngx_sys::NVSDK_NGX_Application_Identifier {
        IdentifierType: nvngx_sys::NVSDK_NGX_Application_Identifier_Type::NVSDK_NGX_Application_Identifier_Type_Project_Id,
        v: nvngx_sys::v {
            ProjectDesc: nvngx_sys::NVSDK_NGX_ProjectIdDescription {
                ProjectId: project_id.as_ptr(),
                EngineType: nvngx_sys::NVSDK_NGX_EngineType::NVSDK_NGX_ENGINE_TYPE_CUSTOM,
                EngineVersion: engine_version.as_ptr(),
            },
        },
    };
    let info = nvngx_sys::NVSDK_NGX_FeatureDiscoveryInfo {
        SDKVersion: nvngx_sys::NVSDK_NGX_Version::NVSDK_NGX_Version_API,
        FeatureID: feature_id,
        Identifier: identifier,
        ApplicationDataPath: application_data_path.as_ptr().cast(),
        FeatureInfo: common_info_storage
            .as_ref()
            .map_or(std::ptr::null(), |s| s.as_ref()),
    };
    let mut out = nvngx_sys::NVSDK_NGX_FeatureRequirement::default();
    Result::from(unsafe {
        nvngx_sys::vk::NVSDK_NGX_VULKAN_GetFeatureRequirements(
            instance.handle(),
            physical_device,
            &info,
            &mut out,
        )
    })
    .map(|()| FeatureRequirement::from_raw(out))
}

/// NVIDIA NGX system (Vulkan).
#[repr(transparent)]
#[derive(Debug)]
pub struct System {
    device: vk::Device,
}

impl System {
    /// Creates a new NVIDIA NGX system.
    ///
    /// `common_info` carries optional feature-DLL search paths and a logging
    /// callback configuration; pass [`None`] to preserve the NGX defaults (no
    /// extra search paths, logging controlled by the registry on Windows).
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
        let engine_type = nvngx_sys::NVSDK_NGX_EngineType::NVSDK_NGX_ENGINE_TYPE_CUSTOM;
        let project_id =
            std::ffi::CString::new(project_id.unwrap_or_else(uuid::Uuid::new_v4).to_string())
                .unwrap();
        let engine_version = std::ffi::CString::new(engine_version).unwrap();
        let application_data_path =
            widestring::WideString::from_str(application_data_path.to_str().unwrap());
        let common_info_storage = common_info.map(crate::common::CommonInfoStorage::new);

        Result::from(unsafe {
            nvngx_sys::vk::NVSDK_NGX_VULKAN_Init_with_ProjectID(
                project_id.as_ptr(),
                engine_type,
                engine_version.as_ptr(),
                application_data_path.as_ptr().cast(),
                instance.handle(),
                physical_device,
                logical_device,
                entry.static_fn().get_instance_proc_addr,
                instance.fp_v1_0().get_device_proc_addr,
                common_info_storage
                    .as_ref()
                    .map_or(std::ptr::null(), |s| s.as_ref()),
                nvngx_sys::NVSDK_NGX_Version::NVSDK_NGX_Version_API,
            )
        })
        .map(|()| Self {
            device: logical_device,
        })
    }

    fn shutdown(&self) -> Result {
        unsafe { nvngx_sys::vk::NVSDK_NGX_VULKAN_Shutdown1(self.device) }.into()
    }

    /// Allocates a new [`FeatureParameters`] map pre-populated with NGX
    /// capabilities and available features.
    ///
    /// Wraps [`nvngx_sys::vk::NVSDK_NGX_VULKAN_GetCapabilityParameters`]. The
    /// upstream header states this *"may only be called after a successful
    /// call into NVSDK_NGX_Init"* — taking `&self` of [`System`] makes that
    /// requirement type-enforced. For Init-free per-feature support checks,
    /// use [`get_feature_requirements()`] instead.
    ///
    /// Parameter maps allocated this way pre-populate NGX capabilities (e.g.
    /// `NVSDK_NGX_Parameter_SuperSampling_Available`) but carry an allocation
    /// overhead — use [`FeatureParameters::new_vk()`] for empty maps when
    /// querying capabilities is not needed.
    ///
    /// May return [`nvngx_sys::NVSDK_NGX_Result::NVSDK_NGX_Result_FAIL_OutOfDate`]
    /// on older drivers that don't support the API.
    pub fn get_capability_parameters(&self) -> Result<FeatureParameters> {
        let mut ptr: *mut nvngx_sys::NVSDK_NGX_Parameter = std::ptr::null_mut();
        Result::from(unsafe {
            nvngx_sys::vk::NVSDK_NGX_VULKAN_GetCapabilityParameters(&mut ptr as *mut _)
        })
        .map(|()| unsafe {
            FeatureParameters::from_raw(ptr, nvngx_sys::vk::NVSDK_NGX_VULKAN_DestroyParameters)
        })
    }

    /// Creates a new [`Feature`] with the logical device used to create
    /// this [`System`].
    pub fn create_feature(
        &self,
        command_buffer: vk::CommandBuffer,
        feature_type: NVSDK_NGX_Feature,
        parameters: Option<FeatureParameters>,
    ) -> Result<Feature> {
        let parameters = match parameters {
            Some(p) => p,
            None => self.get_capability_parameters()?,
        };
        Feature::new_vk(self.device, command_buffer, feature_type, parameters)
    }

    /// Creates a [`SuperSamplingFeature`] (or "DLSS").
    pub fn create_super_sampling_feature(
        &self,
        command_buffer: vk::CommandBuffer,
        feature_parameters: FeatureParameters,
        create_parameters: SuperSamplingCreateParameters,
    ) -> Result<SuperSamplingFeature> {
        Feature::new_super_sampling_vk(
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
        Feature::new_frame_generation_vk(self.device, command_buffer, feature_parameters)
    }

    /// Creates a [`RayReconstructionFeature`].
    pub fn create_ray_reconstruction_feature(
        &self,
        command_buffer: vk::CommandBuffer,
        feature_parameters: FeatureParameters,
        create_parameters: RayReconstructionCreateParameters,
    ) -> Result<RayReconstructionFeature> {
        Feature::new_ray_reconstruction_vk(
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
    /// Sets the [`Self::mode`] to [`VkResourceMode::Writable`].
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
    fn get_required_extensions() {
        assert!(super::RequiredExtensions::get().is_ok());
    }
}
