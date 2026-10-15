//! Level-code sea / liquid surface tables (docs/plan/world_animation.md §3.3). Three mechanisms, each read from the level
//! overlay at run time (no disc bytes in the source; the addresses are code constants of the reference levels):
//!
//! * **The liquid grid module** (levels 03, 05, 07, 08, 09, 14; reference level05 `0x2ce098` init, `0x2ce130` texture and
//!   GS set-up, `0x2775c8` block cull, `0x277900` block draw, `0x2ce720` fog restore): a flat grid of 7×7-cell blocks at
//!   one height, one 124-vertex GS strip per block, textured with a 64×64 image the module blends every frame from two
//!   frames of an FX texture sequence. Its state is a 0x40-byte record in the class's overlay data ([`LiquidGrid`]);
//!   the strip order and the per-vertex ST are module constants ([`LiquidGridModule`]).
//! * **The camera-following ocean** (class 1111 on levels 11 and 16, reference level11 `0x30b358` / `0x30a908`): no
//!   geometry in the data; the per-layer ST scales and scroll speeds are two small overlay tables ([`OceanTables`]).
//! * **Static liquid strips** (class 1901 on level 12, reference level12 `0x30bff0` / `0x30be68`): strip meshes given
//!   by tables of pointers to positions, colours and ST ([`StripMesh`]).

use crate::buf::{invalid, Result};
use crate::water::Overlay;

// ---------------------------------------------------------------------------------------------------
// The liquid grid module

/// Vertices of one block strip (the module's index table is 0xf8 bytes: 124 `s16`).
pub const GRID_STRIP_LEN: usize = 124;
/// Cells per block side (`0x2775c8`: block size = 7 · cell, `0x277900`: 8 × 8 grid points per block).
pub const GRID_BLOCK_CELLS: usize = 7;
/// Grid points per block side.
pub const GRID_BLOCK_POINTS: usize = GRID_BLOCK_CELLS + 1;
/// The animated texture's side (`0x2ce130`: TRXREG 0x40 × 0x40, TEX0 TW = TH = 6).
pub const GRID_TEXTURE_SIDE: u32 = 64;

/// The module's constant tables (reference level05): the block strip order and the per-vertex ST.
#[derive(Clone, Debug, PartialEq)]
pub struct LiquidGridModule {
    /// Grid point (row · 8 + column) of each strip vertex: level05 `0x1cb280`, 124 `s16` byte offsets into the
    /// 8-byte (x, y) points `0x277900` writes to the scratchpad (offset / 8), DMA'd by `0x2ce830`.
    pub order: [u8; GRID_STRIP_LEN],
    /// Per strip vertex (s, t) before the module init's scale: level05 `0x1cb390` (after the 16-byte VIF header at
    /// `0x1cb380`); `0x2ce098` copies `st · scale` to `0x1cb850`, which every block's draw sends (V2-32, Q = 1).
    pub st: [[f32; 2]; GRID_STRIP_LEN],
}

/// The reference level of the module and its data labels.
pub mod grid_ref {
    /// The level whose overlay the addresses below are from.
    pub const LEVEL: u32 = 5;
    /// `0x2ce098`: the module init `(f12 scale, a0 state, a1 FIX)`: `state+0x3f = FIX`, the scaled ST table.
    pub const INIT_FN: u32 = 0x2c_e098;
    /// The ST table block the init reads (a 16-byte VIF header, then 248 floats); the strip order table is the 0x100
    /// bytes before it in every overlay that has the module (03, 05, 07, 08, 09, 14).
    pub const ST_BLOCK: u32 = 0x1c_b380;
}

impl LiquidGridModule {
    /// The tables at the module's ST block `block` (see [`grid_ref::ST_BLOCK`]).
    pub fn parse(ov: &Overlay, block: u32) -> Result<LiquidGridModule> {
        let ob = ov.buf(block - 0x100, GRID_STRIP_LEN * 2)?;
        let mut order = [0u8; GRID_STRIP_LEN];
        for (k, o) in order.iter_mut().enumerate() {
            let off = ob.i16(2 * k)?;
            if off < 0 || off % 8 != 0 || off / 8 >= (GRID_BLOCK_POINTS * GRID_BLOCK_POINTS) as i16 {
                return invalid(format!("liquid grid: strip order entry {k} = {off:#x}"));
            }
            *o = (off / 8) as u8;
        }
        let sb = ov.buf(block + 0x10, GRID_STRIP_LEN * 8)?;
        let mut st = [[0.0f32; 2]; GRID_STRIP_LEN];
        for (k, v) in st.iter_mut().enumerate() { *v = [sb.f32(8 * k)?, sb.f32(8 * k + 4)?]; }
        Ok(LiquidGridModule { order, st })
    }
}

