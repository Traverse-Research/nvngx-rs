# Add DX12 Bindings

## Context

The `nvngx-rs` crate currently only supports Vulkan. DX12 support is needed. PR #38 on upstream attempts this but is over-engineered (trait abstractions, generic platform types, C helper files, `todo!()` stubs). We take inspiration from its structure but keep things concrete and simple.

The DLSS SDK (`v310.5.3`) ships DX12 declarations in `nvsdk_ngx.h`, `nvsdk_ngx_helpers.h`, `nvsdk_ngx_helpers_dlssd.h`, and `nvsdk_ngx_helpers_dlssg.h`. All DX12 functions are already in the linked `nvsdk_ngx` library.

## Approach

**No generics/traits.** Concrete DX12 types parallel to the existing Vulkan types. Code duplication for DX12 vs VK is fine since the APIs differ meaningfully (COM pointers vs Vulkan handles, different resource models).

**`#[cfg(windows)]` gating** on all DX12 code. The `windows` crate dependency is only pulled on Windows.

**Hand-written FFI** for DX12 (no bindgen). The DX12 API surface is small (~15 functions + a few structs). COM interfaces are opaque `*mut c_void` at the sys level.

---

## 1. `nvngx-sys` changes

### `Cargo.toml`
- Keep `ash` as a non-optional dependency (don't break existing users)
- Add `[target.'cfg(windows)'.dependencies]` for `windows` crate with `Win32_Graphics_Direct3D12` and `Win32_Graphics_Dxgi_Common` features

### `src/lib.rs`
- Add `#[cfg(windows)] pub mod dx;`

### NEW `src/dx.rs`
Hand-written module containing:

**Opaque COM types** (zero-sized structs, same as bindgen would generate):
- `ID3D12Device`, `ID3D12Resource`, `ID3D12GraphicsCommandList`, `IDXGIAdapter`, `IUnknown`

**Extern "C" function declarations** (the SDK API):
- `NVSDK_NGX_D3D12_Init_with_ProjectID`
- `NVSDK_NGX_D3D12_Shutdown1`
- `NVSDK_NGX_D3D12_AllocateParameters`
- `NVSDK_NGX_D3D12_GetCapabilityParameters`
- `NVSDK_NGX_D3D12_DestroyParameters`
- `NVSDK_NGX_D3D12_GetScratchBufferSize`
- `NVSDK_NGX_D3D12_CreateFeature`
- `NVSDK_NGX_D3D12_ReleaseFeature`
- `NVSDK_NGX_D3D12_EvaluateFeature_C`
- `NVSDK_NGX_D3D12_GetFeatureRequirements`
- `NVSDK_NGX_Parameter_SetD3d12Resource`
- `NVSDK_NGX_Parameter_GetD3d12Resource`

**DX12-specific structs** (repr(C), matching the C headers):
- `NVSDK_NGX_D3D12_Feature_Eval_Params`
- `NVSDK_NGX_D3D12_GBuffer`
- `NVSDK_NGX_D3D12_DLSS_Eval_Params`

Uses `use super::*` to get common NGX types from `bindings.rs`.

### NEW `src/dx_helpers.rs`
Pure Rust reimplementations of the `static inline` DX12 helper functions from the SDK headers, following the same pattern as `helpers.rs`:
- `d3d12_create_dlss_ext` (equivalent of `NGX_D3D12_CREATE_DLSS_EXT`)
- `d3d12_evaluate_dlss_ext` (equivalent of `NGX_D3D12_EVALUATE_DLSS_EXT`)

---

## 2. `nvngx` changes

### `Cargo.toml`
- Add `[target.'cfg(windows)'.dependencies]` for `windows` crate
- Keep `ash` as non-optional

### `src/lib.rs`
- Add `#[cfg(windows)] pub mod dx;`

### NEW `src/dx/mod.rs`
Parallel to `vk/mod.rs`:
- `System` struct holding `ID3D12Device` (from windows crate)
  - `new()` - calls `NVSDK_NGX_D3D12_Init_with_ProjectID`
  - `shutdown()` / `Drop` - calls `NVSDK_NGX_D3D12_Shutdown1`
  - `create_feature()`, `create_super_sampling_feature()`
- No `RequiredExtensions` equivalent (DX12 doesn't need this)

### NEW `src/dx/feature.rs`
Parallel to `vk/feature.rs`:
- `FeatureHandle` - wraps `*mut NVSDK_NGX_Handle`, calls `NVSDK_NGX_D3D12_ReleaseFeature` on drop
- `FeatureParameters` - wraps `*mut NVSDK_NGX_Parameter`
  - `new()` calls `NVSDK_NGX_D3D12_AllocateParameters`
  - `get_capability_parameters()` calls `NVSDK_NGX_D3D12_GetCapabilityParameters`
  - `release()` calls `NVSDK_NGX_D3D12_DestroyParameters`
  - All getters/setters identical to VK (they use the same NGX parameter functions)
- `Feature` struct with `handle`, `feature_type`, `parameters`
  - `new()` calls `NVSDK_NGX_D3D12_CreateFeature`
  - `new_super_sampling()` uses DX12 DLSS helper
  - `evaluate()` calls `NVSDK_NGX_D3D12_EvaluateFeature_C`

### `src/common.rs` (modified)
Move API-agnostic types here from `vk/super_sampling.rs`:
- `SuperSamplingOptimalSettings` + `get_optimal_settings()`
- `SuperSamplingCreateParameters` + `From<SuperSamplingOptimalSettings>`

These use only `nvngx_sys` common types (`NVSDK_NGX_Parameter`, `NVSDK_NGX_DLSS_Create_Params`, etc.) and the API-agnostic `helpers::dlss_get_optimal_settings()`. Both `vk` and `dx` modules re-export them.

### `src/vk/super_sampling.rs` (modified)
- Remove `SuperSamplingOptimalSettings` and `SuperSamplingCreateParameters` (moved to common)
- Keep `SuperSamplingEvaluationParameters` (VK-specific) and `SuperSamplingFeature`
- Re-export the common types via `pub use crate::common::{SuperSamplingOptimalSettings, SuperSamplingCreateParameters};`

### NEW `src/dx/super_sampling.rs`
- `SuperSamplingEvaluationParameters` - DX12-specific: uses `*mut ID3D12Resource` raw pointers, calls `SetD3d12Resource` for evaluation
- `SuperSamplingFeature` - uses `[u32; 2]` for resolutions instead of `vk::Extent2D`
- Re-exports `SuperSamplingOptimalSettings` and `SuperSamplingCreateParameters` from common

---

## Key files

| File | Action |
|------|--------|
| `crates/nvngx-sys/Cargo.toml` | Add windows dep |
| `crates/nvngx-sys/src/lib.rs` | Add `mod dx` |
| `crates/nvngx-sys/src/dx.rs` | **New** - FFI declarations |
| `crates/nvngx-sys/src/dx_helpers.rs` | **New** - Pure Rust DX12 helpers |
| `crates/nvngx/Cargo.toml` | Add windows dep |
| `crates/nvngx/src/lib.rs` | Add `mod dx` |
| `crates/nvngx/src/dx/mod.rs` | **New** - System |
| `crates/nvngx/src/dx/feature.rs` | **New** - FeatureHandle, FeatureParameters, Feature |
| `crates/nvngx/src/common.rs` | Move shared DLSS types here |
| `crates/nvngx/src/vk/super_sampling.rs` | Remove shared types (re-export from common) |
| `crates/nvngx/src/dx/super_sampling.rs` | **New** - DX12 DLSS evaluation + feature |

## Verification

- `cargo check` on Linux (DX12 modules gated behind `#[cfg(windows)]`, should be invisible)
- `cargo check --target x86_64-pc-windows-msvc` (if cross-compilation toolchain available)
- Confirm no changes to existing Vulkan API surface
