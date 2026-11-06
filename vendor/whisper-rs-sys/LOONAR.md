# whisper-rs-sys 0.15.0 for Loonar Compass

A copy of [whisper-rs-sys](https://codeberg.org/tazz4843/whisper-rs) 0.15.0 (Unlicense) with whisper.cpp and ggml
(MIT, `whisper.cpp/LICENSE`), used through `[patch.crates-io]` in the root `Cargo.toml` (LC-475, speaking instead of
typing). Changes against the published crate:

- `src/bindings.rs`: the bundled bindings are generated on Linux. The layout checks of the libc types that differ on
  Windows (`max_align_t`, `__fsid_t`, `__mbstate_t`, `_G_fpos_t`, `_G_fpos64_t`, `_IO_FILE`,
  `_IO_cookie_io_functions_t`) are removed, and the C enum types are `c_int` as bindgen makes them for MSVC (whisper-rs
  expects that on Windows). The whisper and ggml structures are the same on both systems.
- `build.rs`: always the bundled bindings (no bindgen, so no LLVM/libclang to build); CMake from Visual Studio (vswhere)
  when `cmake` is not on PATH; `GGML_NATIVE=OFF` with AVX2/FMA/F16C/BMI2 (the program checks the CPU before it loads
  the model); `/d1trimfile` so that `__FILE__` holds no build folder; the static C runtime (`CMAKE_MSVC_RUNTIME_LIBRARY`)
  when Rust links it statically (`+crt-static`), also in debug builds.
- `whisper.cpp/ggml/src`: only the CPU backend is kept (CUDA, Metal, Vulkan, SYCL, OpenCL, WebGPU and Hexagon removed).

To update: copy the new crate from the cargo registry and repeat the changes above.
