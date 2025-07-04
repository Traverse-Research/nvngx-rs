use std::env;
use std::path::PathBuf;

#[cfg(feature = "linux")]
const DLSS_LIBRARY_PATH: &str = "DLSS/lib/Linux_x86_64";
#[cfg(feature = "windows")]
const DLSS_LIBRARY_PATH: &str = "DLSS/lib/Windows_x86_64";

fn library_path() -> String {
    let folder = if cfg!(feature = "rel") {
        "rel"
    } else if cfg!(feature = "dev") {
        "dev"
    } else {
        panic!("Select a Debug or Release build!");
    };
    let path = format!("{DLSS_LIBRARY_PATH}/{folder}/");

    PathBuf::from(path)
        .canonicalize()
        .expect("cannot canonicalize path")
        .to_str()
        .unwrap()
        .to_owned()
}

#[cfg(feature = "linux")]
fn compile_helpers() {
    // This is the directory where the `c` library is located.
    let libdir_path = PathBuf::from("./")
        // Canonicalize the path as `rustc-link-search` requires an absolute
        // path.
        .canonicalize()
        .expect("cannot canonicalize path");
    // This is the path to the intermediate object file for our library.
    let obj_path = libdir_path.join("target/ngx_helpers.o");
    // This is the path to the static library file.
    let lib_path = libdir_path.join("target/libngx_helpers.a");

    let git_submodule_update_job = std::process::Command::new("git")
        .arg("submodule")
        .arg("update")
        .arg("--init")
        .arg("--recursive")
        .arg("--depth")
        .arg("1")
        .output()
        .expect("run git successfully");

    if !git_submodule_update_job.status.success() {
        let stdout = String::from_utf8(git_submodule_update_job.stdout).unwrap();
        let stderr = String::from_utf8(git_submodule_update_job.stderr).unwrap();
        panic!("could not checkout the submodules.\nStdout:\n{stdout}\n\nStderr:\n{stderr}");
    }

    // Run `clang` to compile the source code file into an object file.
    let compile_job = std::process::Command::new("clang")
        .arg("-g")
        .arg("-G0")
        .arg("-c")
        .arg("-o")
        .arg(&obj_path)
        .arg(libdir_path.join(SOURCE_FILE_PATH))
        .output()
        .expect("compile using `clang`");

    if !compile_job.status.success() {
        let stdout = String::from_utf8(compile_job.stdout).unwrap();
        let stderr = String::from_utf8(compile_job.stderr).unwrap();
        panic!("could not compile object file.\nStdout:\n{stdout}\n\nStderr:\n{stderr}");
    }

    // Run `ar` to generate the static library.
    if !std::process::Command::new("ar")
        .arg("rcs")
        .arg(lib_path)
        .arg(obj_path)
        .output()
        .expect("could not spawn `ar`")
        .status
        .success()
    {
        // Panic if the command was not successful.
        panic!("could not emit library file");
    }

    // Link against the built helpers wrapper.
    println!(
        "cargo:rustc-link-search={}",
        libdir_path.join("target/").to_str().unwrap()
    );
    println!("cargo:rustc-link-lib=ngx_helpers");
}

fn generate_bindings(header: &str) -> bindgen::Builder {

    // The bindgen::Builder is the main entry point
    // to bindgen, and lets you build up options for
    // the resulting bindings.
    let bindings = bindgen::Builder::default()
        // The input header we would like to generate
        // bindings for.
        .header(header)
        .allowlist_function("NVSDK_NGX_.*")
        .allowlist_type("NVSDK_NGX_.*")
        .allowlist_var("NVSDK_NGX_.*");
        //.allowlist_recursively(false);
        // Tell cargo to invalidate the built crate whenever any of the
        // included header files changed.
        //.parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        //.impl_debug(true)
        //.impl_partialeq(true)
        //.prepend_enum_name(false)
        //.bitfield_enum("NVSDK_NGX_DLSS_Feature_Flags")
        //.disable_name_namespacing()
        //.disable_nested_struct_naming()
        //.default_enum_style(bindgen::EnumVariation::Rust {
        //    non_exhaustive: true,
        //});

    bindings
}
#[cfg(feature = "dx")]
fn compile_dx_headers() {
    const SOURCE_FILE_PATH: &str = "src/dx_source.c";
    const HEADER_FILE_PATH: &str = "src/dx_wrapper.h";

    cc::Build::new().file(SOURCE_FILE_PATH).compile("dx_source");
    println!("cargo:rustc-link-lib=dx_source");
    println!("cargo:rerun-if-changed={HEADER_FILE_PATH}");
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
    let sdk = env::var("VULKAN_SDK").expect("Could Not Locate Vulkan SDK");
    // Tell cargo to invalidate the built crate whenever the wrapper changes
    println!("cargo:rerun-if-changed={HEADER_FILE_PATH}");

    cc::Build::new()
        .file(SOURCE_FILE_PATH)
        .include(PathBuf::from(&sdk).join("Include"))
        .compile("vk_source");

    println!("cargo:rustc-link-lib=vk_source");
    // The bindgen::Builder is the main entry point
    // to bindgen, and lets you build up options for
    // the resulting bindings.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    generate_bindings(HEADER_FILE_PATH)
        .clang_arg(format!("-I{}/Include", sdk))
        // Finish the builder and generate the bindings.
        .generate()
        // Unwrap the Result and panic on failure.
        .expect("Unable to generate bindings")
        // Write the bindings to the $OUT_DIR/bindings.rs file.
        .write_to_file(out_path.join("vk_bindings.rs"))
        .expect("Couldn't write bindings!");
}

fn link_libs() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    // Tell cargo to look for shared libraries in the specified directory
    println!("cargo:rustc-link-search={}", library_path());
    println!("cargo:rustc-link-search={}", out_dir.display());
    #[cfg(feature = "windows")]
    println!(
        "cargo:rustc-link-search={}",
        format!("{DLSS_LIBRARY_PATH}/x64")
    );

    // These arent used? ------------
    //println!("cargo:rustc-link-lib=stdc++");
    //println!("cargo:rustc-link-lib=dl");
    //                   ------------

    #[cfg(feature = "rel")]
    println!("cargo:rustc-link-lib=nvsdk_ngx_d");
    #[cfg(feature = "dev")]
    println!("cargo:rustc-link-lib=nvsdk_ngx_d_dbg");
}

fn main() {
    #[cfg(feature = "linux")]
    compile_helpers();

    link_libs();

    #[cfg(feature = "dx")]
    compile_dx_headers();
    #[cfg(feature = "vk")]
    compile_vk_headers();
}
