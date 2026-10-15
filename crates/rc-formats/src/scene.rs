//! In-engine scenes (cutscenes): the level header's scene table and the WAD-compressed scene chunks.
//! Spec: docs/plan/cutscenes_transitions.md §1 (table), §2 (chunk format); played by
//! `rc_game::scene_player` (§3).
//!
//! **Table** (level header +0x184, [`crate::toc::SceneRecord`]): 15 records of 0x250 bytes, each
//! `{speech VAG sector per language[6], NTSC chunk sectors[71], PAL chunk sectors[71]}` (`fun_00215970`,
//! `FUN_002591d0`). A chunk's size is the sector difference to the next entry; the last used entry is a
//! 1-sector sentinel (zeros). The chunks of a region are contiguous, so the extractor writes the region as
//! one file, `levels/NN/scene/KK_ntsc.bin` / `_pal.bin` (chunks + sentinel), and the speech as
//! `levels/NN/speech/KK_<lang>.bin`.
//!
//! **Chunk** (decompressed into 0x16cd38 and parsed by `FUN_00259288`): 96 ticks on NTSC (80 PAL).
//! Header `{s16 end tick, s16 0, s32 subtitle table offset (< 0x400 = none), s16 audio start tick,
//! s16 −1, u16 actor count, u16 0, s32 camera table offset, s32 actor offsets[count]}`. The camera table
//! has one 0x20-byte [`CamRecord`] per tick plus one (the last equals the next chunk's first). An actor
//! record is `{s32 class, s32 ?, s32 0, s32 position track offset}` followed by a standard moby sequence
//! (header at +0x10, frame offsets relative to +0x10) whose frames run at 30 Hz; the position track is
//! one vec4 per frame. Subtitles are 16-byte `{s16 start, s16 end, s16 text[5] (En Fr De Es It, offsets
//! from the table start), s16 0}` ended by start = −1, then the Latin-1 strings.

use crate::buf::{invalid, Buf, Result};
use crate::moby_anim::{parse_sequence, MobySequence};
use crate::toc::{LevelHeader, SceneRecord, SCENE_RECORDS, SECTOR_SIZE};
use bytemuck::{Pod, Zeroable};
use std::ops::Range;

/// Ticks per chunk: NTSC 96 (0x60), PAL 80 (0x50) (`CutsceneModeUpdate` 0x2aca80).
pub const NTSC_TICKS_PER_CHUNK: u32 = 96;
pub const PAL_TICKS_PER_CHUNK: u32 = 80;
/// Subtitle table offsets below this mean "no table" (`FUN_00259288`).
pub const SUBTITLE_MIN_OFFSET: i32 = 0x400;
/// Subtitle languages in table order: En, Fr, De, Es, It.
pub const SUBTITLE_LANGUAGES: usize = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Region {
    Ntsc,
    Pal,
}

impl Region {
    pub fn ticks_per_chunk(self) -> u32 { if self == Region::Pal { PAL_TICKS_PER_CHUNK } else { NTSC_TICKS_PER_CHUNK } }
    pub fn is_pal(self) -> bool { self == Region::Pal }
    /// File-name suffix (`scene/KK_<name>.bin`).
    pub fn name(self) -> &'static str { if self == Region::Pal { "pal" } else { "ntsc" } }
}

/// One chunk WAD on the disc.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChunkRange {
    pub sector: u32,
    pub sectors: u32,
}

/// One region's chunk list: the chunks and the sentinel sector after them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegionTable {
    pub chunks: Vec<ChunkRange>,
    pub sentinel: u32,
}

impl RegionTable {
    /// From the non-zero leading entries of a record's list (strictly increasing; ≥ 1 entry).
    pub fn new(sectors: &[i32]) -> Result<Option<RegionTable>> {
        let Some(&last) = sectors.last() else { return Ok(None) };
        if sectors.iter().any(|&s| s <= 0) || sectors.windows(2).any(|w| w[1] <= w[0]) {
            return invalid(format!("scene chunk sectors not strictly increasing: {sectors:?}"));
        }
        let chunks = sectors.windows(2).map(|w| ChunkRange { sector: w[0] as u32, sectors: (w[1] - w[0]) as u32 }).collect();
        Ok(Some(RegionTable { chunks, sentinel: last as u32 }))
    }

