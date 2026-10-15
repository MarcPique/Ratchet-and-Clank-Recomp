//! RAC1 table of contents at sector 1500. Spec: docs/formats/disc_layout.md section 2.
//! The game itself reads 6 sectors at LBA 1500 and copies 0x2960 bytes to EE 0x137b80.

use crate::buf::{invalid, Buf, Result};
use bytemuck::{Pod, Zeroable};

pub const SECTOR_SIZE: u64 = 0x800;
pub const TOC_SECTOR: u64 = 1500;
pub const TOC_SIZE: usize = 0x2960;
pub const LEVEL_HEADER_SIZE: usize = 0x2434;
pub const LEVEL_TABLE_OFFSET: usize = 0x28c8;
pub const LEVEL_TABLE_COUNT: usize = 19;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct SectorRange { pub offset: i32, pub size: i32 }          // both in sectors
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct SectorByteRange { pub offset: i32, pub size: i32 }      // offset in sectors, size in bytes

/// Languages of a scene's speech VAG (`sounds[0x15ed88]`): 0 English, 1 unused (0 on the disc), 2 French,
/// 3 German, 4 Spanish, 5 Italian. Names used in the extracted file names (`speech/KK_<lang>.bin`).
pub const SCENE_LANGUAGES: [&str; 6] = ["en", "l1", "fr", "de", "es", "it"];
/// Chunk-sector slots per region (the last used one is a 1-sector sentinel).
pub const SCENE_CHUNK_SLOTS: usize = 71;
/// Scene records in the level header (`0x13a664 + id·0x250`; docs/plan/cutscenes_transitions.md §1).
pub const SCENE_RECORDS: usize = 15;

/// One in-engine scene (cutscene) record, 0x250 bytes: speech VAG sector per language, then the sectors of
/// the NTSC and the PAL chunk WADs (`fun_00215970` / `FUN_002591d0`). A chunk's size is the sector
/// difference to the next entry; the last entry is a 1-sector sentinel. (Wrench reads this block as
/// 30 × 0x128 `{sounds[6], wads[68]}`, which splits every scene into an NTSC and a PAL half.)
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct SceneRecord { pub speech: [i32; 6], pub ntsc: [i32; SCENE_CHUNK_SLOTS], pub pal: [i32; SCENE_CHUNK_SLOTS] }

impl SceneRecord {
    /// The chunk sector list of a region (`pal` = PAL), up to the first 0.
    pub fn chunk_sectors(&self, pal: bool) -> &[i32] {
        let all = if pal { &self.pal[..] } else { &self.ntsc[..] };
        &all[..all.iter().position(|&s| s == 0).unwrap_or(all.len())]
    }

    /// `(first sector, sector count)` of the region's contiguous run: every chunk plus the sentinel
    /// sector (`scene/KK_ntsc.bin` / `_pal.bin`). None when the region has no entry.
    pub fn region_sectors(&self, pal: bool) -> Option<(u32, u32)> {
        let s = self.chunk_sectors(pal);
        let (&first, &last) = (s.first()?, s.last()?);
        Some((first as u32, (last - first) as u32 + 1))
    }
}

/// Amalgamated level header, stored at the front of each level's sector run.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct LevelHeader {
    pub id: i32,
    pub header_size: i32,
    pub data: SectorRange,
    pub gameplay_ntsc: SectorRange,
    pub gameplay_pal: SectorRange,
    pub occlusion: SectorRange,
    pub bindata: [SectorByteRange; 36],
    pub music: [i32; 15],
    pub scenes: [SceneRecord; SCENE_RECORDS],
}
const _: () = assert!(std::mem::size_of::<LevelHeader>() == LEVEL_HEADER_SIZE);
const _: () = assert!(std::mem::size_of::<SceneRecord>() == 0x250);

/// Global-header field `save_game` (spec 2.3): the memory-card template lump (`docs/plan/game_state.md` §3.1).
pub const SAVE_GAME_FIELD: usize = 0x10;

