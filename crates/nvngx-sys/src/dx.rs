//! DX12-specific NGX bindings.
//!
//! The opaque COM types defined here correspond to C forward declarations in
//! `nvsdk_ngx.h`. At the FFI boundary, `*mut ID3D12Device` represents a raw
//! COM interface pointer (what C calls `ID3D12Device*`).
//!
//! To convert from `windows` crate COM types, use:
//! ```ignore
//! use windows::core::Interface;
//! let raw: *mut ID3D12Device = device.as_raw() as *mut _;
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
