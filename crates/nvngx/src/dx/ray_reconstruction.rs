//! DX12-specific ray reconstruction evaluation and feature types.

use nvngx_sys::{NVSDK_NGX_Coordinates, NVSDK_NGX_Dimensions};
use windows::core::Interface;
use windows::Win32::Graphics::Direct3D12;

use super::*;

/// A helpful type alias for [`RayReconstructionFeature`] to quickly mention "DLSS-RR".
pub type RRFeature = RayReconstructionFeature;

/// Evaluation parameters for [`RayReconstructionFeature`] (DX12-specific).
#[derive(Debug)]
pub struct RayReconstructionEvaluationParameters {
    /// The DX12 DLSSD evaluation parameters struct.
    parameters: nvngx_sys::dx::NVSDK_NGX_D3D12_DLSSD_Eval_Params,
}

impl Default for RayReconstructionEvaluationParameters {
    fn default() -> Self {
        Self {
            parameters: unsafe { std::mem::zeroed() },
        }
    }
}

impl RayReconstructionEvaluationParameters {
    /// Creates a new set of evaluation parameters for [`RayReconstructionFeature`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the color input parameter (the image to upscale).
    pub fn set_color_input(&mut self, resource: &Direct3D12::ID3D12Resource) {
        self.parameters.pInColor = resource.as_raw() as *mut nvngx_sys::dx::ID3D12Resource;
    }

    /// Sets the color output (the upscaled image).
    pub fn set_color_output(&mut self, resource: &Direct3D12::ID3D12Resource) {
        self.parameters.pInOutput = resource.as_raw() as *mut nvngx_sys::dx::ID3D12Resource;
    }

    /// Sets the motion vectors.
    /// In case the `scale` argument is omitted, the `1.0f32` scaling is used.
    pub fn set_motions_vectors(
        &mut self,
        resource: &Direct3D12::ID3D12Resource,
        scale: Option<[f32; 2]>,
    ) {
        const DEFAULT_SCALING: [f32; 2] = [1.0f32, 1.0f32];

        self.parameters.pInMotionVectors = resource.as_raw() as *mut nvngx_sys::dx::ID3D12Resource;
        let scales = scale.unwrap_or(DEFAULT_SCALING);
        self.parameters.InMVScaleX = scales[0];
        self.parameters.InMVScaleY = scales[1];
    }

    /// Sets the depth buffer.
    pub fn set_depth_buffer(&mut self, resource: &Direct3D12::ID3D12Resource) {
        self.parameters.pInDepth = resource.as_raw() as *mut nvngx_sys::dx::ID3D12Resource;
    }

    /// Sets the jitter offsets (like TAA).
    pub fn set_jitter_offsets(&mut self, x: f32, y: f32) {
        self.parameters.InJitterOffsetX = x;
        self.parameters.InJitterOffsetY = y;
    }

    /// Sets/unsets the reset flag.
    pub fn set_reset(&mut self, should_reset: bool) {
        self.parameters.InReset = if should_reset { 1 } else { 0 };
    }

    /// Sets the rendering dimensions.
    pub fn set_rendering_dimensions(
        &mut self,
        rendering_offset: [u32; 2],
        rendering_size: [u32; 2],
    ) {
        self.parameters.InColorSubrectBase = NVSDK_NGX_Coordinates {
            X: rendering_offset[0],
            Y: rendering_offset[1],
        };
        self.parameters.InDepthSubrectBase = NVSDK_NGX_Coordinates {
            X: rendering_offset[0],
            Y: rendering_offset[1],
        };
        self.parameters.InTranslucencySubrectBase = NVSDK_NGX_Coordinates {
            X: rendering_offset[0],
            Y: rendering_offset[1],
        };
        self.parameters.InMVSubrectBase = NVSDK_NGX_Coordinates {
            X: rendering_offset[0],
            Y: rendering_offset[1],
        };
        self.parameters.InRenderSubrectDimensions = NVSDK_NGX_Dimensions {
            Width: rendering_size[0],
            Height: rendering_size[1],
        };
    }

    /// Returns the filled Ray Reconstruction parameters.
    pub(crate) fn get_rr_evaluation_parameters(
        &mut self,
    ) -> *mut nvngx_sys::dx::NVSDK_NGX_D3D12_DLSSD_Eval_Params {
        std::ptr::addr_of_mut!(self.parameters)
    }
}

/// A Ray Reconstruction (or "DLSS-RR") [`Feature`] (DX12).
#[derive(Debug)]
pub struct RayReconstructionFeature {
    feature: Feature,
    parameters: RayReconstructionEvaluationParameters,
    rendering_resolution: [u32; 2],
    target_resolution: [u32; 2],
}

impl RayReconstructionFeature {
    /// Creates a new [`RayReconstructionFeature`].
    pub fn new(
        feature: Feature,
        rendering_resolution: [u32; 2],
        target_resolution: [u32; 2],
    ) -> Result<Self> {
        if !feature.is_ray_reconstruction() {
            return Err(nvngx_sys::Error::Other(
                "Attempt to create a ray reconstruction feature with another feature.".to_owned(),
            ));
        }

        Ok(Self {
            feature,
            parameters: RayReconstructionEvaluationParameters::new(),
            rendering_resolution,
            target_resolution,
        })
    }

    /// Returns the inner [`Feature`].
    pub fn get_inner(&self) -> &Feature {
        &self.feature
    }

    /// Returns the inner [`Feature`] (mutable).
    pub fn get_inner_mut(&mut self) -> &mut Feature {
        &mut self.feature
    }

    /// Returns the rendering resolution `[width, height]`.
    pub const fn get_rendering_resolution(&self) -> [u32; 2] {
        self.rendering_resolution
    }

    /// Returns the target resolution `[width, height]`.
    pub const fn get_target_resolution(&self) -> [u32; 2] {
        self.target_resolution
    }

    /// See [`FeatureParameters::is_ray_reconstruction_initialised`].
    pub fn is_initialised(&self) -> bool {
        self.feature
            .get_parameters()
            .is_ray_reconstruction_initialised()
    }

    /// Returns the [`RayReconstructionEvaluationParameters`].
    pub fn get_evaluation_parameters_mut(&mut self) -> &mut RayReconstructionEvaluationParameters {
        &mut self.parameters
    }

    /// Evaluates the feature.
    pub fn evaluate(&mut self, command_list: &Direct3D12::ID3D12GraphicsCommandList) -> Result {
        let raw_cmd = command_list.as_raw() as *mut nvngx_sys::dx::ID3D12GraphicsCommandList;
        Result::from(unsafe {
            nvngx_sys::dx_helpers::d3d12_evaluate_dlssd_ext(
                raw_cmd,
                self.feature.handle.ptr,
                self.feature.parameters.ptr,
                self.parameters.get_rr_evaluation_parameters(),
            )
        })
    }
}
