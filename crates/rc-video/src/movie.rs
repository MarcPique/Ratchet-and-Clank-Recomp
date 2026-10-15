//! A movie file opened for playback: the PSS split into its streams (`rc_formats::pss`), the audio channel of
//! the game's language decoded to 48 kHz stereo PCM with the port's SPU-ADPCM decoder, the video decoder, and
//! the YCbCr → RGBA conversion.
//!
//! **Audio channel.** `StartPssMovie(lsn, size, language)` passes the language 0x15ed88 (0 English, 2 French, 3
//! German, 4 Spanish, 5 Italian) to `sceMpegAddStrCallback` as the ADPCM channel; the files carry either channel 0
//! only or channels 0, 2, 3, 4, 5. A language whose channel is missing falls back to channel 0 here (the game
//! would get no audio and, since `readMpeg` 0x31a308 waits for 4 KiB of audio before it starts the display, show
//! nothing: never the case with the English default) [L].
//!
//! **Colour.** The IPU's colour-space conversion (`CSC`) gives full-range RGB from limited-range BT.601 YCbCr, each
//! chroma sample covering 2×2 pixels (no chroma filtering); the port computes the same with standard BT.601
//! coefficients (16.16 fixed point, rounded, clamped), not the IPU's own 7-bit fixed-point constants.

use crate::mpeg2::{DecodeError, Decoder, Frame, Sequence};
use rc_formats::pss;
use std::fmt;

/// Output sample rate of the port's mixer.
pub const OUTPUT_RATE: u32 = 48_000;

#[derive(Debug, Clone)]
pub struct MovieError(pub String);

impl fmt::Display for MovieError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(&self.0) }
}

impl std::error::Error for MovieError {}

impl From<DecodeError> for MovieError {
    fn from(e: DecodeError) -> Self { MovieError(e.to_string()) }
}

impl From<rc_formats::FormatError> for MovieError {
    fn from(e: rc_formats::FormatError) -> Self { MovieError(e.to_string()) }
}

/// An opened movie.
pub struct Movie {
    pub decoder: Decoder,
    pub sequence: Sequence,
    /// The chosen channel's audio at 48 kHz (None: the file has no audio).
    pub audio: Option<Vec<[i16; 2]>>,
    /// The audio channel played and its rate in the file.
    pub audio_channel: Option<u8>,
    pub audio_rate: u32,
    /// Every audio channel in the file.
    pub channels: Vec<u8>,
    /// The PSS layer's counts (packs, packets).
    pub packs: usize,
    pub video_packets: usize,
    pub audio_packets: usize,
}

impl Movie {
    /// Demuxes `file` (a whole PSS), decodes the audio channel of `language` (channel 0 when absent) and reads the
    /// sequence header.
    pub fn open(file: &[u8], language: u8) -> Result<Movie, MovieError> {
        let d = pss::demux(file)?;
        let channels: Vec<u8> = d.audio.keys().copied().collect();
        let channel = if d.audio.contains_key(&language) { Some(language) } else if d.audio.contains_key(&0) { Some(0) } else { channels.first().copied() };
        let (audio, audio_rate) = match channel {
            Some(c) => {
                let a = pss::Audio::parse(&d.audio[&c])?;
                let pcm = a.decode();
                (Some(resample(&pcm, a.header.rate, OUTPUT_RATE)), a.header.rate)
            }
            None => (None, 0),
        };
        let mut decoder = Decoder::new(d.video);
        let sequence = decoder.sequence()?.ok_or_else(|| MovieError("no sequence header in the video stream".into()))?;
        Ok(Movie { decoder, sequence, audio, audio_channel: channel, audio_rate, channels, packs: d.packs, video_packets: d.video_packets, audio_packets: d.audio_packets })
    }

    /// Frames per second (30 for the NTSC movies, 25 for the PAL copies).
    pub fn fps(&self) -> f64 { self.sequence.fps_num as f64 / self.sequence.fps_den as f64 }

    /// The next frame in display order.
    pub fn next_frame(&mut self) -> Result<Option<std::sync::Arc<Frame>>, MovieError> { Ok(self.decoder.next_frame()?) }
}

