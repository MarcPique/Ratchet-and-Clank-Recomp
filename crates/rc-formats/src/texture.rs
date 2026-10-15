//! Level textures: 8-bit indexed pixels with a 256-entry RGBA32 CLUT in CSM1
//! order. Spec: docs/formats/textures_rac1.md (§3, §4, §9, §12, §12b).
//! Ported from the retired C++ reference extractor (git 2230812); `tests/formats/golden.rs` checks every section against the committed snapshot table (`data/loader_snapshots.tsv`).

use crate::buf::{invalid, Buf, Result};
use crate::level::{LevelCore, ShrubBillboardInfo};

/// One entry of the tfrag/moby/tie/shrub texture tables in the core index (0x10 bytes).
///
/// | Off | Type | Field | Meaning |
/// |---|---|---|---|
/// | 0x0 | s32 | `data_offset` | pixel bytes at `core_data[textures_base_offset + data_offset]` |
/// | 0x4 | s16 | `width` | pixels |
/// | 0x6 | s16 | `height` | pixels |
/// | 0x8 | s16 | `ty` | retail values 1..4; for tfrag textures the mip level count (tfrag_rac1.md §2.5.1) |
/// | 0xa | s16 | `palette` | CLUT at `gs_ram[palette * 0x100]`, 1024 bytes |
/// | 0xc | s16 | `mipmap` | tfrag textures: gs_ram block (0x100 bytes) of mip level 2 (tfrag_rac1.md §2.5.1) |
/// | 0xe | s16 | `pad` | tfrag textures: gs_ram block of mip level 3, -1 when `ty` = 3 |
pub use crate::level::TextureEntry;

/// RGBA8 image, row-major, top-down, alpha already scaled to 0..=255.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Texture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// PS2 CSM1 CLUT order -> linear: within each 32-entry group swap the two middle
/// 8-entry blocks (swap bits 3 and 4 of the index). An involution. Spec §4.3.
pub fn clut_index(i: u32) -> u32 {
    if ((i >> 3) & 1) != ((i >> 4) & 1) { i ^ 0x18 } else { i }
}

/// PS2 alpha 0..=0x80 -> 0..=0xff (0x80 = opaque; anything above clamps to opaque). Spec §4.4.
pub fn scale_alpha(a: u8) -> u8 {
    if a < 0x80 { a * 2 } else { 0xff }
}

/// Decodes `width * height` palette indices with a 256-entry RGBA32 CLUT (1024 bytes, CSM1 order).
pub fn decode_indexed8(indices: &[u8], width: u32, height: u32, clut: &[u8]) -> Result<Texture> {
    let n = width as usize * height as usize;
    if indices.len() < n { return invalid("indexed texture data too small"); }
    if clut.len() < 1024 { return invalid("palette too small"); }
    let mut pal = [[0u8; 4]; 256];
    for (i, p) in pal.iter_mut().enumerate() {
        let e = &clut[clut_index(i as u32) as usize * 4..][..4];
        *p = [e[0], e[1], e[2], scale_alpha(e[3])];
    }
    let mut rgba = Vec::with_capacity(n * 4);
    for &ix in &indices[..n] { rgba.extend_from_slice(&pal[ix as usize]); }
    Ok(Texture { width, height, rgba })
}

/// Which core-index table a texture came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextureTable { Tfrag, Moby, Tie, Shrub, Billboard }

impl TextureTable {
    /// Short table name (snapshot keys, texture keys, export folders).
    pub fn name(self) -> &'static str {
        match self {
            TextureTable::Tfrag => "tfrag",
            TextureTable::Moby => "moby",
            TextureTable::Tie => "tie",
            TextureTable::Shrub => "shrub",
            TextureTable::Billboard => "billboard",
        }
    }
}

/// Where a decoded texture's description came from.
#[derive(Clone, Copy, Debug)]
pub enum TextureSource {
    /// A `TextureEntry` of the tfrag/moby/tie/shrub table (pixels in core_data).
    Entry(TextureEntry),
    /// A shrub class's billboard (pixels and palette in gs_ram, spec §9).
    Billboard { o_class: i32, info: ShrubBillboardInfo },
}

#[derive(Clone, Debug)]
pub struct LevelTexture {
    pub table: TextureTable,
    /// Index into the table (what `ClassEntry::textures[slot]` and tfrag GS packets refer to);
    /// for billboards, the index into `LevelCore::shrub_classes`.
    pub index: usize,
    pub source: TextureSource,
    pub texture: Texture,
}

