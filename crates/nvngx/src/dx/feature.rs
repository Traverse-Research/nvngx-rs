//! DX12-specific feature construction and evaluation.

use super::*;

impl FeatureParameters {
    /// Allocates a new parameter set via the DX12 backend.
    pub fn new_dx(&self) -> Result<Self> {
        let mut ptr: *mut nvngx_sys::NVSDK_NGX_Parameter = std::ptr::null_mut();
        Result::from(unsafe {
            nvngx_sys::dx::NVSDK_NGX_D3D12_AllocateParameters(&mut ptr as *mut _)
        })
        .map(|_| Self::from_raw(ptr, nvngx_sys::dx::NVSDK_NGX_D3D12_DestroyParameters))
    }

    /// Gets a capability parameter set populated with NGX and feature
    /// capabilities, via the DX12 backend.
    pub fn get_capability_parameters_dx() -> Result<Self> {
        let mut ptr: *mut nvngx_sys::NVSDK_NGX_Parameter = std::ptr::null_mut();
        Result::from(unsafe {
            nvngx_sys::dx::NVSDK_NGX_D3D12_GetCapabilityParameters(&mut ptr as *mut _)
        })
        .map(|_| Self::from_raw(ptr, nvngx_sys::dx::NVSDK_NGX_D3D12_DestroyParameters))
    }

    /// Returns [`Ok`] if the DX12 capability parameters support super sampling.
    pub fn supports_super_sampling_dx() -> Result<()> {
        Self::get_capability_parameters_dx()?.supports_super_sampling()
    }

    /// Returns [`Ok`] if the DX12 capability parameters support ray reconstruction.
    pub fn supports_ray_reconstruction_dx() -> Result<()> {
        Self::get_capability_parameters_dx()?.supports_ray_reconstruction()
    }
}

impl Feature {
    /// Creates a new feature via the DX12 backend.
    ///
    /// Note: unlike Vulkan, DX12's `CreateFeature` does not take a device parameter.
    pub fn new_dx(
        command_list: &Direct3D12::ID3D12GraphicsCommandList,
        feature_type: nvngx_sys::NVSDK_NGX_Feature,
        parameters: FeatureParameters,
    ) -> Result<Self> {
        let raw_cmd = command_list.as_raw() as *mut nvngx_sys::dx::ID3D12GraphicsCommandList;
        let mut handle = FeatureHandle::new(nvngx_sys::dx::NVSDK_NGX_D3D12_ReleaseFeature);
        Result::from(unsafe {
            nvngx_sys::dx::NVSDK_NGX_D3D12_CreateFeature(
                raw_cmd,
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

    /// Creates a new [`SuperSamplingFeature`] via the DX12 backend.
    pub fn new_super_sampling_dx(
        command_list: &Direct3D12::ID3D12GraphicsCommandList,
        parameters: FeatureParameters,
        mut super_sampling_create_parameters: SuperSamplingCreateParameters,
    ) -> Result<SuperSamplingFeature> {
        let feature_type = nvngx_sys::NVSDK_NGX_Feature::NVSDK_NGX_Feature_SuperSampling;
        let rendering_resolution = [
            super_sampling_create_parameters.0.Feature.InWidth,
            super_sampling_create_parameters.0.Feature.InHeight,
        ];
        let target_resolution = [
            super_sampling_create_parameters.0.Feature.InTargetWidth,
            super_sampling_create_parameters.0.Feature.InTargetHeight,
        ];
        let raw_cmd = command_list.as_raw() as *mut nvngx_sys::dx::ID3D12GraphicsCommandList;
        unsafe {
            let mut handle = FeatureHandle::new(nvngx_sys::dx::NVSDK_NGX_D3D12_ReleaseFeature);
            Result::from(nvngx_sys::dx_helpers::d3d12_create_dlss_ext(
                raw_cmd,
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

    /// Creates a Frame Generation [`Feature`] via the DX12 backend.
    pub fn new_frame_generation_dx(
        command_list: &Direct3D12::ID3D12GraphicsCommandList,
        parameters: FeatureParameters,
    ) -> Result<Self> {
        let feature_type = nvngx_sys::NVSDK_NGX_Feature::NVSDK_NGX_Feature_FrameGeneration;
        Self::new_dx(command_list, feature_type, parameters)
    }

    /// Creates a new [`RayReconstructionFeature`] via the DX12 backend.
    pub fn new_ray_reconstruction_dx(
        command_list: &Direct3D12::ID3D12GraphicsCommandList,
        parameters: FeatureParameters,
        mut ray_reconstruction_create_parameters: RayReconstructionCreateParameters,
    ) -> Result<RayReconstructionFeature> {
        let feature_type = nvngx_sys::NVSDK_NGX_Feature::NVSDK_NGX_Feature_RayReconstruction;
        let rendering_resolution = [
            ray_reconstruction_create_parameters.0.InWidth,
            ray_reconstruction_create_parameters.0.InHeight,
        ];
        let target_resolution = [
            ray_reconstruction_create_parameters.0.InTargetWidth,
            ray_reconstruction_create_parameters.0.InTargetHeight,
        ];
        let raw_cmd = command_list.as_raw() as *mut nvngx_sys::dx::ID3D12GraphicsCommandList;
        unsafe {
            let mut handle = FeatureHandle::new(nvngx_sys::dx::NVSDK_NGX_D3D12_ReleaseFeature);
            Result::from(nvngx_sys::dx_helpers::d3d12_create_dlssd_ext(
                raw_cmd,
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

    /// Returns the number of bytes needed for the scratch buffer (DX12).
    pub fn get_scratch_buffer_size_dx(&self) -> Result<usize> {
        let mut size = 0usize;
        Result::from(unsafe {
            nvngx_sys::dx::NVSDK_NGX_D3D12_GetScratchBufferSize(
                self.feature_type,
                self.parameters.ptr as _,
                &mut size as *mut _,
            )
        })
        .map(|_| size)
    }

    /// Evaluates the feature via the DX12 backend.
    pub fn evaluate_dx(&self, command_list: &Direct3D12::ID3D12GraphicsCommandList) -> Result {
        let raw_cmd = command_list.as_raw() as *mut nvngx_sys::dx::ID3D12GraphicsCommandList;
        unsafe {
            nvngx_sys::dx::NVSDK_NGX_D3D12_EvaluateFeature_C(
                raw_cmd,
                self.handle.ptr,
                self.parameters.ptr,
                Some(crate::common::feature_progress_callback),
            )
        }
        .into()
    }
}
