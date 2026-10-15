//! Audio → WAV (16-bit PCM) + JSON sidecars, decoded with the port's own SPU ADPCM decoder
//! (`rc_formats::vag::decode`, PCSX2 rounding). Loops (the ADPCM loop-start / repeat flags) go into the WAV `smpl`
//! chunk and the sidecar, in samples.
//!
//! * sound banks (level and global): one WAV per distinct sample the tones play, plus `sound_bank.json`, the bank
//!   index (header, every sound with its grains and tone parameters, and which WAV each tone plays);
//! * VAG files (level music and scene speech, the global music and voice folders): one WAV each.

use super::data::{self, lvl};
use super::jsonv::{hex_bytes, Obj, J};
use super::wav::{self, Loop};
use super::Out;
use crate::Error;
use rc_formats::sound_bank::{self, Bank};
use rc_formats::vag::{self, SampleExtent};
use std::path::Path;

/// Global folders of VAG files (every file a `VAGp` stream).
pub(crate) const GLOBAL_VAG_DIRS: [&str; 5] = ["help_audio", "vendor_audio", "space_audio", "qwark_boss_audio", "post_credits_audio"];

fn loop_json(l: Option<Loop>) -> J {
    l.map_or(J::Null, |l| Obj::new().set("start", l.start).set("end", l.end).set("unit", "samples; end exclusive").build())
}

fn extent_loop(e: &SampleExtent) -> Option<Loop> { e.loop_points().map(|(s, e)| Loop { start: s as u32, end: e as u32 }) }

/// One VAG file → WAV + sidecar.
fn vag_file(out: &Out, wav_rel: &str, source: &str, bytes: &[u8]) -> Result<(), Error> {
    let (h, body) = match vag::parse_vag(bytes) { Ok(v) => v, Err(e) => { out.skip(format!("{source}: {e}")); return Ok(()); } };
    let body_frames = body.len() / vag::FRAME_BYTES;
    let (samples, ext) = match vag::sample_extent(body, 0) {
        Ok(ext) => (vag::decode_extent(body, &ext), Some(ext)),
        // No end flag: the stream player plays the whole body.
        Err(_) => (vag::decode(&body[..body_frames * vag::FRAME_BYTES]), None),
    };
    let lp = ext.as_ref().and_then(extent_loop);
    out.write(wav_rel, &wav::encode(&samples, 1, h.sample_rate, lp))?;
    let side = Obj::new()
        .set("source", source)
        .set("name", h.name.as_str())
        .set("vag_version", h.version)
        .set("sample_rate", h.sample_rate)
        .set("channels", 1)
        .set("samples", samples.len())
        .set("body_frames", body_frames)
        .set("played_frames", ext.map_or(body_frames, |e| e.frames))
        .set("end_flag", ext.is_some())
        .set("loop", loop_json(lp))
        .set("decoder", "SPU ADPCM, rc_formats::vag::decode (prediction rounded as PCSX2/DuckStation)")
        .build();
    out.json(&wav_rel.replace(".wav", ".json"), &side)
}

/// Sample rate a bank sample was recorded at: tones with a negative centre note are PS2-rate (48 kHz), others
/// PS1-style 44.1 kHz (`rc_formats::vag::ps1_note_to_pitch`: the pitch at the centre note is 0x1000 or 44100/48000
/// of it).
fn bank_rate(center_note: i8) -> u32 { if center_note < 0 { 48_000 } else { 44_100 } }

