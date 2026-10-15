//! The `transition` lump (global TOC, `global/transition.bin`, WAD): what `EnterSpaceLoadingLoop` (level01 0x2a5868;
//! the code is identical on all 19 overlays) loads for the flight between planets, and what the flight's update
//! (`SpaceLoadingLoop` 0x2a33b0) and draw (`DrawWorldPaused` 0x2a3b90) read. The lump decompresses to a header of u32
//! words and a data block at `base = hdr[1]` (0x9800); "data" offsets below are relative to `base`, "lump" offsets
//! to the start.
//!
//! | word | what | read by | here |
//! |---|---|---|---|
//! | 0, 2, 3 | the GS upload: data at lump `hdr[0]` (0x800), `hdr[2]` (11) entries `{psm, w, h, address, offset}` at lump `hdr[3]` (the gs_ram entries' layout; VRAM address = data offset) | `fun_00203120` | [`TransitionLump::gs_image`] |
//! | 4, 5 | 3 moby classes, 8-word entries at lump `hdr[5]` (`{data offset, o_class, 0, 0, textures[16]}`): 531, 532, 533 | `MobyClassesRelocate` | [`TransitionLump::classes`]; the blobs, their textures (texture table `hdr[7]`, `hdr[6]` entries, pixels at data `hdr[0xc]`) and CLUTs (GS image 0x4400 / 0x5c00 / 0x7400) are byte-identical to the `spaceships` files' ship classes (checked 2026-10-01), which the port loads |
//! | 8, 9, 0xd, 0xf | 2 particle textures (table lump `hdr[9]`, bank data `hdr[0xd]`, defs lump `hdr[0xf]`) | `ParseParticleTexs` | n/a: the flight's draw runs no particle pass (`DrawWorldPaused` has no `part_proc`) |
//! | 0xa, 0xb, 0xe | 4 FX textures `{palette, texture, width, height}` at lump `hdr[0xb]`, the bank at data `hdr[0xe]` (`0x15f420`, the bank `GetEffectTex` reads during the flight): FX 0 the trail, FX 1 the canopy glass's map, FX 2 / 3 the memory-card icon | `fun_00202800` | [`TransitionLump::fx_textures`] |
//! | 0x10, 0x11 | the moby chrome map TEX0 (texture GS 0x400, CLUT GS 0): byte-identical to every level's (checked 2026-10-01) | `0x182c40` | n/a (the level's map) |
//! | 0x12 | the sky block (6 shells) | `LoadSky` | [`TransitionLump::sky_block`] |
//! | 0x13 | the picture table: `u32 count` (133), then data offsets from the table: entries 0..19 the planet pictures (128×128 PIF), 19 + 19·(lang − 1) + level the area captions (256×32 PIF) | `fun_00202270` | [`TransitionLump::planet_picture`], [`TransitionLump::caption`] |
//! | 0x14..0x18 | the five flight variants (space-scene chunk lumps; one actor of class 533) | `EnterSpaceLoadingLoop`, `SpaceLoadingLoop` | [`TransitionLump::variant`] |
//! | 0x19 | the flight's 989snd bank (5 sounds: sound v plays at the variant's tick 1) | `snd_bank_load_from_ee_cb` | [`TransitionLump::sound_bank`] |

use crate::buf::{invalid, Buf, Result};
use crate::pif::Pif;
use crate::texture::Texture;

/// The flight variants (0..=3 random, 4 the planet approach).
pub const VARIANTS: usize = 5;
/// Levels with a picture and a caption.
pub const LEVELS: usize = 19;
/// Caption languages (`0x15ed88 − 1`, at least 0).
pub const CAPTION_LANGUAGES: usize = 6;

/// The decompressed lump.
#[derive(Clone, Debug)]
pub struct TransitionLump {
    pub bytes: Vec<u8>,
    /// `hdr[1]`: the data block.
    pub base: usize,
}

impl TransitionLump {
    /// From the lump's bytes (WAD or decompressed).
    pub fn parse(lump: &[u8]) -> Result<TransitionLump> {
        let bytes = if crate::wad::is_wad(lump) { crate::wad::decompress(lump)? } else { lump.to_vec() };
        let base = Buf(&bytes).u32(4)? as usize;
        if base == 0 || base > bytes.len() { return invalid(format!("transition lump: data base {base:#x}")); }
        Ok(TransitionLump { bytes, base })
    }

    /// Header word `i`.
    pub fn word(&self, i: usize) -> Result<usize> { Buf(&self.bytes).u32(4 * i).map(|v| v as usize) }

