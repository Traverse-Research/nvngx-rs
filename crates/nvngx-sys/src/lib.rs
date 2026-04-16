//! `nvngx-sys` provides low-level "sys" bindings to the NVIDIA NGX library.

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(clippy::all)]

use libc::wchar_t;

include!("core_bindings.rs");

pub mod error;
pub use error::*;
#[cfg(windows)]
pub mod dx;
#[cfg(windows)]
pub mod dx_helpers;
pub mod helpers;
pub mod vk;
