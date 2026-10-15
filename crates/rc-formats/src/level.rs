//! Level data container and core index. Spec: docs/formats/wad_layouts_rac1.md sections 2.3–2.5.

use crate::buf::{invalid, Buf, Result};
use bytemuck::{Pod, Zeroable};
use std::collections::BTreeSet;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct ByteRange { pub offset: i32, pub size: i32 }
impl ByteRange { pub fn present(&self) -> bool { self.offset >= 0 && self.size > 0 } }

/// Header at the start of a level's uncompressed data container (0x58 bytes).
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct LevelDataHeader {
    pub overlay: ByteRange,
    pub sound_bank: ByteRange,
    pub core_index: ByteRange,
    pub gs_ram: ByteRange,
    pub hud_header: ByteRange,
    pub hud_banks: [ByteRange; 5],
    pub core_data: ByteRange,
}
const _: () = assert!(std::mem::size_of::<LevelDataHeader>() == 0x58);

pub fn parse_level_data_header(data: &[u8]) -> Result<LevelDataHeader> { Buf(data).pod(0, "level data header") }

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct ArrayRange { pub count: i32, pub offset: i32 }

/// LevelCoreHeader (0xbc bytes) at byte 0 of the core index. "index" offsets are
/// relative to the core index, "data" offsets to the decompressed core data.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct LevelCoreHeader {
    pub gs_ram: ArrayRange,
    pub tfrags: i32, pub occlusion: i32, pub sky: i32, pub collision: i32,
    pub moby_classes: ArrayRange, pub tie_classes: ArrayRange, pub shrub_classes: ArrayRange,
    pub tfrag_textures: ArrayRange, pub moby_textures: ArrayRange, pub tie_textures: ArrayRange, pub shrub_textures: ArrayRange,
    pub part_textures: ArrayRange, pub fx_textures: ArrayRange,
    pub textures_base_offset: i32, pub part_bank_offset: i32, pub fx_bank_offset: i32,
    pub part_defs_offset: i32, pub sound_remap_offset: i32, pub unknown_74: i32,
    pub ratchet_seqs: i32, pub scene_view_size: i32,
    pub gadget_count: i32, pub gadget_offset: i32,
    pub assets_compressed_size: i32, pub assets_decompressed_size: i32,
    pub chrome_map_texture: i32, pub chrome_map_palette: i32, pub glass_map_texture: i32, pub glass_map_palette: i32,
    pub unknown_a0: i32, pub heightmap_offset: i32, pub occlusion_oct_offset: i32, pub moby_gs_stash_list: i32,
    pub occlusion_rad_offset: i32, pub moby_sound_remap_offset: i32, pub occlusion_rad2_offset: i32,
}
const _: () = assert!(std::mem::size_of::<LevelCoreHeader>() == 0xbc);

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GsRamEntry { pub psm: i32, pub width: i16, pub height: i16, pub address: i32, pub offset: i32 }
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct ClassEntry { pub offset_in_asset_wad: i32, pub o_class: i32, pub unknown_8: i32, pub unknown_c: i32, pub textures: [u8; 16] }
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct ShrubBillboardInfo { pub width: i16, pub height: i16, pub max_mip: i16, pub palette_offset: i16, pub texture_offset: i16, pub mip1: i16, pub mip2: i16, pub mip3: i16 }
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct ShrubClassEntry { pub base: ClassEntry, pub billboard: ShrubBillboardInfo }
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct TextureEntry { pub data_offset: i32, pub width: i16, pub height: i16, pub ty: i16, pub palette: i16, pub mipmap: i16, pub pad: i16 }
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GadgetEntry { pub offset_in_asset_wad: i32, pub class_number: i32, pub compressed_size: i32, pub pad: i32 }
const _: () = assert!(std::mem::size_of::<ShrubClassEntry>() == 0x30);

#[derive(Clone, Debug)]
pub struct CoreBlock { pub name: String, pub offset: usize, pub size: usize }

