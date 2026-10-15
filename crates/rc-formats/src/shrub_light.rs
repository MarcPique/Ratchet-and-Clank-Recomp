//! Shrub instance lighting: a bit-exact port of the game's `LightShrubs` (level01 `FUN_0029e7e8`,
//! 0x5b4 bytes; boot copy not located) and the VU0 micro routines it drives (program 436083,
//! `vcallms 0` for normals 0-3 into vf09-vf12 and `vcallms 0x160` = entry 0x2c for normals 4-7 into
//! vf13-vf16; the same code as the tie entries 0x58 / 0x84 on other registers). Spec:
//! docs/plan/shrub_lighting.md.
//!
//! Per instance the pass lights the class's 24 normals ([`ShrubClass::normals`], s16 / 32768) once and
//! writes a 24-entry RGBA palette (the instance's 0x60-byte slot at `gp-0x6760`); VU1 program 56467
//! colours each vertex with `palette[vertex.normal]`. Like `LightTies` it rotates the (up to three) light
//! directions into class space with the normalised transpose of the instance matrix; the differences
//! from the tie pass, all read in the disassembly:
//! * the column normalisation is `c * rsqrt(|c|²)` on VU0 (`vrsqrt Q, vf0w, len²; vmulq.xyz`), not the
//!   loader's `1.0 / FastVecLength` (FPU `div.s`) that ties use;
//! * a blended light set scales only the xyz lanes (`vmulw.xyz` / `vmulx.xyz`), so the blended back
//!   factor (colour `.w`) is the plain **sum** of both sets' factors (ties scale xyzw);
//! * the ambient is one colour per instance (`colour`, packed by the loader into the matrix block's
//!   col0.w as `r | g << 8 | b << 16 | 0x80000000`), alpha 0x80;
//! * the point-light origin is the instance's world bounding-sphere centre (run-time record +0x00).
//!
//! All arithmetic runs on PS2 float bit patterns ([`crate::tfrag_light::ps2`]).

use crate::shrub::{ShrubClass, ShrubInstance};
use crate::tfrag_light::{ps2, LightBank, PointLight, POINT_LIGHT_SLOTS};

type V4 = [u32; 4];

/// `I` of VU0 program 436083 (`minii.xyz` after the colour sum): 65536 + 243 / 128.
pub const COLOR_CLAMP: u32 = 0x4780_00f3;
/// Normals per shrub class = palette entries per instance.
pub const PALETTE: usize = 24;

fn bits(v: [f32; 4]) -> V4 { v.map(f32::to_bits) }
fn vmuls(a: V4, s: u32) -> V4 { a.map(|x| ps2::mul(x, s)) }
/// `vmul t, a, a; vadday.x ACC, t, t; vmaddz.x r, vf21 (1.0), t`: `(x*x + y*y) + 1.0*(z*z)`.
fn len2(a: V4) -> u32 {
    let (x, y, z) = (ps2::mul(a[0], a[0]), ps2::mul(a[1], a[1]), ps2::mul(a[2], a[2]));
    ps2::add(ps2::add(x, y), ps2::mul(ps2::ONE, z))
}

/// The ambient word the level loader (`FUN_00255958`, the loop after gameplay pointer 0x3c) stores in
/// the matrix block's col0.w: `b << 16 | g << 8 | 0x80000000 | r` on the raw s32 channels (OR'ed, not
/// masked: a channel above 255 would spill into the next byte; retail channels are 0..255).
pub fn packed_ambient(inst: &ShrubInstance) -> u32 {
    let [r, g, b] = inst.colour;
    (b.wrapping_shl(16) | g.wrapping_shl(8) | r) as u32 | 0x8000_0000
}

/// `lw col0.w; pextlb; pextlh; padduw 0x47800000`: byte i of the packed word -> `65536 + c / 128`.
fn ambient_floats(word: u32) -> V4 { word.to_le_bytes().map(|b| 0x4780_0000 + b as u32) }

/// `FastVecLength` (boot 0x1f9af0) of the instance matrix's three axis columns, as the loader
/// computes them for the bounding radius: `sqrt((x*x + y*y) + 1.0*(z*z))`.
pub fn column_lengths(inst: &ShrubInstance) -> [u32; 3] { std::array::from_fn(|i| ps2::sqrt(len2(bits(inst.matrix[i])))) }

