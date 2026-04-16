//! DX12 bindings to NGX.

use nvngx_sys::{NVSDK_NGX_Feature, Result};
use windows::core::Interface;
use windows::Win32::Graphics::{Direct3D12, Dxgi};

use crate::common::{
    Feature, FeatureHandle, FeatureParameters, FeatureRequirement,
    RayReconstructionCreateParameters, SuperSamplingCreateParameters,
};

mod feature;
pub mod super_sampling;
pub use super_sampling::*;
pub mod ray_reconstruction;
pub use ray_reconstruction::*;

/// Identifies system requirements to support a given NGX feature.
///
/// Per `nvsdk_ngx.h`: *"NVSDK_NGX_Init does NOT need to be called before
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
///
/// `adapter` is the DXGI adapter the application intends to run on.
pub fn get_feature_requirements(
    adapter: &Dxgi::IDXGIAdapter,
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
    let raw_adapter = adapter.as_raw() as *mut nvngx_sys::dx::IDXGIAdapter;
    let mut out = nvngx_sys::NVSDK_NGX_FeatureRequirement::default();
    Result::from(unsafe {
        nvngx_sys::dx::NVSDK_NGX_D3D12_GetFeatureRequirements(raw_adapter, &info, &mut out)
    })
    .map(|()| FeatureRequirement::from_raw(out))
}

/// NVIDIA NGX system (DX12).
///
/// Holds an owning [`Direct3D12::ID3D12Device`] (refcount bump on
/// construction) to keep the device alive for the lifetime of the
/// [`System`]: NGX may or may not bump the device's refcount internally
/// between `Init` and `Shutdown1`, so the caller can't rely on that.
#[derive(Debug)]
pub struct System {
    device: Direct3D12::ID3D12Device,
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
        let common_info_storage = common_info.map(crate::common::CommonInfoStorage::new);

        Result::from(unsafe {
            nvngx_sys::dx::NVSDK_NGX_D3D12_Init_with_ProjectID(
                project_id.as_ptr(),
                engine_type,
                engine_version.as_ptr(),
                application_data_path.as_ptr().cast(),
                device.as_raw() as *mut nvngx_sys::dx::ID3D12Device,
                common_info_storage
                    .as_ref()
                    .map_or(std::ptr::null(), |s| s.as_ref()),
                nvngx_sys::NVSDK_NGX_Version::NVSDK_NGX_Version_API,
            )
        })
        .map(|()| Self {
            device: device.clone(),
        })
    }

    fn shutdown(&self) -> Result {
        unsafe {
            nvngx_sys::dx::NVSDK_NGX_D3D12_Shutdown1(
                self.device.as_raw() as *mut nvngx_sys::dx::ID3D12Device
            )
        }
        .into()
    }

    /// Allocates a new [`FeatureParameters`] map pre-populated with NGX
    /// capabilities and available features.
    ///
    /// Wraps [`nvngx_sys::dx::NVSDK_NGX_D3D12_GetCapabilityParameters`]. The
    /// upstream header states this *"may only be called after a successful
    /// call into NVSDK_NGX_Init"* — taking `&self` of [`System`] makes that
    /// requirement type-enforced. For Init-free per-feature support checks,
    /// use [`get_feature_requirements()`] instead.
    ///
    /// May return [`nvngx_sys::NVSDK_NGX_Result::NVSDK_NGX_Result_FAIL_OutOfDate`]
    /// on older drivers that don't support the API.
    pub fn get_capability_parameters(&self) -> Result<FeatureParameters> {
        let mut ptr: *mut nvngx_sys::NVSDK_NGX_Parameter = std::ptr::null_mut();
        Result::from(unsafe {
            nvngx_sys::dx::NVSDK_NGX_D3D12_GetCapabilityParameters(&mut ptr as *mut _)
        })
        .map(|()| unsafe {
            FeatureParameters::from_raw(ptr, nvngx_sys::dx::NVSDK_NGX_D3D12_DestroyParameters)
        })
    }

    /// Creates a new [`Feature`].
    pub fn create_feature(
        &self,
        command_list: &Direct3D12::ID3D12GraphicsCommandList,
        feature_type: NVSDK_NGX_Feature,
        parameters: Option<FeatureParameters>,
    ) -> Result<Feature> {
        let parameters = match parameters {
            Some(p) => p,
            None => self.get_capability_parameters()?,
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
