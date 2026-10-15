//! Per-level HUD graphics: the `hud_header` lump and its five `hud_banks`. Spec: docs/plan/hud_text.md §1.
//! Ported from the retired C++ reference extractor (git 2230812); snapshot-tested in `tests/formats/golden.rs`.
//!
//! The header (copied to the HUD heap by `LoadHudBanks__Fv`, level01 0x253e28) holds four tables, all
//! offsets relative to the header:
//!
//! * **icons** ([`IconEntry`], 8 bytes, ends with id 0xffff): `Hud_GetIconIndex__Fi` (0x24a4e0) scans them
//!   linearly; `GetIconFrame__Fii` (0x24fe10) turns (icon id, k) into a frame index;
//! * **frames** ([`FrameEntry`], 4 bytes): palette index + texture index;
//! * **palettes** ([`PaletteEntry`], 8 bytes): 256 × RGBA32 CLUT (CSM1 order) at `offset` in its bank;
//! * **textures** ([`TextureEntry`], 8 bytes): `w·h` 8-bit indices at `offset` in its bank, `w = 1 << log2_w`.
//!
//! Bank *b* owns palettes `palette_cum[b−1]..palette_cum[b]` and textures `texture_cum[b−1]..texture_cum[b]`
//! (`LinkHudBank__FiPc` 0x24a848 clears bit 31 of those entries and adds the bank's base address).
//! Banks are WAD-compressed on disc; every function here takes them decompressed.

use crate::buf::{invalid, Buf, Result};
use crate::texture::{clut_index, decode_indexed8, IndexedImage, Texture};
use bytemuck::{Pod, Zeroable};

/// Number of HUD banks (`hud_banks[5]` in the level data header).
pub const BANKS: usize = 5;

/// `hud_header` +0x00..+0xb4 (the four table offsets and the per-bank ranges). Every per-bank array is
/// `u32[8]` of which the first [`BANKS`] are used (the rest are 0). +0x74 and +0x94 are runtime fields
/// (bank pointers, stash handles), 0 on disc.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct HudHeader {
    /// Icon entries including the 0xffff terminator.
    pub icon_count: u16,
    pub frame_count: u16,
    pub icon_offset: u32,
    pub frame_offset: u32,
    pub palette_offset: u32,
    pub texture_offset: u32,
    /// +0x14: cumulative palette count per bank.
    pub palette_cum: [u32; 8],
    /// +0x34: cumulative texture count per bank.
    pub texture_cum: [u32; 8],
    /// +0x54: decompressed size of each bank (0 = absent).
    pub bank_size: [u32; 8],
    /// +0x74, +0x94.
    pub runtime_bank_base: [u32; 8],
    pub runtime_stash: [u32; 8],
}
const _: () = assert!(std::mem::size_of::<HudHeader>() == HEADER_SIZE);

/// Bytes of [`HudHeader`]; the icon table follows (offset 0xb4 on every level).
pub const HEADER_SIZE: usize = 0xb4;

/// Icon table entry: `{u16 id, u16 frame_count, u16 first_frame, u8 anim_mode, u8 ticks_per_frame}`.
/// `anim_mode` / `ticks_per_frame` drive the per-slot frame animation (`FUN_0024f6b0`: 0 static,
/// 1 loop, 2 ping-pong, 3 ping-pong with a random pause).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct IconEntry {
    pub id: u16,
    pub frame_count: u16,
    pub first_frame: u16,
    pub anim_mode: u8,
    pub ticks_per_frame: u8,
}

/// Frame table entry: palette and texture index.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct FrameEntry {
    pub palette: i16,
    pub texture: i16,
}

/// Palette table entry: `offset | 0x80000000` (bit 31 = not linked yet), runtime CBP.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct PaletteEntry {
    pub offset_flags: u32,
    pub cbp: u16,
    pub pad: u16,
}

/// Texture table entry: `offset | 0x80000000`, runtime TBP, log2 of the size.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct TextureEntry {
    pub offset_flags: u32,
    pub tbp: u16,
    pub log2_w: u8,
    pub log2_h: u8,
}

impl PaletteEntry {
    pub fn offset(&self) -> usize { (self.offset_flags & 0x7fff_ffff) as usize }
}

impl TextureEntry {
    pub fn offset(&self) -> usize { (self.offset_flags & 0x7fff_ffff) as usize }
    pub fn width(&self) -> u32 { 1 << (self.log2_w & 0x1f) }
    pub fn height(&self) -> u32 { 1 << (self.log2_h & 0x1f) }
}

/// A parsed HUD set with its decompressed banks.
#[derive(Clone, Debug)]
pub struct Hud {
    pub header: HudHeader,
    /// Including the terminator (id 0xffff).
    pub icons: Vec<IconEntry>,
    pub frames: Vec<FrameEntry>,
    pub palettes: Vec<PaletteEntry>,
    pub textures: Vec<TextureEntry>,
    pub banks: [Vec<u8>; BANKS],
}