/// The instance's world bounding sphere as the loader stores it in the run-time record (+0x00; used by
/// `LightShrubs` for point lights and by `ShrubProc` for the draw-distance / frustum tests):
/// `fun_001f9cf8` (M3x3 · class bsphere.xyz), radius = bsphere.w × max column length (FPU),
/// `fun_001f9a80` (× class `scale`, all lanes), `fun_001f9a10` (+ translation, xyz). So the class
/// bounding sphere is in units of `scale` world units (1024 × the vertex quantum), as Wrench's builder
/// assumes (docs/formats/shrub_sky_rac1.md §1.5).
pub fn instance_centre(class: &ShrubClass, inst: &ShrubInstance) -> [f32; 4] {
    let m = inst.matrix.map(bits);
    let b = bits(class.header.bsphere);
    let mut c = [0u32; 4];
    for (k, ck) in c.iter_mut().enumerate().take(3) {
        *ck = ps2::add(ps2::add(ps2::mul(m[0][k], b[0]), ps2::mul(m[1][k], b[1])), ps2::mul(m[2][k], b[2]));
    }
    let lens = column_lengths(inst);
    let maxlen = f32::from_bits(lens[0]).max(f32::from_bits(lens[1])).max(f32::from_bits(lens[2]));
    c[3] = ps2::mul(b[3], maxlen.to_bits());
    let c = vmuls(c, class.header.scale.to_bits());
    [ps2::add(c[0], m[3][0]), ps2::add(c[1], m[3][1]), ps2::add(c[2], m[3][2]), c[3]].map(f32::from_bits)
}

/// The unit axis columns: `vmul.xyz; vadday.x; vmaddz.x` (|c|²), `vrsqrt Q, vf0w`, `vmulq.xyz`.
fn unit_columns(inst: &ShrubInstance) -> [V4; 3] {
    std::array::from_fn(|i| {
        let c = bits(inst.matrix[i]);
        let q = ps2::rsqrt(ps2::ONE, len2(c));
        [ps2::mul(c[0], q), ps2::mul(c[1], q), ps2::mul(c[2], q), c[3]]
    })
}

/// Point lights for one instance: the 8-slot bank (EE 0x180740) and the run-time record's nibble list
/// (+0x1e; low nibble first, 0xf terminates; 0xffff = none, the load-time value).
pub struct ShrubPointLights<'a> {
    pub bank: &'a [PointLight; POINT_LIGHT_SLOTS],
    pub list: u16,
}

/// The VU0 registers `LightShrubs` sets up before the micro calls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShrubLightRegs {
    /// vf27, vf28, vf29: colour of light A, B and the merged point light (w cleared).
    pub colors: [V4; 3],
    /// vf30.xyz: the back-face factors (`max(d, d * w)`).
    pub back: [u32; 3],
    /// vf24, vf25, vf26: x, y and z components of the three class-space light vectors (lane k = light k).
    pub rows: [V4; 3],
    /// vf31: the ambient colour as floats (`65536 + c / 128`, alpha lane 0x80).
    pub ambient: V4,
}

