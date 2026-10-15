//! Level-code water tables: the strip-mesh descriptors (classes 676, 678, 761, 1225), the ripple patch
//! module (class 751) and the gameplay cuboids its zones use. Spec: docs/plan/world_animation.md §1, §2.
//!
//! Every table lives in the **level overlay** (`LevelFiles::overlay`, `extracted/levels/NN/overlay.bin`),
//! Insomniac's "ratchet executable": `[dest, size, type, entry]` 16-byte headers each followed by `size`
//! bytes copied to EE address `dest` (docs/formats/wad_layouts_rac1.md §4). [`Overlay`] maps EE addresses
//! back to those bytes, so the tables are read at run time and no disc byte is in the source. The table
//! *addresses* and counts below are code constants of the level01 update functions (cited per constant).
//!
//! ## Strip descriptor (0x60 bytes; `FUN_002b96e0` draws `n` of them from a table)
//!
//! | off | type | field |
//! |---|---|---|
//! | 0x00 / 0x04 / 0x08 | ptr | vertices (`f32 x, y, z0, z1`, 16 B) / base UV (`f32 u, v`) / bounding sphere |
//! | 0x0c | s32 | vertex count (one GS tri-strip) |
//! | 0x10 / 0x14 | f32 | layer 1 / 2 scroll speed |
//! | 0x18 | f32 | UV wobble amplitude A |
//! | 0x1c | s32 | wobble phase step: `+0x50 = (+0x50 + step) & 0xffffff` per draw (1 on every Novalis strip) |
//! | 0x20..0x2c | f32 × 4 | directions (dx1, dy1), (dx2, dy2) |
//! | 0x30 | u32 | vertex RGBA (A = 0) |
//! | 0x34 / 0x38 | s32 | FX texture, layer 1 / 2 |
//! | 0x3c / 0x3d | u8 | ALPHA FIX, layer 1 / 2 |
//! | 0x40..0x4c | f32 × 4 | runtime scroll s1 = (u, v), s2 = (u, v) |
//! | 0x50 | s32 | runtime wobble phase |
//! | 0x5c | f32 | z blend w: `z = z0·w + z1·(1 − w)` |
//!
//! ## Ripple patch record (0x1190 bytes, table 0x1e34c0 × 21)
//!
//! +0x00 `f32 x, y, z` centre (z = water level); +0x0c `u8[8]` neighbour links (0xff none); +0x14 / +0x18 FX
//! env / water; +0x1c / +0x1d ALPHA FIX env / water; +0x1e u16 sub-block mask; +0x20..+0x44 UV scroll and
//! wobble state (set by init `0x2b7a48`); +0x44 pin flags; +0x50 three 0x5c0-byte height buffers.

use crate::buf::{invalid, Buf, Result};

// ---------------------------------------------------------------------------------------------------
// Overlay reader

/// One loaded section of a level overlay.
#[derive(Clone, Debug)]
pub struct OverlaySection {
    /// EE destination address.
    pub dest: u32,
    /// Byte offset of the section data in the overlay file.
    pub offset: usize,
    pub size: usize,
    /// ELF section type as stored (1 PROGBITS, 8 NOBITS; the NOBITS data is present in the file).
    pub ty: u32,
}

/// A level overlay as the loader copies it: EE address → bytes.
#[derive(Clone, Debug)]
pub struct Overlay<'a> {
    pub bytes: &'a [u8],
    pub sections: Vec<OverlaySection>,
    pub entry: u32,
}

impl<'a> Overlay<'a> {
    /// Walks the section headers (wad_layouts_rac1.md §4.2): stop at the end of the lump or at the first
    /// header whose entry point differs from the first one.
    pub fn parse(bytes: &'a [u8]) -> Result<Self> {
        let b = Buf(bytes);
        let mut sections = Vec::new();
        let mut off = 0usize;
        let entry = b.u32(4 * 3)?;
        while off + 0x10 <= bytes.len() {
            let (dest, size, ty, e) = (b.u32(off)?, b.u32(off + 4)?, b.u32(off + 8)?, b.u32(off + 12)?);
            if e != entry { break; }
            let size = size as usize;
            b.check(off + 0x10, size, "overlay section")?;
            sections.push(OverlaySection { dest, offset: off + 0x10, size, ty });
            off += 0x10 + size;
        }
        if sections.is_empty() { return invalid("overlay has no sections"); }
        Ok(Overlay { bytes, sections, entry })
    }