/// Linear resampling to `to` Hz (only `mpegs[0]`, 44.1 kHz, needs it; 48 kHz audio is returned as is). The SPU
/// plays such a stream at pitch `rate·0x1000/48000` with its Gaussian interpolation: the port reproduces the
/// rate, not the filter.
pub fn resample(pcm: &[[i16; 2]], from: u32, to: u32) -> Vec<[i16; 2]> {
    if from == to || pcm.is_empty() { return pcm.to_vec(); }
    let n = (pcm.len() as u64 * to as u64 / from as u64) as usize;
    (0..n)
        .map(|i| {
            let pos = i as u64 * from as u64;
            let (k, frac) = ((pos / to as u64) as usize, (pos % to as u64) as i64);
            let a = pcm[k];
            let b = pcm[(k + 1).min(pcm.len() - 1)];
            let lerp = |x: i16, y: i16| (x as i64 + ((y as i64 - x as i64) * frac) / to as i64) as i16;
            [lerp(a[0], b[0]), lerp(a[1], b[1])]
        })
        .collect()
}

/// BT.601 limited range → full-range RGB, 16.16 fixed point.
const KY: i32 = 76_309; // 255/219
const KRV: i32 = 104_597; // 1.402·255/224
const KGU: i32 = 25_675; // 0.344136·255/224
const KGV: i32 = 53_279; // 0.714136·255/224
const KBU: i32 = 132_201; // 1.772·255/224

/// One pixel.
#[inline]
pub fn ycbcr_to_rgb(y: u8, cb: u8, cr: u8) -> [u8; 3] {
    let yy = (y as i32 - 16) * KY + 32_768;
    let (u, v) = (cb as i32 - 128, cr as i32 - 128);
    let c = |x: i32| (x >> 16).clamp(0, 255) as u8;
    [c(yy + KRV * v), c(yy - KGU * u - KGV * v), c(yy + KBU * u)]
}

/// The frame as RGBA8 (alpha 255), rows top to bottom; `out` is resized to width·height·4.
pub fn frame_to_rgba(f: &Frame, out: &mut Vec<u8>) {
    let (w, h) = (f.width, f.height);
    out.resize(w * h * 4, 0);
    let cw = w / 2;
    for y in 0..h {
        let row = &f.y[y * w..y * w + w];
        let (cbr, crr) = (&f.cb[(y / 2) * cw..(y / 2) * cw + cw], &f.cr[(y / 2) * cw..(y / 2) * cw + cw]);
        let o = &mut out[y * w * 4..(y + 1) * w * 4];
        for x in 0..w {
            let [r, g, b] = ycbcr_to_rgb(row[x], cbr[x / 2], crr[x / 2]);
            o[x * 4..x * 4 + 4].copy_from_slice(&[r, g, b, 255]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bt601_limited_range() {
        assert_eq!(ycbcr_to_rgb(16, 128, 128), [0, 0, 0]);
        assert_eq!(ycbcr_to_rgb(235, 128, 128), [255, 255, 255]);
        assert_eq!(ycbcr_to_rgb(126, 128, 128), [128, 128, 128]);
        // 75 % colour bars (BT.601 limited-range codes, themselves rounded): red, green, blue within 1 of 191 / 0.
        assert_eq!(ycbcr_to_rgb(65, 100, 212), [191, 0, 1]);
        assert_eq!(ycbcr_to_rgb(112, 72, 58), [0, 191, 0]);
        assert_eq!(ycbcr_to_rgb(35, 212, 114), [0, 1, 192]);
    }

    #[test]
    fn resample_keeps_48k_and_scales_length() {
        let pcm: Vec<[i16; 2]> = (0..441).map(|i| [i as i16, -(i as i16)]).collect();
        assert_eq!(resample(&pcm, 48_000, 48_000), pcm);
        let r = resample(&pcm, 44_100, 48_000);
        assert_eq!(r.len(), 480);
        assert_eq!(r[0], [0, 0]);
        // Sample 100 at 48 kHz is at 91.875 in the source.
        assert_eq!(r[100], [91, -91]);
    }
}
