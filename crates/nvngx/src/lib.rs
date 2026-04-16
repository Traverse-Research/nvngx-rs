//! `nvngx` is a crate carefully wrapping the NVIDIA NGX library,
//! providing some abstractions in order to make the use easier.
#![deny(missing_docs)]

pub mod common;
pub use common::*;

pub mod vk;
pub use vk::*;

#[cfg(windows)]
pub mod dx;

pub use nvngx_sys as sys;