    /// `len` bytes at EE address `addr` (must lie inside one section).
    pub fn read(&self, addr: u32, len: usize) -> Result<&'a [u8]> {
        for s in &self.sections {
            let a = addr as u64;
            if a >= s.dest as u64 && a + len as u64 <= s.dest as u64 + s.size as u64 {
                let o = s.offset + (addr - s.dest) as usize;
                return Ok(&self.bytes[o..o + len]);
            }
        }
        invalid(format!("EE address {addr:#x} (+{len:#x}) is not in the overlay"))
    }

    pub fn buf(&self, addr: u32, len: usize) -> Result<Buf<'a>> { Ok(Buf(self.read(addr, len)?)) }
    pub fn u32(&self, addr: u32) -> Result<u32> { self.buf(addr, 4)?.u32(0) }
    pub fn i32(&self, addr: u32) -> Result<i32> { self.buf(addr, 4)?.i32(0) }
    pub fn f32(&self, addr: u32) -> Result<f32> { self.buf(addr, 4)?.f32(0) }
}

// ---------------------------------------------------------------------------------------------------
// Strip meshes

/// How a strip class animates its vertex z (docs/plan/world_animation.md §1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StripAnim {
    /// z blend stays at the stored +0x5c (0 on Novalis): z = z1.
    Static,
    /// 761: init `z0 = z1 = z − 0.45` then ±0.05 per rung pair; every tick, before drawing, all strips'
    /// +0x5c = [`crate`]-side `bob(t)` (update `0x2feb58`).
    Bob761,
    /// 1225: z0/z1 from the data; the draw callback `0x309bf8` writes the same w **after** drawing, so the
    /// draw uses last frame's value.
    BobAfterDraw1225,
}

/// One strip descriptor with its referenced arrays.
#[derive(Clone, Debug, PartialEq)]
pub struct StripDescriptor {
    /// EE address of the descriptor.
    pub addr: u32,
    /// `f32 x, y, z0, z1` per vertex as stored (the hash reads their raw bits).
    pub vertices: Vec<[f32; 4]>,
    pub uvs: Vec<[f32; 2]>,
    /// +0x08 target, as stored (678's is computed at init; zero on the disc).
    pub bsphere: [f32; 4],
    pub speed: [f32; 2],
    pub wobble_amp: f32,
    pub phase_step: i32,
    /// (dx1, dy1), (dx2, dy2).
    pub dir: [[f32; 2]; 2],
    pub rgba: u32,
    pub fx: [i32; 2],
    pub fix: [u8; 2],
    pub scroll: [f32; 4],
    pub phase: i32,
    pub z_blend: f32,
}

/// One strip class and its descriptor table.
#[derive(Clone, Debug, PartialEq)]
pub struct StripClass {
    pub class: u16,
    pub table: u32,
    pub anim: StripAnim,
    pub strips: Vec<StripDescriptor>,
}

/// Descriptor size.
pub const STRIP_DESC_SIZE: u32 = 0x60;

/// `FUN_002b96e0(n, table)`, the strip-mesh draw (level-01 label).
pub const STRIP_DRAW_FN: u32 = 0x2b96e0;

/// The strip-mesh water classes the port knows, by their level-01 code: (update, draw callback, animation).
/// 676 `0x2f6128` / `0x2f60f8`, 678 `0x2f6180` / `0x2f6150`, 761 `0x2feb58` / `0x2feb28`, 1225 `0x309c98` /
/// `0x309bf8`. Each callback is `fun_001f76a0` then `FUN_002b96e0(n, table)`: the table and count are read from the
/// callback's code in the level's overlay ([`strip_tables`]). The strip module exists only in the level-01 overlay
/// (no other overlay has a copy of `0x2b96e0`: docs/plan/level_generalisation.md W1), so only Novalis finds any.
pub const STRIP_PORTS: [(u32, u32, StripAnim); 4] = [
    (0x2f6128, 0x2f60f8, StripAnim::Static),
    (0x2f6180, 0x2f6150, StripAnim::Static),
    (0x2feb58, 0x2feb28, StripAnim::Bob761),
    (0x309c98, 0x309bf8, StripAnim::BobAfterDraw1225),
];

