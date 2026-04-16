//! DX12-specific NGX bindings.
//!
//! The opaque types defined here are FFI placeholders for C forward
//! declarations in `nvsdk_ngx.h` — they are only meant to appear as the target
//! type of a raw pointer in a generated NGX function signature, never as
//! values. The high-level `nvngx` crate uses the corresponding `windows` crate
//! COM types in its public API and casts to the placeholders here at the FFI
//! boundary:
//!
//! ```ignore
//! use windows::core::Interface;
//! let raw: *mut ID3D12Device = device.as_raw().cast();
//! ```

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(clippy::all)]

use super::*;

/// Opaque COM interface (forward-declared in `nvsdk_ngx.h`).
#[repr(C)]
#[derive(Debug)]
pub struct ID3D12Device {
    _opaque: [u8; 0],
}

/// Opaque COM interface (forward-declared in `nvsdk_ngx.h`).
#[repr(C)]
#[derive(Debug)]
pub struct ID3D12Resource {
    _opaque: [u8; 0],
}

/// Opaque COM interface (forward-declared in `nvsdk_ngx.h`).
#[repr(C)]
#[derive(Debug)]
pub struct ID3D12GraphicsCommandList {
    _opaque: [u8; 0],
}

/// Opaque COM interface (forward-declared in `nvsdk_ngx.h`).
#[repr(C)]
#[derive(Debug)]
pub struct IDXGIAdapter {
    _opaque: [u8; 0],
}

/// Opaque COM interface (forward-declared in `nvsdk_ngx.h`).
#[repr(C)]
#[derive(Debug)]
pub struct IUnknown {
    _opaque: [u8; 0],
}

/// Opaque DX12 type (forward-declared in `nvsdk_ngx.h`).
#[repr(C)]
#[derive(Debug)]
pub struct D3D12_RESOURCE_DESC {
    _opaque: [u8; 0],
}

/// Opaque DX12 type (forward-declared in `nvsdk_ngx.h`).
#[repr(C)]
#[derive(Debug)]
pub struct CD3DX12_HEAP_PROPERTIES {
    _opaque: [u8; 0],
}

include!("dx_bindings.rs");
