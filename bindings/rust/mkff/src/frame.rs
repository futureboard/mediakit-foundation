use std::os::fd::{FromRawFd, OwnedFd};

use crate::error::{check, Result};

/// A reference-counted decoded video frame. Cloning bumps the
/// underlying C refcount (`mkff_video_frame_retain`) rather than
/// copying pixel data — frames stay GPU-native for their entire
/// lifetime in this crate. Dropping the last clone releases the VA
/// surface back to the decoder's bounded pool.
///
/// Not `Send`/`Sync`: the C side does not document `vaExportSurfaceHandle`
/// (used by [`VideoFrame::export_dmabuf`]) as safe to call concurrently
/// from multiple threads against the same display.
pub struct VideoFrame {
    ptr: *mut mkff_sys::MKFF_VideoFrame,
}

impl VideoFrame {
    /// Takes ownership of a frame reference returned by
    /// `mkff_video_decoder_receive`.
    pub(crate) unsafe fn from_raw_owned(ptr: *mut mkff_sys::MKFF_VideoFrame) -> Self {
        VideoFrame { ptr }
    }

    pub fn info(&self) -> Result<FrameInfo> {
        let mut raw: mkff_sys::MKFF_VideoFrameInfo = unsafe { std::mem::zeroed() };
        raw.struct_size = std::mem::size_of::<mkff_sys::MKFF_VideoFrameInfo>() as u32;
        raw.abi_version = mkff_sys::MKFF_ABI_VERSION;
        check(unsafe { mkff_sys::mkff_video_frame_get_info(self.ptr, &mut raw) })?;
        Ok(FrameInfo {
            width: raw.width,
            height: raw.height,
            format: raw.format,
            pts: (raw.pts != mkff_sys::MKFF_TIMESTAMP_NONE).then_some(raw.pts),
            dts: (raw.dts != mkff_sys::MKFF_TIMESTAMP_NONE).then_some(raw.dts),
            is_key_frame: raw.is_key_frame != 0,
        })
    }

    /// Exports the GPU surface backing this frame as dma-buf objects.
    /// The returned file descriptors are owned by the caller (via
    /// `OwnedFd`, which closes them on drop) and remain valid GPU
    /// buffer references independent of this `VideoFrame`'s lifetime.
    pub fn export_dmabuf(&self) -> Result<LinuxDmaBuf> {
        let mut raw: mkff_sys::MKFF_LinuxDmaBufDesc = unsafe { std::mem::zeroed() };
        raw.struct_size = std::mem::size_of::<mkff_sys::MKFF_LinuxDmaBufDesc>() as u32;
        raw.abi_version = mkff_sys::MKFF_ABI_VERSION;

        check(unsafe { mkff_sys::mkff_linux_video_frame_export_dmabuf(self.ptr, &mut raw) })?;

        // Ownership of every fd in `raw.objects` transfers to the
        // OwnedFds constructed below; we deliberately do NOT call
        // mkff_linux_dmabuf_desc_close() afterward, since that would
        // double-close them (the C API documents both `OwnedFd`-style
        // ownership transfer and desc_close() as valid, mutually
        // exclusive, ways to release the same descriptor).
        let num_objects = raw.num_objects as usize;
        let objects = raw.objects[..num_objects]
            .iter()
            .map(|o| LinuxDmaBufObject {
                fd: unsafe { OwnedFd::from_raw_fd(o.fd) },
                size: o.size,
                modifier: o.modifier,
            })
            .collect();

        let num_planes = raw.num_planes as usize;
        let planes = raw.planes[..num_planes]
            .iter()
            .map(|p| LinuxDmaBufPlane {
                object_index: p.object_index,
                offset: p.offset,
                pitch: p.pitch,
            })
            .collect();

        Ok(LinuxDmaBuf {
            drm_fourcc: raw.drm_fourcc,
            width: raw.width,
            height: raw.height,
            objects,
            planes,
        })
    }
}

impl Clone for VideoFrame {
    fn clone(&self) -> Self {
        let ptr = unsafe { mkff_sys::mkff_video_frame_retain(self.ptr) };
        VideoFrame { ptr }
    }
}

impl Drop for VideoFrame {
    fn drop(&mut self) {
        unsafe { mkff_sys::mkff_video_frame_release(self.ptr) };
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FrameInfo {
    pub width: u32,
    pub height: u32,
    pub format: mkff_sys::MKFF_PixelFormat,
    pub pts: Option<i64>,
    pub dts: Option<i64>,
    pub is_key_frame: bool,
}

/// One dma-buf file descriptor backing (part of) a decoded surface.
/// Closed automatically when dropped.
pub struct LinuxDmaBufObject {
    pub fd: OwnedFd,
    pub size: u32,
    pub modifier: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct LinuxDmaBufPlane {
    pub object_index: u32,
    pub offset: u32,
    pub pitch: u32,
}

pub struct LinuxDmaBuf {
    pub drm_fourcc: u32,
    pub width: u32,
    pub height: u32,
    pub objects: Vec<LinuxDmaBufObject>,
    pub planes: Vec<LinuxDmaBufPlane>,
}
