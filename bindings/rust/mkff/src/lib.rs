//! Safe RAII wrapper around libmkff. Frames stay GPU-native throughout:
//! nothing in this crate reads decoded pixels back to the CPU.

mod context;
mod decoder;
mod error;
mod frame;
mod linux;

pub use context::Context;
pub use decoder::{DecoderInfo, ReceiveOutcome, VideoDecoder};
pub use error::{MkffError, Result};
pub use frame::{FrameInfo, LinuxDmaBuf, LinuxDmaBufObject, LinuxDmaBufPlane, VideoFrame};
pub use linux::{DrmDeviceInfo, VaInfo, VaProfileInfo};

pub use mkff_sys::{MKFF_PixelFormat as PixelFormat, MKFF_VideoCodec as VideoCodec, MKFF_VideoEntrypoint as VideoEntrypoint, MKFF_VideoProfile as VideoProfile};