    pub fn first_sector(&self) -> u32 { self.chunks.first().map_or(self.sentinel, |c| c.sector) }
    /// Size of the region file (`scene/KK_ntsc.bin`): every chunk plus the sentinel sector.
    pub fn file_bytes(&self) -> usize { (self.sentinel + 1 - self.first_sector()) as usize * SECTOR_SIZE as usize }
    /// Byte range of chunk `i` in the region file.
    pub fn chunk_in_file(&self, i: usize) -> Option<Range<usize>> {
        let c = self.chunks.get(i)?;
        let at = (c.sector - self.first_sector()) as usize * SECTOR_SIZE as usize;
        Some(at..at + c.sectors as usize * SECTOR_SIZE as usize)
    }
}

/// One scene record of the level header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneEntry {
    /// Speech VAG sector per language (index = language 0x15ed88: 0 En, 1 unused, 2 Fr, 3 De, 4 Es, 5 It).
    pub speech: [Option<u32>; 6],
    pub ntsc: Option<RegionTable>,
    pub pal: Option<RegionTable>,
}

impl SceneEntry {
    pub fn new(r: &SceneRecord) -> Result<SceneEntry> {
        Ok(SceneEntry {
            speech: r.speech.map(|s| (s > 0).then_some(s as u32)),
            ntsc: RegionTable::new(r.chunk_sectors(false))?,
            pal: RegionTable::new(r.chunk_sectors(true))?,
        })
    }
    pub fn region(&self, region: Region) -> Option<&RegionTable> { if region.is_pal() { self.pal.as_ref() } else { self.ntsc.as_ref() } }
    pub fn is_empty(&self) -> bool { self.speech.iter().all(Option::is_none) && self.ntsc.is_none() && self.pal.is_none() }
}

/// The level header's scene table (15 records).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneTable {
    pub scenes: Vec<SceneEntry>,
}

impl SceneTable {
    pub fn new(h: &LevelHeader) -> Result<SceneTable> { Ok(SceneTable { scenes: h.scenes.iter().map(SceneEntry::new).collect::<Result<_>>()? }) }
    /// From the 0x2434-byte level header (`level_header.bin`).
    pub fn parse(level_header: &[u8]) -> Result<SceneTable> { SceneTable::new(&crate::toc::parse_level_header(level_header)?) }
    pub fn get(&self, k: usize) -> Option<&SceneEntry> { self.scenes.get(k) }
}
const _: () = assert!(SCENE_RECORDS == 15);

// ---------------------------------------------------------------------------------------------------
// Chunks

/// The authoring tool's filler after the last structure of some chunks (a whole number of KiB of the same
/// non-zero bytes, never pointed at by any offset, not read by `FUN_00259288`): its first bytes.
pub const TRAILING_FILLER: [u8; 8] = [0xe4, 0xd5, 0xd9, 0x36, 0x10, 0x25, 0xaa, 0xbc];

/// The chunks of a global scene lump (TOC `anim_looking_thing_2` space scenes and `things` item scenes, mode 6,
/// `FUN_00259628` / `FUN_002594e0`): a 0x800-byte table of `{s32 offset, s32 size}` ended by size 0 (at most 70
/// entries), chunk data at `0x800 + offset`, each a WAD in the chunk format above. The compressed slices, in order.
pub fn lump_chunks(lump: &[u8]) -> Result<Vec<&[u8]>> {
    let b = Buf(lump);
    let mut out = Vec::new();
    for i in 0..70 {
        let (off, size) = (b.i32(8 * i)?, b.i32(8 * i + 4)?);
        if size == 0 { break; }
        if off < 0 || size < 0 { return invalid(format!("scene lump entry {i}: offset {off} size {size}")); }
        let at = 0x800 + off as usize;
        out.push(b.sub(at, size as usize, "scene lump chunk")?.bytes());
    }
    Ok(out)
}

/// The 0x14-byte chunk header before the actor offsets.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct ChunkHeader {
    /// +0x00: scene end tick (the same in every chunk) → 0x16cd20.
    pub end_tick: i16,
    /// +0x02: 0 on the disc.
    pub unknown_02: i16,
    /// +0x04: subtitle table offset; < 0x400 = none → 0x16cd2c.
    pub subtitle_offset: i32,
    /// +0x08: audio start tick → 0x16cd28 (−6 on Novalis; the start overwrites it with −3).
    pub audio_start: i16,
    /// +0x0a: −1 on the disc.
    pub unknown_0a: i16,
    /// +0x0c: actor count → 0x16cd24.
    pub actor_count: u16,
    /// +0x0e: 0 on the disc.
    pub unknown_0e: u16,
    /// +0x10: camera table offset → 0x16cd34.
    pub camera_offset: i32,
}
const _: () = assert!(std::mem::size_of::<ChunkHeader>() == 0x14);