/// (class, table address, count, animation) of every strip class of the level `target`, found through the reference
/// (level-01) overlay: a port's update and callback are the level's copies (code identity,
/// [`crate::level_overlay::Relocation`]), the classes are the `lvl.vtbl` entries that run that update, and `n` /
/// `table` are the `addiu a0, zero, n` and the `lui`/`%lo` address before the callback's `jal` to the level's copy of
/// [`STRIP_DRAW_FN`]. Empty when the level has no strip module.
pub fn strip_tables(target: &crate::level_overlay::LevelOverlay, rel: &crate::level_overlay::Relocation) -> Vec<(u16, u32, usize, StripAnim)> {
    let Some(draw) = rel.func(STRIP_DRAW_FN) else { return Vec::new() };
    let jal = 0x0c00_0000 | ((draw >> 2) & 0x03ff_ffff);
    let vtbl = target.vtbl();
    let mut out = Vec::new();
    for &(update, callback, anim) in &STRIP_PORTS {
        let (Some(u), Some(cb)) = (rel.func(update), rel.func(callback)) else { continue };
        let Some(code) = target.extent(cb).and_then(|n| target.code(cb, n)) else { continue };
        let Some(at) = code.iter().position(|&w| w == jal) else { continue };
        // The arguments are set up to the call and its delay slot.
        let upto = &code[..(at + 2).min(code.len())];
        let n = upto.iter().rev().find(|&&w| w >> 16 == 0x2404).map(|&w| (w & 0xffff) as usize);
        let table = crate::level_overlay::address_refs(upto).into_iter().map(|(_, a)| a).next_back();
        let (Some(n), Some(table)) = (n, table) else { continue };
        for e in vtbl.iter().filter(|e| e.update == u) { out.push((e.o_class as u16, table, n, anim)); }
    }
    out
}

/// Parses one descriptor at `addr`.
pub fn parse_strip(ov: &Overlay, addr: u32) -> Result<StripDescriptor> {
    let d = ov.buf(addr, STRIP_DESC_SIZE as usize)?;
    let count = d.i32(0x0c)?;
    if !(3..=4096).contains(&count) { return invalid(format!("strip {addr:#x}: implausible vertex count {count}")); }
    let n = count as usize;
    let vb = ov.buf(d.u32(0x00)?, n * 16)?;
    let ub = ov.buf(d.u32(0x04)?, n * 8)?;
    let vertices = (0..n).map(|i| Ok([vb.f32(i * 16)?, vb.f32(i * 16 + 4)?, vb.f32(i * 16 + 8)?, vb.f32(i * 16 + 12)?])).collect::<Result<_>>()?;
    let uvs = (0..n).map(|i| Ok([ub.f32(i * 8)?, ub.f32(i * 8 + 4)?])).collect::<Result<_>>()?;
    let bs = ov.buf(d.u32(0x08)?, 16)?;
    let f = |o| d.f32(o);
    Ok(StripDescriptor {
        addr,
        vertices,
        uvs,
        bsphere: [bs.f32(0)?, bs.f32(4)?, bs.f32(8)?, bs.f32(12)?],
        speed: [f(0x10)?, f(0x14)?],
        wobble_amp: f(0x18)?,
        phase_step: d.i32(0x1c)?,
        dir: [[f(0x20)?, f(0x24)?], [f(0x28)?, f(0x2c)?]],
        rgba: d.u32(0x30)?,
        fx: [d.i32(0x34)?, d.i32(0x38)?],
        fix: [d.u8(0x3c)?, d.u8(0x3d)?],
        scroll: [f(0x40)?, f(0x44)?, f(0x48)?, f(0x4c)?],
        phase: d.i32(0x50)?,
        z_blend: f(0x5c)?,
    })
}

