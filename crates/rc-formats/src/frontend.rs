//! The front end's own data (the boot program, before any level): the title world lump (TOC field 0x14e8,
//! `global/unknown_14e8.bin`, WAD) and the boot pictures in the IRX lump (TOC field 0x12c0, `global/irx.bin`, WAD).
//! Spec: docs/plan/progression.md `## saves` ("Boot → title").
//!
//! **The title world** (`transition_load_wad` boot 0x1ea830): a WAD that decompresses to a header of u32 words (offsets
//! from the start unless noted) and the data block at `base = hdr[1]`. Read here:
//! * `hdr[0x21] + base`: the title logo, 256×128 PSMCT32 (uploaded with `sce_gs_set_def_load_image(…, 256, 128)`),
//!   drawn by `transition_default_draw` at (0xec, 0x10).
//! * `hdr[0x16]` / `hdr[0x17]`: the FX texture entries (`{palette, texture, width, height}`, 0x10 bytes), the bank at
//!   `hdr[0x1a] + base` (`0x15f460`, the bank `GetEffectTex` reads): FX 1..3 the fonts, FX 4..8 "PRESS START" in the five
//!   languages (256×128, one palette), the rest the space particles.
//!
//! **The title world as level data** ([`TitleWorld`], G-SAV-012): `transition_load_wad` loads the lump through the
//! level loader's own routines, and its header is a level core header with the words in another order: the GS upload
//! (data `hdr[0]`, `hdr[2]` entries of the gs_ram layout at `hdr[3]`), the tfrags `hdr[4]` (`fun_002040e0`), the sky
//! `hdr[5]` (`fun_002028e0`), the moby / tie / shrub classes `hdr[6..=0xb]` (`fun_00203640` / `fun_00203730` /
//! `fun_00203b08`, the core's 0x20 / 0x20 / 0x30-byte class entries), the tfrag / moby / tie / shrub texture tables
//! `hdr[0xc..=0x13]`, the particle and FX textures `hdr[0x14..=0x17]`, the texture data `hdr[0x18]`, the particle /
//! FX banks `hdr[0x19]` / `hdr[0x1a]`, the particle defs `hdr[0x1b]`, the moby chrome map TEX0 `hdr[0x1c]` / `hdr[0x1d]`,
//! then the gameplay block `hdr[0x1f]` (a gameplay-file image: `level_init_read_settings` 0x1e9b10 reads its level
//! settings, lights, tie instances `+0xd` and shrub instances `+0xf`; the 41 moby instances it also holds are never
//! loaded), the space-scene chunks `hdr[0x20]` (the camera and the actors, `parse_space_scene_chunk`) and the logo
//! `hdr[0x21]`. [`TitleWorld::parse`] rebuilds a core index in the level layout over the same tables, so the level
//! parsers read the title world unchanged.
//!
//! **The boot pictures** (`init_once` boot 0x201650 decompresses the lump to 0x500000; `startlevel` 0x1e9658 draws them
//! with `fun_002012b8`): `{u32 offset, u32 size}` pairs, each a WAD of a raw 512×416 PSMCT32 frame: +0x00 NTSC (+0x08 PAL)
//! the still shown while the title loads, +0x10 + 8·language the "no memory card" warning, +0x40 + 8·language the
//! "insufficient space" warning; from +0x70 the IOP modules.

use crate::buf::{invalid, Buf, Result};
use crate::level::{ArrayRange, LevelCore, LevelCoreHeader};
use crate::texture::Texture;

/// The boot pictures' size (the GS frame).
pub const FRAME_W: u32 = 512;
pub const FRAME_H: u32 = 416;
/// The logo's size.
pub const LOGO_W: u32 = 256;
pub const LOGO_H: u32 = 128;

/// The title world's pictures.
#[derive(Clone, Debug)]
pub struct TitleWad {
    /// The logo (raw GS RGBA, alpha 0..0x80).
    pub logo: Texture,
    /// FX textures by `GetEffectTex` index (raw GS RGBA); None = absent entry.
    pub fx: Vec<Option<Texture>>,
}

