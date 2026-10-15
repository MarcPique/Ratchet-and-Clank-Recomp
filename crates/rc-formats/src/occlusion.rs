//! Precomputed occlusion (potentially-visible sets). Spec: docs/formats/occlusion_rac1.md; the per-frame
//! rule and the renderer plan: docs/plan/occlusion_culling.md. Ported from the retired C++
//! reference extractor (git 2230812); `tests/formats/golden.rs` checks every section against the committed snapshot table (`data/loader_snapshots.tsv`).
//!
//! Three pieces of data take part, all addresses from the level01 overlay:
//! * the core **occlusion block** (core header 0x0c, pointer `0x15f600`): a z → y → x tree of u16 nodes over
//!   4 × 4 × 4-unit cells, each populated cell naming one 0x80-byte mask (1024 visibility bits);
//! * the gameplay **occlusion mappings** (gameplay pointer 0x8c; the level WAD's `occlusion` lump is a copy):
//!   one `(bit_index, occlusion_id)` record per tfrag, tie instance and occlusion-culled moby;
//! * the optional **octant override** (core header 0xa8, pointer `0x15f604`): a centre plus 8 masks, used
//!   only when the camera leaves the grid while the per-frame fallback is 2.
//!
//! At level load (`FUN_00255958`) every object's mapping is folded into a u16 [`OcclBits`]
//! (`byte << 8 | 1 << bit`), stored in tfrag header 0x3a, runtime tie +0x18 and runtime moby +0x36. Every frame
//! `UpdateOcclusion` (0x2193f8) fills the 128-byte mask at `0x174180` from the camera's cell, and
//! `TfragProc` (0x2a80c0), `TieProc` (0x2a9dd8) and `MobyProc` (0x26ab2c) skip an object when
//! `mask[bits >> 8] & (bits & 0xff) == 0`, before any frustum test. Shrubs never consult it.

use crate::buf::{invalid, Buf, Result};
use crate::gameplay::MobyInstance;
use crate::level::LevelCore;
use crate::tfrag::TfragHeader;
use bytemuck::{Pod, Zeroable};

/// Bytes per visibility mask (1024 bits).
pub const MASK_BYTES: usize = 0x80;
/// A 1024-bit visibility mask; bit `b` is `mask[b >> 3] & (1 << (b & 7))`.
pub type VisMask = [u8; MASK_BYTES];
/// Cell edge in world units: `BuildOcclVisibility` scales the camera by 0.25 (`0x3e800000`).
pub const CELL_SIZE: f32 = 4.0;
/// Gameplay pointer-table slot of the occlusion mappings (`FUN_00255958`: `piVar27[0x23]`).
pub const GAMEPLAY_OCCLUSION_MAPPINGS: usize = 0x8c;
/// Size of the octant override block at core header 0xa8: centre (4 × f32) + 8 masks.
pub const OCTANT_BLOCK_SIZE: usize = 0x10 + 8 * MASK_BYTES;

/// One populated grid cell. Coordinates are cell indices (`trunc(world * 0.25)`), `mask` indexes
/// [`Occlusion::masks`]. `#[repr(C)]` = the C++ `OcclusionCell` (u16 x, y, z, mask).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct OcclusionCell { pub x: u16, pub y: u16, pub z: u16, pub mask: u16 }

/// Octant override (core header 0xa8): `BuildOcclVisibility` picks mask
/// `(cam.x > c.x) * 4 + (cam.y > c.y) * 2 + (cam.z > c.z)` (strict `0.0 < cam - c`).
#[derive(Clone, Debug)]
pub struct OctantOverride {
    /// +0x00: centre x, y, z, and a 4th word (0.0 in retail).
    pub centre: [f32; 4],
    /// +0x10: 8 masks, 0x80 bytes each.
    pub masks: [VisMask; 8],
}