/// Every strip class of a level ([`strip_tables`]; empty for levels without a strip module).
pub fn parse_strip_classes(ov: &Overlay, tables: &[(u16, u32, usize, StripAnim)]) -> Result<Vec<StripClass>> {
    tables
        .iter()
        .map(|&(class, table, n, anim)| {
            let strips = (0..n as u32).map(|k| parse_strip(ov, table + k * STRIP_DESC_SIZE)).collect::<Result<_>>()?;
            Ok(StripClass { class, table, anim, strips })
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------------
// Ripple patches (class 751 + the engine module 0x2b7a48..0x2b96c0)

pub const PATCH_SIZE: u32 = 0x1190;
/// Height buffer: 16×16 f32 (row stride 0x40) then the halos, 0x5c0 bytes; three per patch at +0x50.
pub const HEIGHT_BUFFER_SIZE: u32 = 0x5c0;

/// One patch record as stored (the runtime fields are zero on the disc; `0x2b7a48` sets them).
#[derive(Clone, Debug, PartialEq)]
pub struct PatchRecord {
    pub centre: [f32; 3],
    /// +0x0c..+0x13. Which halo each link feeds is in rc_game::water (`MIRROR`).
    pub links: [u8; 8],
    /// +0x14 FX env map (TEX0_2), +0x18 FX water (TEX0_1).
    pub fx_env: i32,
    pub fx_water: i32,
    /// +0x1c ALPHA_2 FIX (env), +0x1d ALPHA_1 FIX (water).
    pub fix_env: u8,
    pub fix_water: u8,
    pub mask: u16,
    pub pins: u32,
}

/// One activation zone (751 pvar cuboid slot `i`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RippleZone {
    /// 0x1fa5e8[i], 0x1fa608[i], 0x1fa628[i].
    pub first_patch: i32,
    pub patch_count: i32,
    /// Random drop odds N (`randi(N) == 0` per tick per patch).
    pub drop_odds: i32,
}

/// The ripple module's tables for one level.
#[derive(Clone, Debug, PartialEq)]
pub struct RippleTables {
    pub base: u32,
    pub patches: Vec<PatchRecord>,
    pub zones: Vec<RippleZone>,
    /// Sub-block mask per patch (u16 at 0x1fa590 + 4·i), written to +0x1e while its zone is active.
    pub zone_masks: Vec<u16>,
    /// Drip start points (0x1fa650, 5 × `f32 x, y, z, w`).
    pub drips: Vec<[f32; 4]>,
    /// Zone-5 mist row table (0x1fa6a0, 20 bytes).
    pub mist_rows: Vec<u8>,
    /// Module constants 0x1cad00..0x1cad20 as stored (751 init overrides all but +0x1c).
    pub consts: [f32; 9],
    /// 0x1cafe0: base water UV of the 46 strip vertices of a 4×4-cell sub-block.
    pub uv_base: Vec<[f32; 2]>,
    /// 0x1cb1b0: wobble angle selector (0 = θ0, else θ1) per strip vertex.
    pub uv_select: Vec<u32>,
    /// 0x1cae00 / 12: the 46-vertex strip of a sub-block as indices into the 17×17 grid (relative to the
    /// sub-block's top-left vertex). 0x1cad40 / 4 (colours) and 0x1cada0 / 8 (env UV) give the same list.
    pub strip_order: Vec<u16>,
}

/// Number of vertices in one sub-block strip (`FUN_00262388`: 0x2e).
pub const SUB_STRIP_LEN: usize = 46;

/// Where the ripple engine module's constant tables are in one overlay: the level-01 addresses
/// ([`RippleModuleAddrs::REFERENCE`]) relocated through the module code that forms them ([`RippleModuleAddrs::locate`]).
/// The module (`0x2b7a48`..`0x2b96c0` and the VU0 kernels) is the same object code in every overlay that has it
/// (01, 05, 07, 11, 12, 13), linked at other addresses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RippleModuleAddrs {
    /// 0x1cad00: patch size, origin, cell, sphere-map scale, damping threshold, clock step (9 × f32).
    pub consts: u32,
    /// 0x1cafe0: base water UV per sub-block strip vertex.
    pub uv_base: u32,
    /// 0x1cb1b0: wobble angle selector per strip vertex.
    pub uv_select: u32,
    /// 0x1cae00 / 0x1cad40 / 0x1cada0: the sub-block strip order (position ×12, colour ×4, env UV ×8).
    pub order_pos: u32,
    pub order_rgba: u32,
    pub order_env: u32,
}

impl RippleModuleAddrs {
    /// The level-01 addresses.
    pub const REFERENCE: RippleModuleAddrs = RippleModuleAddrs { consts: 0x1cad00, uv_base: 0x1cafe0, uv_select: 0x1cb1b0, order_pos: 0x1cae00, order_rgba: 0x1cad40, order_env: 0x1cada0 };

    /// The target overlay's addresses (`rel`: level 01 → target); None when the target has no copy of the module.
    pub fn locate(rel: &crate::level_overlay::Relocation) -> Option<RippleModuleAddrs> {
        let r = RippleModuleAddrs::REFERENCE;
        rel.func(RIPPLE_INIT_FN)?;
        Some(RippleModuleAddrs {
            consts: rel.data(r.consts)?,
            uv_base: rel.data(r.uv_base)?,
            uv_select: rel.data(r.uv_select)?,
            order_pos: rel.data(r.order_pos)?,
            order_rgba: rel.data(r.order_rgba)?,
            order_env: rel.data(r.order_env)?,
        })
    }
}

/// `RipplePatchesInit` 0x2b7a48 (level-01 label): a level that has a copy has the ripple module.
pub const RIPPLE_INIT_FN: u32 = 0x2b7a48;

/// The ripple module's constant tables of one level.
#[derive(Clone, Debug, PartialEq)]
pub struct RippleModule {
    /// The constants as stored (every manager's init overrides all but +0x1c, the damping threshold).
    pub consts: [f32; 9],
    pub uv_base: Vec<[f32; 2]>,
    pub uv_select: Vec<u32>,
    /// The 46-vertex strip of a sub-block as indices into the 17×17 grid (the three order tables agree).
    pub strip_order: Vec<u16>,
}

/// Reads the module tables at `a`.
pub fn parse_ripple_module(ov: &Overlay, a: &RippleModuleAddrs) -> Result<RippleModule> {
    let mut consts = [0f32; 9];
    for (k, c) in consts.iter_mut().enumerate() { *c = ov.f32(a.consts + 4 * k as u32)?; }
    let uv_base = (0..SUB_STRIP_LEN as u32).map(|k| Ok([ov.f32(a.uv_base + 8 * k)?, ov.f32(a.uv_base + 8 * k + 4)?])).collect::<Result<_>>()?;
    let uv_select = (0..SUB_STRIP_LEN as u32).map(|k| ov.u32(a.uv_select + 4 * k)).collect::<Result<_>>()?;
    let order = |table: u32, stride: i16| -> Result<Vec<u16>> {
        (0..SUB_STRIP_LEN as u32)
            .map(|k| {
                let v = ov.buf(table + 2 * k, 2)?.i16(0)?;
                if v < 0 || v % stride != 0 || v / stride > 16 * 17 { return invalid(format!("bad sub-block order entry {v} at {table:#x}")); }
                Ok((v / stride) as u16)
            })
            .collect()
    };
    let strip_order = order(a.order_pos, 12)?;
    if order(a.order_rgba, 4)? != strip_order || order(a.order_env, 8)? != strip_order {
        return invalid("ripple colour / env-UV / position order tables disagree");
    }
    Ok(RippleModule { consts, uv_base, uv_select, strip_order })
}

/// The ripple manager 751's tables (level-01 code `0x2fd0e8`): the patch table and count of its init call
/// `FUN_002b7a48(0x1e34c0, 0x15)`, the zone loop's `< 7`, and the zone / mask / drip / mist tables it reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ripple751Addrs {
    pub patches: u32,
    pub patch_count: usize,
    pub zone_count: usize,
    pub first_patch: u32,
    pub patch_counts: u32,
    pub drop_odds: u32,
    pub masks: u32,
    pub drips: u32,
    pub mist_rows: u32,
}

