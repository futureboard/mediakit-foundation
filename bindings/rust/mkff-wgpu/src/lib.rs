//! Helpers for uploading MKFF software-decoded NV12 frames into wgpu textures.
//!
//! Hardware zero-copy into wgpu (dma-buf / D3D11 shared handle / IOSurface) is
//! not stable across wgpu backends yet. This crate uses the portable path:
//! [`mkff::VideoFrame::map_cpu_planes`] → R8 + RG8 textures → YUV→RGB shader.

use mkff::{Context, CpuPlanes, PixelFormat, ReceiveOutcome, VideoBackend};

#[derive(Debug)]
pub enum UploadError {
    UnsupportedFormat(PixelFormat),
    MissingPlane,
    ZeroSize,
}

impl std::fmt::Display for UploadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UploadError::UnsupportedFormat(fmt) => write!(f, "unsupported pixel format: {fmt:?}"),
            UploadError::MissingPlane => write!(f, "NV12 frame missing Y or UV plane"),
            UploadError::ZeroSize => write!(f, "frame has zero width or height"),
        }
    }
}

impl std::error::Error for UploadError {}

/// Packed NV12 plane bytes copied out of a mapped [`CpuPlanes`] view.
pub struct Nv12Host {
    pub width: u32,
    pub height: u32,
    pub y: Vec<u8>,
    pub y_stride: u32,
    pub uv: Vec<u8>,
    pub uv_stride: u32,
}

impl Nv12Host {
    pub fn from_cpu_planes(planes: &CpuPlanes<'_>) -> Result<Self, UploadError> {
        if planes.format() != PixelFormat::MKFF_PIXEL_FORMAT_NV12 {
            return Err(UploadError::UnsupportedFormat(planes.format()));
        }
        let width = planes.width();
        let height = planes.height();
        if width == 0 || height == 0 {
            return Err(UploadError::ZeroSize);
        }

        let (y_ptr, y_stride, y_lines) = planes.plane(0).ok_or(UploadError::MissingPlane)?;
        let (uv_ptr, uv_stride, uv_lines) = planes.plane(1).ok_or(UploadError::MissingPlane)?;

        let y = copy_plane(y_ptr, y_stride, y_lines, width);
        let uv = copy_plane(uv_ptr, uv_stride, uv_lines, width);

        Ok(Nv12Host {
            width,
            height,
            y,
            y_stride: width,
            uv,
            uv_stride: width,
        })
    }

    /// BT.709 limited-range YUV → packed RGBA8 (for egui / CPU display).
    pub fn to_rgba8(&self) -> Vec<u8> {
        let w = self.width as usize;
        let h = self.height as usize;
        let mut rgba = vec![0u8; w * h * 4];
        for row in 0..h {
            let y_row = row * self.y_stride as usize;
            let uv_row = (row / 2) * self.uv_stride as usize;
            for col in 0..w {
                let y = self.y[y_row + col] as f32;
                let uv = uv_row + (col & !1);
                let u = self.uv[uv] as f32 - 128.0;
                let v = self.uv[uv + 1] as f32 - 128.0;
                // Full-range-ish BT.709 matrix on 0..255 luma (testdata is synthetic).
                let r = y + 1.5748 * v;
                let g = y - 0.1873 * u - 0.4681 * v;
                let b = y + 1.8556 * u;
                let i = (row * w + col) * 4;
                rgba[i] = r.round().clamp(0.0, 255.0) as u8;
                rgba[i + 1] = g.round().clamp(0.0, 255.0) as u8;
                rgba[i + 2] = b.round().clamp(0.0, 255.0) as u8;
                rgba[i + 3] = 255;
            }
        }
        rgba
    }
}

fn copy_plane(ptr: *const u8, stride: u32, lines: u32, row_bytes: u32) -> Vec<u8> {
    let mut out = vec![0u8; (row_bytes * lines) as usize];
    for row in 0..lines {
        let src = unsafe { ptr.add((row * stride) as usize) };
        let dst = (row * row_bytes) as usize;
        unsafe {
            std::ptr::copy_nonoverlapping(src, out[dst..].as_mut_ptr(), row_bytes as usize);
        }
    }
    out
}

/// Creates (or recreates) R8 (Y) + RG8 (UV) textures and uploads packed NV12.
pub fn upload_nv12(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    host: &Nv12Host,
    y_tex: &mut Option<wgpu::Texture>,
    uv_tex: &mut Option<wgpu::Texture>,
) {
    let y_size = wgpu::Extent3d {
        width: host.width,
        height: host.height,
        depth_or_array_layers: 1,
    };
    let uv_size = wgpu::Extent3d {
        width: host.width / 2,
        height: host.height / 2,
        depth_or_array_layers: 1,
    };

    let need_new = match (&*y_tex, &*uv_tex) {
        (Some(y), Some(uv)) => y.size() != y_size || uv.size() != uv_size,
        _ => true,
    };

    if need_new {
        *y_tex = Some(device.create_texture(&wgpu::TextureDescriptor {
            label: Some("mkff-nv12-y"),
            size: y_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        }));
        *uv_tex = Some(device.create_texture(&wgpu::TextureDescriptor {
            label: Some("mkff-nv12-uv"),
            size: uv_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rg8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        }));
    }

    let y = y_tex.as_ref().unwrap();
    let uv = uv_tex.as_ref().unwrap();

    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: y,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &host.y,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(host.y_stride),
            rows_per_image: Some(host.height),
        },
        y_size,
    );

    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: uv,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &host.uv,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(host.uv_stride),
            rows_per_image: Some(host.height / 2),
        },
        uv_size,
    );
}