#[derive(Debug)]
pub struct LevelCore {
    pub header: LevelCoreHeader,
    pub gs_ram: Vec<GsRamEntry>,
    pub moby_classes: Vec<ClassEntry>,
    pub tie_classes: Vec<ClassEntry>,
    pub shrub_classes: Vec<ShrubClassEntry>,
    pub tfrag_textures: Vec<TextureEntry>,
    pub moby_textures: Vec<TextureEntry>,
    pub tie_textures: Vec<TextureEntry>,
    pub shrub_textures: Vec<TextureEntry>,
    pub gadgets: Vec<GadgetEntry>,
    pub ratchet_seqs: Vec<i32>,
    /// Core index words 0x30 / 0x31 (bytes 0xc0 / 0xc4, past [`LevelCoreHeader`]): the **gold** environment map's
    /// texture and palette, byte offsets into the gs_ram lump like `chrome_map_*`. `LoadLevelCoreData` (L01 0x258128)
    /// builds TEX0 0x182c70 from them (128×128 PSMT8, the chrome map's form); `select_world_object_resource_tables`
    /// gives it to a gold weapon's metal packets. [-1, -1] when the index is too short.
    pub gold_map: [i32; 2],
    /// Every block of the decompressed core data with its size from the boundary rule.
    pub blocks: Vec<CoreBlock>,
}

impl LevelCore {
    /// The bytes of the first block named `name` (`tfrags`, `sky`, `moby_class/0042`, `ratchet_seq/007`, …) in the
    /// decompressed core data, or None when the level has no such block.
    pub fn block<'a>(&self, data: &'a [u8], name: &str) -> Option<&'a [u8]> {
        let b = self.blocks.iter().find(|b| b.name == name)?;
        data.get(b.offset..b.offset + b.size)
    }
}

fn table<T: Pod>(idx: Buf, r: ArrayRange, what: &'static str) -> Result<Vec<T>> {
    if r.count <= 0 || r.offset <= 0 { return Ok(Vec::new()); }
    if r.count > 100_000 { return invalid(format!("implausible count for {what}")); }
    idx.pod_slice(r.offset as usize, r.count as usize, what)
}

pub fn parse_level_core(index: &[u8], data_size: usize) -> Result<LevelCore> {
    let idx = Buf(index);
    let h: LevelCoreHeader = idx.pod(0, "core header")?;
    let gadgets = if h.gadget_count > 0 && h.gadget_offset > 0 { idx.pod_slice(h.gadget_offset as usize, h.gadget_count as usize, "gadget table")? } else { Vec::new() };
    let ratchet_seqs = if h.ratchet_seqs > 0 { idx.pod_slice::<i32>(h.ratchet_seqs as usize, 256, "ratchet seq table")? } else { Vec::new() };
    let mut core = LevelCore {
        header: h,
        gs_ram: table(idx, h.gs_ram, "gs_ram table")?,
        moby_classes: table(idx, h.moby_classes, "moby class table")?,
        tie_classes: table(idx, h.tie_classes, "tie class table")?,
        shrub_classes: table(idx, h.shrub_classes, "shrub class table")?,
        tfrag_textures: table(idx, h.tfrag_textures, "tfrag textures")?,
        moby_textures: table(idx, h.moby_textures, "moby textures")?,
        tie_textures: table(idx, h.tie_textures, "tie textures")?,
        shrub_textures: table(idx, h.shrub_textures, "shrub textures")?,
        gadgets,
        ratchet_seqs,
        gold_map: idx.pod(0xc0, "gold map words").unwrap_or([-1, -1]),
        blocks: Vec::new(),
    };

    // Boundary rule (spec 2.5): a block runs to the next known start offset.
    // moby_sound_remap_offset is 0x9c00 in every RAC1 level and is not a data offset here.
    let mut bounds = BTreeSet::new();
    let mut add = |v: i32| { if v > 0 { bounds.insert(v as usize); } };
    for v in [h.tfrags, h.occlusion, h.sky, h.collision, h.textures_base_offset, h.part_bank_offset, h.fx_bank_offset] { add(v); }
    for e in &core.moby_classes { add(e.offset_in_asset_wad); }
    for e in &core.tie_classes { add(e.offset_in_asset_wad); }
    for e in &core.shrub_classes { add(e.base.offset_in_asset_wad); }
    for &v in &core.ratchet_seqs { add(v); }
    for g in &core.gadgets { add(g.offset_in_asset_wad); }
    let end = if h.assets_decompressed_size > 0 { h.assets_decompressed_size as usize } else { data_size };
    bounds.insert(end);

    let block = |blocks: &mut Vec<CoreBlock>, name: String, start: i32| {
        if start <= 0 { return; }
        let start = start as usize;
        let next = bounds.range(start + 1..).next().copied().unwrap_or(end);
        blocks.push(CoreBlock { name, offset: start, size: next - start });
    };
    let mut blocks = Vec::new();
    // tfrags open the data region and run to the first of occlusion/sky/collision.
    let tf_end = if h.occlusion > 0 { h.occlusion } else if h.sky > 0 { h.sky } else { h.collision };
    if tf_end > h.tfrags { blocks.push(CoreBlock { name: "tfrags".into(), offset: h.tfrags as usize, size: (tf_end - h.tfrags) as usize }); }
    block(&mut blocks, "occlusion".into(), h.occlusion);
    block(&mut blocks, "sky".into(), h.sky);
    block(&mut blocks, "collision".into(), h.collision);
    block(&mut blocks, "textures".into(), h.textures_base_offset);
    block(&mut blocks, "part_bank".into(), h.part_bank_offset);
    block(&mut blocks, "fx_bank".into(), h.fx_bank_offset);
    for e in &core.moby_classes { block(&mut blocks, format!("moby_class/{:04}", e.o_class), e.offset_in_asset_wad); }
    for e in &core.tie_classes { block(&mut blocks, format!("tie_class/{:04}", e.o_class), e.offset_in_asset_wad); }
    for e in &core.shrub_classes { block(&mut blocks, format!("shrub_class/{:04}", e.base.o_class), e.base.offset_in_asset_wad); }
    for (i, &v) in core.ratchet_seqs.iter().enumerate() { block(&mut blocks, format!("ratchet_seq/{i:03}"), v); }
    for g in &core.gadgets { block(&mut blocks, format!("gadget/{:04}", g.class_number), g.offset_in_asset_wad); }
    core.blocks = blocks;
    Ok(core)
}

