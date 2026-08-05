# MediaKit Foundation Framework (MKFF)

Cross-Platform Media Engine, Inspired by BeOS.

> Status: **0.1.0-dev**. H.264 hardware decode on Linux (VA-API),
> Windows (D3D11 Video Decode), and macOS (VideoToolbox). No MP4
> demuxing, audio, rendering, or playback UI yet.
>
> **CI status:** [`.github/workflows/build.yml`](.github/workflows/build.yml)
> builds and runs the full CTest suite on all three platforms
> (`ubuntu-latest`/`macos-latest`/`windows-latest`) on every push — all
> green as of the latest commit. None of the CI runners have a GPU
> attached, though, so what's actually exercised there is: the portable
> H.264 parser, DPB/POC logic, CLI plumbing, and each platform's real
> decode API calls up through the point where they need actual hardware
> — at which point they fail cleanly (a graceful device error, not a
> crash) rather than decode a real frame. Linux has additionally been
> run against real VA-API/DRM code paths during development. Windows
> (D3D11 Video Decode) and macOS (VideoToolbox) compile and link
> correctly against each platform's real SDK and pass CI, but neither
> has decoded a real frame on real hardware yet — treat them as "builds
> clean, unverified end-to-end" rather than "proven."

## What this is

MKFF is a portable media engine core (`libmkff`) with dynamically-loaded
platform backends (`libmkff_platform_linux` / `_macos` / `_windows`).
The core is C11, has a stable versioned C ABI, and never touches
decoded pixel data on the CPU: frames stay GPU-native and are exported
as zero-copy handles for downstream consumers (Vulkan/D3D11/Metal,
compositors, encoders).

Each backend implements the same path end to end, natively:

```
Linux:   H.264 Annex-B -> MKFF H.264 parser -> VA-API VLD decode
                        -> reusable NV12 VA surface -> DMA-BUF export
                        -> Rust FFI -> Vulkan VkImage import

Windows: H.264 Annex-B -> MKFF H.264 parser -> D3D11 Video Decode (DXVA
                        short-slice-control VLD) -> NV12 texture-array
                        slice -> shared D3D11 texture export

macOS:   H.264 Annex-B -> MKFF H.264 parser (SPS/PPS + POC only —
                        VideoToolbox reparses slice headers itself) ->
                        VTDecompressionSession -> IOSurface-backed
                        CVPixelBuffer -> IOSurface export
```

The H.264 Annex-B/SPS/PPS/slice-header parser and POC derivation
(`src/codecs/h264/`) are shared, platform-independent C — written once,
used by all three backends. What differs per platform is how parsed
syntax elements feed each OS's own hardware decode API, and how DPB/
reference-picture management is split between MKFF and the platform:
VA-API and D3D11 both need MKFF to track short-term references itself
(D3D11 needs less: its DXVA short-slice-control model reparses each
slice header in hardware, so unlike VA-API it needs no explicit
per-slice reference list construction). VideoToolbox needs neither —
handing it correctly-ordered presentation timestamps is enough for it
to manage its own DPB and B-frame reordering internally.

## Layout

```
include/mkff/            public C API: portable core + linux/macos/windows extensions
src/codecs/h264/          portable H.264 Annex-B/SPS/PPS/slice/POC parser (no
                          platform dependency; shared by every backend)
src/core/                libmkff: context, errors/log, platform loader,
                          frame refcounting, video/pixel types, CPU dispatch
src/platform/linux/      libmkff_platform_linux: DRM, VA-API, surface pool,
                          dma-buf export
src/platform/windows/    libmkff_platform_windows: D3D11 Video Decode (DXVA),
                          texture-array surface pool, shared-handle export
src/platform/macos/      libmkff_platform_macos: VideoToolbox, IOSurface export
src/cli/                 mkff CLI (devices, va-info, decode-test,
                          export-test, benchmark — devices/va-info/export-test
                          are Linux-only today; decode-test/benchmark are
                          codec-generic and run on every platform)
bindings/rust/mkff-sys   raw FFI declarations
bindings/rust/mkff       safe RAII wrapper
bindings/rust/mkff-vk    dma-buf -> VkImage import (zero-copy, no CPU readback;
                          Linux only today)
tests/                   CTest suite
.github/workflows/       CI build matrix: Linux/macOS/Windows
```