impl TitleWad {
    /// From the lump's bytes (WAD or decompressed).
    pub fn parse(lump: &[u8]) -> Result<TitleWad> {
        let bytes = if crate::wad::is_wad(lump) { crate::wad::decompress(lump)? } else { lump.to_vec() };
        let b = Buf(&bytes);
        let h = |i: usize| b.u32(i * 4).map(|v| v as usize);
        let base = h(1)?;
        let logo_at = base + h(0x21)?;
        let n = (LOGO_W * LOGO_H * 4) as usize;
        let logo = Texture { width: LOGO_W, height: LOGO_H, rgba: b.sub(logo_at, n, "title logo")?.bytes().to_vec() };
        let (count, table, bank_at) = (h(0x16)?, h(0x17)?, base + h(0x1a)?);
        if count > 256 { return invalid(format!("title world: {count} FX textures")); }
        let bank = bytes.get(bank_at..).unwrap_or(&[]);
        let mut fx = Vec::with_capacity(count);
        for i in 0..count {
            let e = |k: usize| b.i32(table + 16 * i + 4 * k);
            let (pal, tex, w, hh) = (e(0)?, e(1)?, e(2)?, e(3)?);
            let t = if pal >= 0 && tex >= 0 && w > 0 && hh > 0 {
                let px = Buf(bank).sub(tex as usize, (w * hh) as usize, "fx pixels").ok();
                let clut = Buf(bank).sub(pal as usize, 1024, "fx palette").ok();
                match (px, clut) {
                    (Some(p), Some(c)) => crate::hud::decode_indexed8_raw(p.bytes(), w as u32, hh as u32, c.bytes()).ok(),
                    _ => None,
                }
            } else {
                None
            };
            fx.push(t);
        }
        Ok(TitleWad { logo, fx })
    }

    /// "PRESS START" of language `lang` (0x15ed88): `GetEffectTex(max(lang − 1, 0) + 4)`.
    pub fn press_start(&self, lang: u32) -> Option<&Texture> { self.fx.get((lang as usize).saturating_sub(1) + 4)?.as_ref() }
}

/// The boot pictures of the IRX lump.
#[derive(Clone, Debug)]
pub struct BootPictures {
    bytes: Vec<u8>,
}

/// Which boot picture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BootPicture {
    /// The still shown while the title loads (+0x00 NTSC, +0x08 PAL).
    Still { pal: bool },
    /// `fun_00209168` = 1: no memory card (+0x10 + 8·language).
    NoCard { lang: u32 },
    /// `fun_00209168` = 2: not enough space (+0x40 + 8·language).
    NoSpace { lang: u32 },
}

impl BootPictures {
    pub fn parse(lump: &[u8]) -> Result<BootPictures> {
        let bytes = if crate::wad::is_wad(lump) { crate::wad::decompress(lump)? } else { lump.to_vec() };
        Buf(&bytes).check(0, 0x70, "IRX lump header")?;
        Ok(BootPictures { bytes })
    }

    /// The picture (raw GS RGBA, 512×416).
    pub fn picture(&self, p: BootPicture) -> Result<Texture> {
        let field = match p {
            BootPicture::Still { pal } => if pal { 8 } else { 0 },
            BootPicture::NoCard { lang } => 0x10 + 8 * lang as usize,
            BootPicture::NoSpace { lang } => 0x40 + 8 * lang as usize,
        };
        let b = Buf(&self.bytes);
        let (off, size) = (b.u32(field)? as usize, b.u32(field + 4)? as usize);
        let raw = crate::wad::decompress(b.sub(off, size, "boot picture")?.bytes())?;
        let n = (FRAME_W * FRAME_H * 4) as usize;
        if raw.len() < n { return invalid(format!("boot picture: {} bytes", raw.len())); }
        Ok(Texture { width: FRAME_W, height: FRAME_H, rgba: raw[..n].to_vec() })
    }
}

