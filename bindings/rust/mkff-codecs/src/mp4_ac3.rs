//! Surround AC-3 / E-AC-3 in MP4 / MOV.
//!
//! FilmCraft's MP4 source takes the channel count of an `ac-3` / `ec-3` track from the sample
//! entry's `channelcount`, which muxers set to 2 whatever the stream carries (the real layout is
//! in `dac3` / `dec3`), and keeps only that many decoded channels: a 5.1 track plays as its front
//! left / right channels alone, without centre (dialogue), LFE or surrounds.
//!
//! Until that is fixed upstream, [`crate::MediaFile`] reads such tracks here. This mirrors the
//! source's own audio path — the same packet start table, edit-list offset, one-packet decoder
//! pre-roll after a seek and linear resampling — with the channel count the decoder reports.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use filmcraft_codecs::audio::{
    contiguous_starts, fixed_packet_samples, FixedFrames, PacketDecoder, PacketTime,
};
use filmcraft_isobmff::{ByteSource, CodecConfig, Mp4File, TrackKind};
use filmcraft_media::reader::SharedReader;

use crate::{AudioBuffer, Error, MediaInfo, Result};

/// Decoded packets kept per track (about 2 s of 48 kHz AC-3).
const PACKET_CACHE: usize = 64;

struct Src(SharedReader);

impl ByteSource for Src {
    fn len(&self) -> u64 {
        self.0.len()
    }
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> std::io::Result<()> {
        self.0.read_at(offset, buf)
    }
}

/// Overrides for the audio streams of one file, indexed like [`MediaInfo::audio`].
pub(crate) struct Ac3Tracks {
    tracks: Vec<Option<Ac3Track>>,
}

struct Ac3Track {
    file: Arc<Mp4File>,
    src: Arc<Src>,
    track: usize,
    rate: u32,
    channels: usize,
    /// Packet start positions in source sample frames.
    starts: Vec<i64>,
    /// Edit-list presentation offset in source sample frames.
    offset: i64,
    state: Mutex<State>,
}

struct State {
    decoder: PacketDecoder,
    last: Option<usize>,
    packets: HashMap<usize, Arc<Vec<Vec<f32>>>>,
    order: Vec<usize>,
}

impl Ac3Tracks {
    /// Inspect `reader` (an MP4 / MOV FilmCraft opened as `info`). Returns the overrides and
    /// corrects the channel counts in `info`, or `None` when no stream needs one.
    pub(crate) fn detect(reader: &SharedReader, info: &mut MediaInfo) -> Option<Ac3Tracks> {
        if !matches!(info.container.as_str(), "MPEG-4" | "QuickTime")
            || !info.audio.iter().any(|a| is_ac3_name(&a.codec))
        {
            return None;
        }
        let src = Arc::new(Src(Arc::clone(reader)));
        let file = Arc::new(filmcraft_isobmff::open(src.as_ref()).ok()?);
        // the source's own selection of audio streams
        let atracks: Vec<usize> = file
            .tracks
            .iter()
            .enumerate()
            .filter(|(_, t)| {
                t.kind == TrackKind::Audio
                    && !t.samples.is_empty()
                    && !t.entries.is_empty()
                    && t.timescale > 0
            })
            .map(|(i, _)| i)
            .take(filmcraft_media::MAX_AUDIO_STREAMS)
            .collect();
        if atracks.len() != info.audio.len() {
            // some track was dropped as unplayable: indices can't be matched up safely
            return None;
        }
        let mut any = false;
        let tracks = atracks
            .iter()
            .zip(info.audio.iter_mut())
            .map(|(&ti, stream)| {
                let t = Ac3Track::open(&file, &src, ti, stream.sample_rate)?;
                if t.channels as u32 <= stream.channels {
                    return None;
                }
                stream.channels = t.channels as u32;
                any = true;
                Some(t)
            })
            .collect();
        any.then_some(Ac3Tracks { tracks })
    }

    /// The override for audio stream `stream`, if it has one.
    pub(crate) fn audio(
        &self,
        stream: usize,
        start: i64,
        frames: usize,
        rate: u32,
    ) -> Option<Result<AudioBuffer>> {
        let t = self.tracks.get(stream)?.as_ref()?;
        Some(t.audio(start, frames, rate))
    }
}

fn is_ac3_name(codec: &str) -> bool {
    matches!(
        codec.to_ascii_lowercase().as_str(),
        "ac3" | "ac-3" | "eac3" | "e-ac-3" | "ec-3"
    )
}