/// `WaterSurfaceFxUpdate`, the ripple manager 751 (level-01 label).
pub const RIPPLE_751_FN: u32 = 0x2fd0e8;

impl Ripple751Addrs {
    pub const REFERENCE: Ripple751Addrs = Ripple751Addrs {
        patches: 0x1e34c0,
        patch_count: 21,
        zone_count: 7,
        first_patch: 0x1fa5e8,
        patch_counts: 0x1fa608,
        drop_odds: 0x1fa628,
        masks: 0x1fa590,
        drips: 0x1fa650,
        mist_rows: 0x1fa6a0,
    };

    /// The target overlay's addresses when it has a copy of 751's update (its code forms every table; the counts are
    /// its code constants, equal in a copy); None otherwise (only level 01 has one).
    pub fn locate(rel: &crate::level_overlay::Relocation) -> Option<Ripple751Addrs> {
        let r = Ripple751Addrs::REFERENCE;
        rel.func(RIPPLE_751_FN)?;
        Some(Ripple751Addrs {
            patches: rel.data(r.patches)?,
            first_patch: rel.data(r.first_patch)?,
            patch_counts: rel.data(r.patch_counts)?,
            drop_odds: rel.data(r.drop_odds)?,
            masks: rel.data(r.masks)?,
            drips: rel.data(r.drips)?,
            mist_rows: rel.data(r.mist_rows)?,
            ..r
        })
    }
}