/// Byte ranges of HEVC access units in an Annex-B bitstream (VCL-boundary split).
pub fn split_hevc_access_units(data: &[u8]) -> Vec<std::ops::Range<usize>> {
    let Some(first) = find_start_code(data, 0) else {
        return Vec::new();
    };

    let mut au_start = first;
    let mut seen_vcl = false;
    let mut ranges = Vec::new();
    let mut pos = first + 3;

    while pos <= data.len() {
        let next = find_start_code(data, pos);
        let nal_start = pos;
        let nal_end = next.unwrap_or(data.len());

        if nal_end > nal_start + 1 {
            let nal_unit_type = (data[nal_start] >> 1) & 0x3F;
            let is_vcl = nal_unit_type <= 9 || (16..=21).contains(&nal_unit_type);
            if is_vcl {
                if seen_vcl {
                    let end = nal_start.saturating_sub(3);
                    if end > au_start {
                        ranges.push(au_start..end);
                    }
                    au_start = end;
                }
                seen_vcl = true;
            }
        }

        match next {
            Some(n) => pos = n + 3,
            None => break,
        }
    }

    if au_start < data.len() {
        ranges.push(au_start..data.len());
    }
    ranges
}

fn find_start_code(data: &[u8], from: usize) -> Option<usize> {
    if data.len() < 3 || from >= data.len() {
        return None;
    }
    let end = data.len().saturating_sub(2);
    for i in from..end {
        if data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
            return Some(i);
        }
    }
    None
}

/// Software-decode every access unit in an HEVC Main Annex-B file to NV12.
pub fn decode_hevc_software_file(path: &std::path::Path) -> Result<Vec<Nv12Host>, String> {
    if let Some(msg) = reject_container_path(path) {
        return Err(msg);
    }
    let bytes = std::fs::read(path).map_err(|e| format!("read: {e}"))?;
    if looks_like_isom_container(&bytes) {
        return Err(
            "this looks like an MP4/ISOBMFF file; MKFF has no demuxer — \
             pass a raw HEVC Annex-B elementary stream (.hevc / .h265)"
                .into(),
        );
    }
    decode_hevc_software_bytes(&bytes)
}

pub fn decode_hevc_software_bytes(bytes: &[u8]) -> Result<Vec<Nv12Host>, String> {
    let aus = split_hevc_access_units(bytes);
    if aus.is_empty() {
        return Err(
            "no HEVC Annex-B access units found (need start codes 00 00 01 / \
             00 00 00 01; containers like MP4 are not supported)"
                .into(),
        );
    }

    let ctx = Context::new().map_err(|e| format!("context: {e}"))?;
    let mut decoder = ctx
        .video_decoder_hevc_with_backend(8, VideoBackend::MKFF_VIDEO_BACKEND_SOFTWARE_ONLY)
        .map_err(|e| format!("decoder create (software): {e}"))?;

    let mut frames = Vec::new();
    for (i, range) in aus.iter().enumerate() {
        let au = &bytes[range.clone()];
        decoder.submit(au, Some(i as i64), Some(i as i64)).map_err(|e| {
            format!(
                "submit AU {i}: {e} — software path is HEVC Main (8-bit) Annex-B only \
                 (no MP4; Main10 needs hardware)"
            )
        })?;
        drain_frames(&mut decoder, &mut frames)?;
    }
    decoder.flush().map_err(|e| format!("flush: {e}"))?;
    drain_frames(&mut decoder, &mut frames)?;

    if frames.is_empty() {
        return Err("no frames produced (bitstream may lack VCL NALs)".into());
    }
    Ok(frames)
}

fn reject_container_path(path: &std::path::Path) -> Option<String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "mp4" | "m4v" | "mov" | "mkv" | "webm" | "avi" | "ts" | "m2ts" | "mpeg" | "mpg" => {
            Some(format!(
                ".{ext} is a container; MKFF has no demuxer — use raw HEVC Annex-B (.hevc / .h265), \
                 e.g. testdata/tiny_main_p_256x144.hevc"
            ))
        }
        _ => None,
    }
}

/// ISOBMFF / MP4 files typically begin with a size + `ftyp` box.
fn looks_like_isom_container(bytes: &[u8]) -> bool {
    bytes.len() >= 8 && &bytes[4..8] == b"ftyp"
}

fn drain_frames(
    decoder: &mut mkff::VideoDecoder<'_>,
    frames: &mut Vec<Nv12Host>,
) -> Result<(), String> {
    loop {
        match decoder.receive().map_err(|e| format!("receive: {e}"))? {
            ReceiveOutcome::Frame(frame) => {
                let info = frame.info().map_err(|e| format!("frame info: {e}"))?;
                if info.format != PixelFormat::MKFF_PIXEL_FORMAT_NV12 {
                    return Err(format!("expected NV12, got {:?}", info.format));
                }
                let planes = frame
                    .map_cpu_planes()
                    .map_err(|e| format!("map_cpu_planes: {e}"))?;
                frames.push(Nv12Host::from_cpu_planes(&planes).map_err(|e| e.to_string())?);
            }
            ReceiveOutcome::NotReady | ReceiveOutcome::EndOfStream => break,
        }
    }
    Ok(())
}
