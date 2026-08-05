# MediaKit Foundation Framework (MKFF)

Cross-Platform Media Engine, Inspired by BeOS.

> Status: **0.1.0-dev** — Linux-only foundation milestone. H.264 decode
> only. No MP4 demuxing, audio, rendering, or playback UI yet.

## What this is

MKFF is a portable media engine core (`libmkff`) with dynamically-loaded
platform backends (`libmkff_platform_linux`, ...). The core is C11, has a
stable versioned C ABI, and never touches decoded pixel data on the CPU:
frames stay GPU-native and are exported as dma-buf objects for downstream
consumers (Vulkan/WGPU, compositors, encoders).

This milestone implements exactly one path end to end on Linux:

```
H.264 Annex-B -> MKFF H.264 parser -> VA-API VLD decode
              -> reusable NV12 VA surface -> DMA-BUF export -> Rust FFI
```

## Layout

```
include/mkff/            public C API (portable core + linux/ extensions)
src/core/                libmkff.so: context, errors/log, platform loader,
                          frame refcounting, video/pixel types, CPU dispatch
src/platform/linux/      libmkff_platform_linux.so: DRM, VA-API, H.264
                          parser, surface pool, dma-buf export
src/cli/                 mkff CLI (devices, va-info, decode-test,
                          export-test, benchmark)
bindings/rust/mkff-sys   raw FFI declarations
bindings/rust/mkff       safe RAII wrapper
tests/                   CTest suite
```

## Requirements

- C11 compiler (Clang preferred; GCC works)
- CMake >= 3.20, Ninja
- pkg-config, `libva`, `libva-drm`, `libdrm` development packages
- Rust toolchain (stable) for the bindings

No FFmpeg, GStreamer, or GPL/LGPL dependency is used anywhere in this
project.

## Building

```sh
cmake --preset linux-clang-debug
cmake --build --preset linux-clang-debug
ctest --preset linux-clang-debug

cargo build --manifest-path bindings/rust/Cargo.toml
cargo test --manifest-path bindings/rust/Cargo.toml
```

## CLI

```sh
mkff devices
mkff va-info
mkff decode-test input.h264 --frames 120
mkff export-test input.h264 --frames 10
mkff benchmark input.h264 --seconds 10
```

## Non-goals (this milestone)

MP4/container demuxing, audio, WGPU/Vulkan rendering, playback UI,
seeking, HEVC/VP9/AV1 decoding, software codecs, GUI frameworks.

## License

MIT — see [LICENSE](LICENSE).
