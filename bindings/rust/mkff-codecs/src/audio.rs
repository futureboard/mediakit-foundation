use filmcraft_codecs::audio::{self as fa, PacketDecoder};

use crate::{Error, Result};

/// Audio codecs with a software decoder.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AudioCodec {
    /// AAC-LC and the AAC-LC core of HE-AAC v1 / v2 (upsampled to the SBR output rate).
    Aac,
    Opus,
    /// AC-3 and E-AC-3 (Dolby Digital / Dolby Digital Plus).
    Ac3,
    /// MPEG-1/2 audio layers I, II and III (MP3); the layer is read from the frame header.
    MpegAudio,
    Flac,
    Alac,
    Vorbis,
}

impl AudioCodec {
    pub const ALL: [AudioCodec; 7] = [
        AudioCodec::Aac,
        AudioCodec::Opus,
        AudioCodec::Ac3,
        AudioCodec::MpegAudio,
        AudioCodec::Flac,
        AudioCodec::Alac,
        AudioCodec::Vorbis,
    ];

    pub fn name(self) -> &'static str {
        match self {
            AudioCodec::Aac => "aac",
            AudioCodec::Opus => "opus",
            AudioCodec::Ac3 => "ac3",
            AudioCodec::MpegAudio => "mpeg-audio",
            AudioCodec::Flac => "flac",
            AudioCodec::Alac => "alac",
            AudioCodec::Vorbis => "vorbis",
        }
    }

    /// Parse a codec name (`aac`, `opus`, `ac3`/`eac3`, `mp1`/`mp2`/`mp3`, `flac`, `alac`,
    /// `vorbis`), case-insensitively.
    pub fn from_name(s: &str) -> Option<AudioCodec> {
        Some(match s.to_ascii_lowercase().as_str() {
            "aac" | "mp4a" => AudioCodec::Aac,
            "opus" => AudioCodec::Opus,
            "ac3" | "ac-3" | "eac3" | "e-ac-3" | "ec-3" => AudioCodec::Ac3,
            "mp1" | "mp2" | "mp3" | "mpa" | "mpeg-audio" => AudioCodec::MpegAudio,
            "flac" => AudioCodec::Flac,
            "alac" => AudioCodec::Alac,
            "vorbis" => AudioCodec::Vorbis,
            _ => return None,
        })
    }
}

/// Decoded audio: planar f32, one `Vec` per channel, all the same length. Surround layouts are in
/// WAV / SMPTE order (L R C LFE Ls Rs …).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AudioBuffer {
    pub sample_rate: u32,
    pub channels: Vec<Vec<f32>>,
}

impl AudioBuffer {
    /// Samples per channel.
    pub fn frames(&self) -> usize {
        self.channels.first().map_or(0, Vec::len)
    }

    pub fn channel_count(&self) -> usize {
        self.channels.len()
    }

    /// Interleaved samples (L R L R … for stereo).
    pub fn interleaved(&self) -> Vec<f32> {
        let (n, ch) = (self.frames(), self.channels.len());
        let mut out = Vec::with_capacity(n * ch);
        for i in 0..n {
            for c in &self.channels {
                out.push(c.get(i).copied().unwrap_or(0.0));
            }
        }
        out
    }
}

/// A stateful software audio decoder for one elementary stream.
///
/// | Codec | Packet | Configuration |
/// |-------|--------|---------------|
/// | AAC | Raw access unit, or an ADTS frame when no configuration is given | `AudioSpecificConfig` (`esds` / Matroska `CodecPrivate`) |
/// | Opus | One Opus packet | `OpusHead` (Ogg / Matroska) or `dOps` payload; empty: stereo, no pre-skip |
/// | AC-3 / E-AC-3 | One or more syncframes | — |
/// | MPEG audio | One or more frames | — |
/// | FLAC | One FLAC frame | `STREAMINFO` (with or without the `fLaC` marker / block header) |
/// | ALAC | One packet | ALAC magic cookie |
/// | Vorbis | One packet | Identification + setup headers, or Matroska Xiph-laced `CodecPrivate` |
pub struct AudioDecoder {
    inner: PacketDecoder,
    codec: AudioCodec,
    /// AAC configured from in-band ADTS headers.
    adts: bool,
    sample_rate: u32,
}

