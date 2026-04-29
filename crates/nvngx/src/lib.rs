//! `nvngx` is a crate carefully wrapping the NVIDIA NGX library,
//! providing some abstractions in order to make the use easier.
#![deny(missing_docs)]

#[cfg(all(feature = "linked", feature = "loaded"))]
compile_error!("features `linked` and `loaded` are mutually exclusive");

pub mod common;
pub use common::*;

/// High-level Vulkan bindings to NGX.
#[cfg(any(feature = "linked", feature = "loaded"))]
pub mod vk;
#[cfg(any(feature = "linked", feature = "loaded"))]
pub use vk::*;

pub use nvngx_sys as sys;