/// The EE half of `LightShrubs` for one instance: light-set select and blend, point-light merge,
/// the rotation of the light directions into class space and the ambient.
pub fn light_regs(class: &ShrubClass, inst: &ShrubInstance, bank: &LightBank, points: Option<&ShrubPointLights>) -> ShrubLightRegs {
    // Run-time record +0x1c = (u16) dir_lights.
    let sel = inst.dir_lights as u16;
    let set = |i: u16| bank.sets[(i & 0xf) as usize];
    let (mut ca, mut da, mut cb, mut db);
    if sel & 0xff00 == 0 {
        // lqc2 vf27, vf24, vf28, vf25 = qw 0..3 of the set: no renormalisation.
        let s = set(sel);
        (ca, da, cb, db) = (bits(s.color_a), bits(s.dir_a), bits(s.color_b), bits(s.dir_b));
    } else {
        // vitof12.x t = ((sel >> 4) & 0xff0); vsubx.w w = 1 - t; set A * w and set B * t on xyz only
        // (vmulw.xyz / vmulx.xyz), then vadd.xyzw: the w lanes (back factors) add unscaled.
        let t = ps2::itof12(((sel as u32 >> 4) & 0xff0) as i32);
        let w = ps2::sub(ps2::ONE, t);
        let (a, b) = (set(sel), set(sel >> 4));
        let blend = |x: [f32; 4], y: [f32; 4]| -> V4 {
            let (x, y) = (bits(x), bits(y));
            [ps2::add(ps2::mul(x[0], w), ps2::mul(y[0], t)), ps2::add(ps2::mul(x[1], w), ps2::mul(y[1], t)),
             ps2::add(ps2::mul(x[2], w), ps2::mul(y[2], t)), ps2::add(x[3], y[3])]
        };
        ca = blend(a.color_a, b.color_a);
        cb = blend(a.color_b, b.color_b);
        da = blend(a.dir_a, b.dir_a);
        db = blend(a.dir_b, b.dir_b);
        // vrsqrt Q, vf0w, |d|²; vmulq.xyz.
        for d in [&mut da, &mut db] {
            let q = ps2::rsqrt(ps2::ONE, len2(*d));
            *d = [ps2::mul(d[0], q), ps2::mul(d[1], q), ps2::mul(d[2], q), d[3]];
        }
    }
    // vaddw.x / vaddw.y vf30, vf0, colour: the back factors; vsubw.w clears the colour w lanes.
    let (wa, wb) = (ps2::add(0, ca[3]), ps2::add(0, cb[3]));
    ca[3] = 0;
    cb[3] = 0;

    // Point lights: sum of unit vectors (centre - light) and of colours * (1 - dist / r) over the lights
    // whose radius reaches the instance centre; renormalised only when at least two contributed (v1 = -2).
    let (mut dp, mut cp) = ([0u32; 4], [0u32; 4]);
    if let Some(p) = points {
        let centre = bits(instance_centre(class, inst));
        let mut n_in = 0;
        let mut list = p.list as u32 | 0xf_0000;
        while list & 0xf != 0xf {
            let pl = &p.bank[(list & 0xf) as usize % POINT_LIGHT_SLOTS];
            list >>= 4;
            let (col, pos) = (bits(pl.color), bits(pl.pos));
            // vdiv Q, vf0w, r; vsub.xyz v = centre - pos (w stays r); |v|² with vf21.
            let inv_r = ps2::div(ps2::ONE, pos[3]);
            let v: V4 = [ps2::sub(centre[0], pos[0]), ps2::sub(centre[1], pos[1]), ps2::sub(centre[2], pos[2]), pos[3]];
            let dist2 = len2(v);
            // vsubx.w vf0, vf4, vf4 = r*r - |v|²; cfc2 MAC flag 0x10 (sign of w): out of range.
            if ps2::sub(ps2::mul(pos[3], pos[3]), dist2) & ps2::SIGN != 0 { continue; }
            let dist = ps2::sqrt(dist2);
            // vmulq.w vf11 = (1.0 * 1/r) * dist; vsubw.w vf11 = 1 - that; vdiv Q = 1 / (1.0 * dist).
            let a = ps2::sub(ps2::ONE, ps2::mul(ps2::mul(ps2::ONE, inv_r), dist));
            let inv_d = ps2::div(ps2::ONE, ps2::mul(ps2::ONE, dist));
            // vmulaq.xyz ACC = v * Q; vmaddw.xyz dir = ACC + dir * 1.0; vadd.xyzw colour += col * a.
            for k in 0..3 { dp[k] = ps2::add(ps2::mul(v[k], inv_d), ps2::mul(dp[k], ps2::ONE)); }
            let c = vmuls(col, a);
            cp = [0, 1, 2, 3].map(|k| ps2::add(cp[k], c[k]));
            n_in += 1;
        }
        if n_in >= 2 {
            let q = ps2::rsqrt(ps2::ONE, len2(dp));
            for c in dp.iter_mut().take(3) { *c = ps2::mul(*c, q); }
        }
    }
    let wp = ps2::add(0, cp[3]);
    cp[3] = 0;
    dp[3] = 0;

    // vf4..vf6 = rows of -N (N = unit columns): vf4 = (-c0.x, -c1.x, -c2.x), vf5 = (.y), vf6 = (.z);
    // L = vf4 * d.x + vf5 * d.y + vf6 * d.z = -N^T d (vmulax / vmadday / vmaddz .xyz).
    let n = unit_columns(inst);
    let neg = |c: usize, k: usize| ps2::sub(0, n[c][k]);
    let class_space = |d: V4| -> V4 {
        let mut l = [0u32; 4];
        for (c, lc) in l.iter_mut().enumerate().take(3) {
            *lc = ps2::add(ps2::add(ps2::mul(neg(c, 0), d[0]), ps2::mul(neg(c, 1), d[1])), ps2::mul(neg(c, 2), d[2]));
        }
        l
    };
    let (la, lb, lp) = (class_space(da), class_space(db), class_space(dp));
    // vaddx.x vf24, vf0, vf1 ...: vf24 = (La.x, Lb.x, Lp.x, 0) etc. (each lane through vf0 + x).
    let rows = std::array::from_fn(|i| [ps2::add(0, la[i]), ps2::add(0, lb[i]), ps2::add(0, lp[i]), 0]);
    ShrubLightRegs { colors: [ca, cb, cp], back: [wa, wb, wp], rows, ambient: ambient_floats(packed_ambient(inst)) }
}

