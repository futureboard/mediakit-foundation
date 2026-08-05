//! Helpers for uploading MKFF software-decoded NV12 frames into wgpu textures.
//!
//! Hardware zero-copy into wgpu (dma-buf / D3D11 shared handle / IOSurface) is
//! not stable across wgpu backends yet. This crate uses the portable path:
//! [`mkff::VideoFrame::map_cpu_planes`] → R8 + RG8 textures → YUV→RGB shader.

use mkff::{CpuPlanes, PixelFormat};

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
