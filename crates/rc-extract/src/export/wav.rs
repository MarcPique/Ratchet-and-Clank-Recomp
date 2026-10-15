//! A WAV writer for decoded SPU ADPCM: RIFF/WAVE, `fmt ` PCM 16-bit, `data`, and a `smpl` chunk carrying the
//! loop (the ADPCM loop-start/repeat flags, in samples) so samplers and game engines loop the sound as the SPU
//! does. The same loop points are in each file's sidecar JSON.

/// A sample loop, in sample frames: plays `start..end` again and again (`end` exclusive).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Loop { pub start: u32, pub end: u32 }

/// Size of the `smpl` chunk body with one loop (36 + 24 bytes).
const SMPL_ONE_LOOP: u32 = 60;

pub fn encode(samples: &[i16], channels: u16, rate: u32, lp: Option<Loop>) -> Vec<u8> {
    assert!(channels >= 1 && samples.len().is_multiple_of(channels as usize), "wav: interleaved sample count");
    let data_len = (samples.len() * 2) as u32;
    let smpl_len = if lp.is_some() { 8 + SMPL_ONE_LOOP } else { 0 };
    let mut out = Vec::with_capacity(44 + data_len as usize + smpl_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(4 + 24 + 8 + data_len + data_len % 2 + smpl_len).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * channels as u32 * 2).to_le_bytes());
    out.extend_from_slice(&(channels * 2).to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples { out.extend_from_slice(&s.to_le_bytes()); }
    if let Some(l) = lp {
        out.extend_from_slice(b"smpl");
        out.extend_from_slice(&SMPL_ONE_LOOP.to_le_bytes());
        let period_ns = (1_000_000_000u64 / rate.max(1) as u64) as u32;
        // manufacturer, product, sample period, MIDI unity note (60), pitch fraction, SMPTE format, SMPTE offset,
        // loop count, sampler data; then the loop: cue id, type 0 (forward), start, end (inclusive), fraction,
        // play count 0 (forever).
        for v in [0, 0, period_ns, 60, 0, 0, 0, 1, 0, 0, 0, l.start, l.end.saturating_sub(1), 0, 0] { out.extend_from_slice(&v.to_le_bytes()); }
    }
    out
}

#[cfg(test)]
pub(crate) mod decode {
    //! Test-only WAV reader: the chunks this writer produces.

    pub struct Wav { pub channels: u16, pub rate: u32, pub bits: u16, pub samples: Vec<i16>, pub lp: Option<super::Loop> }

    pub fn decode(b: &[u8]) -> Result<Wav, String> {
        let u32_at = |o: usize| b.get(o..o + 4).map(|s| u32::from_le_bytes(s.try_into().unwrap())).ok_or("wav: short");
        let u16_at = |o: usize| b.get(o..o + 2).map(|s| u16::from_le_bytes(s.try_into().unwrap())).ok_or("wav: short");
        if b.get(..4) != Some(b"RIFF") || b.get(8..12) != Some(b"WAVE") { return Err("wav: not RIFF/WAVE".into()); }
        if u32_at(4)? as usize + 8 != b.len() { return Err("wav: RIFF size".into()); }
        let (mut fmt, mut data, mut lp) = (None, None, None);
        let mut at = 12;
        while at < b.len() {
            let id = &b[at..at + 4];
            let len = u32_at(at + 4)? as usize;
            let body = b.get(at + 8..at + 8 + len).ok_or("wav: chunk past end")?;
            match id {
                b"fmt " => fmt = Some((u16_at(at + 8)?, u16_at(at + 10)?, u32_at(at + 12)?, u32_at(at + 16)?, u16_at(at + 20)?, u16_at(at + 22)?)),
                b"data" => data = Some(body.as_chunks::<2>().0.iter().map(|c| i16::from_le_bytes(*c)).collect::<Vec<_>>()),
                b"smpl" => {
                    if u32_at(at + 8 + 28)? != 1 { return Err("wav: smpl loop count".into()); }
                    lp = Some(super::Loop { start: u32_at(at + 8 + 44)?, end: u32_at(at + 8 + 48)? + 1 });
                }
                _ => {}
            }
            at += 8 + len + len % 2;
        }
        let (format, channels, rate, byte_rate, align, bits) = fmt.ok_or("wav: no fmt")?;
        if format != 1 || bits != 16 || align != channels * 2 || byte_rate != rate * align as u32 { return Err("wav: fmt fields".into()); }
        Ok(Wav { channels, rate, bits, samples: data.ok_or("wav: no data")?, lp })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_fields_samples_and_loop_round_trip() {
        let s: Vec<i16> = (0..1000).map(|i| (i * 37 - 18000) as i16).collect();
        let b = encode(&s, 1, 44_100, None);
        assert_eq!(b.len(), 44 + 2000);
        assert_eq!(&b[36..40], b"data");
        let w = decode::decode(&b).unwrap();
        assert_eq!((w.channels, w.rate, w.bits, w.lp), (1, 44_100, 16, None));
        assert_eq!(w.samples, s);

        let lp = Loop { start: 28, end: 980 };
        let b = encode(&s, 1, 48_000, Some(lp));
        let w = decode::decode(&b).unwrap();
        assert_eq!((w.rate, w.lp), (48_000, Some(lp)));
        assert_eq!(w.samples, s);

        let st = encode(&[1, -1, 2, -2], 2, 22_050, None);
        let w = decode::decode(&st).unwrap();
        assert_eq!((w.channels, w.samples.len()), (2, 4));
    }
}
