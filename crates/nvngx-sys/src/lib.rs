//! `nvngx-sys` provides low-level "sys" bindings to the NVIDIA NGX library.

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(clippy::all)]

// use libc::wchar_t;

// include!("bindings.rs");

pub mod error;
pub use error::*;

pub mod vulkan;
pub use vulkan::*;

// pub mod directx;
// pub use directx::*;
