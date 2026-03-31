//! Regenerates `nvngx-sys/src/bindings.rs` from the DLSS SDK headers.
//!
//! Run with `cargo run -p api_gen` after updating the DLSS submodule. Requires
//! the Vulkan SDK headers (via `VULKAN_SDK` on Windows, system include path on Linux).

use std::{
    env,
    path::{Path, PathBuf},
};

const HEADER_FILE_PATH: &str = "src/bindings.h";

fn vulkan_sdk_include_directory() -> Option<PathBuf> {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_else(|_| {
        if cfg!(target_os = "windows") {
            "windows".to_string()
        } else {
            "linux".to_string()
        }
    });
    let is_windows = target_os.as_str() == "windows";

    match env::var("VULKAN_SDK") {
        Ok(v) => Some(PathBuf::from(v).join(if is_windows { "Include" } else { "include" })),
        Err(env::VarError::NotPresent) if is_windows => {
            panic!("When targeting Windows, the VULKAN_SDK environment variable must be set")
        }
        Err(env::VarError::NotPresent) => None,
        Err(env::VarError::NotUnicode(e)) => {
            panic!("VULKAN_SDK environment variable is not Unicode: {e:?}")
        }
    }
}

fn main() {
    // Resolve `nvngx-sys` relative to this binary's manifest dir so the tool
    // works regardless of where `cargo run` is invoked from.
    let nvngx_sys_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("api_gen crate must live under crates/")
        .join("nvngx-sys");

    let header_path = nvngx_sys_dir.join(HEADER_FILE_PATH);
    let out_path = nvngx_sys_dir.join("src").join("bindings.rs");

    let msrv = bindgen::RustTarget::stable(70, 0).unwrap();

    let mut bindings = bindgen::Builder::default()
        .rust_target(msrv)
        .header(header_path.to_str().expect("header path is not UTF-8"))
        // Types and functions defined by the SDK:
        .allowlist_item(r"(PFN_)?NVSDK_NGX_\w+")
        // Single exception for a function that doesn't adhere to the naming standard:
        .allowlist_function("GetNGXResultAsString")
        // Disallow DirectX and CUDA APIs, for which we do not yet provide/implement bindings
        .blocklist_item(r"\w+D3[Dd]1[12]\w+")
        .blocklist_type("PFN_NVSDK_NGX_ResourceReleaseCallback")
        .blocklist_item(r"\w+CUDA\w+")
        // Disallow all other dependencies, like those from libc or Vulkan.
        .allowlist_recursively(false)
        .impl_debug(true)
        .impl_partialeq(true)
        .derive_default(true)
        .prepend_enum_name(false)
        .bitfield_enum("NVSDK_NGX_DLSS_Feature_Flags")
        .bitfield_enum("NVSDK_NGX_Feature_Support_Result")
        .disable_name_namespacing()
        .disable_nested_struct_naming()
        .default_enum_style(bindgen::EnumVariation::Rust {
            non_exhaustive: true,
        });

    if let Some(inc) = vulkan_sdk_include_directory() {
        bindings = bindings.clang_arg(format!("-I{}", inc.display()));
    }

    bindings
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file(&out_path)
        .expect("Couldn't write bindings!");

    println!("Wrote {}", out_path.display());
}