impl LevelTexture {
    /// The texture's key (the retired C++ extractor's file stem), e.g. `tfrag/000_128x128_t4`
    /// or `billboard/0123_32x64`.
    pub fn key(&self) -> String {
        match self.source {
            TextureSource::Entry(e) => format!("{}/{:03}_{}x{}_t{}", self.table.name(), self.index, e.width, e.height, e.ty),
            TextureSource::Billboard { o_class, info } => format!("billboard/{:04}_{}x{}", o_class, info.width, info.height),
        }
    }
}

/// One stored 8-bit indexed image: the `width × height` indices and the 256-entry RGBA32 CLUT (1024 bytes) exactly
/// as the disc holds them (CLUT in CSM1 order, alpha 0x80 = 1.0). [`IndexedImage::decode`] is the decode every
/// loader here applies; exporters use the raw parts to keep the indices and the CLUT.
#[derive(Clone, Copy, Debug)]
pub struct IndexedImage<'a> {
    pub width: u32,
    pub height: u32,
    pub indices: &'a [u8],
    pub clut: &'a [u8],
}

impl IndexedImage<'_> {
    /// [`decode_indexed8`] of the image.
    pub fn decode(&self) -> Result<Texture> { decode_indexed8(self.indices, self.width, self.height, self.clut) }
}

fn entry_image_in<'a>(tex: Buf<'a>, gs: Buf<'a>, e: &TextureEntry) -> Result<IndexedImage<'a>> {
    let px = tex.sub(e.data_offset as usize, e.width as usize * e.height as usize, "texture pixels")?;
    let clut = gs.sub(e.palette as usize * 0x100, 1024, "texture palette")?;
    Ok(IndexedImage { width: e.width as u32, height: e.height as u32, indices: px.bytes(), clut: clut.bytes() })
}

/// The stored image of a tfrag/moby/tie/shrub table entry (level 0 only), as [`parse_textures`] decodes it.
pub fn entry_image<'a>(core: &LevelCore, core_data: &'a [u8], gs_ram: &'a [u8], e: &TextureEntry) -> Result<IndexedImage<'a>> {
    let Some(blk) = core.blocks.iter().find(|b| b.name == "textures") else { return invalid("level core has no textures block"); };
    entry_image_in(Buf(core_data).sub(blk.offset, blk.size, "textures block")?, Buf(gs_ram), e)
}

/// The stored image of a shrub billboard (level 0 only), as [`parse_textures`] decodes it.
pub fn billboard_image<'a>(gs_ram: &'a [u8], b: &ShrubBillboardInfo) -> Result<IndexedImage<'a>> {
    let gs = Buf(gs_ram);
    if b.texture_offset < 0 || b.palette_offset < 0 { return invalid("negative billboard gs_ram offset"); }
    let px = gs.sub(b.texture_offset as usize * 0x100, b.width as usize * b.height as usize, "billboard pixels")?;
    let clut = gs.sub(b.palette_offset as usize * 0x100, 1024, "billboard palette")?;
    Ok(IndexedImage { width: b.width as u32, height: b.height as u32, indices: px.bytes(), clut: clut.bytes() })
}

/// Decodes every level texture, in table order:
/// the tfrag, moby, tie and shrub tables, then shrub billboards.
///
/// `core_data` is the whole decompressed core data lump (texture pixels are read
/// from its `textures` block, bounded as by `LevelCore::blocks`), `gs_ram` the raw gs_ram lump.
/// Entries with non-positive size or negative offsets are skipped; any other
/// entry that does not decode is an error (none do on the retail disc).
pub fn parse_textures(core: &LevelCore, core_data: &[u8], gs_ram: &[u8]) -> Result<Vec<LevelTexture>> {
    let Some(blk) = core.blocks.iter().find(|b| b.name == "textures") else { return invalid("level core has no textures block"); };
    let tex = Buf(core_data).sub(blk.offset, blk.size, "textures block")?;
    let gs = Buf(gs_ram);
    let mut out = Vec::new();
    let tables = [
        (TextureTable::Tfrag, &core.tfrag_textures),
        (TextureTable::Moby, &core.moby_textures),
        (TextureTable::Tie, &core.tie_textures),
        (TextureTable::Shrub, &core.shrub_textures),
    ];
    for (table, entries) in tables {
        for (index, e) in entries.iter().enumerate() {
            if e.width <= 0 || e.height <= 0 || e.data_offset < 0 || e.palette < 0 { continue; }
            let texture = entry_image_in(tex, gs, e)?.decode()?;
            out.push(LevelTexture { table, index, source: TextureSource::Entry(*e), texture });
        }
    }
    for (index, c) in core.shrub_classes.iter().enumerate() {
        let b = &c.billboard;
        if b.width <= 0 || b.height <= 0 { continue; }
        let texture = billboard_image(gs_ram, b)?.decode()?;
        out.push(LevelTexture { table: TextureTable::Billboard, index, source: TextureSource::Billboard { o_class: c.base.o_class, info: *b }, texture });
    }
    Ok(out)
}

