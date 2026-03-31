//! `nvngx` is a crate carefully wrapping the NVIDIA NGX library,
//! providing some abstractions in order to make the use easier.
#![deny(missing_docs)]

/// High-level Vulkan bindings to NGX.
///
/// Currently only available with the `linked` feature. Support for the
/// `loaded` (libloading) path is planned.
#[cfg(feature = "linked")]
pub mod vk;
#[cfg(feature = "linked")]
pub use vk::*;

pub use nvngx_sys as sys;
