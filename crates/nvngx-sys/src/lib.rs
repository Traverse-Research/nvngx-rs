//! `nvngx-sys` provides low-level "sys" bindings to the NVIDIA NGX library.

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(clippy::all)]

#[cfg(all(feature = "linked", feature = "loaded"))]
compile_error!("features `linked` and `loaded` are mutually exclusive");

use ash::vk::{
    Buffer as VkBuffer, CommandBuffer as VkCommandBuffer, Device as VkDevice,
    ExtensionProperties as VkExtensionProperties, Format as VkFormat, Image as VkImage,
    ImageSubresourceRange as VkImageSubresourceRange, ImageView as VkImageView,
    Instance as VkInstance, PFN_vkGetDeviceProcAddr, PFN_vkGetInstanceProcAddr,
    PhysicalDevice as VkPhysicalDevice,
};
use libc::wchar_t;

include!("bindings.rs");

pub mod error;
pub use error::*;

/// `extern "C"` function declarations for link-time binding.
#[cfg(feature = "linked")]
pub mod linked;
#[cfg(feature = "linked")]
pub use linked::*;

/// Pure Rust reimplementations of the SDK helper macros.
#[cfg(feature = "linked")]
pub mod helpers;

/// Runtime-loaded NGX library via `libloading`.
#[cfg(feature = "loaded")]
pub mod library;
