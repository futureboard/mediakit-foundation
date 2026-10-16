//! Pure-Rust software demux and decode for MKFF: every codec, no FFmpeg.
//!
//! `libmkff` decodes H.264 and HEVC on the platform's video hardware. This crate covers everything
//! else in software, in safe Rust, on any platform, by adapting the FilmCraft codec crates
//! (<https://github.com/storytold/filmcraft>, MIT OR Apache-2.0), which are written from the
//! public specifications:
//!
//! | Kind | Formats |
//! |------|---------|
//! | Containers | MP4 / MOV / 3GP, Matroska / WebM, AVI (OpenDML), MXF (OP1a / OP-Atom), MPEG-TS / M2TS, MPEG-PS / VOB, Ogg, IVF, YUV4MPEG2, raw elementary streams |
//! | Video | H.264 (8-bit 4:2:0), HEVC (Main / Main 10), VP8, VP9 (profiles 0-3), AV1, MPEG-1 / MPEG-2, ProRes (422 / 4444), DNxHD / DNxHR (VC-3), APV, Motion JPEG, uncompressed |
//! | Audio | AAC (LC, HE-AAC core, ADTS / LATM), Opus, AC-3 / E-AC-3, MPEG audio layers I / II / III, FLAC, ALAC, Vorbis, PCM / LPCM |
//!
//! Two levels of API:
//!
//! - [`MediaFile`]: open any supported file, read its [`MediaInfo`], get the [`Frame`] at a frame
//!   index or time (GOP-aware seeking: decode from the preceding sync sample), and read audio as
//!   planar f32 at any sample rate.
//! - [`VideoDecoder`] / [`AudioDecoder`]: stateful packet decoders for elementary streams, e.g.
//!   the Annex-B access units from `mkff::Mp4Demux`, or [`ivf::IvfReader`] frames.
//!
//! Frames come out as planar Y'CbCr (8 or 16-bit, 4:2:0 / 4:2:2 / 4:4:4, optional alpha) or RGBA;
//! [`Frame::to_nv12`], [`Frame::to_p010`] and [`Frame::to_rgba8`] convert to the formats the rest
//! of MKFF (and a GPU upload) expects.

mod audio;
mod error;
mod frame;
pub mod ivf;
mod media;
mod mp4_ac3;
mod video;

pub use audio::{AudioBuffer, AudioCodec, AudioDecoder};
pub use error::{Error, Result};
pub use frame::{ChromaFormat, ColorDesc, Frame, Matrix, PixelLayout, Planes};
pub use media::{AudioStreamInfo, MediaFile, MediaInfo, MediaKind, VideoStreamInfo};
pub use video::{VideoCodec, VideoDecoder};

/// The FilmCraft crates this one adapts, for callers that need the full engine-level API.
pub mod filmcraft {
    pub use filmcraft_codecs as codecs;
    pub use filmcraft_frame as frame;
    pub use filmcraft_media as media;
}

/// File extensions [`MediaFile::open`] recognises (video and audio; stills are opened too).
pub fn supported_extensions() -> impl Iterator<Item = &'static str> {
    filmcraft_media::VIDEO_EXTENSIONS
        .iter()
        .chain(filmcraft_media::AUDIO_EXTENSIONS)
        .copied()
}