fn bank(out: &Out, dir: &str, source: &str, bytes: &[u8]) -> Result<(), Error> {
    let b: Bank = match sound_bank::parse_bank(bytes) { Ok(b) => b, Err(e) => { out.skip(format!("{source}: {e}")); return Ok(()); } };
    let mut samples = Vec::with_capacity(b.vags.len());
    for (vi, ext) in b.vags.iter().enumerate() {
        let tone = b.grains.iter().filter(|g| g.is_tone()).map(|g| g.tone()).find(|t| t.sample_offset as usize == ext.offset);
        let (cn, cf) = tone.map_or((-1, 0), |t| (t.center_note, t.center_fine));
        let rate = bank_rate(cn);
        let pcm = vag::decode_extent(&b.samples, ext);
        let lp = extent_loop(ext);
        let rel = format!("{dir}/sound_bank/{vi:03}.wav");
        out.write(&rel, &wav::encode(&pcm, 1, rate, lp))?;
        let side = Obj::new()
            .set("source", source)
            .set("sample_index", vi)
            .set("offset", ext.offset)
            .set("frames", ext.frames)
            .set("samples", pcm.len())
            .set("sample_rate", rate)
            .set("center_note", cn)
            .set("center_fine", cf)
            .set("loop", loop_json(lp))
            .set("next_flags", ext.next_flags)
            .set("rate_rule", "centre note < 0: PS2-rate 48000 Hz; else PS1-style 44100 Hz (sceSdNote2Pitch at the centre note)")
            .build();
        out.json(&rel.replace(".wav", ".json"), &side)?;
        samples.push(Obj::new().set("index", vi).set("file", format!("sound_bank/{vi:03}.wav")).set("offset", ext.offset).set("frames", ext.frames)
            .set("loop_start_frame", ext.loop_start).set("looped", ext.looped).set("sample_rate", rate).build());
    }
    let h = &b.header;
    let mut sounds = Vec::with_capacity(b.sounds.len());
    for (si, s) in b.sounds.iter().enumerate() {
        let grains: Vec<J> = b.sound_grains(si).iter().map(|g| {
            let mut o = Obj::new().set("kind", g.kind).set("delay", g.delay).set("data", hex_bytes(&g.data));
            if g.is_tone() {
                let t = g.tone();
                o.put("tone", Obj::new().set("priority", t.priority).set("vol", t.vol).set("center_note", t.center_note).set("center_fine", t.center_fine)
                    .set("pan", t.pan).set("map_low", t.map_low).set("map_high", t.map_high).set("pb_low", t.pb_low).set("pb_high", t.pb_high)
                    .set("adsr1", t.adsr1).set("adsr2", t.adsr2).set("flags", t.flags).set("sample_offset", t.sample_offset)
                    .set("sample", b.vag_at(t.sample_offset)));
            }
            o.build()
        }).collect();
        sounds.push(Obj::new().set("index", si).set("vol", s.vol).set("vol_group", s.vol_group).set("pan", s.pan).set("instance_limit", s.instance_limit)
            .set("flags", s.flags).set("looped", s.looped()).set("first_grain", s.first_grain).set("grains", J::Arr(grains)).build());
    }
    let idx = Obj::new()
        .set("source", source)
        .set("file_type", b.file_type)
        .set("chunks", J::Arr(b.chunks.iter().map(|&(o, s)| Obj::new().set("offset", o).set("size", s).build()).collect()))
        .set("header", Obj::new().set("version", h.version).set("flags", h.flags).set("bank_id", h.bank_id).set("bank_num", h.bank_num)
            .set("n_sounds", h.n_sounds).set("n_grains", h.n_grains).set("n_vags", h.n_vags).set("first_sound", h.first_sound).set("first_grain", h.first_grain)
            .set("vags_in_sr", h.vags_in_sr).set("vag_data_size", h.vag_data_size).set("sram_alloc_size", h.sram_alloc_size).set("next_block", h.next_block))
        .set("samples", J::Arr(samples))
        .set("sounds", J::Arr(sounds))
        .build();
    out.json(&format!("{dir}/sound_bank.json"), &idx)
}

/// Every `*.bin` of an archive folder, sorted.
fn list(data: &Path, rel_dir: &str) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(data.join(rel_dir))
        .map(|rd| rd.flatten().filter_map(|e| e.file_name().to_str().filter(|n| n.ends_with(".bin")).map(str::to_string)).collect())
        .unwrap_or_default();
    v.sort();
    v
}

fn vag_dir(data: &Path, out: &Out, src_dir: &str, dst_dir: &str) -> Result<(), Error> {
    for f in list(data, src_dir) {
        let rel = format!("{src_dir}/{f}");
        let bytes = data::read(data, &rel)?;
        vag_file(out, &format!("{dst_dir}/{}", f.replace(".bin", ".wav")), &rel, &bytes)?;
    }
    Ok(())
}

pub(crate) fn export_level(data: &Path, out: &Out, id: u32) -> Result<(), Error> {
    let dir = format!("audio/levels/{id:02}");
    bank(out, &dir, &lvl(id, "sound_bank.bin"), &data::read(data, &lvl(id, "sound_bank.bin"))?)?;
    vag_dir(data, out, &lvl(id, "music"), &format!("{dir}/music"))?;
    vag_dir(data, out, &lvl(id, "speech"), &format!("{dir}/speech"))?;
    if let Ok(t) = sound_bank::music_table(&data::read(data, &lvl(id, "level_header.bin"))?) {
        out.json(&format!("{dir}/music.json"), &Obj::new().set("source", lvl(id, "level_header.bin")).set("music_sectors", t)
            .set("note", "track k is music/k.wav; 0 = no track; tracks come in (start, loop) pairs").build())?;
    }
    Ok(())
}

pub(crate) fn export_global(data: &Path, out: &Out) -> Result<(), Error> {
    bank(out, "audio/global", "global/sound_bank.bin", &data::read(data, "global/sound_bank.bin")?)?;
    vag_file(out, "audio/global/music.wav", "global/music.bin", &data::read(data, "global/music.bin")?)?;
    for d in GLOBAL_VAG_DIRS { vag_dir(data, out, &format!("global/{d}"), &format!("audio/global/{d}"))?; }
    Ok(())
}
