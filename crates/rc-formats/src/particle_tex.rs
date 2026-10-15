//! Particle textures, particle frame lists (`part_defs`) and FX textures of a level.
//! Specs: docs/formats/textures_rac1.md §8, docs/plan/particles.md §6. Ported from the retired
//! C++ reference extractor (git 2230812); `tests/formats/golden.rs` checks every section against the committed snapshot table (`data/loader_snapshots.tsv`).
//!
//! Level load (`ParseParticleTexs`, level01 0x253648 = boot 0x2026c8) reads three core-index tables:
//!
//! * **`part_textures`** (core header +0x50 count / +0x54 index offset): [`PartTextureEntry`], 0x10 bytes.
//! * **`part_defs`** (core header +0x6c, index offset): [`PartDefs`], one frame list per particle *type*.
//! * the part bank (core header +0x64, core_data offset) holds every palette and pixel block.
//!
//! and fills the runtime table 0x1b1f00 (128 × u64, [`PartTextureEntry::runtime_words`]), the def pointer
//! table 0x1b2500 (81 pointers) and the def blob copy 0x1b2700. FX textures (`GetEffectTex`, level01 0x21ae98)
//! come from +0x58/+0x5c and the FX bank (+0x68); they share the paging queue but are not particles.
//!
//! Every texture is 8-bit indexed (PSMT8) with its own 256 × RGBA32 CLUT in CSM1 order, alpha 0x80 = 1.0,
//! decoded with [`crate::texture::decode_indexed8`] (the level-texture rules).

use crate::buf::{invalid, Buf, Result};
use crate::level::LevelCore;
use crate::texture::{IndexedImage, Texture};
use bytemuck::{Pod, Zeroable};

/// Number of particle types (entries of the update table 0x1b2300 and of `part_defs`).
pub const PART_TYPES: usize = 81;

/// One `part_textures` entry (0x10 bytes, core index).
///
/// | Off | Type | Field | Meaning |
/// |---|---|---|---|
/// | 0x0 | s32 | `palette` | CLUT byte offset in the part bank (1024 bytes, CSM1) |
/// | 0x4 | s32 | `unk4` | CLUT slot: TEX0.CSA; the upload sends 0x400 − unk4·0x100 CLUT bytes (0 on every retail level) |
/// | 0x8 | s32 | `texture` | pixel byte offset in the part bank (`side²` bytes) |
/// | 0xc | s32 | `side` | edge length; square (32 on every retail level) |
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct PartTextureEntry {
    pub palette: i32,
    pub unk4: i32,
    pub texture: i32,
    pub side: i32,
}

impl PartTextureEntry {
    /// The two words `ParseParticleTexs` stores in 0x1b1f00[i] for a part bank at EE address `bank`:
    /// `lo = (bank + palette)·16 + unk4`, `hi = (bank + texture)·16 + log2(side)` (`fun_001f97a0` = log2).
    /// `_part_load_tex` (0x27d46c) unpacks them: CLUT address `lo >> 4`, CSA `lo & 0xf`, image `hi >> 4`.
    pub fn runtime_words(&self, bank: u32) -> (u32, u32) {
        let lo = bank.wrapping_add(self.palette as u32).wrapping_mul(16).wrapping_add(self.unk4 as u32);
        let hi = bank.wrapping_add(self.texture as u32).wrapping_mul(16).wrapping_add(log2_side(self.side));
        (lo, hi)
    }
}

/// `fun_001f97a0`: the exponent of a power of two (the TW/TH field).
fn log2_side(side: i32) -> u32 { if side > 0 { 31 - (side as u32).leading_zeros() } else { 0 } }

/// One `fx_textures` entry (0x10 bytes, core index); all −1 = absent (none on the retail disc).
///
/// | Off | Type | Field | Meaning |
/// |---|---|---|---|
/// | 0x0 | s32 | `palette` | CLUT byte offset in the FX bank |
/// | 0x4 | s32 | `texture` | pixel byte offset in the FX bank |
/// | 0x8 | s32 | `width` | pixels (power of two) |
/// | 0xc | s32 | `height` | pixels (power of two) |
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct FxTextureEntry {
    pub palette: i32,
    pub texture: i32,
    pub width: i32,
    pub height: i32,
}

impl FxTextureEntry {
    pub fn present(&self) -> bool { self.width > 0 && self.height > 0 && self.palette >= 0 && self.texture >= 0 }
}