impl OctantOverride {
    pub fn mask_for(&self, camera: [f32; 3]) -> &VisMask {
        let c = self.centre;
        let i = ((0.0 < camera[0] - c[0]) as usize) * 4 + ((0.0 < camera[1] - c[1]) as usize) * 2 + (0.0 < camera[2] - c[2]) as usize;
        &self.masks[i]
    }
}

/// The core occlusion block, decoded.
#[derive(Clone, Debug)]
pub struct Occlusion {
    /// Block +0x00: byte offset of the mask array from the block start.
    pub masks_offset: i32,
    /// Block +0x04: first z cell; +0x06: number of z slots (each a u16 node offset in 4-byte units, 0 = empty).
    pub z_base: u16,
    pub z_count: u16,
    /// Every populated cell in tree order (z, then y, then x).
    pub cells: Vec<OcclusionCell>,
    /// Masks 0 ..= highest index any cell names (the game never stores a count).
    pub masks: Vec<VisMask>,
    /// Octant override, when core header 0xa8 is set (levels 11, 13, 17 in retail).
    pub octants: Option<OctantOverride>,
    /// The raw block; [`Occlusion::lookup`] walks it exactly like `ParseOcclGrid`.
    pub raw: Vec<u8>,
}

/// Camera position → cell indices, as `BuildOcclVisibility` (0x219008): `cvt.w.s(pos * 0.25)`, i.e.
/// truncation toward zero (so cell 0 spans −4 < p < 4). Rust's `as i32` truncates and saturates like the EE.
pub fn cell_coords(pos: [f32; 3]) -> [i32; 3] { pos.map(|p| (p * 0.25) as i32) }

/// `ParseOcclGrid` (level01 0x218e78) on a raw block: z, y and x each rebased by their node's u16 base and
/// range-checked against its u16 count; a z or y slot of 0 or an x slot of 0xffff means "no cell".
/// Returns the mask index. Reads are bounds-checked; the game reads unchecked, retail data never needs it.
pub fn lookup_mask(block: &[u8], x: i32, y: i32, z: i32) -> Option<u16> {
    let b = Buf(block);
    let z = z - b.u16(4).ok()? as i32;
    if z < 0 || z >= b.u16(6).ok()? as i32 { return None; }
    let zo = b.u16(8 + z as usize * 2).ok()?;
    if zo == 0 { return None; }
    let yn = zo as usize * 4;
    let y = y - b.u16(yn).ok()? as i32;
    if y < 0 || y >= b.u16(yn + 2).ok()? as i32 { return None; }
    let yo = b.u16(yn + 4 + y as usize * 2).ok()?;
    if yo == 0 { return None; }
    let xn = yo as usize * 4;
    let x = x - b.u16(xn).ok()? as i32;
    if x < 0 || x >= b.u16(xn + 2).ok()? as i32 { return None; }
    let m = b.u16(xn + 4 + x as usize * 2).ok()?;
    (m != 0xffff).then_some(m)
}

impl Occlusion {
    /// `ParseOcclGrid` on this block: the mask index of cell `(x, y, z)`.
    pub fn lookup(&self, x: i32, y: i32, z: i32) -> Option<u16> { lookup_mask(&self.raw, x, y, z) }

    /// The cell containing `pos` (world units), with the game's integer mapping ([`cell_coords`]).
    pub fn cell_for(&self, pos: [f32; 3]) -> Option<OcclusionCell> {
        let [x, y, z] = cell_coords(pos);
        self.lookup(x, y, z).map(|mask| OcclusionCell { x: x as u16, y: y as u16, z: z as u16, mask })
    }

    pub fn mask(&self, index: u16) -> &VisMask { &self.masks[index as usize] }

    /// The per-frame mask for a camera inside `cell`: the stored mask with bit 1023 forced on, as the
    /// last store of `BuildOcclVisibility` (`0x1741ff |= 0x80`) does.
    pub fn frame_mask(&self, cell: OcclusionCell) -> VisMask { with_always_bit(*self.mask(cell.mask)) }
}