/// A `SectorRange` field of the global header at byte offset `field` (e.g. [`SAVE_GAME_FIELD`]).
pub fn global_sector_range(toc: &[u8], field: usize) -> Result<SectorRange> {
    let b = Buf(toc);
    Ok(SectorRange { offset: b.i32(field)?, size: b.i32(field + 4)? })
}

/// The level table: absolute sector of each level header (0 = empty slot).
pub fn level_header_sectors(toc: &[u8]) -> Result<Vec<Option<u32>>> {
    let b = Buf(toc);
    if b.i32(0)? != 1 { return invalid("TOC: version != 1"); }
    let size = b.i32(4)?;
    if size <= 8 || size as usize > toc.len() { return invalid("TOC: implausible header size"); }
    (0..LEVEL_TABLE_COUNT)
        .map(|i| Ok(match b.i32(LEVEL_TABLE_OFFSET + i * 8)? { 0 => None, s if s > 0 => Some(s as u32), _ => return invalid("TOC: negative level sector") }))
        .collect()
}

pub fn parse_level_header(bytes: &[u8]) -> Result<LevelHeader> {
    let h: LevelHeader = Buf(bytes).pod(0, "level header")?;
    if h.header_size as usize != LEVEL_HEADER_SIZE { return invalid("level header lacks the 0x2434 signature"); }
    Ok(h)
}

/// Size of a lump addressed by a bare `Sector32` (spec 2.5), from the first bytes at that sector:
/// a VAG header gives `0x30 + data_size` (big-endian), a WAD header its compressed size; anything
/// else is one sector. The bool says whether the size is exact (as the retired C++ `probe_lump`).
pub fn probe_lump_size(head: &[u8]) -> (u64, bool) {
    if head.len() >= 0x30 && &head[..4] == b"VAGp" {
        (0x30 + u32::from_be_bytes([head[0x0c], head[0x0d], head[0x0e], head[0x0f]]) as u64, true)
    } else if let Ok(n) = crate::wad::compressed_size(head) {
        (n as u64, true)
    } else {
        (SECTOR_SIZE, false)
    }
}

/// A level's audio or scene lump: `name` is the path the extractor writes it under
/// `levels/NN/` minus `.bin` (e.g. `music/003`, `bindata/017`, `speech/05_en`, `scene/05_ntsc`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamLump { pub name: String, pub sector: u32, pub bytes: u64 }

/// The audio (`bindata`, `music`) and scene lumps of a level, in the order and with the sizes
/// the retired C++ `level_lumps` produced. `probe(sector)` returns `probe_lump_size` of that
/// sector's first bytes. Per scene record k: `speech/KK_<lang>` (one VAG per language, probed size),
/// then `scene/KK_ntsc` and `scene/KK_pal` (the region's whole sector run, chunks + sentinel;
/// `crate::scene`).
pub fn level_stream_lumps<E>(h: &LevelHeader, mut probe: impl FnMut(u32) -> std::result::Result<u64, E>) -> std::result::Result<Vec<StreamLump>, E> {
    let mut out = Vec::new();
    for (i, r) in h.bindata.iter().enumerate() {
        if r.offset == 0 && r.size == 0 { continue; }
        out.push(StreamLump { name: format!("bindata/{i:03}"), sector: r.offset as u32, bytes: r.size as u32 as u64 });
    }
    for (i, &s) in h.music.iter().enumerate() {
        if s != 0 { out.push(StreamLump { name: format!("music/{i:03}"), sector: s as u32, bytes: probe(s as u32)? }); }
    }
    for (k, scene) in h.scenes.iter().enumerate() {
        for (lang, &s) in SCENE_LANGUAGES.iter().zip(&scene.speech) {
            if s != 0 { out.push(StreamLump { name: format!("speech/{k:02}_{lang}"), sector: s as u32, bytes: probe(s as u32)? }); }
        }
        for (pal, region) in [(false, "ntsc"), (true, "pal")] {
            if let Some((sector, sectors)) = scene.region_sectors(pal) {
                out.push(StreamLump { name: format!("scene/{k:02}_{region}"), sector, bytes: sectors as u64 * SECTOR_SIZE });
            }
        }
    }
    Ok(out)
}
