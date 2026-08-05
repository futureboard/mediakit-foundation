//! Safe RAII wrapper around libmkff. Hardware frames stay GPU-native;
//! software-decoded frames expose CPU planes via [`VideoFrame::map_cpu_planes`].

mod context;
mod decoder;
mod error;
mod frame;
#[cfg(target_os = "linux")]
mod linux;

pub use context::Context;
pub use decoder::{DecoderInfo, ReceiveOutcome, VideoDecoder};
pub use error::{MkffError, Result};
pub use frame::{CpuPlanes, FrameInfo, VideoFrame};
#[cfg(target_os = "linux")]
pub use frame::{LinuxDmaBuf, LinuxDmaBufObject, LinuxDmaBufPlane};
#[cfg(target_os = "linux")]
pub use linux::{DrmDeviceInfo, VaInfo, VaProfileInfo};

pub use mkff_sys::{
    MKFF_PixelFormat as PixelFormat, MKFF_VideoBackend as VideoBackend, MKFF_VideoCodec as VideoCodec,
    MKFF_VideoEntrypoint as VideoEntrypoint, MKFF_VideoProfile as VideoProfile,
};