/// One liquid grid's state record (0x40 bytes in the class's overlay data; level05 `0x211b20` for class 879).
///
/// | off | type | field |
/// |---|---|---|
/// | 0x00 | f32 × 3, f32 | origin (x, y, z); w = the squared cull distance of a block centre (160000 = 400²) |
/// | 0x10 | f32 × 2 | cell size (x, y) |
/// | 0x18 / 0x1a | u16 | cells across / down (a multiple of 7 on every level) |
/// | 0x1c | ptr | the 64×64 image buffer (runtime) |
/// | 0x20..0x2c | f32 × 4 | the fog the draw uses: near, far (integer depth units), near / far intensity |
/// | 0x30..0x32 | u8 × 3 | FOGCOL |
/// | 0x33 | u8 | ticks per animation frame |
/// | 0x34 | ptr | per-block vertex colours: 0x200 bytes per block (a VIF UNPACK V4-8 header, then 124 RGBA) |
/// | 0x38 | ptr | per-block flags (one byte per block, 0 = no water there) |
/// | 0x3c / 0x3e | u16 / u8 | first FX texture of the animation / frame count |
/// | 0x3f | u8 | ALPHA FIX (written by the module init) |
#[derive(Clone, Debug, PartialEq)]
pub struct LiquidGrid {
    /// The record's address in the overlay.
    pub addr: u32,
    pub origin: [f32; 3],
    pub cull_dist2: f32,
    pub cell: [f32; 2],
    pub cells: [u16; 2],
    pub fog: [f32; 4],
    pub fog_rgb: [u8; 3],
    pub period: u8,
    /// Per block, row-major (blocks of one row, then the next row).
    pub flags: Vec<u8>,
    /// Per block, the strip's 124 vertex colours (GS RGBA bytes, R low).
    pub colours: Vec<[u32; GRID_STRIP_LEN]>,
    pub tex: u16,
    pub frames: u8,
}

impl LiquidGrid {
    /// Blocks across / down (`0x2775c8` steps 7 cells while cells remain).
    pub fn blocks(&self) -> [usize; 2] { self.cells.map(|c| (c as usize).div_ceil(GRID_BLOCK_CELLS)) }

    pub fn parse(ov: &Overlay, addr: u32) -> Result<LiquidGrid> {
        let b = ov.buf(addr, 0x40)?;
        let f = |o: usize| b.f32(o);
        let cells = [b.u16(0x18)?, b.u16(0x1a)?];
        let mut g = LiquidGrid {
            addr,
            origin: [f(0)?, f(4)?, f(8)?],
            cull_dist2: f(0xc)?,
            cell: [f(0x10)?, f(0x14)?],
            cells,
            fog: [f(0x20)?, f(0x24)?, f(0x28)?, f(0x2c)?],
            fog_rgb: [b.u8(0x30)?, b.u8(0x31)?, b.u8(0x32)?],
            period: b.u8(0x33)?,
            flags: Vec::new(),
            colours: Vec::new(),
            tex: b.u16(0x3c)?,
            frames: b.u8(0x3e)?,
        };
        if g.period == 0 || g.frames == 0 || cells.contains(&0) { return invalid(format!("liquid grid {addr:#x}: empty animation or grid")); }
        let [bx, by] = g.blocks();
        let n = bx * by;
        g.flags = ov.read(b.u32(0x38)?, n)?.to_vec();
        let cb = b.u32(0x34)?;
        for k in 0..n {
            let blk = ov.buf(cb + 0x200 * k as u32, 0x200)?;
            let mut c = [0u32; GRID_STRIP_LEN];
            for (i, v) in c.iter_mut().enumerate() { *v = blk.u32(0x10 + 4 * i)?; }
            g.colours.push(c);
        }
        Ok(g)
    }
}

// ---------------------------------------------------------------------------------------------------
// The camera-following ocean (class 1111)

/// Class 1111's two layer tables (level11 labels; the update reads them by address).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OceanTables {
    /// `$gp − 0x4e08` (level11 `0x161df8`): each layer's ST repeat per two tile sizes.
    pub scale: [f32; 2],
    /// Level11 `0x1d9af0`: each layer's (s, t) scroll per second.
    pub speed: [[f32; 2]; 2],
}