/// Sets bit 1023 (`0x7f`, `0x80`), which [`OcclBits::ALWAYS`] objects test.
pub fn with_always_bit(mut m: VisMask) -> VisMask { m[0x7f] |= 0x80; m }

/// The core occlusion block (sized by the core-index boundary rule).
pub fn occlusion_block<'a>(core: &LevelCore, core_data: &'a [u8]) -> Result<&'a [u8]> {
    let Some(blk) = core.blocks.iter().find(|b| b.name == "occlusion") else { return invalid("level has no occlusion block") };
    Ok(Buf(core_data).sub(blk.offset, blk.size, "occlusion block")?.bytes())
}

/// Decodes a raw occlusion block (no octant override: that lives elsewhere in the core data).
pub fn parse_occlusion_block(block: &[u8]) -> Result<Occlusion> {
    let b = Buf(block);
    let masks_offset = b.i32(0)?;
    let (z_base, z_count) = (b.u16(4)?, b.u16(6)?);
    if masks_offset <= 8 || masks_offset as usize > block.len() { return invalid("occlusion: bad mask offset"); }
    let mut cells = Vec::new();
    for zi in 0..z_count {
        let zo = b.u16(8 + zi as usize * 2)?;
        if zo == 0 { continue; }
        let yn = zo as usize * 4;
        let (y_base, y_count) = (b.u16(yn)?, b.u16(yn + 2)?);
        for yi in 0..y_count {
            let yo = b.u16(yn + 4 + yi as usize * 2)?;
            if yo == 0 { continue; }
            let xn = yo as usize * 4;
            let (x_base, x_count) = (b.u16(xn)?, b.u16(xn + 2)?);
            for xi in 0..x_count {
                let mask = b.u16(xn + 4 + xi as usize * 2)?;
                if mask == 0xffff { continue; }
                cells.push(OcclusionCell {
                    x: x_base.wrapping_add(xi), y: y_base.wrapping_add(yi), z: z_base.wrapping_add(zi), mask,
                });
            }
        }
    }
    let count = cells.iter().map(|c| c.mask as usize + 1).max().unwrap_or(0);
    let masks = b.pod_slice::<VisMask>(masks_offset as usize, count, "occlusion masks")?;
    Ok(Occlusion { masks_offset, z_base, z_count, cells, masks, octants: None, raw: block.to_vec() })
}

/// Parses the level's occlusion grid (core header 0x0c) and octant override (core header 0xa8).
/// The loader (`FUN_00258128`) sets `0x15f600 = core_data + header[0x0c]` and `0x15f604 = core_data + header[0xa8]`,
/// each 0 when the header word is 0.
pub fn parse_occlusion(core: &LevelCore, core_data: &[u8]) -> Result<Occlusion> {
    let mut o = parse_occlusion_block(occlusion_block(core, core_data)?)?;
    let oct = core.header.occlusion_oct_offset;
    if oct > 0 {
        let b = Buf(core_data).sub(oct as usize, OCTANT_BLOCK_SIZE, "occlusion octant block")?;
        let centre = [b.f32(0)?, b.f32(4)?, b.f32(8)?, b.f32(12)?];
        let v = b.pod_slice::<VisMask>(0x10, 8, "octant masks")?;
        o.octants = Some(OctantOverride { centre, masks: v.try_into().unwrap() });
    }
    Ok(o)
}

/// One mapping record (8 bytes): the bit the object tests, and the key the loader matches it by.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct OcclusionMapping {
    /// +0x00: bit index 0..1023 in the frame mask.
    pub bit_index: i32,
    /// +0x04: match key. tfrag: header byte 0x3d; tie: instance `occlusion_index`; moby: instance +0x0c (spawn id).
    pub occlusion_id: i32,
}

/// Gameplay occlusion mappings: `s32 tfrag_count, tie_count, moby_count, pad`, then the records in that order.
#[derive(Clone, Debug, Default)]
pub struct OcclusionMappings {
    pub tfrag: Vec<OcclusionMapping>,
    pub tie: Vec<OcclusionMapping>,
    pub moby: Vec<OcclusionMapping>,
}

