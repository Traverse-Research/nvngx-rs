use std::env;
use std::path::PathBuf;

// fn is_docs_rs_build() -> bool {
// std::env::var("DOCS_RS").is_ok()
// }

fn generate_bindings(header: &str) -> bindgen::Builder {
    println!("cargo:rerun-if-changed={header}");
    // The bindgen::Builder is the main entry point
    // to bindgen, and lets you build up options for
    // the resulting bindings.
    bindgen::Builder::default()
        // The input header we would like to generate
        // bindings for.
        .header(header)
        //.allowlist_function("NVSDK_NGX_.*")
        .allowlist_function(".*NGX.*")
        .allowlist_type("(PFN_)?NVSDK_NGX_.*")
        .allowlist_var("NVSDK_NGX_.*")
        .blocklist_item(".*D3[dD]11.*")
        .blocklist_item(".*CUDA.*")
        .allowlist_recursively(false)
        // Tell cargo to invalidate the built crate whenever any of the
        // included header files changed.
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .impl_debug(true)
        .impl_partialeq(true)
        .derive_default(true)
        .prepend_enum_name(false)
        .bitfield_enum("NVSDK_NGX_DLSS_Feature_Flags")
        .disable_name_namespacing()
        .disable_nested_struct_naming()
        .default_enum_style(bindgen::EnumVariation::Rust {
            non_exhaustive: true,
        })
}

#[cfg(feature = "dx")]
fn compile_dx_headers() {
    const SOURCE_FILE_PATH: &str = "src/dx_source.c";
    const HEADER_FILE_PATH: &str = "src/dx_wrapper.h";

    cc::Build::new().file(SOURCE_FILE_PATH).compile("dx_source");
    // Tell cargo to invalidate the built crate whenever the wrapper changes
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    generate_bindings(HEADER_FILE_PATH)
        .generate()
        // Unwrap the Result and panic on failure.
        .expect("Unable to generate bindings")
        // Write the bindings to the $OUT_DIR/bindings.rs file.
        .write_to_file(out_path.join("dx_bindings.rs"))
        .expect("Couldn't write bindings!");
}

#[cfg(feature = "vk")]
fn compile_vk_headers() {
    const SOURCE_FILE_PATH: &str = "src/vk_source.c";
    const HEADER_FILE_PATH: &str = "src/vk_wrapper.h";
    // TODO: This should be in the default include path already
    // let sdk = PathBuf::from(env::var("VULKAN_SDK").expect("Could Not Locate Vulkan SDK"));

    cc::Build::new()
        .file(SOURCE_FILE_PATH)
        // .include(sdk.join("include"))
        .compile("vk_source");

    // The bindgen::Builder is the main entry point
    // to bindgen, and lets you build up options for
    // the resulting bindings.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    generate_bindings(HEADER_FILE_PATH)
        .blocklist_item(".*D3[dD]12.*")
        // .clang_arg(format!("-I{}", sdk.join("include").display()))
        // Finish the builder and generate the bindings.
        .generate()
        // Unwrap the Result and panic on failure.
        .expect("Unable to generate bindings")
        // Write the bindings to the $OUT_DIR/bindings.rs file.
        .write_to_file(out_path.join("vk_bindings.rs"))
        .expect("Couldn't write bindings!");
}

fn windows_mt_suffix() -> &'static str {
    let target_features = std::env::var("CARGO_CFG_TARGET_FEATURE").unwrap();
    // TODO: + prefix?
    if target_features.contains("crt-static") {
        "_s"
    } else {
        "_d"
    }
}

fn link_libs() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()); // Gets location of the toml file
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    let dlss_library_path = match target_os.as_str() {
        "windows" => manifest_dir.join("DLSS/lib/Windows_x86_64"),
        "linux" => manifest_dir.join("DLSS/lib/Linux_x86_64"),
        x => todo!("No libraries for {x}"),
    };

    println!(
        "cargo:warning=Working Dir is: {}",
        dlss_library_path.display()
    );
    // First link our Rust project against the right version of nvsdk_ngx
    match target_os.as_str() {
        "windows" => {
            // TODO: Only one architecture is included (and for vs201x)
            let link_library_path = dlss_library_path.join("x64");
            println!("cargo:rustc-link-search={}", link_library_path.display());
            println!(
                "cargo:rustc-link-search={}",
                dlss_library_path.join("rel").display()
            );
            let windows_mt_suffix = windows_mt_suffix();
            #[cfg(feature = "rel")]
            println!("cargo:rustc-link-lib=nvsdk_ngx{windows_mt_suffix}");
            #[cfg(feature = "dev")]
            println!("cargo:rustc-link-lib=nvsdk_ngx{windows_mt_suffix}_dbg");
        }
        "linux" => {
            // On Linux there is only one link-library
            println!("cargo:rustc-link-lib=nvsdk_ngx");
            println!("cargo:rustc-link-search={}", dlss_library_path.display());
        }
        x => todo!("No libraries for {x}"),
    }

    // Second, copy the dlls to the target directory
    // let runtime_library_folder = if cfg!(feature = "rel") {
    //     "rel"
    // } else if cfg!(feature = "dev") {
    //     "dev"
    // } else {
    //     panic!("Select a Debug or Release build!");
    // };
}

fn main() {
    link_libs();

    #[cfg(feature = "dx")]
    compile_dx_headers();
    #[cfg(feature = "vk")]
    compile_vk_headers();
}
