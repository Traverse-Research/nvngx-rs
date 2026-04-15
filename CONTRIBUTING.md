# Contributing

## Regenerating `nvngx-sys` bindings

The Rust bindings in `crates/nvngx-sys/src/bindings.rs` are generated from the
DLSS SDK headers via [bindgen]. Regenerate them with:

```sh
cargo run -p api_gen
```

Requirements:

- The `DLSS` git submodule must be checked out (`git submodule update --init`).
- The Vulkan SDK headers must be discoverable. On Windows, set `VULKAN_SDK` to
  the SDK install root. On Linux, install `libvulkan-dev` (or equivalent).

Commit the regenerated `bindings.rs` together with the change that motivated
the regeneration (e.g. a DLSS submodule bump).

[bindgen]: https://rust-lang.github.io/rust-bindgen/