pub fn parse_patch(ov: &Overlay, addr: u32) -> Result<PatchRecord> {
    let d = ov.buf(addr, 0x50)?;
    let mut links = [0u8; 8];
    links.copy_from_slice(&d.bytes()[0x0c..0x14]);
    Ok(PatchRecord {
        centre: [d.f32(0)?, d.f32(4)?, d.f32(8)?],
        links,
        fx_env: d.i32(0x14)?,
        fx_water: d.i32(0x18)?,
        fix_env: d.u8(0x1c)?,
        fix_water: d.u8(0x1d)?,
        mask: d.u16(0x1e)?,
        pins: d.u32(0x44)?,
    })
}

/// The 751 ripple tables at `a` with the module tables at `m` ([`Ripple751Addrs::locate`], [`RippleModuleAddrs::locate`]).
pub fn parse_ripple_tables(ov: &Overlay, a: &Ripple751Addrs, m: &RippleModuleAddrs) -> Result<RippleTables> {
    let (base, n) = (a.patches, a.patch_count);
    let patches = parse_patches(ov, base, n)?;
    let zones = (0..a.zone_count as u32)
        .map(|i| Ok(RippleZone { first_patch: ov.i32(a.first_patch + 4 * i)?, patch_count: ov.i32(a.patch_counts + 4 * i)?, drop_odds: ov.i32(a.drop_odds + 4 * i)? }))
        .collect::<Result<Vec<_>>>()?;
    let zone_masks = (0..n as u32).map(|i| ov.buf(a.masks + 4 * i, 2)?.u16(0)).collect::<Result<_>>()?;
    let drips = (0..5u32).map(|i| { let b = ov.buf(a.drips + 16 * i, 16)?; Ok([b.f32(0)?, b.f32(4)?, b.f32(8)?, b.f32(12)?]) }).collect::<Result<_>>()?;
    let mist_rows = ov.read(a.mist_rows, 20)?.to_vec();
    let RippleModule { consts, uv_base, uv_select, strip_order } = parse_ripple_module(ov, m)?;
    Ok(RippleTables { base, patches, zones, zone_masks, drips, mist_rows, consts, uv_base, uv_select, strip_order })
}

/// `n` patch records from `base` (stride [`PATCH_SIZE`]).
pub fn parse_patches(ov: &Overlay, base: u32, n: usize) -> Result<Vec<PatchRecord>> {
    (0..n as u32).map(|i| parse_patch(ov, base + i * PATCH_SIZE)).collect()
}

