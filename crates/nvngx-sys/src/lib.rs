//! `nvngx-sys` provides low-level "sys" bindings to the NVIDIA NGX library.

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(clippy::all)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

pub mod error;
pub use error::*;