pub mod ocean_ref {
    pub const LEVEL: u32 = 11;
    pub const UPDATE_FN: u32 = 0x30_b358;
    pub const DRAW_FN: u32 = 0x30_a908;
    pub const SCALE: u32 = 0x16_1df8;
    pub const SPEED: u32 = 0x1d_9af0;
}

impl OceanTables {
    pub fn parse(ov: &Overlay, scale: u32, speed: u32) -> Result<OceanTables> {
        Ok(OceanTables {
            scale: [ov.f32(scale)?, ov.f32(scale + 4)?],
            speed: [[ov.f32(speed)?, ov.f32(speed + 4)?], [ov.f32(speed + 8)?, ov.f32(speed + 12)?]],
        })
    }
}

// ---------------------------------------------------------------------------------------------------
// Static liquid strips (class 1901)

/// One GS strip of a static liquid mesh (`FUN_00220d68(n, positions, colours, st, 1)`, level01 `0x21fda8`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StripMesh {
    pub pos: Vec<[f32; 3]>,
    /// GS RGBA bytes, R low, as stored.
    pub rgba: Vec<u32>,
    pub st: Vec<[f32; 2]>,
}

/// `n` strips from four tables: pointers to the ST, positions and colours, and the vertex counts.
pub fn parse_strip_meshes(ov: &Overlay, st_ptrs: u32, pos_ptrs: u32, rgba_ptrs: u32, counts: u32, n: usize) -> Result<Vec<StripMesh>> {
    let mut out = Vec::with_capacity(n);
    for k in 0..n as u32 {
        let c = ov.i32(counts + 4 * k)?;
        if !(0..=0x1000).contains(&c) { return invalid(format!("liquid strip {k}: {c} vertices")); }
        let c = c as usize;
        let (sp, pp, cp) = (ov.u32(st_ptrs + 4 * k)?, ov.u32(pos_ptrs + 4 * k)?, ov.u32(rgba_ptrs + 4 * k)?);
        let (sb, pb, cb) = (ov.buf(sp, 8 * c)?, ov.buf(pp, 12 * c)?, ov.buf(cp, 4 * c)?);
        let mut m = StripMesh::default();
        for i in 0..c {
            m.st.push([sb.f32(8 * i)?, sb.f32(8 * i + 4)?]);
            m.pos.push([pb.f32(12 * i)?, pb.f32(12 * i + 4)?, pb.f32(12 * i + 8)?]);
            m.rgba.push(cb.u32(4 * i)?);
        }
        out.push(m);
    }
    Ok(out)
}

/// Class 1901's tables (level12 labels).
pub mod hoven_ref {
    pub const LEVEL: u32 = 12;
    pub const UPDATE_FN: u32 = 0x30_bff0;
    pub const DRAW_FN: u32 = 0x30_be68;
    /// Group 1 (drawn while the camera is in the moby's cuboid): ST / position / colour pointer tables, counts, 10 strips.
    pub const G1: (u32, u32, u32, u32, usize) = (0x20_5a08, 0x20_59e0, 0x20_5a58, 0x1f_bd10, 10);
    /// Group 2 (always drawn, two layers): 5 strips.
    pub const G2: (u32, u32, u32, u32, usize) = (0x20_8ab0, 0x20_8a98, 0x20_8ae0, 0x20_5a80, 5);
    /// `$gp` globals (level12): the second layer's ALPHA FIX (`0x1620d4`), group 1's scroll speed (s, t)
    /// (`0x1620d8`, `0x1620dc`), the layers' scroll speeds `0x1620e0 + 8·layer` (s, t).
    pub const FIX2: u32 = 0x16_20d4;
    pub const G1_SPEED: u32 = 0x16_20d8;
    pub const LAYER_SPEED: u32 = 0x16_20e0;
}

// ---------------------------------------------------------------------------------------------------
// Liquid meshes drawn through the generic strip emitters (G-REN-026)

/// One culled liquid mesh: a bounding sphere (`FastBSphereCheck(256, record)`: x, y, z, r) and its strips. A strip
/// without stored colours (the lava flows, the two-texture strips) takes its colours from the caller; one without
/// normals has none (only the two-texture strips' sphere map reads them).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LiquidMesh {
    /// None: drawn without the sphere check.
    pub sphere: Option<[f32; 4]>,
    pub strips: Vec<StripMesh>,
    /// Per strip, the stored normals (empty: none).
    pub normals: Vec<Vec<[f32; 3]>>,
}