/// One camera record (one per 60 Hz tick), `FUN_002ac8d8`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct CamRecord {
    /// +0x00: eye position → 0x167240.
    pub eye: [f32; 3],
    /// +0x0c: the byte is the cut flag (returned); the whole word is copied to 0x16724c.
    pub cut: u32,
    /// +0x10 / +0x14 / +0x18: angles (rad) for `sceVu0RotMatrixX` / `Y` / `Z` (rc_game::scene_player).
    pub angles: [f32; 3],
    /// +0x1c: tan(hfov / 2) → 0x16cf70.
    pub tan_half_fov: f32,
}
const _: () = assert!(std::mem::size_of::<CamRecord>() == 0x20);

impl CamRecord {
    /// The cut flag (byte +0xc): snap actors across the cut on odd ticks.
    pub fn is_cut(&self) -> bool { self.cut & 0xff != 0 }
}

/// One actor of a chunk (persisting across chunks by index).
#[derive(Clone, Debug, PartialEq)]
pub struct SceneActor {
    /// Record offset in the chunk.
    pub offset: u32,
    /// +0x00: moby class (`CreateMoby`).
    pub class: i32,
    /// +0x04: one value per scene, its chunk count as the authoring tool counted it (the NTSC count, also in most PAL
    /// copies; e.g. 0x10 for Novalis scene 5); `FUN_00259288` never reads it (checked on every chunk on the disc).
    pub unknown_04: i32,
    /// +0x08: this chunk's index in the scene (0, 1, …); not read by the game either.
    pub unknown_08: i32,
    /// +0x0c: position track offset (chunk-relative).
    pub track_offset: i32,
    /// +0x2c: frame offsets as stored (relative to +0x10).
    pub frame_offsets: Vec<u32>,
    /// +0x10: the streamed sequence (frame offsets relative to +0x10), a new class sequence slot.
    pub sequence: MobySequence,
    /// One position per animation frame (30 Hz), w = 0.
    pub positions: Vec<[f32; 4]>,
}

/// One subtitle line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Subtitle {
    /// Scene ticks, inclusive.
    pub start: i16,
    pub end: i16,
    /// Offsets from the table start (En, Fr, De, Es, It) and the trailing 0.
    pub text_offsets: [i16; SUBTITLE_LANGUAGES],
    pub pad: i16,
    /// The NUL-terminated Latin-1 strings (without the NUL).
    pub text: [Vec<u8>; SUBTITLE_LANGUAGES],
}

impl Subtitle {
    /// Whether the line shows at scene tick `tick` (`fun_001f4be0`: start ≤ tick ≤ end).
    pub fn covers(&self, tick: i32) -> bool { self.start as i32 <= tick && tick <= self.end as i32 }
}

/// One decompressed and parsed chunk.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneChunk {
    pub header: ChunkHeader,
    pub camera: Vec<CamRecord>,
    pub actors: Vec<SceneActor>,
    pub subtitles: Vec<Subtitle>,
}

/// Camera records of chunk `index` of a scene ending at `end_tick`: the chunk's ticks plus one.
pub fn camera_records(end_tick: i32, index: usize, region: Region) -> Result<usize> {
    let tpc = region.ticks_per_chunk() as i32;
    let left = end_tick - index as i32 * tpc;
    if left <= 0 { return invalid(format!("scene chunk {index} starts at or after the end tick {end_tick}")); }
    Ok(left.min(tpc) as usize + 1)
}