/// VU0 436083 for one normal (entry 0 / 0x2c, one of the four lanes): `itof15.xyz`, three dot
/// products, back-face `max(d, d * w)`, `ambient * 1.0 + A * dA + B * dB + P * dP` (xyzw),
/// `minii.xyz` 243; then the EE's `ppach/ppacb` keeps the low byte of each lane.
pub fn light_normal(regs: &ShrubLightRegs, normal: [i16; 4]) -> [u8; 4] {
    // itof15: exact (|n| < 2^24, power-of-two scale).
    let n = [0, 1, 2].map(|k| (normal[k] as f32 / 32768.0).to_bits());
    let [r24, r25, r26] = regs.rows;
    let mut f = [0u32; 3];
    for (k, fk) in f.iter_mut().enumerate() {
        let d = ps2::add(ps2::add(ps2::mul(r24[k], n[0]), ps2::mul(r25[k], n[1])), ps2::mul(r26[k], n[2]));
        *fk = ps2::max(d, ps2::mul(d, regs.back[k]));
    }
    let mut out = [0u8; 4];
    for (c, o) in out.iter_mut().enumerate() {
        let mut acc = ps2::mul(regs.ambient[c], ps2::ONE);
        for (l, fl) in f.iter().enumerate() { acc = ps2::add(acc, ps2::mul(regs.colors[l][c], *fl)); }
        if c < 3 { acc = ps2::min(acc, COLOR_CLAMP); }
        *o = acc as u8;
    }
    out
}

/// Runs `LightShrubs` for one instance with point lights: the 24 palette colours, indexed by normal
/// (0x80 = 1.0 under GS MODULATE; RGB <= 243; alpha 0x80).
pub fn light_shrub_instance_with_points(class: &ShrubClass, inst: &ShrubInstance, bank: &LightBank, points: Option<&ShrubPointLights>) -> [[u8; 4]; PALETTE] {
    let regs = light_regs(class, inst, bank, points);
    std::array::from_fn(|j| light_normal(&regs, class.normals.get(j).copied().unwrap_or([0; 4])))
}

/// Runs `LightShrubs` for one instance as at level load (no point lights: the loader sets every
/// record's list to 0xffff).
pub fn light_shrub_instance(class: &ShrubClass, inst: &ShrubInstance, bank: &LightBank) -> [[u8; 4]; PALETTE] {
    light_shrub_instance_with_points(class, inst, bank, None)
}

/// The loader's column-1 w word: the per-channel integer mean of the 24 palette colours
/// (`sum / 24` each, `b << 16 | g << 8 | r`); `ShrubProc` colours billboards with it.
pub fn average_colour(palette: &[[u8; 4]; PALETTE]) -> [u8; 3] {
    let s = palette.iter().fold([0u32; 3], |a, c| [a[0] + c[0] as u32, a[1] + c[1] as u32, a[2] + c[2] as u32]);
    s.map(|x| (x / PALETTE as u32) as u8)
}

