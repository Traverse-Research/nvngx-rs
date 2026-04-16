//! Vulkan-specific feature construction and evaluation.

use super::*;

impl FeatureParameters {
    /// Allocates a new parameter set via the Vulkan backend.
    pub fn new_vk(&self) -> Result<Self> {
        let mut ptr: *mut nvngx_sys::NVSDK_NGX_Parameter = std::ptr::null_mut();
        Result::from(unsafe { nvngx_sys::NVSDK_NGX_VULKAN_AllocateParameters(&mut ptr as *mut _) })
            .map(|_| Self::from_raw(ptr, nvngx_sys::NVSDK_NGX_VULKAN_DestroyParameters))
    }

    /// Gets a capability parameter set populated with NGX and feature
    /// capabilities, via the Vulkan backend.
    pub fn get_capability_parameters_vk() -> Result<Self> {
        let mut ptr: *mut nvngx_sys::NVSDK_NGX_Parameter = std::ptr::null_mut();
        Result::from(unsafe {
            nvngx_sys::NVSDK_NGX_VULKAN_GetCapabilityParameters(&mut ptr as *mut _)
        })
        .map(|_| Self::from_raw(ptr, nvngx_sys::NVSDK_NGX_VULKAN_DestroyParameters))
    }

    /// Returns [`Ok`] if the Vulkan capability parameters support super sampling.
    pub fn supports_super_sampling_vk() -> Result<()> {
        Self::get_capability_parameters_vk()?.supports_super_sampling()
    }

    /// Returns [`Ok`] if the Vulkan capability parameters support ray reconstruction.
    pub fn supports_ray_reconstruction_vk() -> Result<()> {
        Self::get_capability_parameters_vk()?.supports_ray_reconstruction()
    }
}

impl Feature {
    /// Creates a new feature via the Vulkan backend.
    pub fn new_vk(
        device: vk::Device,
        command_buffer: vk::CommandBuffer,
        feature_type: nvngx_sys::NVSDK_NGX_Feature,
        parameters: FeatureParameters,
    ) -> Result<Self> {
        let mut handle = FeatureHandle::new(nvngx_sys::NVSDK_NGX_VULKAN_ReleaseFeature);
        Result::from(unsafe {
            nvngx_sys::NVSDK_NGX_VULKAN_CreateFeature1(
                device,
                command_buffer,
                feature_type,
                parameters.ptr,
                &mut handle.ptr as *mut _,
            )
        })
        .map(|_| Self {
            handle: handle.into(),
            feature_type,
            parameters: parameters.into(),
        })
    }

    /// Creates a new [`SuperSamplingFeature`] via the Vulkan backend.
    pub fn new_super_sampling_vk(
        device: vk::Device,
        command_buffer: vk::CommandBuffer,
        parameters: FeatureParameters,
        mut super_sampling_create_parameters: SuperSamplingCreateParameters,
    ) -> Result<SuperSamplingFeature> {
        let feature_type = NVSDK_NGX_Feature::NVSDK_NGX_Feature_SuperSampling;
        let rendering_resolution = vk::Extent2D::default()
            .width(super_sampling_create_parameters.0.Feature.InWidth)
            .height(super_sampling_create_parameters.0.Feature.InHeight);
        let target_resolution = vk::Extent2D::default()
            .width(super_sampling_create_parameters.0.Feature.InTargetWidth)
            .height(super_sampling_create_parameters.0.Feature.InTargetHeight);
        unsafe {
            let mut handle = FeatureHandle::new(nvngx_sys::NVSDK_NGX_VULKAN_ReleaseFeature);
            Result::from(nvngx_sys::helpers::vulkan_create_dlss_ext1(
                device,
                command_buffer,
                1,
                1,
                &mut handle.ptr as *mut _,
                parameters.ptr,
                &mut super_sampling_create_parameters.0 as *mut _,
            ))
            .and_then(|_| {
                SuperSamplingFeature::new(
                    Self {
                        handle: handle.into(),
                        feature_type,
                        parameters: parameters.into(),
                    },
                    rendering_resolution,
                    target_resolution,
                )
            })
        }
    }

    /// Creates a Frame Generation [`Feature`] via the Vulkan backend.
    pub fn new_frame_generation_vk(
        device: vk::Device,
        command_buffer: vk::CommandBuffer,
        parameters: FeatureParameters,
    ) -> Result<Self> {
        let feature_type = NVSDK_NGX_Feature::NVSDK_NGX_Feature_FrameGeneration;
        Self::new_vk(device, command_buffer, feature_type, parameters)
    }

    /// Creates a new [`RayReconstructionFeature`] via the Vulkan backend.
    pub fn new_ray_reconstruction_vk(
        device: vk::Device,
        command_buffer: vk::CommandBuffer,
        parameters: FeatureParameters,
        mut ray_reconstruction_create_parameters: RayReconstructionCreateParameters,
    ) -> Result<RayReconstructionFeature> {
        let feature_type = NVSDK_NGX_Feature::NVSDK_NGX_Feature_RayReconstruction;
        let rendering_resolution = vk::Extent2D::default()
            .width(ray_reconstruction_create_parameters.0.InWidth)
            .height(ray_reconstruction_create_parameters.0.InHeight);
        let target_resolution = vk::Extent2D::default()
            .width(ray_reconstruction_create_parameters.0.InTargetWidth)
            .height(ray_reconstruction_create_parameters.0.InTargetHeight);

        unsafe {
            let mut handle = FeatureHandle::new(nvngx_sys::NVSDK_NGX_VULKAN_ReleaseFeature);
            Result::from(nvngx_sys::helpers::vulkan_create_dlssd_ext1(
                device,
                command_buffer,
                1,
                1,
                &mut handle.ptr as *mut _,
                parameters.ptr,
                &mut ray_reconstruction_create_parameters.0 as *mut _,
            ))
            .and_then(|_| {
                RayReconstructionFeature::new(
                    Self {
                        handle: handle.into(),
                        feature_type,
                        parameters: parameters.into(),
                    },
                    rendering_resolution,
                    target_resolution,
                )
            })
        }
    }

    /// Returns the number of bytes needed for the scratch buffer (Vulkan).
    pub fn get_scratch_buffer_size_vk(&self) -> Result<usize> {
        let mut size = 0usize;
        Result::from(unsafe {
            nvngx_sys::NVSDK_NGX_VULKAN_GetScratchBufferSize(
                self.feature_type,
                self.parameters.ptr as _,
                &mut size as *mut _,
            )
        })
        .map(|_| size)
    }

    /// Evaluates the feature via the Vulkan backend.
    pub fn evaluate_vk(&self, command_buffer: vk::CommandBuffer) -> Result {
        unsafe {
            nvngx_sys::NVSDK_NGX_VULKAN_EvaluateFeature_C(
                command_buffer,
                self.handle.ptr,
                self.parameters.ptr,
                Some(crate::common::feature_progress_callback),
            )
        }
        .into()
    }
}