    /// Data-relative offset `o` as a lump offset.
    fn data(&self, o: usize) -> usize { self.base + o }

    /// Variant `v`'s chunk lump (`hdr[0x14 + v]`, to the next variant or the sound bank).
    pub fn variant(&self, v: usize) -> Result<&[u8]> {
        if v >= VARIANTS { return invalid(format!("transition lump: no flight variant {v}")); }
        let at = self.data(self.word(0x14 + v)?);
        let end = if v + 1 < VARIANTS { self.data(self.word(0x15 + v)?) } else { self.data(self.word(0x19)?) };
        Ok(Buf(&self.bytes).sub(at, end.saturating_sub(at), "flight variant")?.bytes())
    }

    /// The sky block (`hdr[0x12]`, to the picture table).
    pub fn sky_block(&self) -> Result<&[u8]> {
        let (at, end) = (self.data(self.word(0x12)?), self.data(self.word(0x13)?));
        Ok(Buf(&self.bytes).sub(at, end.saturating_sub(at), "flight sky")?.bytes())
    }

    /// Picture `k` of the table at data `hdr[0x13]` (`fun_00202270(table + entry[k], TEX0)`).
    pub fn picture(&self, k: usize) -> Result<Pif<'_>> {
        let b = Buf(&self.bytes);
        let table = self.data(self.word(0x13)?);
        let count = b.u32(table)? as usize;
        if k >= count { return invalid(format!("transition lump: picture {k} of {count}")); }
        let off = b.u32(table + 4 + 4 * k)? as usize;
        Pif::parse_at(&self.bytes, table + off)
    }

    /// The destination's planet picture (`DAT_00160640`, entry `level`).
    pub fn planet_picture(&self, level: i32) -> Result<Pif<'_>> {
        let Some(k) = usize::try_from(level).ok().filter(|&l| l < LEVELS) else { return invalid(format!("transition lump: no picture for level {level}")) };
        self.picture(k)
    }

    /// The destination's area caption (`0x160648`): entry `19 + 19·max(lang − 1, 0) + level`, `lang` = 0x15ed88.
    pub fn caption(&self, lang: i32, level: i32) -> Result<Pif<'_>> {
        let l = (lang - 1).max(0) as usize;
        let Some(level) = usize::try_from(level).ok().filter(|&x| x < LEVELS) else { return invalid(format!("transition lump: no caption for level {level}")) };
        self.picture(LEVELS + LEVELS * l + level)
    }

    /// The FX textures `GetEffectTex` reads during the flight (decoded like the level's FX textures: alpha scaled to
    /// 0..0xff, `texture::decode_indexed8`); None = an absent entry.
    pub fn fx_textures(&self) -> Result<Vec<Option<Texture>>> {
        let b = Buf(&self.bytes);
        let (count, table) = (self.word(0xa)?, self.word(0xb)?);
        if count > 64 { return invalid(format!("transition lump: {count} FX textures")); }
        let bank = self.bytes.get(self.data(self.word(0xe)?)..).unwrap_or(&[]);
        (0..count)
            .map(|i| {
                let e = |k: usize| b.i32(table + 16 * i + 4 * k);
                let (pal, tex, w, h) = (e(0)?, e(1)?, e(2)?, e(3)?);
                if pal < 0 || tex < 0 || w <= 0 || h <= 0 { return Ok(None); }
                crate::particle_tex::decode_bank_texture(bank, pal, tex, w, h).map(Some)
            })
            .collect()
    }

    /// The flight's sound bank (`hdr[0x19]` to the end; a 989snd bank file).
    pub fn sound_bank(&self) -> Result<&[u8]> {
        let at = self.data(self.word(0x19)?);
        Ok(Buf(&self.bytes).tail(at, "flight sound bank")?.bytes())
    }

    /// The moby classes: `(o_class, data offset)` (documentation: the port draws the `spaceships` copies).
    pub fn classes(&self) -> Result<Vec<(i32, usize)>> {
        let b = Buf(&self.bytes);
        let (count, table) = (self.word(4)?, self.word(5)?);
        (0..count.min(16)).map(|i| Ok((b.i32(table + 32 * i + 4)?, b.u32(table + 32 * i)? as usize))).collect()
    }

    /// The GS upload's image (lump `hdr[0]` to the data block): VRAM address = offset here.
    pub fn gs_image(&self) -> Result<&[u8]> {
        let at = self.word(0)?;
        Ok(Buf(&self.bytes).sub(at, self.base.saturating_sub(at), "flight GS image")?.bytes())
    }
}
