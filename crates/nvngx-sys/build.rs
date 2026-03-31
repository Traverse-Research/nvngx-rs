use std::env;
#[cfg(any(feature = "linked", feature = "generate-bindings"))]
use std::path::PathBuf;

fn main() {
    #[cfg(feature = "linked")]
    link_libraries();

    #[cfg(feature = "generate-bindings")]
    generate_bindings();
}

#[cfg(feature = "linked")]
fn link_libraries() {
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    assert_eq!(
        target_arch, "x86_64",
        "No libraries available for architecture `{target_arch}`"
    );

    // Tell cargo to tell rustc to link to the libraries.
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let dlss_library_path = std::path::Path::new(match target_os.as_str() {
        "windows" => "DLSS/lib/Windows_x86_64",
        "linux" => "DLSS/lib/Linux_x86_64",
        x => panic!("No libraries available for OS `{x}`"),
    });

    // Make the path relative to the crate source, where the DLSS submodule exists
    let dlss_library_path =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join(dlss_library_path);

    match target_os.as_str() {
        "windows" => {
            // TODO: Only one architecture is included (and for vs201x)
            let link_library_path = dlss_library_path.join("x64");
            let windows_mt_suffix = windows_mt_suffix();
            // TODO select debug and/or _iterator0/1 when /MTd or /MDd are set.
            let dbg_suffix = if true { "" } else { "_dbg" };
            println!("cargo:rustc-link-lib=nvsdk_ngx{windows_mt_suffix}{dbg_suffix}");
            println!("cargo:rustc-link-search={}", link_library_path.display());
        }
        "linux" => {
            // On Linux there is only one link-library
            println!("cargo:rustc-link-lib=nvsdk_ngx");
            println!("cargo:rustc-link-lib=stdc++");
            println!("cargo:rustc-link-search={}", dlss_library_path.display());
        }
        x => todo!("No libraries for {x}"),
    }
}

#[cfg(feature = "linked")]
fn windows_mt_suffix() -> &'static str {
    let target_features = env::var("CARGO_CFG_TARGET_FEATURE").unwrap();
    if target_features.contains("crt-static") {
        "_s"
    } else {
        "_d"
    }
}

#[cfg(feature = "generate-bindings")]
fn vulkan_sdk_include_directory() -> Option<PathBuf> {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
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

/// Shared bindgen configuration: header, parse callbacks, blocklists, enum styles, etc.
/// Does NOT include allowlists — each generation pass adds its own.
#[cfg(feature = "generate-bindings")]
fn bindgen_builder() -> bindgen::Builder {
    const HEADER_FILE_PATH: &str = "src/bindings.h";

    println!("cargo:rerun-if-changed={HEADER_FILE_PATH}");

    let msrv = bindgen::RustTarget::stable(70, 0).unwrap();

    let mut builder = bindgen::Builder::default()
        .rust_target(msrv)
        .header(HEADER_FILE_PATH)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        // Disallow DirectX, CUDA, and resource release callback
        .blocklist_item(r"\w+D3[Dd]1[12]\w+")
        .blocklist_type("PFN_NVSDK_NGX_ResourceReleaseCallback")
        .blocklist_item(r"\w+CUDA\w+")
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
        builder = builder.clang_arg(format!("-I{}", inc.display()))
    }

    builder
}

#[cfg(feature = "generate-bindings")]
fn generate_bindings() {
    let out_path = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("src");

    // bindings.rs: types, constants, enums, PFN_ typedefs (no functions)
    bindgen_builder()
        .allowlist_type(r"(PFN_)?NVSDK_NGX_\w+")
        .allowlist_var(r"NVSDK_NGX_\w+")
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");

    // linked.rs: extern "C" function declarations only
    bindgen_builder()
        .allowlist_function(r"NVSDK_NGX_\w+")
        .allowlist_function("GetNGXResultAsString")
        .raw_line("use super::*;")
        .generate()
        .expect("Unable to generate linked bindings")
        .write_to_file(out_path.join("linked.rs"))
        .expect("Couldn't write linked bindings!");

    // library.rs: libloading-based Library struct with function pointers
    bindgen_builder()
        .allowlist_function(r"NVSDK_NGX_\w+")
        .allowlist_function("GetNGXResultAsString")
        .dynamic_library_name("Library")
        .dynamic_link_require_all(true)
        .raw_line("use super::*;")
        .generate()
        .expect("Unable to generate library bindings")
        .write_to_file(out_path.join("library.rs"))
        .expect("Couldn't write library bindings!");
}