/// Bank owning entry `i` of a table with cumulative per-bank counts `cum`.
fn owning_bank(cum: &[u32; 8], i: usize) -> Option<usize> { cum[..BANKS].iter().position(|&c| i < c as usize) }

/// Just the header record (to learn which banks exist before decompressing them).
pub fn parse_header(header: &[u8]) -> Result<HudHeader> { Buf(header).pod(0, "hud header") }

/// Parses `hud_header` and keeps the decompressed `banks` (absent banks empty).
pub fn parse_hud(header: &[u8], banks: [&[u8]; BANKS]) -> Result<Hud> {
    let b = Buf(header);
    let h = parse_header(header)?;
    let n_pal = h.palette_cum[BANKS - 1] as usize;
    let n_tex = h.texture_cum[BANKS - 1] as usize;
    if h.icon_count == 0 || h.icon_count > 0x1000 || h.frame_count > 0x4000 || n_pal > 0x1000 || n_tex > 0x1000 {
        return invalid("implausible hud header counts");
    }
    let icons: Vec<IconEntry> = b.pod_slice(h.icon_offset as usize, h.icon_count as usize, "hud icon table")?;
    if icons.last().map(|e| e.id) != Some(0xffff) { return invalid("hud icon table has no 0xffff terminator"); }
    let frames: Vec<FrameEntry> = b.pod_slice(h.frame_offset as usize, h.frame_count as usize, "hud frame table")?;
    let palettes: Vec<PaletteEntry> = b.pod_slice(h.palette_offset as usize, n_pal, "hud palette table")?;
    let textures: Vec<TextureEntry> = b.pod_slice(h.texture_offset as usize, n_tex, "hud texture table")?;
    for (i, &want) in h.bank_size[..BANKS].iter().enumerate() {
        if banks[i].len() < want as usize { return invalid(format!("hud bank {i} shorter than its header size")); }
    }
    Ok(Hud { header: h, icons, frames, palettes, textures, banks: banks.map(|s| s.to_vec()) })
}

/// Decodes 8-bit indices with a CSM1 CLUT keeping the GS alpha as stored (0x80 = 1.0, no scaling):
/// what the GS samples (TCC = 1, CT32 CLUT). [`decode_indexed8`] is the same with `scale_alpha`.
pub fn decode_indexed8_raw(indices: &[u8], width: u32, height: u32, clut: &[u8]) -> Result<Texture> {
    let n = width as usize * height as usize;
    if indices.len() < n { return invalid("indexed texture data too small"); }
    if clut.len() < 1024 { return invalid("palette too small"); }
    let mut rgba = Vec::with_capacity(n * 4);
    for &ix in &indices[..n] {
        let e = clut_index(ix as u32) as usize * 4;
        rgba.extend_from_slice(&clut[e..e + 4]);
    }
    Ok(Texture { width, height, rgba })
}

impl Hud {
    /// Bank of palette `i` / texture `i`.
    pub fn palette_bank(&self, i: usize) -> Option<usize> { owning_bank(&self.header.palette_cum, i) }
    pub fn texture_bank(&self, i: usize) -> Option<usize> { owning_bank(&self.header.texture_cum, i) }

    /// `Hud_GetIconIndex__Fi`: linear scan; not found → the terminator's index.
    pub fn icon_index(&self, id: u16) -> usize {
        self.icons.iter().position(|e| e.id == 0xffff || e.id == id).unwrap_or(self.icons.len() - 1)
    }

    /// `GetIconFrame__Fii(id, k)` with every bank resident: `first_frame + k` if the icon exists and
    /// `k < frame_count`, else frame 0. (The game also returns 0 while the frame's palette or texture is
    /// not linked, i.e. its bank is not loaded.)
    pub fn icon_frame(&self, id: u16, k: i32) -> usize {
        let e = &self.icons[self.icon_index(id)];
        if e.id == 0xffff || k >= e.frame_count as i32 { return 0; }
        (e.first_frame as i32 + k).max(0) as usize
    }

    /// Size in texels of frame `i` (its texture's power-of-two size).
    pub fn frame_size(&self, i: usize) -> Option<(u32, u32)> {
        let f = self.frames.get(i)?;
        let t = self.textures.get(usize::try_from(f.texture).ok()?)?;
        Some((t.width(), t.height()))
    }