/// `part_defs` (core index +0x6c): header `{count = 81, texture count, data_off, data_size}`, `count` s32
/// offsets (relative to the start of `part_defs`), and at `data_off` a blob of u8 part-texture indices.
///
/// `ParseParticleTexs`: pointer[type] = blob + (offset − data_off), or the blob start when offset = 0.
/// The game stores no frame counts: each update function knows how many frames its type animates
/// through (type 1: `def[(T − t)·9 / T]`); a spawner takes `*def[type]` as the first frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PartDefs {
    /// `{count, texture count, data_off, data_size}` as stored.
    pub header: [i32; 4],
    /// Raw per-type offsets (relative to `part_defs`; 0 = null).
    pub offsets: Vec<i32>,
    /// The index blob (`data_size` bytes), copied to 0x1b2700 by the game.
    pub blob: Vec<u8>,
}

impl PartDefs {
    /// Index into [`PartDefs::blob`] of type `ty`'s frame list, by the game's rule (null → 0).
    pub fn start(&self, ty: usize) -> Option<usize> {
        let off = *self.offsets.get(ty)?;
        let s = if off == 0 { 0 } else { off.checked_sub(self.header[2])? as usize };
        (s < self.blob.len()).then_some(s)
    }

    /// `*def[ty]`: the first frame (a `part_textures` index) as every spawner reads it.
    pub fn first_frame(&self, ty: usize) -> Option<u8> { self.start(ty).map(|s| self.blob[s]) }

    /// Type `ty`'s frames up to the next distinct list start (Wrench's reading of the table; the game
    /// itself only indexes from [`PartDefs::start`]). Null types share the run at the blob start.
    pub fn frames(&self, ty: usize) -> &[u8] {
        let Some(s) = self.start(ty) else { return &[] };
        let end = (0..self.offsets.len()).filter_map(|t| self.start(t)).filter(|&t| t > s).min().unwrap_or(self.blob.len());
        &self.blob[s..end]
    }
}

/// Everything particle- and FX-texture related of one level.
#[derive(Clone, Debug)]
pub struct ParticleTextures {
    pub entries: Vec<PartTextureEntry>,
    /// Decoded `entries[i]`, RGBA8 (alpha scaled 0x80 → 0xff), `side × side`.
    pub textures: Vec<Texture>,
    pub defs: PartDefs,
    pub fx_entries: Vec<FxTextureEntry>,
    /// Decoded `fx_entries[i]`; `None` for an absent entry.
    pub fx_textures: Vec<Option<Texture>>,
}

/// Reads a core-index `{count, offset}` table of 0x10-byte records.
fn table<T: Pod>(idx: Buf, count: i32, offset: i32, what: &'static str) -> Result<Vec<T>> {
    if count <= 0 || offset <= 0 { return Ok(Vec::new()); }
    if count > 0x1000 { return invalid(format!("implausible count for {what}")); }
    idx.pod_slice(offset as usize, count as usize, what)
}

/// Parses `part_defs` at `offset` in the core index.
pub fn parse_part_defs(index: &[u8], offset: i32) -> Result<PartDefs> {
    let idx = Buf(index);
    if offset <= 0 { return invalid("level has no part_defs"); }
    let base = offset as usize;
    let header: [i32; 4] = idx.pod(base, "part_defs header")?;
    let [count, _, data_off, data_size] = header;
    if !(0..=0x100).contains(&count) { return invalid("implausible part_defs count"); }
    if data_off < 0 || data_size < 0 { return invalid("negative part_defs blob range"); }
    let offsets = idx.pod_slice::<i32>(base + 0x10, count as usize, "part_defs offsets")?;
    let blob = idx.sub(base + data_off as usize, data_size as usize, "part_defs blob")?.bytes().to_vec();
    Ok(PartDefs { header, offsets, blob })
}

/// A core-data bank as bounded by the core's block list (`LevelCore::blocks`, name `part_bank` / `fx_bank`).
fn bank<'a>(core: &LevelCore, core_data: &'a [u8], name: &str) -> Result<Buf<'a>> {
    let Some(b) = core.blocks.iter().find(|b| b.name == name) else { return invalid(format!("level core has no {name} block")) };
    Buf(core_data).sub(b.offset, b.size, "particle bank")
}

/// Decodes one 8-bit image with its CLUT from `bank`.
pub fn decode_bank_texture(bank: &[u8], palette: i32, texture: i32, width: i32, height: i32) -> Result<Texture> {
    bank_texture_image(bank, palette, texture, width, height)?.decode()
}

/// The stored indices and CLUT of one bank image, the parts [`decode_bank_texture`] decodes.
pub fn bank_texture_image(bank: &[u8], palette: i32, texture: i32, width: i32, height: i32) -> Result<IndexedImage<'_>> {
    if palette < 0 || texture < 0 || width <= 0 || height <= 0 { return invalid("bank texture with negative offset or size"); }
    let b = Buf(bank);
    let px = b.sub(texture as usize, width as usize * height as usize, "bank texture pixels")?;
    let clut = b.sub(palette as usize, 1024, "bank texture palette")?;
    Ok(IndexedImage { width: width as u32, height: height as u32, indices: px.bytes(), clut: clut.bytes() })
}

