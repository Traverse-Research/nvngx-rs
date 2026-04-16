//! DX12 bindings to NGX.

use nvngx_sys::Result;
use windows::core::Interface;
use windows::Win32::Graphics::Direct3D12;

use crate::common::{
    Feature, FeatureHandle, FeatureParameters, RayReconstructionCreateParameters,
    SuperSamplingCreateParameters,
};

mod feature;
pub mod super_sampling;
pub use super_sampling::*;
pub mod ray_reconstruction;
pub use ray_reconstruction::*;

/// NVIDIA NGX system (DX12).
#[derive(Debug)]
pub struct System {
    /// Raw device pointer, stored for `Shutdown1`.
    device: *mut nvngx_sys::dx::ID3D12Device,
}

impl System {
    /// Creates a new NVIDIA NGX system backed by a DX12 device.
    pub fn new(
        project_id: Option<uuid::Uuid>,
        engine_version: &str,
        application_data_path: &std::path::Path,
        device: &Direct3D12::ID3D12Device,
        common_info: Option<&crate::common::FeatureCommonInfo<'_>>,
    ) -> Result<Self> {
        let engine_type = nvngx_sys::NVSDK_NGX_EngineType::NVSDK_NGX_ENGINE_TYPE_CUSTOM;
        let project_id =
            std::ffi::CString::new(project_id.unwrap_or_else(uuid::Uuid::new_v4).to_string())
                .unwrap();
        let engine_version = std::ffi::CString::new(engine_version).unwrap();
        let application_data_path =
            widestring::WideString::from_str(application_data_path.to_str().unwrap());

        let raw_device = device.as_raw() as *mut nvngx_sys::dx::ID3D12Device;

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
            nvngx_sys::dx::NVSDK_NGX_D3D12_Init_with_ProjectID(
                project_id.as_ptr(),
                engine_type,
                engine_version.as_ptr(),
                application_data_path.as_ptr().cast(),
                raw_device,
                common_info_ptr,
                nvngx_sys::NVSDK_NGX_Version::NVSDK_NGX_Version_API,
            )
        })
        .map(|_| Self { device: raw_device })
    }

    fn shutdown(&self) -> Result {
        unsafe { nvngx_sys::dx::NVSDK_NGX_D3D12_Shutdown1(self.device) }.into()
    }

    /// Creates a new [`Feature`].
    pub fn create_feature(
        &self,
        command_list: &Direct3D12::ID3D12GraphicsCommandList,
        feature_type: nvngx_sys::NVSDK_NGX_Feature,
        parameters: Option<FeatureParameters>,
    ) -> Result<Feature> {
        let parameters = match parameters {
            Some(p) => p,
            None => FeatureParameters::get_capability_parameters_dx()?,
        };
        Feature::new_dx(command_list, feature_type, parameters)
    }

    /// Creates a [`SuperSamplingFeature`] (or "DLSS").
    pub fn create_super_sampling_feature(
        &self,
        command_list: &Direct3D12::ID3D12GraphicsCommandList,
        feature_parameters: FeatureParameters,
        create_parameters: SuperSamplingCreateParameters,
    ) -> Result<SuperSamplingFeature> {
        Feature::new_super_sampling_dx(command_list, feature_parameters, create_parameters)
    }

    /// Creates a Frame Generation [`Feature`].
    pub fn create_frame_generation_feature(
        &self,
        command_list: &Direct3D12::ID3D12GraphicsCommandList,
        feature_parameters: FeatureParameters,
    ) -> Result<Feature> {
        Feature::new_frame_generation_dx(command_list, feature_parameters)
    }

    /// Creates a [`RayReconstructionFeature`].
    pub fn create_ray_reconstruction_feature(
        &self,
        command_list: &Direct3D12::ID3D12GraphicsCommandList,
        feature_parameters: FeatureParameters,
        create_parameters: RayReconstructionCreateParameters,
    ) -> Result<RayReconstructionFeature> {
        Feature::new_ray_reconstruction_dx(command_list, feature_parameters, create_parameters)
    }
}

impl Drop for System {
    fn drop(&mut self) {
        if let Err(e) = self.shutdown() {
            log::error!("Couldn't shutdown the NGX DX12 system: {e}");
        }
    }
}