/// `FUN_00259288` on a decompressed chunk: chunk `index` of its scene.
pub fn parse_scene_chunk(dec: &[u8], index: usize, region: Region) -> Result<SceneChunk> {
    let b = Buf(dec);
    let header: ChunkHeader = b.pod(0, "scene chunk header")?;
    let n_cam = camera_records(header.end_tick as i32, index, region)?;
    if header.camera_offset < 0 { return invalid("scene chunk: negative camera offset"); }
    let camera: Vec<CamRecord> = b.pod_slice(header.camera_offset as usize, n_cam, "scene camera table")?;
    let offsets: Vec<u32> = b.pod_slice(0x14, header.actor_count as usize, "scene actor offsets")?;
    let mut actors = Vec::with_capacity(offsets.len());
    for &at in &offsets {
        let a = at as usize;
        let seq_base = b.tail(a + 0x10, "scene actor sequence")?;
        // Scene sequences store 0xff in the trigger-count byte (+0x12) and no trigger words follow the
        // frame offsets: parse as 0 triggers, keep the stored byte in the header.
        let sequence = if seq_base.u8(0x12)? == 0xff {
            let mut patched = seq_base.bytes().to_vec();
            patched[0x12] = 0;
            let mut s = parse_sequence(&patched, 0)?;
            s.header.trigger_count = 0xff;
            s
        } else {
            parse_sequence(seq_base.bytes(), 0)?
        };
        let fc = sequence.header.frame_count as usize;
        let track_offset = b.i32(a + 0xc)?;
        if track_offset < 0 { return invalid("scene actor: negative position track offset"); }
        actors.push(SceneActor {
            offset: at,
            class: b.i32(a)?,
            unknown_04: b.i32(a + 4)?,
            unknown_08: b.i32(a + 8)?,
            track_offset,
            frame_offsets: b.pod_slice(a + 0x2c, fc, "scene actor frame offsets")?,
            positions: b.pod_slice(track_offset as usize, fc, "scene actor position track")?,
            sequence,
        });
    }
    let mut subtitles = Vec::new();
    if header.subtitle_offset >= SUBTITLE_MIN_OFFSET {
        let base = header.subtitle_offset as usize;
        let mut at = base;
        loop {
            let start = b.i16(at)?;
            if start == -1 { break; }
            let text_offsets: [i16; SUBTITLE_LANGUAGES] = std::array::from_fn(|k| b.i16(at + 4 + 2 * k).unwrap_or(-1));
            b.check(at, 16, "scene subtitle entry")?;
            let mut text: [Vec<u8>; SUBTITLE_LANGUAGES] = Default::default();
            for (t, &o) in text.iter_mut().zip(&text_offsets) {
                if o < 0 { continue; }
                let s = b.tail(base + o as usize, "scene subtitle text")?.bytes();
                let Some(nul) = s.iter().position(|&c| c == 0) else { return invalid("scene subtitle text without a NUL") };
                *t = s[..nul].to_vec();
            }
            subtitles.push(Subtitle { start, end: b.i16(at + 2)?, text_offsets, pad: b.i16(at + 14)?, text });
            at += 16;
        }
    }
    Ok(SceneChunk { header, camera, actors, subtitles })
}

// ---------------------------------------------------------------------------------------------------
// A whole scene

/// Every chunk of one scene region, decompressed and parsed.
#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    /// Record index in the level's scene table (the id `DialogStreamStart` takes).
    pub index: usize,
    pub region: Region,
    pub chunks: Vec<SceneChunk>,
}

impl Scene {
    /// Scene `k` of `region` from the level header and the region file (`scene/KK_ntsc.bin`, i.e.
    /// `Disc::scene_region`), decompressing every chunk.
    pub fn load(header: &LevelHeader, region_file: &[u8], k: usize, region: Region) -> Result<Scene> {
        let Some(entry) = header.scenes.get(k) else { return invalid(format!("no scene record {k}")) };
        let Some(table) = SceneEntry::new(entry)?.region(region).cloned() else { return invalid(format!("scene {k} has no {} chunks", region.name())) };
        if region_file.len() < table.file_bytes() {
            return invalid(format!("scene {k} {}: region file is {} bytes, the table needs {}", region.name(), region_file.len(), table.file_bytes()));
        }
        let chunks = (0..table.chunks.len())
            .map(|i| {
                let dec = crate::wad::decompress(&region_file[table.chunk_in_file(i).unwrap()])?;
                parse_scene_chunk(&dec, i, region)
            })
            .collect::<Result<Vec<_>>>()?;
        Scene::from_chunks(k, region, chunks)
    }

    /// A scene from already parsed chunks (checks that they agree on the end tick and cover it).
    pub fn from_chunks(index: usize, region: Region, chunks: Vec<SceneChunk>) -> Result<Scene> {
        let Some(first) = chunks.first() else { return invalid(format!("scene {index}: no chunks")) };
        let end = first.header.end_tick as i32;
        let tpc = region.ticks_per_chunk() as i32;
        if chunks.iter().any(|c| c.header.end_tick as i32 != end) { return invalid(format!("scene {index}: chunks disagree on the end tick")); }
        if (end + tpc - 1) / tpc != chunks.len() as i32 { return invalid(format!("scene {index}: {} chunks for end tick {end}", chunks.len())); }
        Ok(Scene { index, region, chunks })
    }

