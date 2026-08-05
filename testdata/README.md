# Testdata

Checked-in Annex-B elementary streams and progressive MP4 containers.

| File | Codec | Notes |
|------|-------|-------|
| `tiny_baseline_64x64.h264` | H.264 Baseline | Minimal SPS/PPS fixture for CLI smoke |
| `tiny_baseline_64x64.mp4` | H.264 `avc1` | Progressive moov+mdat remux of the `.h264` fixture |
| `tiny_main_64x64.hevc` | HEVC Main | Synthetic VPS/SPS/PPS only (no VCL) |
| `tiny_main_256x144.hevc` | HEVC Main | Single IDR; software decode must succeed |
| `tiny_main_256x144.mp4` | HEVC `hvc1` | Progressive moov+mdat remux of the `.hevc` fixture |
| `tiny_main_p_256x144.hevc` | HEVC Main | I+P sample (optional inter coverage) |
| `tiny_main10_64x64.hevc` | HEVC Main10 | Single IDR 10-bit; HW → P010; SW returns CODEC_UNAVAILABLE |

`tiny_main_256x144.hevc` / `tiny_main_p_256x144.hevc` originate from the
[oxideav-h265](https://github.com/OxideAV/oxideav-h265) test fixtures
(Apache-2.0 / MIT-compatible open test data). `tiny_main10_64x64.hevc`
was generated with libx265 (`profile=main10`, WPP off).

Progressive `.mp4` fixtures are generated offline (no FFmpeg runtime dependency)
via `scripts/gen_tiny_mp4.py` from the matching Annex-B files, then checked in.
