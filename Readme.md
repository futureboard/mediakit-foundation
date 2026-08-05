<p align="center">
  <img src="assets/banner.png" alt="MediaKit Foundation" width="720">
</p>

<p align="center">
  <a href="https://github.com/futureboard/mediakit-foundation/actions/workflows/build.yml"><img src="https://github.com/futureboard/mediakit-foundation/actions/workflows/build.yml/badge.svg" alt="build"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="license"></a>
  <img src="https://img.shields.io/badge/platforms-Linux%20%7C%20Windows%20%7C%20macOS-lightgrey.svg" alt="platforms">
  <img src="https://img.shields.io/badge/C-11-00599C.svg" alt="C11">
  <img src="https://img.shields.io/badge/Rust-bindings-orange.svg" alt="Rust">
</p>

# MediaKit Foundation Framework (MKFF)

Cross-Platform Media Engine, Inspired by BeOS.

> Status: **0.1.0-dev**. H.264 and HEVC (Main / Main10) decode on Linux
> (VA-API), Windows (D3D11 Video Decode), and macOS (VideoToolbox), plus
> an optional Apache-2.0 libhevc software fallback for HEVC. No MP4
> demuxing, audio, rendering, or playback UI yet.
>
> [`.github/workflows/build.yml`](.github/workflows/build.yml) builds and
> runs the full CTest suite on all three platforms
> (`ubuntu-latest`/`macos-latest`/`windows-latest`) on every push.
> CI runners typically have no GPU HEVC, so hardware decode fails
> cleanly; with `MKFF_ENABLE_HEVC_SOFTWARE` (default ON) the software
> path is exercised end-to-end against checked-in Annex-B fixtures.

## What this is

MKFF is a portable media engine core (`libmkff`) with dynamically-loaded
platform backends (`libmkff_platform_linux` / `_macos` / `_windows`).
The core is C11, has a stable versioned C ABI, and keeps hardware frames
GPU-native (zero-copy export). Software-decoded HEVC frames expose CPU
NV12 / P010 planes via `mkff_video_frame_map_cpu_planes`.

```
Linux:   Annex-B -> parser -> VA-API VLD -> NV12/P010 surface -> DMA-BUF
Windows: Annex-B -> parser -> D3D11/DXVA VLD -> NV12/P010 texture -> shared handle
macOS:   Annex-B -> parser -> VideoToolbox -> IOSurface-backed CVPixelBuffer
HEVC SW: Annex-B -> in-core libhevc wrapper -> CPU NV12 (Main) / P010 (Main10)
```

**Backend selection** (`MKFF_VideoBackend` on `MKFF_VideoDecoderDesc`):

| Value | Behavior |
|-------|----------|
| `AUTO` (default) | Try platform HW; on failure fall back to software HEVC if built |
| `HARDWARE_ONLY` | Platform HW only; fail with `CODEC_UNAVAILABLE` if unsupported |
| `SOFTWARE_ONLY` | In-core libhevc only (when `MKFF_ENABLE_HEVC_SOFTWARE` is ON) |

H.264 paths are unchanged and remain hardware-oriented (no software H.264).

## Layout

```
include/mkff/            public C API (portable + platform extensions)
src/codecs/h264/         portable H.264 Annex-B/SPS/PPS/slice/POC parser
src/codecs/hevc/         portable HEVC VPS/SPS/PPS/slice/POC parser
src/codecs/hevc/software libhevc C wrapper (optional software fallback)
src/core/                libmkff: context, loader, backend selection, SW glue
src/platform/linux/      VA-API, DMA-BUF, H.264 + HEVC DPB
src/platform/windows/    D3D11/DXVA, shared handles, H.264 + HEVC DPB
src/platform/macos/      VideoToolbox, IOSurface export
src/cli/                 mkff CLI
bindings/rust/mkff-sys   raw FFI
bindings/rust/mkff       safe RAII wrapper
bindings/rust/mkff-vk    dma-buf -> VkImage (Linux)
bindings/rust/mkff-wgpu  HEVC SW decode → NV12 → wgpu viewer
tests/                   CTest suite
testdata/                tiny Annex-B H.264 / HEVC fixtures
```

## Requirements

- C11 compiler: Clang (Linux/macOS) or MSVC (Windows)
- CMake >= 3.20, Ninja
- Linux: `libva`, `libva-drm`, `libdrm` development packages
- Windows: Windows SDK (D3D11/DXGI/DXVA)
- macOS: Xcode command line tools
- Rust toolchain (stable) for bindings
- Network at CMake configure time when fetching libhevc (or vendored
  `third_party/libhevc/`)