## Requirements

- C11 compiler: Clang (Linux/macOS) or MSVC (Windows)
- CMake >= 3.20, Ninja (Visual Studio generator on Windows)
- Linux: pkg-config, `libva`, `libva-drm`, `libdrm` development packages
- Windows: Windows SDK (D3D11/DXGI/DXVA headers — ships with Visual
  Studio / Build Tools)
- macOS: Xcode command line tools (CoreMedia/CoreVideo/VideoToolbox/
  IOSurface are system frameworks, nothing extra to install)
- Rust toolchain (stable) for the bindings — Linux only for now (see
  "Non-goals" below)
- A Vulkan driver with `VK_EXT_image_drm_format_modifier` support, only
  if you use `mkff-vk` (optional; the C library and CLI don't need it)

No FFmpeg, GStreamer, or GPL/LGPL dependency is used anywhere in this
project.

## Building

```sh
# Linux
cmake --preset linux-clang-debug
cmake --build --preset linux-clang-debug
ctest --preset linux-clang-debug

# macOS
cmake --preset macos-clang-debug
cmake --build --preset macos-clang-debug
ctest --preset macos-clang-debug

# Windows (Developer Command Prompt not required — the Visual Studio
# generator locates MSVC itself)
cmake --preset windows-msvc-debug
cmake --build --preset windows-msvc-debug
ctest --preset windows-msvc-debug

# Rust bindings (Linux only today)
cargo build --manifest-path bindings/rust/Cargo.toml
cargo test --manifest-path bindings/rust/Cargo.toml
```

## CLI

```sh
mkff devices                                    # Linux only
mkff va-info                                    # Linux only
mkff decode-test input.h264 --frames 120        # every platform
mkff export-test input.h264 --frames 10         # Linux only
mkff benchmark input.h264 --seconds 10          # every platform
```

## Vulkan import (`mkff-vk`)

`mkff::VideoFrame::export_dmabuf()` gives you an `mkff::LinuxDmaBuf`
(DRM fourcc, per-plane fd/offset/pitch, per-object DRM format
modifier) — everything `VK_EXT_image_drm_format_modifier` needs.
`mkff-vk::VulkanImporter::import()` turns that directly into a
`VkImage` with no CPU copy:

```rust
let importer = mkff_vk::VulkanImporter::new()?;
let dmabuf = frame.export_dmabuf()?;
let image = importer.import(dmabuf)?; // VkImage, backed by the decoder's surface
```

Current scope: NV12 only, and only the single-dma-buf-object layout
`libmkff_platform_linux` actually exports (`VA_EXPORT_SURFACE_COMPOSED_LAYERS`).
Run `cargo run -p mkff-vk --example probe` to check whether a given
GPU/driver has what's needed (`VK_KHR_external_memory_fd`,
`VK_EXT_external_memory_dma_buf`, `VK_EXT_image_drm_format_modifier`,
`VK_EXT_queue_family_foreign`, and the `samplerYcbcrConversion`
feature) before wiring up a decode pipeline.

## Non-goals (this milestone)

MP4/container demuxing, audio, playback UI, seeking, HEVC/VP9/AV1
decoding, software codecs, GUI frameworks, disjoint multi-object
dma-buf/D3D11 import, mid-stream resolution change, long-term H.264
reference pictures, WGPU (Vulkan only for now — wgpu's external-memory
import hooks are still unstable `wgpu-hal` surface), Rust bindings for
the Windows/macOS backends (`mkff-sys`/`mkff` link against `libmkff`
generically and should build anywhere in principle, but only the Linux
path has actually been exercised), Windows/macOS equivalents of the
`devices`/`va-info`/`export-test` CLI commands.

## License

MIT — see [LICENSE](LICENSE).