/// Reads `c` positions (12 bytes) and ST pairs (8 bytes) from `pos` / `st` into a strip without colours.
fn strip_at(ov: &Overlay, pos: u32, st: u32, c: usize) -> Result<StripMesh> {
    let (pb, sb) = (ov.buf(pos, 12 * c)?, ov.buf(st, 8 * c)?);
    let mut m = StripMesh::default();
    for i in 0..c {
        m.pos.push([pb.f32(12 * i)?, pb.f32(12 * i + 4)?, pb.f32(12 * i + 8)?]);
        m.st.push([sb.f32(8 * i)?, sb.f32(8 * i + 4)?]);
    }
    Ok(m)
}

/// The lava-flow mesh table of level 9's 317 callback (`0x21e8c0(t, table, n, fx a, fx b)`): `n` records of 0x40 bytes:
/// +0x00 the bounding sphere, +0x10 the positions, +0x14 the ST, +0x1c the strip count k, +0x20 k + 1 `s16` vertex
/// starts (strip j = vertices start[j] .. start[j + 1] of the two arrays).
pub fn parse_flow_meshes(ov: &Overlay, table: u32, n: usize) -> Result<Vec<LiquidMesh>> {
    let mut out = Vec::with_capacity(n);
    for k in 0..n as u32 {
        let b = ov.buf(table + 0x40 * k, 0x40)?;
        let sphere = [b.f32(0)?, b.f32(4)?, b.f32(8)?, b.f32(0xc)?];
        let (pos, st, count) = (b.u32(0x10)?, b.u32(0x14)?, b.i32(0x1c)?);
        if !(0..=15).contains(&count) { return invalid(format!("liquid flow mesh {k}: {count} strips")); }
        let mut m = LiquidMesh { sphere: Some(sphere), ..Default::default() };
        for j in 0..count as usize {
            let (a, e) = (b.i16(0x20 + 2 * j)?, b.i16(0x22 + 2 * j)?);
            if a < 0 || e < a { return invalid(format!("liquid flow mesh {k}: strip {j} {a}..{e}")); }
            m.strips.push(strip_at(ov, pos + 12 * a as u32, st + 8 * a as u32, (e - a) as usize)?);
            m.normals.push(Vec::new());
        }
        out.push(m);
    }
    Ok(out)
}

/// A table of `n` one-strip mesh records of 0x20 bytes (level 9's grid-textured meshes `0x2c1978(state, table, n)`,
/// the two-texture strip module level12 `0x2bc210(table, n, tex0, tex1)`): +0x00 the bounding sphere, +0x10 the
/// positions, +0x14 the ST, +0x18 the normals (read with `normals`), +0x1c the vertex count.
pub fn parse_mesh_records(ov: &Overlay, table: u32, n: usize, normals: bool) -> Result<Vec<LiquidMesh>> {
    let mut out = Vec::with_capacity(n);
    for k in 0..n as u32 {
        let b = ov.buf(table + 0x20 * k, 0x20)?;
        let sphere = [b.f32(0)?, b.f32(4)?, b.f32(8)?, b.f32(0xc)?];
        let c = b.i32(0x1c)?;
        if !(0..=0x1000).contains(&c) { return invalid(format!("liquid mesh {k}: {c} vertices")); }
        let c = c as usize;
        let strip = strip_at(ov, b.u32(0x10)?, b.u32(0x14)?, c)?;
        let nrm = if normals {
            let nb = ov.buf(b.u32(0x18)?, 12 * c)?;
            (0..c).map(|i| Ok([nb.f32(12 * i)?, nb.f32(12 * i + 4)?, nb.f32(12 * i + 8)?])).collect::<Result<Vec<_>>>()?
        } else {
            Vec::new()
        };
        out.push(LiquidMesh { sphere: Some(sphere), strips: vec![strip], normals: vec![nrm] });
    }
    Ok(out)
}