impl Ac3Track {
    fn open(file: &Arc<Mp4File>, src: &Arc<Src>, ti: usize, rate: u32) -> Option<Ac3Track> {
        let t = file.tracks.get(ti)?;
        let codec = &t.entries.first()?.codec;
        let fixed = match codec {
            CodecConfig::Ac3 { .. } => FixedFrames::Ac3,
            CodecConfig::Eac3 { .. } => FixedFrames::Eac3,
            _ => return None,
        };
        let rate = rate.max(1);
        let mut decoder = PacketDecoder::for_isobmff(codec, rate).ok()?;
        let first = file.read_sample(src.as_ref(), ti, 0).ok()?;
        let channels = decoder.decode(&first, 0).ok()?.len();
        decoder.reset();

        // packet starts, as the MP4 source computes them
        let ts = i128::from(t.timescale.max(1));
        let mut total = 0i128;
        let stamps: Vec<i64> = t
            .samples
            .iter()
            .map(|s| {
                let at = total * i128::from(rate) / ts;
                total += i128::from(s.duration);
                i64::try_from(at).unwrap_or(i64::MAX)
            })
            .collect();
        let starts = match fixed_packet_samples(fixed, &first, rate) {
            Some(frame) => {
                let unit = i64::try_from((i128::from(rate) + ts - 1) / ts).unwrap_or(i64::MAX);
                contiguous_starts(
                    stamps.iter().map(|&stamp| PacketTime {
                        stamp,
                        stamped: true,
                        samples: Some(frame),
                    }),
                    (frame / 2).max(unit.saturating_mul(2).saturating_add(1)),
                )
            }
            None => stamps,
        };
        let offset = i128::from(t.edit_offset) * i128::from(rate) / ts;
        Some(Ac3Track {
            file: Arc::clone(file),
            src: Arc::clone(src),
            track: ti,
            rate,
            channels,
            starts,
            offset: i64::try_from(offset).ok()?,
            state: Mutex::new(State {
                decoder,
                last: None,
                packets: HashMap::new(),
                order: Vec::new(),
            }),
        })
    }

    fn packet(&self, st: &mut State, i: usize) -> Arc<Vec<Vec<f32>>> {
        if let Some(p) = st.packets.get(&i) {
            return Arc::clone(p);
        }
        let read = |j: usize| self.file.read_sample(self.src.as_ref(), self.track, j);
        // random access: prime the decoder with the previous packet (MDCT overlap)
        if st.last.is_none_or(|l| l + 1 != i) {
            st.decoder.reset();
            if let Some(prev) = i.checked_sub(1).and_then(|j| read(j).ok()) {
                let _ = st.decoder.decode(&prev, 0);
            }
        }
        let pk_start = self.starts.get(i).copied().unwrap_or(0);
        let decoded = read(i)
            .map_err(|e| e.to_string())
            .and_then(|d| {
                st.decoder
                    .decode(&d, pk_start.max(0) as u64)
                    .map_err(|e| e.to_string())
            })
            .unwrap_or_else(|_| {
                let n = self.file.tracks[self.track].samples[i].duration as usize;
                vec![vec![0.0; n]; self.channels]
            });
        st.last = Some(i);
        let p = Arc::new(decoded);
        st.packets.insert(i, Arc::clone(&p));
        st.order.push(i);
        if st.order.len() > PACKET_CACHE {
            let old = st.order.remove(0);
            st.packets.remove(&old);
        }
        p
    }

    fn audio(&self, start: i64, frames: usize, out_rate: u32) -> Result<AudioBuffer> {
        if out_rate == 0 {
            return Err(Error::InvalidArgument("sample rate 0".into()));
        }
        let ch = self.channels.max(1);
        let ratio = f64::from(self.rate) / f64::from(out_rate);
        let p0 = (start as f64 * ratio).floor() as i64;
        let s0 = p0.saturating_sub(self.offset);
        let need = (frames as f64 * ratio).ceil() as i64 + 2;
        let mut src = vec![vec![0.0f32; need.max(0) as usize]; ch];
        {
            let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
            let samples = self.file.tracks[self.track].samples.len();
            let mut i = self
                .starts
                .partition_point(|&x| x <= s0.max(0))
                .saturating_sub(1);
            while i < samples {
                let Some(&pk_start) = self.starts.get(i) else {
                    break;
                };
                if pk_start >= s0.saturating_add(need) {
                    break;
                }
                let pk = self.packet(&mut st, i);
                for (c, dst) in src.iter_mut().enumerate() {
                    let Some(chan) = pk.get(c) else { continue };
                    for (k, v) in chan.iter().enumerate() {
                        let pos = pk_start.saturating_add(k as i64).saturating_sub(s0);
                        if pos >= 0 && (pos as usize) < dst.len() {
                            dst[pos as usize] = *v;
                        }
                    }
                }
                i += 1;
            }
        }
        let mut out = vec![vec![0.0f32; frames]; ch];
        if self.rate == out_rate {
            for (o, s) in out.iter_mut().zip(&src) {
                let n = frames.min(s.len());
                o[..n].copy_from_slice(&s[..n]);
            }
        } else {
            let frac = p0 as f64 - start as f64 * ratio;
            for k in 0..frames {
                let pos = k as f64 * ratio - frac;
                let i0 = pos.floor().max(0.0) as usize;
                let f = (pos - i0 as f64) as f32;
                for (o, s) in out.iter_mut().zip(&src) {
                    let a = s.get(i0).copied().unwrap_or(0.0);
                    let b = s.get(i0 + 1).copied().unwrap_or(a);
                    o[k] = a + (b - a) * f;
                }
            }
        }
        Ok(AudioBuffer {
            sample_rate: out_rate,
            channels: out,
        })
    }
}
