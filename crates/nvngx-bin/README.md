# Prebuilt NVIDIA NGX (DLSS) shared libraries.

This crate provides path accessors for the prebuilt DLSS shared libraries bundled in the DLSS SDK submodule. Applications use these paths to locate and copy the DLSS inference DLLs at build or deploy time.

## Warning: git/path dependency only

This crate resolves the DLSS binaries via a relative path into the `nvngx-sys/DLSS` submodule (`CARGO_MANIFEST_DIR/../nvngx-sys/DLSS/lib`). This **only works as a git or path dependency** — it cannot be published to crates.io (the binaries are 170 MB+ of proprietary NVIDIA blobs that exceed the 10 MB crate size limit, and cannot be downloaded at build time from a public URL).