    /// A global scene lump ([`lump_chunks`]), decompressing every chunk; `index` is the lump's TOC index.
    pub fn from_lump(lump: &[u8], index: usize, region: Region) -> Result<Scene> {
        let chunks = lump_chunks(lump)?
            .into_iter()
            .enumerate()
            .map(|(i, c)| parse_scene_chunk(&crate::wad::decompress(c)?, i, region))
            .collect::<Result<Vec<_>>>()?;
        Scene::from_chunks(index, region, chunks)
    }

    pub fn end_tick(&self) -> i32 { self.chunks[0].header.end_tick as i32 }
    pub fn ticks_per_chunk(&self) -> u32 { self.region.ticks_per_chunk() }
    /// The chunk header's audio start tick (chunk 0).
    pub fn audio_start(&self) -> i32 { self.chunks[0].header.audio_start as i32 }
    /// Actor classes (chunk 0 order = the slot order).
    pub fn actor_classes(&self) -> Vec<i32> { self.chunks[0].actors.iter().map(|a| a.class).collect() }

    /// `(chunk, chunk tick)` the player is on at scene tick `tick` ≥ 1: the chunk rolls over when the
    /// chunk tick reaches ticks-per-chunk (record 0 of the next chunk = the last record of this one).
    pub fn chunk_at(&self, tick: i32) -> (usize, u32) {
        let tpc = self.ticks_per_chunk() as i32;
        let t = tick.max(0);
        ((t / tpc) as usize, (t % tpc) as u32)
    }

    /// The camera record shown at scene tick `tick` (`FUN_002ac8d8`, no interpolation).
    pub fn camera_at(&self, tick: i32) -> Option<&CamRecord> {
        let (c, r) = self.chunk_at(tick);
        self.chunks.get(c)?.camera.get(r as usize)
    }

    /// Scene ticks whose camera record has the cut flag (each tick once: record 0 of chunk n > 0 is
    /// the last record of chunk n − 1).
    pub fn cut_ticks(&self) -> Vec<i32> {
        (1..self.end_tick()).filter(|&t| self.camera_at(t).is_some_and(CamRecord::is_cut)).collect()
    }