/// The full mip chain of a tfrag texture, level 0 first, all decoded with the entry's one CLUT
/// (TEX0.CBP; MIPTBP1 carries only TBP/TBW per level). Spec: tfrag_rac1.md §2.5.1, textures_rac1.md §3.1.
///
/// `e.ty` is the level count (MXL + 1). Where each level's 8-bit indices live:
/// * level 0: `textures[data_offset]`, `w × h` (paged into GS memory per frame);
/// * level 1: `textures[data_offset + w·h]`, `w/2 × h/2`: `BuildTfragTextureDma` (boot 0x234d48) sends the
///   base level from the texture's source address and mip 1 from `address + 4·(w/2)²` (= `+ w·h`, all
///   tfrag textures are square: the paging DMA uses `w = h = 1 << n` from the table at 0x1e0900);
/// * level 2: `gs_ram[mipmap · 0x100]`, `w/4 × h/4` (MIPTBP1 TBP2 = `mipmap + base`, resident);
/// * level 3: `gs_ram[pad · 0x100]`, `w/8 × h/8` (MIPTBP1 TBP3 = `pad + base`, resident; −1 when `ty` = 3).
///
/// Only verified for the tfrag table (`TextureEntry::ty` is the level count there); other tables are not known
/// to follow the same layout.
pub fn decode_tfrag_mip_levels(core: &LevelCore, core_data: &[u8], gs_ram: &[u8], e: &TextureEntry) -> Result<Vec<Texture>> {
    tfrag_mip_images(core, core_data, gs_ram, e)?.iter().map(IndexedImage::decode).collect()
}

/// The stored images of a tfrag texture's mip chain (level 0 first), the parts [`decode_tfrag_mip_levels`] decodes.
pub fn tfrag_mip_images<'a>(core: &LevelCore, core_data: &'a [u8], gs_ram: &'a [u8], e: &TextureEntry) -> Result<Vec<IndexedImage<'a>>> {
    let Some(blk) = core.blocks.iter().find(|b| b.name == "textures") else { return invalid("level core has no textures block"); };
    let tex = Buf(core_data).sub(blk.offset, blk.size, "textures block")?;
    let gs = Buf(gs_ram);
    if e.width <= 0 || e.height <= 0 || e.data_offset < 0 || e.palette < 0 { return invalid("texture entry has no pixels"); }
    if !(1..=4).contains(&e.ty) { return invalid("tfrag texture level count outside 1..=4"); }
    let clut = gs.sub(e.palette as usize * 0x100, 1024, "texture palette")?;
    let (w, h) = (e.width as usize, e.height as usize);
    let mut out = Vec::with_capacity(e.ty as usize);
    for level in 0..e.ty as usize {
        let (lw, lh) = (w >> level, h >> level);
        if lw == 0 || lh == 0 { return invalid("mip level smaller than one texel"); }
        let px = match level {
            0 => tex.sub(e.data_offset as usize, lw * lh, "mip 0 pixels")?,
            1 => tex.sub(e.data_offset as usize + w * h, lw * lh, "mip 1 pixels")?,
            2 if e.mipmap >= 0 => gs.sub(e.mipmap as usize * 0x100, lw * lh, "mip 2 pixels")?,
            3 if e.pad >= 0 => gs.sub(e.pad as usize * 0x100, lw * lh, "mip 3 pixels")?,
            _ => return invalid("mip level without a gs_ram block"),
        };
        out.push(IndexedImage { width: lw as u32, height: lh as u32, indices: px.bytes(), clut: clut.bytes() });
    }
    Ok(out)
}