/// VU1 data-memory quadwords (addresses wrap).
pub const VU1_QWC: u16 = 0x400;
/// The two instance buffers of VU1 program 56467 (batch count at the base; `ShrubProc` alternates them
/// per `MSCALF` batch, `0x269 - base`).
pub const VU1_INSTANCE_BUFFERS: [u16; 2] = [0xee, 0x17b];
/// Quadwords per instance slot (4 matrix columns + 24 palette entries).
pub const VU1_SLOT_QWC: u16 = 0x1c;
/// Instances per `MSCALF` batch.
pub const VU1_BATCH: usize = 5;

/// `vi13` while VU1 56467 draws instance `k` of a batch in the buffer at `buffer`: the instance's palette
/// base = `buffer + 1 + 0x1c k` (slot) `+ 4` (matrix) (`iaddiu vi13, vi13, 5` after the matrix loads, then
/// `+ 0x1c` per instance).
pub fn vu1_palette_base(buffer: u16, k: usize) -> u16 { buffer + 5 + VU1_SLOT_QWC * k as u16 }

/// The VU1 address vertex `vertex` of a packet reads its colour from (`lq.xyz vfXX, 0(vi07)`,
/// `vi07 = (n & 0x7fff) + vi13`). Quirk (docs/formats/shrub_sky_rac1.md §1.3b): when the stop flag is on
/// vertex 2 (a packet of 6 written vertices), the loop leaves at instruction 182 (`ibltz vi01, L12`) with
/// `vi07` already = n + vi13 for vertex 3, and L12 adds `vi13` again (instruction 308): vertex 3 reads
/// `n + 2·vi13`. Only the colour (RGB) is affected; alpha stays the instance's.
pub fn vu1_colour_address(written_vertices: usize, vertex: usize, normal: u8, palette_base: u16) -> u16 {
    let extra = if written_vertices == 6 && vertex == 3 { palette_base } else { 0 };
    (normal as u16 + palette_base + extra) % VU1_QWC
}

/// What an address of VU1 data memory holds while the shrub program runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Vu1ShrubData {
    /// Palette entry `entry` of instance slot `slot` of an instance buffer.
    Palette { buffer: u16, slot: usize, entry: u8 },
    /// Matrix column of instance slot `slot` (floats: the colour bytes would be the low bytes of the bits).
    Matrix { buffer: u16, slot: usize, column: u8 },
    /// Anything else: constants, the input buffers, a batch count, or a GS-packet output buffer
    /// (0x208 / 0x2b0 / 0x358, 0xa8 qw each: tags, ad-gifs and the ST / RGBAQ / XYZF2 of earlier kicks).
    Other(u16),
}