pub fn parse_occlusion_mappings(section: &[u8]) -> Result<OcclusionMappings> {
    let b = Buf(section);
    let (nt, ni, nm) = (b.i32(0)?, b.i32(4)?, b.i32(8)?);
    if nt < 0 || ni < 0 || nm < 0 || nt + ni + nm > 100_000 { return invalid("occlusion mappings: bad counts"); }
    let (nt, ni, nm) = (nt as usize, ni as usize, nm as usize);
    Ok(OcclusionMappings {
        tfrag: b.pod_slice(0x10, nt, "tfrag mappings")?,
        tie: b.pod_slice(0x10 + nt * 8, ni, "tie mappings")?,
        moby: b.pod_slice(0x10 + (nt + ni) * 8, nm, "moby mappings")?,
    })
}

/// Mappings of a decompressed gameplay file; `None` when pointer 0x8c is 0 (the loader then prints
/// "no occlusion", marks everything always visible and sets the occlusion mode to off).
pub fn parse_gameplay_occlusion_mappings(gameplay: &[u8]) -> Result<Option<OcclusionMappings>> {
    let b = Buf(gameplay);
    let ofs = b.u32(GAMEPLAY_OCCLUSION_MAPPINGS)? as usize;
    if ofs == 0 { return Ok(None); }
    parse_occlusion_mappings(b.tail(ofs, "occlusion mappings")?.bytes()).map(Some)
}

/// An object's load-time occlusion word: high byte = mask byte, low byte = bit within it.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct OcclBits(pub u16);

impl OcclBits {
    /// Bit 1023, which every frame mask has set: the loader's "always visible" (`0x7f80`).
    pub const ALWAYS: OcclBits = OcclBits(0x7f80);
    /// `(bit >> 3) << 8 | 1 << (bit & 7)`, truncated to u16 like the loader's `sh`.
    pub fn from_bit(bit: i32) -> Self { OcclBits((((bit >> 3) << 8) | (1 << (bit & 7))) as u16) }
    pub fn byte(self) -> u8 { (self.0 >> 8) as u8 }
    pub fn bit(self) -> u8 { self.0 as u8 }
    /// The test the three procs run: `mask[byte] & bit != 0`. A byte past 0x7f (bit index ≥ 1024, never in
    /// retail) would read scratchpad beyond the mask on the EE; here it counts as not visible.
    pub fn visible(self, frame_mask: &VisMask) -> bool {
        frame_mask.get(self.byte() as usize).is_some_and(|&m| m & self.bit() != 0)
    }
}

/// Load-time resolution of every object's [`OcclBits`] (`FUN_00255958`, the block after the moby loader).
#[derive(Clone, Debug, Default)]
pub struct LevelOcclusion {
    /// Per tfrag (header table order), what the loader writes to header 0x3a.
    pub tfrag: Vec<OcclBits>,
    /// Per tie instance (gameplay order = runtime order), runtime tie +0x18.
    pub tie: Vec<OcclBits>,
    /// Per static gameplay moby instance, runtime moby +0x36.
    pub moby: Vec<OcclBits>,
    /// Tfrag count or any header byte 0x3d disagreed: all tfrags always visible ("occlusion out of date on tfrag").
    pub tfrag_out_of_date: bool,
    /// Ties took the positional path (counts equal and every `(s16)` key equal).
    pub ties_positional: bool,
    /// Ties / occlusion-culled mobys with no mapping ("occlusion out of date on ties/mobys (%d / %d)").
    pub ties_not_found: usize,
    pub mobys_not_found: usize,
}