/// The mip chain of a shrub billboard texture, level 0 first, all with the descriptor's one CLUT. The class init
/// (boot `fun_00203b08`) builds TEX1 MXL = `max_mip − 1`, TEX0 TBP0 = `texture_offset`, MIPTBP1 TBP1..3 =
/// `mip1..mip3` (gs_ram blocks of 0x100 bytes, all resident), so level k is `gs_ram[block_k · 0x100]`,
/// `w >> k × h >> k` 8-bit indices (the same linear layout as level 0, which `parse_textures` decodes).
pub fn decode_billboard_mip_levels(gs_ram: &[u8], b: &ShrubBillboardInfo) -> Result<Vec<Texture>> {
    billboard_mip_images(gs_ram, b)?.iter().map(IndexedImage::decode).collect()
}

/// The stored images of a billboard's mip chain (level 0 first), the parts [`decode_billboard_mip_levels`] decodes.
pub fn billboard_mip_images<'a>(gs_ram: &'a [u8], b: &ShrubBillboardInfo) -> Result<Vec<IndexedImage<'a>>> {
    let gs = Buf(gs_ram);
    if b.width <= 0 || b.height <= 0 || b.palette_offset < 0 { return invalid("billboard has no texture"); }
    if !(1..=4).contains(&b.max_mip) { return invalid("billboard level count outside 1..=4"); }
    let clut = gs.sub(b.palette_offset as usize * 0x100, 1024, "billboard palette")?;
    let blocks = [b.texture_offset, b.mip1, b.mip2, b.mip3];
    let mut out = Vec::with_capacity(b.max_mip as usize);
    for (level, &blk) in blocks.iter().enumerate().take(b.max_mip as usize) {
        let (lw, lh) = (b.width as usize >> level, b.height as usize >> level);
        if lw == 0 || lh == 0 { return invalid("mip level smaller than one texel"); }
        if blk < 0 { return invalid("billboard mip level without a gs_ram block"); }
        let px = gs.sub(blk as usize * 0x100, lw * lh, "billboard mip pixels")?;
        out.push(IndexedImage { width: lw as u32, height: lh as u32, indices: px.bytes(), clut: clut.bytes() });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clut_index_swaps_middle_blocks() {
        // per 32-entry group: stored blocks 0,1,2,3 -> linear 0,2,1,3
        for g in 0..8u32 {
            let b = g * 32;
            for k in 0..8 {
                assert_eq!(clut_index(b + k), b + k);
                assert_eq!(clut_index(b + 8 + k), b + 16 + k);
                assert_eq!(clut_index(b + 16 + k), b + 8 + k);
                assert_eq!(clut_index(b + 24 + k), b + 24 + k);
            }
        }
        for i in 0..256 { assert_eq!(clut_index(clut_index(i)), i, "involution"); }
    }

    #[test]
    fn scale_alpha_maps_0x80_to_opaque() {
        assert_eq!(scale_alpha(0x00), 0x00);
        assert_eq!(scale_alpha(0x01), 0x02);
        assert_eq!(scale_alpha(0x40), 0x80);
        assert_eq!(scale_alpha(0x7f), 0xfe);
        assert_eq!(scale_alpha(0x80), 0xff);
        assert_eq!(scale_alpha(0x81), 0xff);
        assert_eq!(scale_alpha(0xff), 0xff);
    }

    #[test]
    fn decode_indexed8_applies_clut_order_and_alpha() {
        // entry j of the stored CLUT = (j, j, j, 0x80 if j even else 0x10)
        let clut: Vec<u8> = (0..256u32).flat_map(|j| [j as u8, j as u8, j as u8, if j % 2 == 0 { 0x80 } else { 0x10 }]).collect();
        let t = decode_indexed8(&[0, 8, 16, 25], 2, 2, &clut).unwrap();
        assert_eq!((t.width, t.height), (2, 2));
        assert_eq!(t.rgba, [0, 0, 0, 0xff, 16, 16, 16, 0xff, 8, 8, 8, 0xff, 25, 25, 25, 0x20]);
        assert!(decode_indexed8(&[0; 3], 2, 2, &clut).is_err());
        assert!(decode_indexed8(&[0; 4], 2, 2, &clut[..1020]).is_err());
    }
}