    /// The stored indices and CLUT of frame `i` (the parts [`Hud::decode_frame`] decodes).
    pub fn frame_image(&self, i: usize) -> Result<IndexedImage<'_>> {
        let (indices, clut, width, height) = self.frame_parts(i)?;
        Ok(IndexedImage { width, height, indices, clut })
    }

    fn frame_parts(&self, i: usize) -> Result<(&[u8], &[u8], u32, u32)> {
        let Some(f) = self.frames.get(i) else { return invalid("hud frame index out of range") };
        let (Ok(pi), Ok(ti)) = (usize::try_from(f.palette), usize::try_from(f.texture)) else {
            return invalid("hud frame with negative palette or texture");
        };
        let (Some(p), Some(t)) = (self.palettes.get(pi), self.textures.get(ti)) else { return invalid("hud frame entry out of range") };
        let (Some(pb), Some(tb)) = (self.palette_bank(pi), self.texture_bank(ti)) else { return invalid("hud entry owned by no bank") };
        let (w, h) = (t.width(), t.height());
        let px = Buf(&self.banks[tb]).sub(t.offset(), w as usize * h as usize, "hud texture pixels")?;
        let clut = Buf(&self.banks[pb]).sub(p.offset(), 1024, "hud palette")?;
        Ok((px.bytes(), clut.bytes(), w, h))
    }

    /// Frame `i` as RGBA8 with alpha scaled like every level texture ([`decode_indexed8`]).
    pub fn decode_frame(&self, i: usize) -> Result<Texture> {
        let (px, clut, w, h) = self.frame_parts(i)?;
        decode_indexed8(px, w, h, clut)
    }

    /// Frame `i` with the raw GS alpha (0x80 = 1.0), as the renderer samples it.
    pub fn decode_frame_raw(&self, i: usize) -> Result<Texture> {
        let (px, clut, w, h) = self.frame_parts(i)?;
        decode_indexed8_raw(px, w, h, clut)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic() -> (Vec<u8>, Vec<u8>) {
        // Header 0xb4, icons at 0xb4 (2 + terminator), frames, 1 palette (bank 1), 1 texture 2×2 (bank 0).
        let mut h = HudHeader { icon_count: 3, frame_count: 2, icon_offset: 0xb4, ..Default::default() };
        h.frame_offset = h.icon_offset + 3 * 8;
        h.palette_offset = h.frame_offset + 2 * 4;
        h.texture_offset = h.palette_offset + 8;
        h.palette_cum = [0, 1, 1, 1, 1, 0, 0, 0];
        h.texture_cum = [1, 1, 1, 1, 1, 0, 0, 0];
        h.bank_size = [4, 1024, 0, 0, 0, 0, 0, 0];
        let mut hb = bytemuck::bytes_of(&h).to_vec();
        for e in [IconEntry { id: 30031, frame_count: 2, first_frame: 0, ..Default::default() }, IconEntry { id: 7, frame_count: 1, first_frame: 1, ..Default::default() }, IconEntry { id: 0xffff, ..Default::default() }] {
            hb.extend_from_slice(bytemuck::bytes_of(&e));
        }
        for f in [FrameEntry { palette: 0, texture: 0 }, FrameEntry { palette: 0, texture: 0 }] { hb.extend_from_slice(bytemuck::bytes_of(&f)); }
        hb.extend_from_slice(bytemuck::bytes_of(&PaletteEntry { offset_flags: 0x8000_0000, cbp: 0, pad: 0 }));
        hb.extend_from_slice(bytemuck::bytes_of(&TextureEntry { offset_flags: 0x8000_0000, tbp: 0, log2_w: 1, log2_h: 1 }));
        let mut pal = vec![0u8; 1024];
        // Stored entry 8 = linear 16 (CSM1).
        pal[8 * 4..8 * 4 + 4].copy_from_slice(&[10, 20, 30, 0x40]);
        pal[4..8].copy_from_slice(&[1, 2, 3, 0x80]);
        (hb, pal)
    }

    #[test]
    fn parse_lookup_and_decode() {
        let (hb, pal) = synthetic();
        let px = [16u8, 1, 1, 16];
        let hud = parse_hud(&hb, [&px, &pal, &[], &[], &[]]).unwrap();
        assert_eq!(hud.icon_index(7), 1);
        assert_eq!(hud.icon_index(1234), 2, "missing id → terminator");
        assert_eq!(hud.icon_frame(30031, 1), 1);
        assert_eq!(hud.icon_frame(30031, 2), 0, "k past the count → frame 0");
        assert_eq!(hud.icon_frame(1234, 0), 0);
        assert_eq!(hud.frame_size(0), Some((2, 2)));
        assert_eq!((hud.palette_bank(0), hud.texture_bank(0)), (Some(1), Some(0)));
        let raw = hud.decode_frame_raw(0).unwrap();
        assert_eq!(&raw.rgba[..8], &[10, 20, 30, 0x40, 1, 2, 3, 0x80]);
        let scaled = hud.decode_frame(0).unwrap();
        assert_eq!(&scaled.rgba[..8], &[10, 20, 30, 0x80, 1, 2, 3, 0xff]);
        assert!(parse_hud(&hb, [&px[..3], &pal, &[], &[], &[]]).is_err(), "bank shorter than its size");
    }
}