/// Tfrags: mapping `i` ↔ tfrag `i`, accepted only if the counts match and every header byte 0x3d
/// (`TfragHeader::tri_count`) equals `occlusion_id` (compared as u32). Otherwise all always visible.
pub fn resolve_tfrag_bits(maps: &OcclusionMappings, tfrags: &[TfragHeader]) -> (Vec<OcclBits>, bool) {
    let stale = maps.tfrag.len() != tfrags.len()
        || tfrags.iter().zip(&maps.tfrag).any(|(t, m)| t.tri_count as u32 != m.occlusion_id as u32);
    let bits = if stale { vec![OcclBits::ALWAYS; tfrags.len()] } else { maps.tfrag.iter().map(|m| OcclBits::from_bit(m.bit_index)).collect() };
    (bits, stale)
}

/// Ties: positional when the counts match and every `(s16)` occlusion_index equals `(s16)` occlusion_id;
/// otherwise each tie takes the first mapping whose `occlusion_id` (u32) equals its u16 occlusion_index,
/// or always visible. Returns (bits, positional, not_found).
pub fn resolve_tie_bits(maps: &OcclusionMappings, occlusion_index: &[i32]) -> (Vec<OcclBits>, bool, usize) {
    let positional = maps.tie.len() == occlusion_index.len()
        && occlusion_index.iter().zip(&maps.tie).all(|(&o, m)| o as i16 == m.occlusion_id as i16);
    if positional { return (maps.tie.iter().map(|m| OcclBits::from_bit(m.bit_index)).collect(), true, 0); }
    let mut missing = 0;
    let bits = occlusion_index.iter().map(|&o| {
        let key = o as u16 as u32;
        match maps.tie.iter().find(|m| m.occlusion_id as u32 == key) {
            Some(m) => OcclBits::from_bit(m.bit_index),
            None => { missing += 1; OcclBits::ALWAYS }
        }
    }).collect();
    (bits, false, missing)
}

/// Mobys: an instance takes part only when its gameplay `occlusion` word is 0 (otherwise moby+0x36 = 0x7f80);
/// it takes the first mapping whose `occlusion_id` equals its `(s16)` spawn id (instance +0x0c, moby+0xb2),
/// or always visible. Mobys spawned later start at 0x7f80 (`InitMobyInstance`). Returns (bits, not_found).
pub fn resolve_moby_bits(maps: &OcclusionMappings, mobys: &[MobyInstance]) -> (Vec<OcclBits>, usize) {
    let mut missing = 0;
    let bits = mobys.iter().map(|mb| {
        if mb.occlusion != 0 { return OcclBits::ALWAYS; }
        let key = mb.spawn_id as i16 as i32;
        match maps.moby.iter().find(|m| m.occlusion_id == key) {
            Some(m) => OcclBits::from_bit(m.bit_index),
            None => { missing += 1; OcclBits::ALWAYS }
        }
    }).collect();
    (bits, missing)
}

/// All three resolutions. `maps == None` (no grid or no mappings) = the loader's "no occlusion" path.
pub fn resolve_level_occlusion(maps: Option<&OcclusionMappings>, tfrags: &[TfragHeader], tie_occlusion_index: &[i32], mobys: &[MobyInstance]) -> LevelOcclusion {
    let Some(maps) = maps else {
        return LevelOcclusion {
            tfrag: vec![OcclBits::ALWAYS; tfrags.len()], tie: vec![OcclBits::ALWAYS; tie_occlusion_index.len()],
            moby: vec![OcclBits::ALWAYS; mobys.len()], ..Default::default()
        };
    };
    let (tfrag, tfrag_out_of_date) = resolve_tfrag_bits(maps, tfrags);
    let (tie, ties_positional, ties_not_found) = resolve_tie_bits(maps, tie_occlusion_index);
    let (moby, mobys_not_found) = resolve_moby_bits(maps, mobys);
    LevelOcclusion { tfrag, tie, moby, tfrag_out_of_date, ties_positional, ties_not_found, mobys_not_found }
}