impl RippleTables {
    /// The module part of the tables.
    pub fn module(&self) -> RippleModule {
        RippleModule { consts: self.consts, uv_base: self.uv_base.clone(), uv_select: self.uv_select.clone(), strip_order: self.strip_order.clone() }
    }
}

// ---------------------------------------------------------------------------------------------------
// Gameplay cuboids (section 0x60) and the 751 pvar

/// One shape record (0x80 bytes, wad_layouts_rac1.md §3): matrix (translation row at +0x30) and the 3×4
/// inverse at +0x40 that `0x274820` applies to `p − position`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cuboid {
    pub matrix: [[f32; 4]; 4],
    pub inverse: [[f32; 4]; 3],
}

/// Gameplay pointer of the cuboid section: `s32 count`, pad to 0x10, then `count` × 0x80.
pub const CUBOIDS_POINTER: usize = 0x60;

pub fn parse_cuboids(gameplay: &[u8]) -> Result<Vec<Cuboid>> {
    let g = Buf(gameplay);
    let s = g.u32(CUBOIDS_POINTER)? as usize;
    if s == 0 { return Ok(Vec::new()); }
    let n = g.i32(s)?;
    if !(0..=0x10000).contains(&n) { return invalid("implausible cuboid count"); }
    (0..n as usize)
        .map(|i| {
            let r = g.sub(s + 0x10 + i * 0x80, 0x80, "cuboid")?;
            let row = |o: usize| -> Result<[f32; 4]> { Ok([r.f32(o)?, r.f32(o + 4)?, r.f32(o + 8)?, r.f32(o + 12)?]) };
            Ok(Cuboid { matrix: [row(0)?, row(0x10)?, row(0x20)?, row(0x30)?], inverse: [row(0x40)?, row(0x50)?, row(0x60)?] })
        })
        .collect()
}