impl AudioDecoder {
    /// `sample_rate`: the stream's rate from the container (0 when unknown: it is then read from
    /// the configuration or the first frame header where the codec has one).
    pub fn new(codec: AudioCodec, sample_rate: u32, config: &[u8]) -> Result<Self> {
        use symphonia::core::codecs::{CODEC_TYPE_ALAC, CODEC_TYPE_FLAC, CODEC_TYPE_VORBIS};
        let mut rate = sample_rate;
        let mut adts = false;
        let inner = match codec {
            AudioCodec::Aac if config.is_empty() => {
                adts = true;
                PacketDecoder::lazy_aac()
            }
            AudioCodec::Aac => {
                let out = fa::aac_output_rate(config, std::iter::empty()).ok_or_else(|| {
                    Error::Unsupported("AAC: AudioSpecificConfig not supported".into())
                })?;
                // HE-AAC with implicit SBR: the container's rate is the output rate
                rate = if sample_rate == out * 2 {
                    sample_rate
                } else {
                    out
                };
                PacketDecoder::aac(config, rate)?
            }
            AudioCodec::Opus => {
                let head = if config.is_empty() {
                    filmcraft_opus::OpusHead::simple(2)
                } else if config.starts_with(b"OpusHead") {
                    filmcraft_opus::OpusHead::parse(config)
                } else {
                    filmcraft_opus::OpusHead::from_dops(config)
                }
                .map_err(|e| Error::Unsupported(format!("Opus: {e}")))?;
                rate = fa::OPUS_RATE;
                PacketDecoder::opus(head)?
            }
            AudioCodec::Ac3 => PacketDecoder::ac3()?,
            AudioCodec::MpegAudio => PacketDecoder::lazy_mpeg_audio(sample_rate),
            AudioCodec::Flac => {
                let info = flac_streaminfo(config);
                if let Some(r) = info.and_then(|s| s.get(10..13)).map(|b| {
                    (u32::from(b[0]) << 12) | (u32::from(b[1]) << 4) | (u32::from(b[2]) >> 4)
                }) {
                    rate = r;
                }
                PacketDecoder::new(CODEC_TYPE_FLAC, rate, info.map(<[u8]>::to_vec))?
            }
            AudioCodec::Alac => PacketDecoder::new(
                CODEC_TYPE_ALAC,
                sample_rate,
                (!config.is_empty()).then(|| config.to_vec()),
            )?,
            AudioCodec::Vorbis => {
                let extra = match xiph_unlace(config) {
                    Some(h) if h.len() == 3 => [h[0], h[2]].concat(),
                    _ => config.to_vec(),
                };
                if let Some(r) = extra
                    .get(12..16)
                    .filter(|_| extra.get(1..7) == Some(b"vorbis"))
                {
                    rate = u32::from_le_bytes([r[0], r[1], r[2], r[3]]);
                }
                PacketDecoder::new(CODEC_TYPE_VORBIS, rate, Some(extra))?
            }
        };
        Ok(Self {
            inner,
            codec,
            adts,
            sample_rate: rate,
        })
    }

    pub fn codec(&self) -> AudioCodec {
        self.codec
    }

    /// The output sample rate (0 until known: for header-configured codecs, after the first
    /// packet).
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Decode one packet. `ts` is the packet's timestamp (only some codecs use it).
    pub fn decode(&mut self, packet: &[u8], ts: u64) -> Result<AudioBuffer> {
        let channels = if self.adts {
            let (asc, raw) = fa::adts_split(packet).ok_or_else(|| {
                Error::Decode(
                    "AAC: not an ADTS frame (pass an AudioSpecificConfig for raw AAC)".into(),
                )
            })?;
            self.inner.ensure_aac(&asc)?;
            if let Some(r) = asc_rate(&asc) {
                self.sample_rate = r;
            }
            self.inner.decode(raw, ts)?
        } else {
            if self.sample_rate == 0 {
                self.sample_rate = match self.codec {
                    AudioCodec::MpegAudio => mpeg_audio_rate(packet),
                    AudioCodec::Ac3 => ac3_rate(packet),
                    _ => None,
                }
                .unwrap_or(0);
            }
            self.inner.decode(packet, ts)?
        };
        Ok(AudioBuffer {
            sample_rate: self.sample_rate,
            channels,
        })
    }

    /// Forget decoding state (after a seek).
    pub fn reset(&mut self) {
        self.inner.reset();
    }
}

impl std::fmt::Debug for AudioDecoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioDecoder")
            .field("codec", &self.codec)
            .field("sample_rate", &self.sample_rate)
            .finish()
    }
}

/// The 34-byte FLAC `STREAMINFO` body from a `fLaC` stream header, a metadata block (4-byte
/// header) or the bare body.
fn flac_streaminfo(c: &[u8]) -> Option<&[u8]> {
    let body = if c.starts_with(b"fLaC") {
        c.get(8..42)
    } else if c.len() >= 38 && c[0] & 0x7F == 0 && c[1..4] == [0, 0, 34] {
        c.get(4..38)
    } else {
        c.get(..34)
    };
    body.filter(|b| b.len() == 34)
}