No FFmpeg, GStreamer, or GPL/LGPL dependency is used in the product.

See [THIRD_PARTY_NOTICES](THIRD_PARTY_NOTICES) for Apache-2.0 libhevc
attribution when the software fallback is enabled.

## Rust crate (Cargo git dependency)

```toml
[dependencies]
mkff = { git = "https://github.com/futureboard/mediakit-foundation" }
```

Default features: `bundled`, `hevc`, `hevc-software-fallback`.

| Feature | Effect |
|---------|--------|
| `bundled` | Build native libs via CMake during `cargo build` |
| `hevc` | `MKFF_ENABLE_HEVC=ON` (parser + HW paths) |
| `hevc-software-fallback` | Also `MKFF_ENABLE_HEVC_SOFTWARE=ON` (libhevc) |

Disable software fallback:

```toml
mkff = { git = "...", default-features = false, features = ["bundled", "hevc"] }
```

Helpers: `Context::video_decoder_hevc()`, `video_decoder_hevc_with_backend()`,
`VideoBackend`, `PixelFormat::P010`, `VideoFrame::map_cpu_planes()`.

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

# Windows (Ninja + cl on PATH, e.g. Developer Command Prompt / vcvars)
cmake --preset windows-msvc-debug
cmake --build --preset windows-msvc-debug
ctest --preset windows-msvc-debug

# Rust
cargo build -p mkff
cargo test -p mkff
```

CMake options:

- `MKFF_ENABLE_HEVC` (default ON)
- `MKFF_ENABLE_HEVC_SOFTWARE` (default ON) — FetchContent libhevc

## CLI

```sh
mkff devices                                              # Linux only
mkff va-info                                              # Linux only
mkff decode-test input.h264 --frames 120
mkff decode-test input.hevc --codec hevc --backend auto --frames 120
mkff decode-test input.hevc --codec hevc --backend sw --frames 10
mkff export-test input.h264 --frames 10                   # Linux only
mkff benchmark input.hevc --codec hevc --seconds 10
mkff codec-info hevc --backend sw
```

## Codec coverage (this milestone)

| Codec | Profiles | Output | HW | SW |
|-------|----------|--------|----|----|
| H.264 | Baseline/Main (existing) | NV12 | Linux/Windows/macOS | — |
| HEVC | Main (8-bit 4:2:0), Main10 (10-bit 4:2:0) | NV12 / P010 | VA-API / D3D11 DXVA / VideoToolbox | libhevc Main→NV12 (optional) |

Software fallback uses Ittiam libhevc, which is **8-bit only** in the
pinned tree (`IHEVCD_UNSUPPORTED_BIT_DEPTH` for Main10). Main10 decode
and P010 output are hardware-backed (VA-API / D3D11 / VideoToolbox when
the device supports them). `SOFTWARE_ONLY` on a Main10 stream returns
`MKFF_RESULT_ERROR_CODEC_UNAVAILABLE`.

Rejected: 4:2:2 / 4:4:4, non-8/10 bit depth, tiles/WPP on HW paths this milestone.

## Vulkan import (`mkff-vk`)

Linux dma-buf → `VkImage` via `VK_EXT_image_drm_format_modifier`. NV12
today; P010 drm fourcc is exported when Main10 surfaces are available.

## WGPU viewer (`mkff-wgpu`)

Portable demo: software-decode HEVC Main → `map_cpu_planes` (NV12) →
upload Y/UV as R8 + RG8 textures → YUV→RGB WGSL pass in a winit window.

```sh
cargo run -p mkff-wgpu --example view
cargo run -p mkff-wgpu --example view -- path/to/clip.hevc
```

Uses `SOFTWARE_ONLY` + libhevc (8-bit Main). Hardware zero-copy into wgpu
is not wired yet. Needs a GPU/display; not part of default CI.

## Non-goals (this milestone)

MP4/container demuxing, audio, playback UI, seeking, VP9/AV1, software
H.264, GUI frameworks, mid-stream resolution change, long-term reference
picture edge cases beyond the supported Main/Main10 subset.

## License

MIT — see [LICENSE](LICENSE). Third-party libhevc is Apache-2.0; see
[THIRD_PARTY_NOTICES](THIRD_PARTY_NOTICES).
