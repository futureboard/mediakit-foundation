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
> an optional Apache-2.0 libhevc software fallback for HEVC. Progressive
> MP4/MOV demux (`avc1` / `hvc1` / `hev1` → Annex-B).
>
> **Every other codec, in software, without FFmpeg:** [`mkff-codecs`](#software-codecs-mkff-codecs)
> (Rust) and `libmkff_swcodecs` ([`mkff/codecs.h`](include/mkff/codecs.h), C) decode
> MP4/MOV, MKV/WebM, AVI, MXF, MPEG-TS/PS and Ogg with H.264, HEVC Main/Main10,
> VP8, VP9, AV1, MPEG-1/2, ProRes, DNxHD/HR, APV and MJPEG video plus AAC,
> Opus, AC-3/E-AC-3, MP1/2/3, FLAC, ALAC and Vorbis audio — adapted from the
> pure-Rust [FilmCraft](https://github.com/storytold/filmcraft/tree/main/crates)
> codec crates.
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
GPU-native (zero-copy export). CPU NV12 / P010 planes are available via
`mkff_video_frame_map_cpu_planes` for software frames and for hardware
frames when the platform implements readback (D3D11 staging / CVPixelBuffer
lock / VA derive-map).

```
MP4:     progressive moov+mdat (avc1 / hvc1 / hev1) -> Annex-B AUs
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
src/demux/mp4/           progressive MP4/MOV demux (avc1 / hvc1 / hev1)
src/platform/linux/      VA-API, DMA-BUF, H.264 + HEVC DPB
src/platform/windows/    D3D11/DXVA, shared handles, H.264 + HEVC DPB
src/platform/macos/      VideoToolbox, IOSurface export
src/cli/                 mkff CLI
include/mkff/codecs.h    C API of libmkff_swcodecs (software codecs)
bindings/rust/mkff-sys   raw FFI
bindings/rust/mkff       safe RAII wrapper (incl. Mp4Demux)
bindings/rust/mkff-codecs       pure-Rust demux + decode of every codec (FilmCraft)
bindings/rust/mkff-codecs-capi  libmkff_swcodecs: C ABI for mkff-codecs
bindings/rust/mkff-vk    dma-buf -> VkImage (Linux)
bindings/rust/mkff-wgpu  decode → map_cpu_planes → wgpu / egui player
tests/                   CTest suite
testdata/                tiny Annex-B + progressive .mp4 fixtures
testdata/codecs/         one tiny clip per container / codec (mkff-codecs)
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
(`scripts/gen_codec_fixtures.sh` uses an ffmpeg binary only to *encode* the
test fixtures in `testdata/codecs/`; nothing links or runs it.)

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
`Context::video_decoder()`, `Mp4Demux`, `VideoBackend`, `PixelFormat::P010`,
`VideoFrame::map_cpu_planes()`.

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

## Software codecs (`mkff-codecs`)

Pure-Rust, safe-code demux and decode for every container and codec MKFF
handles, on every platform, with no native codec library. It adapts the
[FilmCraft](https://github.com/storytold/filmcraft/tree/main/crates) codec
crates (MIT OR Apache-2.0, written from the public specifications), pinned
to one revision.

| Kind | Supported |
|------|-----------|
| Containers | MP4 / MOV / 3GP, Matroska / WebM, AVI (OpenDML), MXF (OP1a / OP-Atom), MPEG-TS / M2TS, MPEG-PS / VOB, Ogg, YUV4MPEG2, standalone audio files; IVF and Annex-B elementary streams via the packet decoders |
| Video | H.264 (8-bit 4:2:0), HEVC Main / Main10, VP8, VP9 (profiles 0-3), AV1, MPEG-1 / MPEG-2, ProRes 422 / 4444 (+alpha), DNxHD / DNxHR, APV, Motion JPEG, uncompressed |
| Audio | AAC (LC, HE-AAC core; raw, ADTS, LATM), Opus, AC-3 / E-AC-3, MPEG audio layers I / II / III, FLAC, ALAC, Vorbis, PCM / LPCM |

Checked against an independent decoder on `testdata/codecs/`: H.264, HEVC
Main10, VP8, VP9 and AV1 are bit-exact; ProRes, DNxHR and MPEG-2 within ±1
LSB (IDCT rounding the specs allow).

**Rust:**

```toml
[dependencies]
mkff-codecs = { git = "https://github.com/futureboard/mediakit-foundation" }
# or, alongside libmkff:  mkff = { git = "...", features = ["software-codecs"] }  → mkff::codecs
```

```rust
use mkff_codecs::{MediaFile, VideoCodec, VideoDecoder};

// Any file: random access by frame, audio at any rate.
let clip = MediaFile::open("clip.mkv")?;
let frame = clip.video_frame(42)?;            // planar YUV 8/10/12-bit or RGBA
let nv12 = frame.to_nv12();                   // or to_p010() / to_rgba8()
let pcm = clip.audio(0, 48_000, 48_000)?;     // 1 s of stream 0 at 48 kHz

// Elementary streams, e.g. Annex-B AUs from mkff::Mp4Demux:
let mut dec = VideoDecoder::new(VideoCodec::H264)?;
for frame in dec.decode(access_unit, pts)? { /* … */ }
```

`cargo run -p mkff-codecs --release --example probe -- clip.mxf` prints what
it sees and can dump NV12 / native planes.

**C (`libmkff_swcodecs`):** build with `cargo build -p mkff-codecs-capi
--release` (→ `target/release/libmkff_swcodecs.{so,dylib,a}` /
`mkff_swcodecs.dll`), include [`mkff/codecs.h`](include/mkff/codecs.h). It is
independent of `libmkff`; link either or both.

```c
MKFF_SwMedia *m;
mkff_sw_media_open_path("clip.webm", &m);
MKFF_SwFrame *f;
mkff_sw_media_read_video_frame(m, 0, &f);
MKFF_SwFrameInfo fi; MKFF_INIT_STRUCT_HEADER(&fi);
mkff_sw_frame_get_info(f, &fi);               /* planes, depth, chroma, colour */
mkff_sw_frame_destroy(f);
mkff_sw_media_destroy(m);
```

Packet decoders (`mkff_sw_video_decoder_*`, `mkff_sw_audio_decoder_*`) follow
the same submit / receive / flush contract as `mkff_video_decoder_*`.
`MKFF_VideoCodec` gains `VP8`, `MPEG1`, `MPEG2`, `PRORES`, `DNXHD`, `APV` and
`MJPEG` (software-only; libmkff's hardware path rejects them as before).

## Codec coverage (this milestone)

| Codec | Profiles | Output | HW | SW |
|-------|----------|--------|----|----|
| H.264 | Baseline/Main (existing) | NV12 | Linux/Windows/macOS | — |
| HEVC | Main (8-bit 4:2:0), Main10 (10-bit 4:2:0) | NV12 / P010 | VA-API / D3D11 DXVA / VideoToolbox | libhevc Main→NV12 (optional) |

Everything else (and software H.264 / HEVC Main10) is in
[`mkff-codecs`](#software-codecs-mkff-codecs).

Software fallback uses Ittiam libhevc, which is **8-bit only** in the
pinned tree (`IHEVCD_UNSUPPORTED_BIT_DEPTH` for Main10). Main10 decode
and P010 output are hardware-backed (VA-API / D3D11 / VideoToolbox when
the device supports them). `SOFTWARE_ONLY` on a Main10 stream returns
`MKFF_RESULT_ERROR_CODEC_UNAVAILABLE`.

Rejected: 4:2:2 / 4:4:4, non-8/10 bit depth, tiles/WPP on HW paths this milestone.

## Vulkan import (`mkff-vk`)

Linux dma-buf → `VkImage` via `VK_EXT_image_drm_format_modifier`. NV12
today; P010 drm fourcc is exported when Main10 surfaces are available.

## MP4 demux

Progressive (non-fragmented) MP4/MOV with `moov` + `mdat`. First `vide`
track only; sample entries **`avc1`** (H.264) and **`hvc1`/`hev1`** (HEVC).
Samples are converted to Annex-B (length-prefixed NALs → start codes;
`avcC`/`hvcC` parameter sets prepended on the first / sync samples).

C API: `mkff_mp4_demux_open_path` / `open_memory`, `get_video_track`,
`read_access_unit`, `seek_sample`. Rust: `mkff::Mp4Demux`.

Not supported: audio tracks, fragmented MP4 (`moof`/`mdat` segments),
encryption (`encv`), edit-list reordering beyond basic `ctts`.

Fixtures: `testdata/tiny_baseline_64x64.mp4` (`avc1`),
`testdata/tiny_main_256x144.mp4` (`hvc1`).

## WGPU demos (`mkff-wgpu`)

Portable demos: decode → `map_cpu_planes` (NV12) → upload / egui.

**Fullscreen WGSL viewer** (R8 + RG8 upload → YUV→RGB; HEVC Annex-B SW):

```sh
cargo run -p mkff-wgpu --example view
cargo run -p mkff-wgpu --example view -- path/to/clip.hevc
```

**Simple player** (egui on wgpu — play/pause, frame step, loop, FPS). Plays
any container / codec: MP4 and Annex-B go to libmkff's hardware decoders
first; everything else, and anything libmkff can't decode on this machine,
falls back to `mkff-codecs`:

```sh
cargo run -p mkff-wgpu --example player
cargo run -p mkff-wgpu --example player -- testdata/tiny_main_256x144.mp4
cargo run -p mkff-wgpu --example player -- testdata/tiny_baseline_64x64.mp4
cargo run -p mkff-wgpu --example player -- path/to/clip.hevc
cargo run -p mkff-wgpu --example player -- testdata/codecs/vp9_opus.webm
cargo run -p mkff-wgpu --example player -- testdata/codecs/prores_hq.mov
```

Defaults to `testdata/tiny_main_256x144.mp4`. Progressive MP4 uses demux +
`AUTO` decode (H.264 HW on Windows when available; HEVC may use HW or
libhevc SW for Main). Annex-B `.hevc` still uses `SOFTWARE_ONLY` + libhevc.
Frames are decoded up front; no audio. Hardware zero-copy into wgpu is
not wired yet. Needs a GPU/display; not part of default CI.

## Non-goals (this milestone)

Fragmented MP4 in libmkff's own demuxer, bitstream seeking as a product
feature, hardware VP9/AV1, production GUI, mid-stream resolution change,
long-term reference picture edge cases beyond the supported Main/Main10
subset.

## License

MIT — see [LICENSE](LICENSE). Third-party libhevc is Apache-2.0; the
FilmCraft codec crates used by `mkff-codecs` are MIT OR Apache-2.0 (used
under MIT), and their audio bootstrap decoders come from Symphonia
(MPL-2.0, unmodified); see [THIRD_PARTY_NOTICES](THIRD_PARTY_NOTICES).
