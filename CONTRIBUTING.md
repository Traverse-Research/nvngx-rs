# Contributing

## Updating the DLSS submodule

When bumping the DLSS SDK to a new version:

1. **Update the submodule:**
   ```sh
   cd crates/nvngx-sys/DLSS
   git fetch
   git checkout <new-tag>
   cd ../../..
   ```

2. **Update version metadata** in `Cargo.toml` (workspace-level `version` field) and any
   crate-level overrides to reflect the new `+vX.Y.Z` DLSS SDK version.

3. **Regenerate bindings:**
   ```sh
   cargo build -p nvngx-sys -F generate-bindings
   ```
   This requires the Vulkan SDK headers to be installed (and `VULKAN_SDK` set on Windows).

4. **Review helper macro changes.** The Rust helpers in `crates/nvngx-sys/src/helpers.rs`
   are manual reimplementations of `static inline` C helper macros from the DLSS SDK headers:

   | Rust function | C macro source |
   |---|---|
   | `dlss_get_optimal_settings` | `DLSS/include/nvsdk_ngx_helpers.h` `NGX_DLSS_GET_OPTIMAL_SETTINGS` |
   | `vulkan_create_dlss_ext1` | `DLSS/include/nvsdk_ngx_helpers_vk.h` `NGX_VULKAN_CREATE_DLSS_EXT1` |
   | `vulkan_evaluate_dlss_ext` | `DLSS/include/nvsdk_ngx_helpers_vk.h` `NGX_VULKAN_EVALUATE_DLSS_EXT` |
   | `vulkan_create_dlssd_ext1` | `DLSS/include/nvsdk_ngx_helpers_dlssd_vk.h` `NGX_VULKAN_CREATE_DLSSD_EXT1` |
   | `vulkan_evaluate_dlssd_ext` | `DLSS/include/nvsdk_ngx_helpers_dlssd_vk.h` `NGX_VULKAN_EVALUATE_DLSSD_EXT` |

   Diff the header macros against the Rust implementations and update `helpers.rs` if
   parameters were added, removed, or reordered.

5. **Run the full CI checks locally:**
   ```sh
   cargo clippy --workspace --all-targets --all-features -- -Dwarnings
   cargo test --workspace --all-features
   cargo fmt --all -- --check
   ```