/// The part bank (`"part_bank"`) or FX bank (`"fx_bank"`) of a level's core data, as [`parse_particle_textures`]
/// bounds it; entries' `palette`/`texture` offsets are relative to it.
pub fn core_bank<'a>(core: &LevelCore, core_data: &'a [u8], name: &str) -> Result<&'a [u8]> { bank(core, core_data, name).map(|b| b.bytes()) }

/// Parses and decodes the particle tables of a level. `index` is the raw core index (the tables are
/// index-relative), `core_data` the whole decompressed core data (the banks are data-relative).
pub fn parse_particle_textures(core: &LevelCore, index: &[u8], core_data: &[u8]) -> Result<ParticleTextures> {
    let h = &core.header;
    let idx = Buf(index);
    let entries: Vec<PartTextureEntry> = table(idx, h.part_textures.count, h.part_textures.offset, "part_textures")?;
    let fx_entries: Vec<FxTextureEntry> = table(idx, h.fx_textures.count, h.fx_textures.offset, "fx_textures")?;
    let defs = parse_part_defs(index, h.part_defs_offset)?;
    let part = if entries.is_empty() { Buf(&[]) } else { bank(core, core_data, "part_bank")? };
    let textures = entries
        .iter()
        .map(|e| decode_bank_texture(part.bytes(), e.palette, e.texture, e.side, e.side))
        .collect::<Result<Vec<_>>>()?;
    let fx = if fx_entries.iter().any(FxTextureEntry::present) { bank(core, core_data, "fx_bank")? } else { Buf(&[]) };
    let fx_textures = fx_entries
        .iter()
        .map(|e| if e.present() { decode_bank_texture(fx.bytes(), e.palette, e.texture, e.width, e.height).map(Some) } else { Ok(None) })
        .collect::<Result<Vec<_>>>()?;
    Ok(ParticleTextures { entries, textures, defs, fx_entries, fx_textures })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defs() -> PartDefs {
        // data_off 0x20; types: 0 null, 1 at blob 0 (3 frames), 2 at blob 3 (2 frames), 3 null.
        PartDefs { header: [4, 5, 0x20, 5], offsets: vec![0, 0x20, 0x23, 0], blob: vec![0, 1, 2, 4, 3] }
    }

    #[test]
    fn part_defs_null_rule_and_runs() {
        let d = defs();
        assert_eq!(d.start(0), Some(0));
        assert_eq!(d.start(2), Some(3));
        assert_eq!(d.first_frame(2), Some(4));
        assert_eq!(d.frames(1), &[0, 1, 2]);
        assert_eq!(d.frames(0), &[0, 1, 2], "null types share the blob start");
        assert_eq!(d.frames(2), &[4, 3]);
        assert_eq!(d.start(9), None);
    }

    #[test]
    fn parse_part_defs_reads_header_offsets_and_blob() {
        let mut idx = vec![0u8; 0x10];
        for v in [2i32, 7, 0x18, 3, 0, 0x19] { idx.extend_from_slice(&v.to_le_bytes()); }
        idx.extend_from_slice(&[9, 8, 7]);
        let d = parse_part_defs(&idx, 0x10).unwrap();
        assert_eq!(d.header, [2, 7, 0x18, 3]);
        assert_eq!(d.offsets, [0, 0x19]);
        assert_eq!(d.blob, [9, 8, 7]);
        assert_eq!(d.first_frame(1), Some(8));
        assert!(parse_part_defs(&idx[..idx.len() - 1], 0x10).is_err());
        assert!(parse_part_defs(&idx, 0).is_err());
    }

    #[test]
    fn runtime_words_pack_address_csa_and_log2() {
        let e = PartTextureEntry { palette: 0x800, unk4: 0, texture: 0xc00, side: 32 };
        let (lo, hi) = e.runtime_words(0x0010_0000);
        assert_eq!(lo >> 4, 0x0010_0800);
        assert_eq!(lo & 0xf, 0);
        assert_eq!(hi >> 4, 0x0010_0c00);
        assert_eq!(hi & 0xf, 5);
    }

    #[test]
    fn bank_texture_uses_its_own_clut() {
        let mut bank = vec![0u8; 1024 + 4];
        // stored CLUT entry 8 is linear entry 16 (CSM1): put it at stored index 8 and read index 16.
        bank[8 * 4..8 * 4 + 4].copy_from_slice(&[10, 20, 30, 0x40]);
        bank[1024..].copy_from_slice(&[16, 0, 16, 0]);
        let t = decode_bank_texture(&bank, 0, 1024, 2, 2).unwrap();
        assert_eq!(&t.rgba[..4], &[10, 20, 30, 0x80]);
        assert!(decode_bank_texture(&bank, 0, 1024, 4, 4).is_err());
    }
}