    /// The subtitle drawn at `tick`: the first entry of the current chunk's table covering it.
    pub fn subtitle_at(&self, tick: i32) -> Option<&Subtitle> {
        let (c, _) = self.chunk_at(tick);
        self.chunks.get(c)?.subtitles.iter().find(|s| s.covers(tick))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A synthetic decompressed chunk: `n_actors` actors of `frames` one-joint frames each; camera eye.x =
    /// scene tick of the record, cut flag on `cuts` (scene ticks); one subtitle line.
    pub fn synth_chunk(end: i16, index: usize, n_actors: usize, frames: u8, cuts: &[i32], subtitle: Option<(i16, i16, &str)>) -> Vec<u8> {
        let region = Region::Ntsc;
        let n_cam = camera_records(end as i32, index, region).unwrap();
        let mut d = vec![0u8; 0x14 + 4 * n_actors];
        let put32 = |d: &mut Vec<u8>, at: usize, v: u32| d[at..at + 4].copy_from_slice(&v.to_le_bytes());
        while !d.len().is_multiple_of(16) { d.push(0); }
        let cam_off = d.len();
        for r in 0..n_cam {
            let tick = (index * 96 + r) as f32;
            let rec = CamRecord { eye: [tick, 1.0, 2.0], cut: cuts.contains(&(tick as i32)) as u32, angles: [0.1, 0.2, 0.3], tan_half_fov: 0.414 };
            d.extend_from_slice(bytemuck::bytes_of(&rec));
        }
        for a in 0..n_actors {
            let at = d.len();
            put32(&mut d, 0x14 + 4 * a, at as u32);
            d.extend_from_slice(&[0; 0x10]);
            put32(&mut d, at, [0u32, 10, 530][a % 3]);
            put32(&mut d, at + 4, 0x10);
            let seq = at + 0x10;
            let hdr = crate::moby_anim::MobySequenceHeader { frame_count: frames, loop_sound: 0xff, trigger_count: 0, ..Default::default() };
            d.extend_from_slice(bytemuck::bytes_of(&hdr));
            let ptrs = d.len();
            d.extend(std::iter::repeat_n(0u8, 4 * frames as usize));
            for f in 0..frames as usize {
                let fo = d.len() - seq;
                put32(&mut d, ptrs + 4 * f, fo as u32);
                // One joint: identity quaternion, no scale/translation records.
                let fh = crate::moby_anim::MobyFrameHeader { rate: 0.25, time: (f * 16) as i16, qwc: 1, quat_bytes: 8, scale_count: 0, trans_offset: 8, trans_count: 0 };
                d.extend_from_slice(bytemuck::bytes_of(&fh));
                d.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0x80, 0, 0, 0, 0, 0, 0, 0, 0]);
            }
            let track = d.len();
            put32(&mut d, at + 0xc, track as u32);
            for f in 0..frames as usize {
                let p = [(index * 48 + f) as f32 * 2.0, a as f32, 0.0, 0.0f32];
                d.extend_from_slice(bytemuck::cast_slice(&p));
            }
        }
        let mut sub_off = 0;
        if let Some((s, e, text)) = subtitle {
            while d.len() < SUBTITLE_MIN_OFFSET as usize { d.push(0); }
            sub_off = d.len();
            let strings = 32;
            for v in [s, e, strings, strings, strings, strings, strings, 0] { d.extend_from_slice(&v.to_le_bytes()); }
            d.extend_from_slice(&(-1i16).to_le_bytes());
            d.extend_from_slice(&[0; 14]);
            d.extend_from_slice(text.as_bytes());
            d.push(0);
        }
        let h = ChunkHeader { end_tick: end, unknown_02: 0, subtitle_offset: sub_off as i32, audio_start: -6, unknown_0a: -1, actor_count: n_actors as u16, unknown_0e: 0, camera_offset: cam_off as i32 };
        d[..0x14].copy_from_slice(bytemuck::bytes_of(&h));
        d
    }

    #[test]
    fn parses_a_synthetic_chunk() {
        let d = synth_chunk(200, 1, 2, 49, &[107], Some((100, 150, "Hello")));
        let c = parse_scene_chunk(&d, 1, Region::Ntsc).unwrap();
        assert_eq!(c.camera.len(), 97);
        assert_eq!(c.camera[0].eye[0], 96.0);
        assert!(c.camera[11].is_cut() && !c.camera[10].is_cut());
        assert_eq!(c.actors.len(), 2);
        assert_eq!(c.actors[1].class, 10);
        assert_eq!(c.actors[0].sequence.frames.len(), 49);
        assert_eq!(c.actors[0].positions[3], [(48 + 3) as f32 * 2.0, 0.0, 0.0, 0.0]);
        assert_eq!(c.subtitles.len(), 1);
        assert_eq!(c.subtitles[0].text[2], b"Hello");
        assert!(c.subtitles[0].covers(150) && !c.subtitles[0].covers(151));
        // Last chunk of a 200-tick scene: 200 − 2·96 = 8 ticks → 9 records.
        let d = synth_chunk(200, 2, 1, 6, &[], None);
        let c = parse_scene_chunk(&d, 2, Region::Ntsc).unwrap();
        assert_eq!((c.camera.len(), c.subtitles.len()), (9, 0));
        assert!(parse_scene_chunk(&d, 3, Region::Ntsc).is_err());
    }

    #[test]
    fn scene_camera_and_chunk_rollover() {
        let chunks: Vec<SceneChunk> = (0..3).map(|i| parse_scene_chunk(&synth_chunk(200, i, 1, 49, &[107], None), i, Region::Ntsc).unwrap()).collect();
        let s = Scene::from_chunks(5, Region::Ntsc, chunks).unwrap();
        assert_eq!(s.end_tick(), 200);
        assert_eq!(s.chunk_at(95), (0, 95));
        assert_eq!(s.chunk_at(96), (1, 0));
        for t in 1..200 { assert_eq!(s.camera_at(t).unwrap().eye[0], t as f32); }
        assert_eq!(s.cut_ticks(), [107]);
        assert!(Scene::from_chunks(5, Region::Ntsc, vec![]).is_err());
    }

    #[test]
    fn region_table_slices_the_region_file() {
        let t = RegionTable::new(&[100, 103, 110, 111]).unwrap().unwrap();
        assert_eq!(t.chunks, [ChunkRange { sector: 100, sectors: 3 }, ChunkRange { sector: 103, sectors: 7 }, ChunkRange { sector: 110, sectors: 1 }]);
        assert_eq!(t.file_bytes(), 12 * 0x800);
        assert_eq!(t.chunk_in_file(1), Some(3 * 0x800..10 * 0x800));
        assert!(RegionTable::new(&[]).unwrap().is_none());
        assert!(RegionTable::new(&[5, 4]).is_err());
    }
}