fn visible_in<'a>(bits: &'a [OcclBits], mask: &'a VisMask) -> impl Iterator<Item = usize> + 'a {
    bits.iter().enumerate().filter(move |(_, b)| b.visible(mask)).map(|(i, _)| i)
}

impl LevelOcclusion {
    /// Tfrag indices (header table order) that pass the occlusion test under `frame_mask`.
    pub fn visible_tfrags<'a>(&'a self, frame_mask: &'a VisMask) -> impl Iterator<Item = u16> + 'a { visible_in(&self.tfrag, frame_mask).map(|i| i as u16) }
    /// Tie instance indices (gameplay order).
    pub fn visible_ties<'a>(&'a self, frame_mask: &'a VisMask) -> impl Iterator<Item = u32> + 'a { visible_in(&self.tie, frame_mask).map(|i| i as u32) }
    /// Static gameplay moby instance indices.
    pub fn visible_mobys<'a>(&'a self, frame_mask: &'a VisMask) -> impl Iterator<Item = u32> + 'a { visible_in(&self.moby, frame_mask).map(|i| i as u32) }
}

/// `DAT_0016c4f4`: the debug menu's "occlusion" item (`off` / `freeze` / `active`). The level loader sets
/// `Active` when the level has a grid and mappings, else `Off`; retail never changes it after that.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OcclusionMode { Off = 0, Freeze = 1, Active = 2 }

/// `DAT_0015f608`: what to do when the camera's cell is not in the grid. Reset to 0 at the end of every
/// frame render (`0x21a1b8`) and at level load (`ResetDrawGlobals`); set to 1 by `FUN_002cb540` and to 2 by
/// `UpdateModeFreeze` for the frame that follows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OcclusionFallback {
    /// 0: try the 6 face neighbours (one per axis, nearer side first), else keep the previous mask.
    Neighbours = 0,
    /// 1: everything visible.
    AllVisible = 1,
    /// 2: the octant override if the level has one, else the previous mask.
    Octants = 2,
}

/// Where `0x15f610` ("previous mask") points.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviousMask { None, Grid(u16), Union }

/// The per-frame state of `UpdateOcclusion` / `BuildOcclVisibility`.
#[derive(Clone, Debug)]
pub struct OcclusionState {
    /// `0x174180`: the mask the procs test this frame.
    pub mask: VisMask,
    /// `0x174200`: OR of the neighbour cells' masks.
    pub union: VisMask,
    /// `0x15f610`.
    pub previous: PreviousMask,
    /// `0x15f60c`: 1 when the camera's own cell was not in the grid (written, never read in level01).
    pub missed: bool,
}

impl Default for OcclusionState {
    fn default() -> Self { OcclusionState { mask: [0xff; MASK_BYTES], union: [0; MASK_BYTES], previous: PreviousMask::None, missed: false } }
}

impl OcclusionState {
    /// `UpdateOcclusion` (0x2193f8) for one frame. `camera` = the render camera position (`0x167240`, world
    /// units); `debug_camera` = `DAT_0016c4ec != 0` (debug-menu camera mode, 0 in normal play).
    pub fn update(&mut self, occl: Option<&Occlusion>, mode: OcclusionMode, fallback: OcclusionFallback, debug_camera: bool, camera: [f32; 3]) -> &VisMask {
        match (mode, occl) {
            (OcclusionMode::Off, _) | (_, None) => self.mask = [0xff; MASK_BYTES],
            (OcclusionMode::Freeze, _) => {}
            (OcclusionMode::Active, Some(o)) => self.build(o, fallback, debug_camera, camera),
        }
        &self.mask
    }