/// The level height grid (core header +0xa4 `heightmap_offset`, a data-space offset; G-LVL-008): what
/// `LoadLevelCoreData` 0x258128 hands to `FUN_002530f0`, which keeps the header in 0x15fc98..0x15fca4 and points
/// 0x15fca8 at the cells (a zero offset clears all five). Present on Batalia (08), Orxon (12) and Oltanis (14), whose
/// weather emitter 1400 reads it. Layout: `s32 width, s32 rows, f32 low, f32 high`, then `u8 cells[rows][width]`.
#[derive(Clone, Debug, PartialEq)]
pub struct HeightGrid {
    /// 0x15fc98: cells per row (x).
    pub width: i32,
    /// 0x15fc9c: rows (y; the game never reads it).
    pub rows: i32,
    /// 0x15fca0 / 0x15fca4: the heights of cell values 255 and 0.
    pub low: f32,
    pub high: f32,
    pub cells: Vec<u8>,
}

impl HeightGrid {
    /// The grid of a level (None when the header's offset is 0 or the block does not fit the data).
    pub fn parse(header: &LevelCoreHeader, data: &[u8]) -> Option<HeightGrid> {
        let o = usize::try_from(header.heightmap_offset).ok().filter(|&o| o > 0)?;
        let b = Buf(data);
        let (width, rows) = (b.i32(o).ok()?, b.i32(o + 4).ok()?);
        let (low, high) = (f32::from_bits(b.u32(o + 8).ok()?), f32::from_bits(b.u32(o + 12).ok()?));
        let n = usize::try_from(width).ok()?.checked_mul(usize::try_from(rows).ok()?)?;
        Some(HeightGrid { width, rows, low, high, cells: data.get(o + 16..o + 16 + n)?.to_vec() })
    }

    /// `FUN_00278020(p)` (level01 0x278020): `(high − low)·(1 − cell/255) + low` of the cell `width·(int)y + (int)x` (the `cvt.w.s`
    /// truncation). None outside the grid (the game reads whatever lies there) [L].
    pub fn height(&self, x: f32, y: f32) -> Option<f32> {
        let i = self.width.checked_mul(y as i32)?.checked_add(x as i32)?;
        let c = *self.cells.get(usize::try_from(i).ok()?)?;
        Some((self.high - self.low) * (1.0 - c as f32 / 255.0) + self.low)
    }
}