/// `n` unculled meshes of one strip each from four tables (class 1848's draw `0x30f0e0`): the vertex counts (`s32`) and
/// pointers to the positions, the normals and the colours (GS RGBA bytes, R low). No ST is stored (the draw computes
/// it): the strips' ST are zeros.
pub fn parse_pointer_meshes(ov: &Overlay, counts: u32, pos_ptrs: u32, normal_ptrs: u32, rgba_ptrs: u32, n: usize) -> Result<Vec<LiquidMesh>> {
    let mut out = Vec::with_capacity(n);
    for k in 0..n as u32 {
        let c = ov.i32(counts + 4 * k)?;
        if !(0..=0x1000).contains(&c) { return invalid(format!("pointer mesh {k}: {c} vertices")); }
        let c = c as usize;
        let (pb, nb, cb) = (ov.buf(ov.u32(pos_ptrs + 4 * k)?, 12 * c)?, ov.buf(ov.u32(normal_ptrs + 4 * k)?, 12 * c)?, ov.buf(ov.u32(rgba_ptrs + 4 * k)?, 4 * c)?);
        let mut m = StripMesh::default();
        let mut nrm = Vec::with_capacity(c);
        for i in 0..c {
            m.pos.push([pb.f32(12 * i)?, pb.f32(12 * i + 4)?, pb.f32(12 * i + 8)?]);
            nrm.push([nb.f32(12 * i)?, nb.f32(12 * i + 4)?, nb.f32(12 * i + 8)?]);
            m.rgba.push(cb.u32(4 * i)?);
            m.st.push([0.0; 2]);
        }
        out.push(LiquidMesh { sphere: None, strips: vec![m], normals: vec![nrm] });
    }
    Ok(out)
}

/// The ALPHA FIX of GS contexts 1 and 2 in a strip-state packet (a GIF tag and eight A+D registers: TEST_1/2, TEX1_1,
/// CLAMP_1, ALPHA_1 (0x42), TEX1_2, CLAMP_2, ALPHA_2 (0x43)): the two-texture module's (level12 `0x1cb7a0`, level14
/// `0x1cba20`) and the lava flows' (level09 `0x16eaa0`). Every packet's ALPHA is `FIX << 32 | 0x64`.
pub fn strip_state_fix(ov: &Overlay, packet: u32) -> Result<[u8; 2]> {
    let mut fix = [None, None];
    for i in 1..9u32 {
        let b = ov.buf(packet + 0x10 * i, 0x10)?;
        let (lo, hi, reg) = (b.u32(0)?, b.u32(4)?, b.u32(8)?);
        if (reg == 0x42 || reg == 0x43) && lo == 0x64 { fix[(reg - 0x42) as usize] = Some(hi as u8); }
    }
    match fix {
        [Some(a), Some(b)] => Ok([a, b]),
        _ => invalid(format!("strip state packet {packet:#x}: no ALPHA_1 / ALPHA_2 of the form FIX << 32 | 0x64")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The grid state record's fields at their offsets, the block tables behind its pointers.
    #[test]
    fn liquid_grid_record_layout() {
        // One section at 0x1000: the record, one block's flags and colours.
        let mut data = vec![0u8; 0x400];
        let put = |d: &mut Vec<u8>, o: usize, b: &[u8]| d[o..o + b.len()].copy_from_slice(b);
        for (o, v) in [(0, -260.0f32), (4, -220.0), (8, 60.0), (0xc, 160000.0), (0x10, 30.0), (0x14, 30.0), (0x20, 10000.0), (0x24, 260080.0), (0x28, 255.0), (0x2c, 0.0)] {
            put(&mut data, o, &v.to_le_bytes());
        }
        put(&mut data, 0x18, &7u16.to_le_bytes());
        put(&mut data, 0x1a, &7u16.to_le_bytes());
        put(&mut data, 0x30, &[1, 2, 3, 15]);
        put(&mut data, 0x34, &0x1100u32.to_le_bytes());
        put(&mut data, 0x38, &0x1080u32.to_le_bytes());
        put(&mut data, 0x3c, &[43, 0, 16, 0x80]);
        put(&mut data, 0x80, &[1]);
        put(&mut data, 0x100 + 0x10, &0x0015_0707u32.to_le_bytes());
        let mut lump = Vec::new();
        for w in [0x1000u32, data.len() as u32, 1, 0x100] { lump.extend_from_slice(&w.to_le_bytes()); }
        lump.extend_from_slice(&data);
        let ov = Overlay::parse(&lump).unwrap();
        let g = LiquidGrid::parse(&ov, 0x1000).unwrap();
        assert_eq!((g.origin, g.cull_dist2, g.cell, g.cells), ([-260.0, -220.0, 60.0], 160000.0, [30.0, 30.0], [7, 7]));
        assert_eq!((g.fog, g.fog_rgb, g.period, g.tex, g.frames), ([10000.0, 260080.0, 255.0, 0.0], [1, 2, 3], 15, 43, 16));
        assert_eq!((g.blocks(), g.flags.clone()), ([1, 1], vec![1]));
        assert_eq!(g.colours[0][0], 0x0015_0707);
    }
}