    fn previous_mask<'a>(&'a self, o: &'a Occlusion) -> Option<VisMask> {
        match self.previous {
            PreviousMask::None => None,
            PreviousMask::Grid(i) => Some(*o.mask(i)),
            PreviousMask::Union => Some(self.union),
        }
    }

    /// `BuildOcclVisibility` (0x219008).
    fn build(&mut self, o: &Occlusion, fallback: OcclusionFallback, debug_camera: bool, camera: [f32; 3]) {
        let [x, y, z] = cell_coords(camera);
        if let Some(m) = o.lookup(x, y, z) {
            self.missed = false;
            self.mask = *o.mask(m);
            self.previous = PreviousMask::Grid(m);
        } else {
            self.missed = true;
            let mut done = false;
            if fallback == OcclusionFallback::Neighbours {
                // GetOcclGridFromPair (0x218f50): fraction = p*0.25 - (float)cell; < 0.5 tries the -1 side first.
                let pair = |frac: f32, lo: [i32; 3], hi: [i32; 3]| {
                    let (a, b) = if frac < 0.5 { (lo, hi) } else { (hi, lo) };
                    o.lookup(a[0], a[1], a[2]).or_else(|| o.lookup(b[0], b[1], b[2]))
                };
                let n = [
                    pair(camera[0] * 0.25 - x as f32, [x - 1, y, z], [x + 1, y, z]),
                    pair(camera[1] * 0.25 - y as f32, [x, y - 1, z], [x, y + 1, z]),
                    pair(camera[2] * 0.25 - z as f32, [x, y, z - 1], [x, y, z + 1]),
                ];
                if n.iter().any(Option::is_some) {
                    self.union = [0; MASK_BYTES];
                    for m in n.into_iter().flatten() {
                        for (u, v) in self.union.iter_mut().zip(o.mask(m)) { *u |= v; }
                    }
                    self.previous = PreviousMask::Union;
                    self.mask = self.union;
                    done = true;
                }
            }
            if !done {
                let all = [0xff; MASK_BYTES];
                self.mask = match fallback {
                    OcclusionFallback::AllVisible => all,
                    OcclusionFallback::Octants if o.octants.is_some() => *o.octants.as_ref().unwrap().mask_for(camera),
                    _ => if debug_camera { all } else { self.previous_mask(o).unwrap_or(all) },
                };
            }
        }
        self.mask[0x7f] |= 0x80;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A two-cell grid: z 3, y 5, x 7..9 with x=8 empty; masks 0 and 1.
    fn grid() -> Occlusion {
        let mut b = vec![0u8; 0x140];
        let w16 = |b: &mut Vec<u8>, o: usize, v: u16| b[o..o + 2].copy_from_slice(&v.to_le_bytes());
        b[0..4].copy_from_slice(&0x40i32.to_le_bytes());
        w16(&mut b, 4, 3); w16(&mut b, 6, 1); w16(&mut b, 8, 0x10 / 4);
        w16(&mut b, 0x10, 5); w16(&mut b, 0x12, 1); w16(&mut b, 0x14, 0x18 / 4);
        w16(&mut b, 0x18, 7); w16(&mut b, 0x1a, 3); w16(&mut b, 0x1c, 0); w16(&mut b, 0x1e, 0xffff); w16(&mut b, 0x20, 1);
        b[0x40] = 0x01;          // mask 0: bit 0
        b[0x40 + 0x80 + 1] = 0x04; // mask 1: bit 10
        parse_occlusion_block(&b).unwrap()
    }

    #[test]
    fn tree_walk_and_cell_mapping() {
        let o = grid();
        assert_eq!(o.cells, vec![OcclusionCell { x: 7, y: 5, z: 3, mask: 0 }, OcclusionCell { x: 9, y: 5, z: 3, mask: 1 }]);
        assert_eq!(o.masks.len(), 2);
        assert_eq!(o.lookup(8, 5, 3), None);
        assert_eq!(o.lookup(6, 5, 3), None);
        assert_eq!(o.lookup(10, 5, 3), None);
        assert_eq!(o.cell_for([28.0, 20.0, 12.0]).map(|c| c.mask), Some(0));
        assert_eq!(o.cell_for([39.99, 23.9, 15.9]).map(|c| c.mask), Some(1));
        // Truncation toward zero: -3.9 maps to cell 0, like 3.9.
        assert_eq!(cell_coords([-3.9, 3.9, -4.0]), [0, 0, -1]);
    }

    #[test]
    fn bits_and_fallbacks() {
        let o = grid();
        assert_eq!(OcclBits::from_bit(10), OcclBits(0x0104));
        assert_eq!(OcclBits::from_bit(1023), OcclBits::ALWAYS);
        let mut s = OcclusionState::default();
        let m = *s.update(Some(&o), OcclusionMode::Active, OcclusionFallback::Neighbours, false, [36.5, 20.5, 12.5]);
        assert!(OcclBits::from_bit(10).visible(&m) && !OcclBits::from_bit(0).visible(&m) && OcclBits::ALWAYS.visible(&m));
        // x cell 8 is empty: fraction 0.25 tries x=7 first, then x=9; one hit each on x only.
        let m = *s.update(Some(&o), OcclusionMode::Active, OcclusionFallback::Neighbours, false, [33.0, 20.5, 12.5]);
        assert!(OcclBits::from_bit(0).visible(&m) && !OcclBits::from_bit(10).visible(&m));
        assert_eq!(s.previous, PreviousMask::Union);
        // Far outside: previous (the union) is kept; with the debug camera everything is visible.
        let m = *s.update(Some(&o), OcclusionMode::Active, OcclusionFallback::Neighbours, false, [500.0, 0.0, 0.0]);
        assert!(OcclBits::from_bit(0).visible(&m) && !OcclBits::from_bit(5).visible(&m));
        let m = *s.update(Some(&o), OcclusionMode::Active, OcclusionFallback::Neighbours, true, [500.0, 0.0, 0.0]);
        assert!(OcclBits::from_bit(5).visible(&m));
        let m = *s.update(Some(&o), OcclusionMode::Active, OcclusionFallback::AllVisible, false, [36.5, 20.5, 12.5]);
        assert!(!OcclBits::from_bit(0).visible(&m), "a hit ignores the fallback");
        let m = *s.update(Some(&o), OcclusionMode::Freeze, OcclusionFallback::AllVisible, false, [500.0, 0.0, 0.0]);
        assert!(!OcclBits::from_bit(0).visible(&m), "freeze keeps the mask");
        let m = *s.update(Some(&o), OcclusionMode::Off, OcclusionFallback::Neighbours, false, [36.5, 20.5, 12.5]);
        assert!(OcclBits::from_bit(0).visible(&m));
    }

    #[test]
    fn mapping_resolution_rules() {
        let rec = |b: i32, id: i32| OcclusionMapping { bit_index: b, occlusion_id: id };
        let maps = OcclusionMappings { tfrag: vec![rec(3, 10), rec(4, 20)], tie: vec![rec(7, 1), rec(8, 2)], moby: vec![rec(9, 42)] };
        let mut t = [TfragHeader::default(); 2];
        t[0].tri_count = 10; t[1].tri_count = 20;
        assert_eq!(resolve_tfrag_bits(&maps, &t), (vec![OcclBits::from_bit(3), OcclBits::from_bit(4)], false));
        t[1].tri_count = 21;
        assert_eq!(resolve_tfrag_bits(&maps, &t), (vec![OcclBits::ALWAYS; 2], true));
        assert_eq!(resolve_tie_bits(&maps, &[1, 2]), (vec![OcclBits::from_bit(7), OcclBits::from_bit(8)], true, 0));
        assert_eq!(resolve_tie_bits(&maps, &[2, 5]), (vec![OcclBits::from_bit(8), OcclBits::ALWAYS], false, 1));
        let mut m = [MobyInstance::zeroed(); 3];
        m[0].spawn_id = 42; m[1].spawn_id = 42; m[1].occlusion = 1; m[2].spawn_id = 7;
        assert_eq!(resolve_moby_bits(&maps, &m), (vec![OcclBits::from_bit(9), OcclBits::ALWAYS, OcclBits::ALWAYS], 1));
    }
}
