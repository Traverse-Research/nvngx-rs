use std::{
    env,
    path::{Path, PathBuf},
};

const SOURCE_FILE_PATH: &str = "src/bindings.c";

fn vulkan_sdk_include_directory() -> Option<PathBuf> {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let is_windows = target_os.as_str() == "windows";

    // Mostly on Windows, the Vulkan headers don't exist in a common location but can be found based
    // on VULKAN_SDK, set by the Vulkan SDK installer.
    match env::var("VULKAN_SDK") {
        Ok(v) => Some(PathBuf::from(v).join(
            // On the Windows SDK the `Include` directory is capitalized
            if is_windows { "Include" } else { "include" },
        )),
        // TODO: On Windows, perhaps this should be an error with a link to the SDK installation?
        Err(env::VarError::NotPresent) if is_windows => {
            // On Windows there's no common include directory like `/usr/include` where Vulkan headers can be found
            panic!("When targeting Windows, the VULKAN_SDK environment variable must be set")
        }
        Err(env::VarError::NotPresent) => None,
        Err(env::VarError::NotUnicode(e)) => {
            panic!("VULKAN_SDK environment variable is not Unicode: {e:?}")
        }
    }
}

fn compile_helpers() {
    let mut build = cc::Build::new();
    build.file(SOURCE_FILE_PATH);
    if let Some(inc) = vulkan_sdk_include_directory() {
        build.include(inc);
    }
    build.compile("ngx_helpers");
}

fn main() {
    compile_helpers();

    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    assert_eq!(
        target_arch, "x86_64",
        "No libraries available for architecture `{target_arch}`"
    );

    // Tell cargo to tell rustc to link to the libraries.
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let dlss_library_path = Path::new(match target_os.as_str() {
        "windows" => "DLSS/lib/Windows_x86_64",
        "linux" => "DLSS/lib/Linux_x86_64",
        x => panic!("No libraries available for OS `{x}`"),
    });

    // Make the path relative to the crate source, where the DLSS submodule exists
    let dlss_library_path =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join(dlss_library_path);

    // First link our Rust project against the right version of nvsdk_ngx
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

fn windows_mt_suffix() -> &'static str {
    let target_features = env::var("CARGO_CFG_TARGET_FEATURE").unwrap();
    if target_features.contains("crt-static") {
        "_s"
    } else {
        "_d"
    }
}