/// Split Xiph-laced packets (Matroska `CodecPrivate` for Vorbis / Theora).
fn xiph_unlace(c: &[u8]) -> Option<Vec<&[u8]>> {
    let count = usize::from(*c.first()?) + 1;
    let mut pos = 1;
    let mut sizes = Vec::with_capacity(count);
    for _ in 0..count - 1 {
        let mut n = 0usize;
        loop {
            let b = *c.get(pos)?;
            pos += 1;
            n += usize::from(b);
            if b != 255 {
                break;
            }
        }
        sizes.push(n);
    }
    let mut out = Vec::with_capacity(count);
    for n in sizes {
        out.push(c.get(pos..pos + n)?);
        pos += n;
    }
    out.push(c.get(pos..)?);
    Some(out)
}

const AAC_RATES: [u32; 13] = [
    96000, 88200, 64000, 48000, 44100, 32000, 24000, 22050, 16000, 12000, 11025, 8000, 7350,
];

/// The core sampling rate of an AudioSpecificConfig (explicit-rate escape included).
fn asc_rate(asc: &[u8]) -> Option<u32> {
    let idx = ((asc.first()? & 7) << 1) | (asc.get(1)? >> 7);
    if idx == 15 {
        let b = asc.get(1..5)?;
        return Some(
            ((u32::from(b[0]) & 0x7F) << 17)
                | (u32::from(b[1]) << 9)
                | (u32::from(b[2]) << 1)
                | (u32::from(b[3]) >> 7),
        );
    }
    AAC_RATES.get(usize::from(idx)).copied()
}

/// Sampling rate named by an MPEG-1/2/2.5 audio frame header.
fn mpeg_audio_rate(f: &[u8]) -> Option<u32> {
    let (b1, b2) = (*f.get(1)?, *f.get(2)?);
    if f[0] != 0xFF || b1 & 0xE0 != 0xE0 {
        return None;
    }
    let base = [44100, 48000, 32000]
        .get(usize::from((b2 >> 2) & 3))
        .copied()?;
    match (b1 >> 3) & 3 {
        3 => Some(base),
        2 => Some(base / 2),
        0 => Some(base / 4),
        _ => None,
    }
}

/// Sampling rate named by an AC-3 / E-AC-3 syncframe header.
fn ac3_rate(f: &[u8]) -> Option<u32> {
    if f.get(..2)? != [0x0B, 0x77] {
        return None;
    }
    let bsid = f.get(5)? >> 3;
    let fscod = f.get(4)? >> 6;
    if bsid > 10 {
        // E-AC-3: fscod 3 selects the reduced rates through fscod2
        return if fscod == 3 {
            [24000, 22050, 16000]
                .get(usize::from((f[4] >> 4) & 3))
                .copied()
        } else {
            [48000, 44100, 32000].get(usize::from(fscod)).copied()
        };
    }
    let base: u32 = [48000, 44100, 32000].get(usize::from(fscod)).copied()?;
    Some(base >> bsid.saturating_sub(8))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_rates() {
        assert_eq!(asc_rate(&[0x12, 0x10]), Some(44100));
        assert_eq!(asc_rate(&[0x11, 0x90]), Some(48000));
        assert_eq!(mpeg_audio_rate(&[0xFF, 0xFB, 0x90, 0x00]), Some(44100));
        assert_eq!(mpeg_audio_rate(&[0xFF, 0xF3, 0x84, 0x00]), Some(24000));
        assert_eq!(ac3_rate(&[0x0B, 0x77, 0, 0, 0x00, 0x40]), Some(48000));
        assert_eq!(ac3_rate(&[0x0B, 0x77, 0, 0, 0x40, 0x40]), Some(44100));
    }

    #[test]
    fn xiph_lacing() {
        let c = [2u8, 3, 1, b'a', b'b', b'c', b'd', b'x', b'y'];
        assert_eq!(
            xiph_unlace(&c).unwrap(),
            vec![&b"abc"[..], &b"d"[..], &b"xy"[..]]
        );
    }

    #[test]
    fn flac_streaminfo_forms() {
        let body: Vec<u8> = (0..34).collect();
        let mut block = vec![0x80, 0, 0, 34];
        block.extend_from_slice(&body);
        let mut stream = b"fLaC".to_vec();
        stream.extend_from_slice(&block);
        assert_eq!(flac_streaminfo(&body), Some(&body[..]));
        assert_eq!(flac_streaminfo(&block), Some(&body[..]));
        assert_eq!(flac_streaminfo(&stream), Some(&body[..]));
    }
}