/// Decodes a VU1 data address against the 56467 memory map.
pub fn vu1_shrub_data(addr: u16) -> Vu1ShrubData {
    let addr = addr % VU1_QWC;
    for buffer in VU1_INSTANCE_BUFFERS {
        let rel = addr.wrapping_sub(buffer);
        if (1..=VU1_SLOT_QWC * VU1_BATCH as u16).contains(&rel) {
            let (slot, qw) = (((rel - 1) / VU1_SLOT_QWC) as usize, ((rel - 1) % VU1_SLOT_QWC) as u8);
            return if qw < 4 { Vu1ShrubData::Matrix { buffer, slot, column: qw } } else { Vu1ShrubData::Palette { buffer, slot, entry: qw - 4 } };
        }
    }
    Vu1ShrubData::Other(addr)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tfrag_light::DirLightSet;

    #[test]
    fn vu1_quirk_addresses() {
        let base = vu1_palette_base(0xee, 0);
        assert_eq!(base, 0xf3);
        // Normal vertices read their own palette entry.
        assert_eq!(vu1_colour_address(6, 2, 7, base), 0xf3 + 7);
        assert_eq!(vu1_colour_address(8, 3, 7, base), 0xf3 + 7);
        assert_eq!(vu1_shrub_data(0xf3 + 7), Vu1ShrubData::Palette { buffer: 0xee, slot: 0, entry: 7 });
        // Vertex 3 of a 6-vertex packet, slot 0 of buffer 0xee: n + 0x1e6 lands in the other buffer's slot 3
        // (palette entry 18 + n for n < 6), slot 4's matrix (n = 6..9) or slot 4's palette (n - 10).
        assert_eq!(vu1_shrub_data(vu1_colour_address(6, 3, 2, base)), Vu1ShrubData::Palette { buffer: 0x17b, slot: 3, entry: 20 });
        assert_eq!(vu1_shrub_data(vu1_colour_address(6, 3, 7, base)), Vu1ShrubData::Matrix { buffer: 0x17b, slot: 4, column: 1 });
        assert_eq!(vu1_shrub_data(vu1_colour_address(6, 3, 20, base)), Vu1ShrubData::Palette { buffer: 0x17b, slot: 4, entry: 10 });
        // Any later slot, or the 0x17b buffer, lands in a GS-packet output buffer.
        assert_eq!(vu1_shrub_data(vu1_colour_address(6, 3, 0, vu1_palette_base(0xee, 1))), Vu1ShrubData::Other(0x21e));
        assert_eq!(vu1_shrub_data(vu1_colour_address(6, 3, 23, vu1_palette_base(0x17b, 4))), Vu1ShrubData::Other(0x3f7));
    }

    fn f(x: f32) -> u32 { x.to_bits() }

    fn class_with_normals(normals: &[[i16; 4]]) -> ShrubClass {
        let mut c = ShrubClass::default();
        c.header.scale = 1.0;
        c.normals = (0..PALETTE).map(|i| normals.get(i).copied().unwrap_or([0; 4])).collect();
        c
    }

    fn instance(m: [[f32; 4]; 4], sel: i32, colour: [i32; 3]) -> ShrubInstance {
        ShrubInstance { matrix: m, dir_lights: sel, colour, draw_distance: 32.0, ..Default::default() }
    }

    const ID: [[f32; 4]; 4] = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [5.0, 6.0, 7.0, 0.01]];

    fn sun_bank() -> LightBank {
        let mut bank = LightBank::default();
        // Light A travels straight down, colour (1.0, 0.5, 0.25); B travels along +X, 0.25 grey, back factor -0.5.
        bank.sets[2] = DirLightSet {
            color_a: [1.0, 0.5, 0.25, 0.0],
            dir_a: [0.0, 0.0, -1.0, 0.0],
            color_b: [0.25, 0.25, 0.25, -0.5],
            dir_b: [1.0, 0.0, 0.0, 0.0],
        };
        bank.count = 3;
        bank
    }

    #[test]
    fn normals_decode_by_32768() {
        // 16384 / 32768 = 0.5 exactly: light A adds 64 / 32 / 16; 32767 / 32768 floors 127.99 to 127.
        let class = class_with_normals(&[[0, 0, 16384, 0], [0, 0, 32767, 0], [0, 0, -32768, 0]]);
        let inst = instance(ID, 2, [40, 48, 56]);
        let p = light_shrub_instance(&class, &inst, &sun_bank());
        assert_eq!(p[0], [40 + 64, 48 + 32, 56 + 16, 0x80]);
        assert_eq!(p[1], [40 + 127, 48 + 63, 56 + 31, 0x80]);
        // Facing down: dA = -1, light A has back factor 0 -> max(-1, -0) = -0 adds nothing.
        assert_eq!(p[2], [40, 48, 56, 0x80]);
        // A zero normal gets the ambient only.
        assert_eq!(p[3], [40, 48, 56, 0x80]);
    }

    #[test]
    fn one_light_and_back_factor() {
        // -X faces against B's travel: +31 each; +X is lit from behind: max(d, -0.5 d) = 0.49998 -> +15.
        let class = class_with_normals(&[[-32767, 0, 0, 0], [32767, 0, 0, 0]]);
        let p = light_shrub_instance(&class, &instance(ID, 2, [40, 48, 56]), &sun_bank());
        assert_eq!(p[0], [40 + 31, 48 + 31, 56 + 31, 0x80]);
        assert_eq!(p[1], [40 + 15, 48 + 15, 56 + 15, 0x80]);
    }

    #[test]
    fn clamp_is_243_and_alpha_is_0x80() {
        let class = class_with_normals(&[[0, 0, 32767, 0]]);
        let p = light_shrub_instance(&class, &instance(ID, 2, [248, 200, 255]), &sun_bank());
        // 248 + 127 and 200 + 63 clamp to 0xf3; 255 + 31 too.
        assert_eq!(p[0], [243, 243, 243, 0x80]);
        // Retail-range colours never touch the alpha lane.
        assert_eq!(packed_ambient(&instance(ID, 2, [1, 2, 3])), 0x8003_0201);
        // The loader ORs the raw channels: 0x1ff red spills into green's byte.
        assert_eq!(packed_ambient(&instance(ID, 2, [0x1ff, 0, 0])), 0x8000_01ff);
    }

    #[test]
    fn blend_scales_xyz_and_sums_back_factors() {
        let class = class_with_normals(&[[0, 0, 32767, 0], [32767, 0, 0, 0]]);
        // 50 % of set 2 and the empty set 3: colours halve, directions renormalise -> +63 / +31 / +15 up.
        let inst = instance(ID, 0x80 << 8 | 3 << 4 | 2, [40, 48, 56]);
        let p = light_shrub_instance(&class, &inst, &sun_bank());
        assert_eq!(p[0], [40 + 63, 48 + 31, 56 + 15, 0x80]);
        // The back factor of B is -0.5 + 0 (not -0.25 as a xyzw blend would give): +X lit from behind
        // by 0.125 grey: max(-0.99997, 0.49998) * 0.125 * 128 = 7.99 -> +7.
        let regs = light_regs(&class, &inst, &sun_bank(), None);
        assert_eq!(regs.back, [0, f(-0.5), 0]);
        assert_eq!(regs.colors[1], [f(0.125), f(0.125), f(0.125), 0]);
        assert_eq!(p[1], [40 + 7, 48 + 7, 56 + 7, 0x80]);
        // Set 15 is empty: ambient only.
        assert_eq!(light_shrub_instance(&class, &instance(ID, 15, [40, 48, 56]), &sun_bank())[0], [40, 48, 56, 0x80]);
    }

    #[test]
    fn light_follows_rotation_mirror_and_ignores_scale() {
        // Class normal +X; the instance rotates class X onto world +Z with scale 3.
        let class = class_with_normals(&[[32767, 0, 0, 0]]);
        let m = [[0.0, 0.0, 3.0, 0.0], [0.0, 3.0, 0.0, 0.0], [-3.0, 0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 0.01]];
        let p = light_shrub_instance(&class, &instance(m, 2, [0, 0, 0]), &sun_bank());
        assert_eq!(p[0], [127, 63, 31, 0x80]);
        // vrsqrt: 3 * (1 / sqrt(9)) = 3 * 0.33333331 = 0x3f7fffff, one ULP below 1.0 (the class-space light).
        let regs = light_regs(&class, &instance(m, 2, [0, 0, 0]), &sun_bank(), None);
        assert_eq!(regs.rows[0][0], 0x3f7f_ffff);
        // Mirrored instance (column 0 negated): class +X now faces world -Z, away from the sun.
        let mut mm = m;
        mm[0] = [0.0, 0.0, -3.0, 0.0];
        assert_eq!(light_shrub_instance(&class, &instance(mm, 2, [0, 0, 0]), &sun_bank())[0], [0, 0, 0, 0x80]);
    }

    #[test]
    fn point_lights_merge_at_the_bounding_sphere_centre() {
        let mut class = class_with_normals(&[[0, 0, 32767, 0]]);
        // Centre offset (0, 0, 1) in scale units; scale 2 -> world centre = translation + (0, 0, 2).
        class.header.bsphere = [0.0, 0.0, 1.0, 1.5];
        class.header.scale = 2.0;
        let inst = instance(ID, 15, [0, 0, 0]);
        assert_eq!(instance_centre(&class, &inst), [5.0, 6.0, 9.0, 3.0]);
        let mut pbank = [PointLight::default(); POINT_LIGHT_SLOTS];
        pbank[3] = PointLight { color: [1.0, 1.0, 1.0, 0.0], pos: [5.0, 6.0, 17.0, 16.0] };
        pbank[4] = PointLight { color: [1.0, 0.0, 0.0, 0.0], pos: [5.0, 6.0, 40.0, 16.0] }; // out of range
        let p = ShrubPointLights { bank: &pbank, list: 0xff43 };
        let regs = light_regs(&class, &inst, &LightBank::default(), Some(&p));
        // Colour (1 - 8/16) = 0.5; direction (centre - light) / 8 = (0, 0, -1) -> class-space L = (0, 0, +1).
        assert_eq!(regs.colors[2], [f(0.5), f(0.5), f(0.5), 0]);
        assert_eq!([regs.rows[0][2], regs.rows[1][2], regs.rows[2][2]], [0, 0, f(1.0)]);
        assert_eq!(light_shrub_instance_with_points(&class, &inst, &LightBank::default(), Some(&p))[0], [63, 63, 63, 0x80]);
        let none = ShrubPointLights { bank: &pbank, list: 0xffff };
        assert_eq!(light_shrub_instance_with_points(&class, &inst, &LightBank::default(), Some(&none))[0], [0, 0, 0, 0x80]);
    }

    #[test]
    fn average_is_integer_mean() {
        let mut p = [[0u8; 4]; PALETTE];
        p[0] = [24, 48, 47, 0x80];
        assert_eq!(average_colour(&p), [1, 2, 1]);
    }

    /// Lights every shrub instance of every level (skipped without `extracted/`): every class resolves,
    /// the colours respect the clamp, and the level-load inputs stay in the ranges the renderer assumes.
    #[test]
    fn lights_every_retail_instance() {
        let root = crate::test_data::root();
        if !root.join("toc.bin").exists() { eprintln!("skipped: no extracted/"); return; }
        let (mut total, mut quirk_packets) = (0usize, 0usize);
        for i in 0..19 {
            let dir = root.join(format!("levels/{i:02}"));
            let core_index = std::fs::read(dir.join("core_index.bin")).unwrap();
            let core_data = crate::test_data::core_data(i).unwrap();
            let gameplay = crate::test_data::gameplay(i).unwrap();
            let core = crate::level::parse_level_core(&core_index, core_data.len()).unwrap();
            let classes = crate::shrub::parse_level_shrubs(&core, &core_data).unwrap();
            let insts = crate::shrub::parse_shrub_instances(&gameplay).unwrap();
            let bank = crate::tfrag_light::parse_light_bank(&gameplay).unwrap();
            let mut sum = [0u64; 3];
            // Packets whose vertex 3 is drawn and reads its colour through the 6-vertex quirk.
            for c in &classes {
                for (pi, p) in c.class.packets.iter().enumerate() {
                    if p.vertices.len() == 6 && p.draws.iter().any(|d| d.vertices.contains(&3)) {
                        let n = insts.iter().filter(|x| x.o_class == c.o_class).count();
                        eprintln!("level {i:02}: class {} packet {pi}: vertex 3 (normal {}) colour quirk, {n} instances", c.o_class, p.vertices[3].normal);
                        quirk_packets += 1;
                    }
                }
            }
            for inst in &insts {
                assert!(inst.colour.iter().all(|c| (0..=255).contains(c)), "colour channel outside a byte");
                let c = classes.iter().find(|c| c.o_class == inst.o_class).expect("instance class resolves");
                assert_eq!(c.class.normals.len(), PALETTE);
                for col in light_shrub_instance(&c.class, inst, &bank) {
                    assert!(col[..3].iter().all(|&x| x <= 243));
                    assert_eq!(col[3], 0x80);
                    for k in 0..3 { sum[k] += col[k] as u64; }
                }
            }
            let n = (insts.len() * PALETTE).max(1) as u64;
            eprintln!("level {i:02}: {} shrub instances, mean palette colour {:?}", insts.len(), sum.map(|s| s / n));
            total += insts.len();
        }
        assert_eq!(total, 25_572);
        eprintln!("{quirk_packets} packets draw a vertex 3 through the 6-vertex colour quirk");
    }
}
