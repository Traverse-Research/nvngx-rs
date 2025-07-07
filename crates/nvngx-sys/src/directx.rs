#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![cfg(feature = "dx")]

use widestring::WideChar as wchar_t;
use windows::Win32::Graphics::Direct3D12::*;
use windows::core::IUnknown;
use windows::Win32::Graphics::Dxgi::IDXGIAdapter;

// helper struct for initialization. Should be ABI compatible
// https://learn.microsoft.com/en-us/windows/win32/direct3d12/cd3dx12-heap-properties
type CD3DX12_HEAP_PROPERTIES = D3D12_HEAP_PROPERTIES;
include!(concat!(env!("OUT_DIR"), "/dx_bindings.rs"));