/// The 751 instance's zone cuboid indices: pvar +0x00..+0x18, one s32 per zone (−1 = none).
pub fn ripple_zone_cuboids(pvar: &[u8], zones: usize) -> Result<Vec<i32>> {
    let b = Buf(pvar);
    (0..zones).map(|i| b.i32(4 * i)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_walk_and_read() {
        let mut ov = Vec::new();
        for (dest, data) in [(0x1000u32, vec![1u8, 2, 3, 4]), (0x2000, vec![5, 6, 7, 8, 9, 10, 11, 12])] {
            for w in [dest, data.len() as u32, 1, 0xabc] { ov.extend_from_slice(&w.to_le_bytes()); }
            ov.extend_from_slice(&data);
        }
        // A trailing header with a different entry point ends the walk.
        for w in [0x3000u32, 4, 1, 0xdef] { ov.extend_from_slice(&w.to_le_bytes()); }
        let o = Overlay::parse(&ov).unwrap();
        assert_eq!(o.sections.len(), 2);
        assert_eq!(o.read(0x1002, 2).unwrap(), &[3, 4]);
        assert_eq!(o.u32(0x2004).unwrap(), u32::from_le_bytes([9, 10, 11, 12]));
        assert!(o.read(0x1002, 4).is_err());
        assert!(o.read(0x3000, 1).is_err());
    }

    /// overlay, core index, core data (WAD), gameplay (WAD) of Novalis from `extracted/`.
    fn novalis() -> Option<[Vec<u8>; 4]> {
        let root = crate::test_data::root().join("levels/01");
        let read = |n: &str| std::fs::read(root.join(n)).ok();
        Some([read("overlay.bin")?, read("core_index.bin")?, read("core_data.bin")?, read("gameplay_ntsc.bin")?])
    }

    /// The Novalis tables match docs/plan/world_animation.md and every FX texture they name exists.
    #[test]
    fn novalis_water_tables() {
        let Some([ov_bytes, index, data_wad, gameplay_wad]) = novalis() else { eprintln!("skipped: no extracted/levels/01"); return };
        let ov = Overlay::parse(&ov_bytes).unwrap();
        let lo = crate::level_overlay::LevelOverlay::parse(&ov_bytes).unwrap();
        let rel = crate::level_overlay::Relocation::new(&lo, &lo);
        let classes = parse_strip_classes(&ov, &strip_tables(&lo, &rel)).unwrap();
        let counts: Vec<(u16, usize)> = classes.iter().map(|c| (c.class, c.strips.len())).collect();
        assert_eq!(counts, [(676, 8), (678, 2), (761, 4), (1225, 3)]);
        let vcounts = |c: u16| classes.iter().find(|k| k.class == c).unwrap().strips.iter().map(|s| s.vertices.len()).collect::<Vec<_>>();
        assert_eq!(vcounts(761), [48, 48, 48, 40]);
        assert_eq!(vcounts(678), [4, 4]);
        assert!(vcounts(676).iter().all(|&n| (30..=38).contains(&n)));
        assert!(vcounts(1225).iter().all(|&n| (70..=94).contains(&n)));
        for c in &classes {
            for s in &c.strips {
                assert_eq!(s.phase_step, 1, "class {} strip {:#x}: wobble phase step", c.class, s.addr);
                assert_eq!(s.rgba >> 24, 0, "vertex alpha 0");
                let (fx, fix) = match c.class { 761 => (44, 0x28), 1225 => (43, 0x30), _ => (44, 0x30) };
                assert_eq!((s.fx, s.fix), ([fx, fx], [fix, fix]), "class {}", c.class);
            }
        }
        // Waterfalls: strips 4 and 6 of 676 drop from z 60.2 to below 39.
        let c676 = &classes[0].strips;
        for k in [4, 6] {
            let z: Vec<f32> = c676[k].vertices.iter().map(|v| v[3]).collect();
            assert!(z[0] > 60.0 && z.iter().cloned().fold(f32::MAX, f32::min) < 39.5, "676 strip {k} z {z:?}");
        }

        let rt = parse_ripple_tables(&ov, &Ripple751Addrs::locate(&rel).unwrap(), &RippleModuleAddrs::locate(&rel).unwrap()).unwrap();
        assert_eq!(rt.patches.len(), 21);
        assert_eq!(rt.patches.iter().filter(|p| p.centre != [0.0; 3]).count(), 18, "18 patches used");
        assert_eq!(rt.zones.iter().map(|z| z.patch_count).collect::<Vec<_>>(), [4, 2, 6, 1, 0, 4, 1]);
        assert_eq!(rt.zones.iter().map(|z| z.drop_odds).collect::<Vec<_>>(), [200, 2000, 2000, 2000, 2000, 200, 2000]);
        assert_eq!(rt.strip_order[..12], [0, 17, 1, 18, 2, 19, 3, 20, 4, 21, 21, 17]);
        assert_eq!(rt.consts[7], 0.16, "damping threshold 0x1cad1c");
        assert_eq!(rt.patches[15].centre, [186.1, 170.0, 39.0]);

        let data = crate::wad::decompress(&data_wad).unwrap();
        let core = crate::level::parse_level_core(&index, data.len()).unwrap();
        let tex = crate::particle_tex::parse_particle_textures(&core, &index, &data).unwrap();
        let has = |i: i32| tex.fx_textures.get(i as usize).is_some_and(|t| t.is_some());
        for c in &classes { for s in &c.strips { assert!(has(s.fx[0]) && has(s.fx[1])); } }
        for p in rt.patches.iter().take(18) { assert!(has(p.fx_env) && has(p.fx_water), "patch {p:?}"); }

        let gameplay = crate::wad::decompress(&gameplay_wad).unwrap();
        let cuboids = parse_cuboids(&gameplay).unwrap();
        let inst = crate::gameplay::parse_moby_instances(&gameplay).unwrap();
        let pvars = crate::gameplay::parse_pvars(&gameplay).unwrap();
        let m751 = inst.iter().find(|m| m.o_class == 751).expect("751 instance");
        let idx = ripple_zone_cuboids(m751.pvar(&pvars).unwrap(), 7).unwrap();
        assert_eq!(idx, [2, 3, 4, 5, 6, 7, 8]);
        let c0 = cuboids[idx[0] as usize];
        assert!((c0.matrix[3][0] - 164.85).abs() < 0.01 && (c0.matrix[3][1] - 96.32).abs() < 0.01);
    }
}