/// The title world (`global/unknown_14e8.bin`) as level data (module docs).
#[derive(Debug)]
pub struct TitleWorld {
    /// A core index in the level layout: the rebuilt [`LevelCoreHeader`] then the lump's header region (its tables) at
    /// [`TitleWorld::INDEX_TABLES`].
    pub index: Vec<u8>,
    /// The parsed core (blocks by the level's boundary rule over [`TitleWorld::data`]).
    pub core: LevelCore,
    /// The data block (`hdr[1]` to the end): the core data.
    pub data: Vec<u8>,
    /// The GS upload image (lump `hdr[0]` to the data block; VRAM address = offset).
    pub gs: Vec<u8>,
    /// The gameplay block (`hdr[0x1f]` to the end of the data).
    pub gameplay: Vec<u8>,
    /// The space-scene chunk lump (`hdr[0x20]` to the logo `hdr[0x21]`).
    pub scene: Vec<u8>,
}

impl TitleWorld {
    /// Where the lump's header region (its tables) starts in [`TitleWorld::index`].
    pub const INDEX_TABLES: usize = 0x100;

    /// From the lump's bytes (WAD or decompressed).
    pub fn parse(lump: &[u8]) -> Result<TitleWorld> {
        let bytes = if crate::wad::is_wad(lump) { crate::wad::decompress(lump)? } else { lump.to_vec() };
        let b = Buf(&bytes);
        let h = |i: usize| b.i32(i * 4);
        let base = h(1)?;
        if base <= 0 || base as usize > bytes.len() { return invalid(format!("title world: data base {base:#x}")); }
        let base_u = base as usize;
        let p = Self::INDEX_TABLES as i32;
        let range = |n: usize, o: usize| -> Result<ArrayRange> { Ok(ArrayRange { count: h(n)?, offset: h(o)? + p }) };
        let mut hd: LevelCoreHeader = bytemuck::Zeroable::zeroed();
        hd.gs_ram = range(2, 3)?;
        hd.tfrags = h(4)?;
        hd.sky = h(5)?;
        hd.moby_classes = range(6, 7)?;
        hd.tie_classes = range(8, 9)?;
        hd.shrub_classes = range(0xa, 0xb)?;
        hd.tfrag_textures = range(0xc, 0xd)?;
        hd.moby_textures = range(0xe, 0xf)?;
        hd.tie_textures = range(0x10, 0x11)?;
        hd.shrub_textures = range(0x12, 0x13)?;
        hd.part_textures = range(0x14, 0x15)?;
        hd.fx_textures = range(0x16, 0x17)?;
        hd.textures_base_offset = h(0x18)?;
        hd.part_bank_offset = h(0x19)?;
        hd.fx_bank_offset = h(0x1a)?;
        hd.part_defs_offset = h(0x1b)? + p;
        hd.chrome_map_texture = h(0x1c)?;
        hd.chrome_map_palette = h(0x1d)?;
        let data = bytes[base_u..].to_vec();
        hd.assets_decompressed_size = data.len() as i32;
        let mut index = vec![0u8; Self::INDEX_TABLES];
        index[..std::mem::size_of::<LevelCoreHeader>()].copy_from_slice(bytemuck::bytes_of(&hd));
        index.extend_from_slice(&bytes[..base_u]);
        let core = crate::level::parse_level_core(&index, data.len())?;
        let gs_at = h(0)? as usize;
        let gs = b.sub(gs_at, base_u.saturating_sub(gs_at), "title world GS image")?.bytes().to_vec();
        let gameplay = Buf(&data).tail(h(0x1f)? as usize, "title world gameplay block")?.bytes().to_vec();
        let (scene_at, logo_at) = (h(0x20)? as usize, h(0x21)? as usize);
        let scene = Buf(&data).sub(scene_at, logo_at.saturating_sub(scene_at), "title world scene chunks")?.bytes().to_vec();
        Ok(TitleWorld { index, core, data, gs, gameplay, scene })
    }
}
