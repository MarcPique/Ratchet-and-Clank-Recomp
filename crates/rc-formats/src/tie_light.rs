//! Tie instance lighting: a bit-exact port of the game's `LightTies` (boot 0x237370, level01
//! overlay 0x2ab218) and the VU0 micro routines it drives (program 436083, entries 0x58 / 0x84).
//! Spec and derivation: docs/plan/tie_lighting.md.
//!
//! Ties are instanced, so the game lights the class's 64 **light slots** once per instance, not the
//! vertices: slot `j` has a class normal (`TieClass::normals[j]`, s16 / 32768) and an instance
//! ambient colour (`TieInstance::ambient_rgbas[j]`, RGBA5551). The result is a 64-entry RGBA table
//! per instance (the 0x100 bytes at +0x40 of the instance's 0x1c0-byte run-time record), which
//! `TieProc` uploads to VU1 (qw 0x346 / 0x386) and VU1 program 13507 indexes with each vertex's
//! colour-index byte ([`crate::tie::TieVertex::color`]).
//!
//! Instead of rotating the 64 normals into world space, the pass rotates the (up to three) light
//! directions into class space with the normalised transpose of the instance matrix; the colour
//! arithmetic then matches `LightTfrags` (docs/plan/tfrag_lighting.md §4), with two differences:
//! a third light (the merged point lights) and a clamp to **243** (`I = 0x478000f3`), not 255.
//!
//! All arithmetic runs on PS2 float bit patterns ([`crate::tfrag_light::ps2`]).

use crate::tfrag_light::{ps2, LightBank, PointLight, POINT_LIGHT_SLOTS};
use crate::tie::{TieClass, TieInstance};

type V4 = [u32; 4];

/// `I` of VU0 program 436083 (`minii.xyz` clamp after the colour sum): 65536 + 243 / 128.
pub const COLOR_CLAMP: u32 = 0x4780_00f3;
/// Number of light slots per tie class / instance.
pub const SLOTS: usize = 64;

fn bits(v: [f32; 4]) -> V4 { v.map(f32::to_bits) }
fn vmuls(a: V4, s: u32) -> V4 { a.map(|x| ps2::mul(x, s)) }
fn vadd(a: V4, b: V4) -> V4 { [0, 1, 2, 3].map(|k| ps2::add(a[k], b[k])) }
/// `vmul t, a, a; vadday.x ACC, t, t; vmaddz.x r, one, t`: `(x*x + y*y) + 1.0*(z*z)`.
fn len2(a: V4) -> u32 {
    let (x, y, z) = (ps2::mul(a[0], a[0]), ps2::mul(a[1], a[1]), ps2::mul(a[2], a[2]));
    ps2::add(ps2::add(x, y), ps2::mul(ps2::ONE, z))
}
/// Colour byte -> `65536 + c / 128` (`pext5`, `pextlb/pextlh` to words, `padduw` 0x47800000).
fn color_float(c: [u8; 4]) -> V4 { c.map(|b| 0x4780_0000 + b as u32) }
/// PEXT5 of an RGBA5551 halfword.
fn pext5(c: u16) -> [u8; 4] { crate::tfrag_light::pext5(c) }

/// The instance's world-space bounding-sphere centre as the level loader stores it in the run-time
/// record (+0x00, read by `LightTies` for point lights and by `TieProc` for culling):
/// `fun_001f9cf8` (M3x3 · bsphere.xyz, w = bsphere.w), `fun_001f9a80` (× class scale, all lanes),
/// `fun_001f9a10` (+ translation, xyz). The radius lane is overwritten by the loader with
/// `bsphere.w × max column length`, then scaled; it is returned in `.w`.
pub fn instance_centre(class: &TieClass, inst: &TieInstance) -> [f32; 4] {
    let m = inst.matrix.map(bits);
    let b = bits(class.header.bsphere);
    // vmulax / vmadday / vmaddaz with the three columns, vmaddw with vf0 = (0, 0, 0, 1).
    let mut c = [0u32; 4];
    for k in 0..4 {
        let acc = ps2::add(ps2::add(ps2::mul(m[0][k], b[0]), ps2::mul(m[1][k], b[1])), ps2::mul(m[2][k], b[2]));
        c[k] = ps2::add(acc, ps2::mul(if k == 3 { ps2::ONE } else { 0 }, b[3]));
    }
    // piVar37[3] = bsphere.w * max(|c0|, |c1|, |c2|) (FPU mul.s; the max is a float compare).
    let lens = column_lengths(inst);
    let maxlen = f32::from_bits(lens[0]).max(f32::from_bits(lens[1])).max(f32::from_bits(lens[2]));
    c[3] = ps2::mul(b[3], maxlen.to_bits());
    let c = vmuls(c, class.header.scale.to_bits());
    [ps2::add(c[0], m[3][0]), ps2::add(c[1], m[3][1]), ps2::add(c[2], m[3][2]), c[3]].map(f32::from_bits)
}

/// `FastVecLength` (boot 0x1f9af0) of the instance matrix's three axis columns:
/// `sqrt((x*x + y*y) + 1.0*(z*z))` on VU0.
pub fn column_lengths(inst: &TieInstance) -> [u32; 3] {
    std::array::from_fn(|i| ps2::sqrt(len2(bits(inst.matrix[i]))))
}

/// The unit axis columns the loader leaves in the run-time matrix: column i's w lane is set to
/// `1.0 / length` (EE FPU `div.s`), and `LightTies` scales the column by it (`vmulw.xyz`).
fn unit_columns(inst: &TieInstance) -> [V4; 3] {
    let lens = column_lengths(inst);
    std::array::from_fn(|i| {
        let inv = ps2::div(ps2::ONE, lens[i]);
        let c = bits(inst.matrix[i]);
        [ps2::mul(c[0], inv), ps2::mul(c[1], inv), ps2::mul(c[2], inv), inv]
    })
}

/// Point lights for one instance: the 8-slot bank (EE 0x180740) and the instance's nibble list
/// (run-time record +0x1e; low nibble first, 0xf terminates; 0xffff = none, the load-time value).
pub struct TiePointLights<'a> {
    pub bank: &'a [PointLight; POINT_LIGHT_SLOTS],
    pub list: u16,
}

/// The three lights in VU0 register form: colours (w = 0) and back-face factors, and the
/// class-space "to-light" vectors transposed into vf24..vf26 (lane k = light k).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TieLightRegs {
    /// vf27, vf28, vf29: colour of light A, B and the merged point light (w cleared).
    pub colors: [V4; 3],
    /// vf30.xyz: `color.w` of each light, the back-face factor (`max(d, d * w)`).
    pub back: [u32; 3],
    /// vf24, vf25, vf26: x, y and z components of the three class-space light vectors.
    pub rows: [V4; 3],
}

/// The EE half of `LightTies` for one instance: light-set select and blend, point-light merge,
/// and the rotation of the light directions into class space.
pub fn light_regs(class: &TieClass, inst: &TieInstance, bank: &LightBank, points: Option<&TiePointLights>) -> TieLightRegs {
    // Run-time record +0x1c = (u16) instance directional_lights.
    let sel = inst.directional_lights as u16;
    let set = |i: u16| bank.sets[(i & 0xf) as usize];
    let (mut ca, mut da, mut cb, mut db);
    if sel & 0xff00 == 0 {
        let s = set(sel);
        (ca, da, cb, db) = (bits(s.color_a), bits(s.dir_a), bits(s.color_b), bits(s.dir_b));
    } else {
        // vitof12.x t = ((sel >> 4) & 0xff0); vsubx.w w = 1 - t; set A (low nibble) * w + set B (next nibble) * t.
        let t = ps2::itof12(((sel as u32 >> 4) & 0xff0) as i32);
        let w = ps2::sub(ps2::ONE, t);
        let (a, b) = (set(sel), set(sel >> 4));
        let blend = |x: [f32; 4], y: [f32; 4]| vadd(vmuls(bits(x), w), vmuls(bits(y), t));
        ca = blend(a.color_a, b.color_a);
        cb = blend(a.color_b, b.color_b);
        da = blend(a.dir_a, b.dir_a);
        db = blend(a.dir_b, b.dir_b);
        // vrsqrt Q, vf0w, |d|^2; vmulq.xyz (w kept).
        for d in [&mut da, &mut db] {
            let q = ps2::rsqrt(ps2::ONE, len2(*d));
            *d = [ps2::mul(d[0], q), ps2::mul(d[1], q), ps2::mul(d[2], q), d[3]];
        }
    }
    let (wa, wb) = (ps2::add(0, ca[3]), ps2::add(0, cb[3]));
    ca[3] = 0;
    cb[3] = 0;

    // Point lights: sum of unit vectors (centre - light) and of colours * (1 - dist / r) over the
    // lights whose radius reaches the instance centre; the direction is renormalised only when at
    // least two lights contributed (v1 starts at -2).
    let (mut dp, mut cp) = ([0u32; 4], [0u32; 4]);
    let mut n_in = 0;
    if let Some(p) = points {
        let centre = bits(instance_centre(class, inst));
        let mut list = p.list as u32 | 0xf_0000;
        while list & 0xf != 0xf {
            let pl = &p.bank[(list & 0xf) as usize % POINT_LIGHT_SLOTS];
            list >>= 4;
            let (col, pos) = (bits(pl.color), bits(pl.pos));
            let inv_r = ps2::div(ps2::ONE, pos[3]);
            let d: V4 = [ps2::sub(centre[0], pos[0]), ps2::sub(centre[1], pos[1]), ps2::sub(centre[2], pos[2]), pos[3]];
            let dist2 = len2(d);
            // vsubx.w vf0, vf4, vf4 (r*r - dist^2), cfc2 MAC flag 0x10 (sign of w): out of range.
            if ps2::sub(ps2::mul(pos[3], pos[3]), dist2) & ps2::SIGN != 0 { continue; }
            let dist = ps2::sqrt(dist2);
            let a = ps2::sub(ps2::ONE, ps2::mul(ps2::mul(ps2::ONE, inv_r), dist));
            let inv_d = ps2::div(ps2::ONE, ps2::mul(ps2::ONE, dist));
            for k in 0..3 { dp[k] = ps2::add(ps2::mul(d[k], inv_d), ps2::mul(dp[k], ps2::ONE)); }
            cp = vadd(cp, vmuls(col, a));
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

    // vf4..vf6 = rows of -N (N = unit axis columns), then L = vf4 * d.x + vf5 * d.y + vf6 * d.z,
    // i.e. L = -N^T d: the light vector in class space, pointing towards the light.
    let n = unit_columns(inst);
    let neg = |c: usize, k: usize| ps2::sub(0, n[c][k]);
    let class_space = |d: V4| -> V4 {
        let mut l = [0u32; 4];
        for (c, lc) in l.iter_mut().enumerate().take(3) {
            let acc = ps2::add(ps2::mul(neg(c, 0), d[0]), ps2::mul(neg(c, 1), d[1]));
            *lc = ps2::add(acc, ps2::mul(neg(c, 2), d[2]));
        }
        l
    };
    let (la, lb, lp) = (class_space(da), class_space(db), class_space(dp));
    // Transpose: vf24 = (La.x, Lb.x, Lp.x, 0), vf25 = (.y ...), vf26 = (.z ...).
    let rows = std::array::from_fn(|i| [la[i], lb[i], lp[i], 0]);
    TieLightRegs { colors: [ca, cb, cp], back: [wa, wb, wp], rows }
}

/// VU0 436083 at 0x2c0 / 0x420 for one slot: `itof15` normal, three dot products, back-face
/// `max(d, d * w)`, `ambient + A * dA + B * dB + P * dP`, `minii.xyz` 243; then the EE's
/// `ppach/ppacb` keeps the low byte of each lane.
pub fn light_slot(regs: &TieLightRegs, normal: [i16; 4], ambient: u16) -> [u8; 4] {
    // Exact: |n| < 2^24 and the scale is a power of two.
    let n = [0, 1, 2].map(|k| (normal[k] as f32 / 32768.0).to_bits());
    let [r24, r25, r26] = regs.rows;
    let mut f = [0u32; 3];
    for (k, fk) in f.iter_mut().enumerate() {
        let d = ps2::add(ps2::add(ps2::mul(r24[k], n[0]), ps2::mul(r25[k], n[1])), ps2::mul(r26[k], n[2]));
        *fk = ps2::max(d, ps2::mul(d, regs.back[k]));
    }
    let base = color_float(pext5(ambient));
    let mut out = [0u8; 4];
    for (c, o) in out.iter_mut().enumerate() {
        let mut acc = ps2::mul(base[c], ps2::ONE);
        for (l, fl) in f.iter().enumerate() { acc = ps2::add(acc, ps2::mul(regs.colors[l][c], *fl)); }
        if c < 3 { acc = ps2::min(acc, COLOR_CLAMP); }
        *o = acc as u8;
    }
    out
}

/// Runs `LightTies` for one instance: the 64 lit RGBA colours, indexed by light slot
/// (0x80 = 1.0 under GS MODULATE; RGB <= 243; alpha = the ambient's bit 15 as 0x80 / 0).
pub fn light_tie_instance(class: &TieClass, inst: &TieInstance, bank: &LightBank, points: Option<&TiePointLights>) -> [[u8; 4]; SLOTS] {
    let regs = light_regs(class, inst, bank, points);
    std::array::from_fn(|j| {
        let n = class.normals.get(j).copied().unwrap_or([0; 4]);
        light_slot(&regs, n, inst.ambient_rgbas[j])
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tfrag_light::DirLightSet;
    use bytemuck::Zeroable;

    fn f(x: f32) -> u32 { x.to_bits() }

    fn class_with_normals(normals: &[[i16; 4]]) -> TieClass {
        let mut c = TieClass::default();
        c.header.scale = 1.0;
        c.normals = (0..64).map(|i| normals.get(i).copied().unwrap_or([0; 4])).collect();
        c
    }

    fn instance(m: [[f32; 4]; 4], sel: i32, ambient: u16) -> TieInstance {
        let mut inst: TieInstance = Zeroable::zeroed();
        inst.matrix = m;
        inst.directional_lights = sel;
        inst.ambient_rgbas = [ambient; 64];
        inst
    }

    const ID: [[f32; 4]; 4] = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [5.0, 6.0, 7.0, 0.01]];

    fn sun_bank() -> LightBank {
        let mut bank = LightBank::default();
        // Light A travels straight down with colour (1.0, 0.5, 0.25); B travels along +X, 0.25 grey, back factor -0.5.
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
    fn identity_instance_matches_the_tfrag_rules() {
        // Slot 0 faces up (+Z, 32767/32768), slot 1 faces -X, slot 2 faces +X.
        let class = class_with_normals(&[[0, 0, 32767, 0], [-32767, 0, 0, 0], [32767, 0, 0, 0]]);
        // Ambient (40, 48, 56), alpha bit set.
        let inst = instance(ID, 2, 0x8000 | 5 | 6 << 5 | 7 << 10);
        let c = light_tie_instance(&class, &inst, &sun_bank(), None);
        // Up: dA = 32767/32768 -> floor(128 * 0.99997) = 127, 63, 31 on top of the ambient.
        assert_eq!(c[0], [40 + 127, 48 + 63, 56 + 31, 0x80]);
        // -X faces against B's travel: dB = +0.99997 -> +31 each.
        assert_eq!(c[1], [40 + 31, 48 + 31, 56 + 31, 0x80]);
        // +X is lit from behind: dB = -0.99997, max(d, -0.5 d) = 0.49998 -> +15.
        assert_eq!(c[2], [40 + 15, 48 + 15, 56 + 15, 0x80]);
        // Slots with a zero normal get the ambient only; the alpha bit selects 0x80 / 0.
        assert_eq!(c[3], [40, 48, 56, 0x80]);
        let inst0 = instance(ID, 2, 5 | 6 << 5 | 7 << 10);
        assert_eq!(light_tie_instance(&class, &inst0, &sun_bank(), None)[3], [40, 48, 56, 0]);
    }

    #[test]
    fn clamp_is_243_not_255() {
        let class = class_with_normals(&[[0, 0, 32767, 0]]);
        let inst = instance(ID, 2, 0x8000 | 31 | 31 << 5 | 31 << 10);
        // 248 + 127 and 248 + 63 both clamp to 0xf3; alpha is never clamped.
        assert_eq!(light_tie_instance(&class, &inst, &sun_bank(), None)[0], [243, 243, 243, 0x80]);
    }

    #[test]
    fn light_follows_the_instance_rotation_and_ignores_scale() {
        // Class normal +X; the instance rotates class X onto world +Z (column 0 = (0, 0, 3): scale 3).
        let class = class_with_normals(&[[32767, 0, 0, 0]]);
        let m = [[0.0, 0.0, 3.0, 0.0], [0.0, 3.0, 0.0, 0.0], [-3.0, 0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 0.01]];
        let inst = instance(m, 2, 0x8000);
        let c = light_tie_instance(&class, &inst, &sun_bank(), None);
        // The normal now faces up in the world: the sun adds 127 / 63 / 31, B is perpendicular.
        assert_eq!(c[0], [127, 63, 31, 0x80]);
        // The class-space light vectors: L_A = -N^T (0, 0, -1) = (+1, 0, 0) in class space (lane 0 of vf24),
        // but through the PS2 unit column: 3 * trunc(1/3) = 0x3f7fffff, one ULP below 1.0.
        let regs = light_regs(&class, &inst, &sun_bank(), None);
        assert_eq!(regs.rows[0][0], 0x3f7f_ffff);
        assert_eq!(regs.back, [0, f(-0.5), 0]);
    }

    #[test]
    fn blended_set_and_unused_set() {
        let class = class_with_normals(&[[0, 0, 32767, 0]]);
        // 50 % of set 2 and the empty set 3: colour halves, direction renormalises -> +63 red.
        let inst = instance(ID, 0x80 << 8 | 3 << 4 | 2, 0x8000 | 5 | 6 << 5 | 7 << 10);
        assert_eq!(light_tie_instance(&class, &inst, &sun_bank(), None)[0], [40 + 63, 48 + 31, 56 + 15, 0x80]);
        // Set 15 is zero: ambient only.
        let inst = instance(ID, 15, 0x8000 | 5 | 6 << 5 | 7 << 10);
        assert_eq!(light_tie_instance(&class, &inst, &sun_bank(), None)[0], [40, 48, 56, 0x80]);
    }

    #[test]
    fn point_lights_merge_into_the_third_light() {
        let mut class = class_with_normals(&[[0, 0, 32767, 0]]);
        class.header.bsphere = [0.0, 0.0, 0.0, 1.0];
        // Instance centre = translation (5, 6, 7); a white light 8 units above it with r = 16.
        let inst = instance(ID, 15, 0x8000);
        assert_eq!(instance_centre(&class, &inst), [5.0, 6.0, 7.0, 1.0]);
        let mut pbank = [PointLight::default(); POINT_LIGHT_SLOTS];
        pbank[3] = PointLight { color: [1.0, 1.0, 1.0, 0.0], pos: [5.0, 6.0, 15.0, 16.0] };
        pbank[4] = PointLight { color: [1.0, 0.0, 0.0, 0.0], pos: [5.0, 6.0, 40.0, 16.0] }; // out of range
        let p = TiePointLights { bank: &pbank, list: 0xff43 };
        let regs = light_regs(&class, &inst, &LightBank::default(), Some(&p));
        // Colour (1 - 8/16) = 0.5; direction (centre - light) / 8 = (0, 0, -1) -> class-space L = (0, 0, +1).
        assert_eq!(regs.colors[2], [f(0.5), f(0.5), f(0.5), 0]);
        assert_eq!([regs.rows[0][2], regs.rows[1][2], regs.rows[2][2]], [0, 0, f(1.0)]);
        // Up-facing slot: 0.5 * 0.99997 -> +63.
        assert_eq!(light_tie_instance(&class, &inst, &LightBank::default(), Some(&p))[0], [63, 63, 63, 0x80]);
        // No list: nothing.
        let none = TiePointLights { bank: &pbank, list: 0xffff };
        assert_eq!(light_tie_instance(&class, &inst, &LightBank::default(), Some(&none))[0], [0, 0, 0, 0x80]);
    }

    /// Lights every tie instance of every level (skipped without `extracted/`): every instance
    /// class resolves, and the colours respect the clamp.
    #[test]
    fn lights_every_retail_instance() {
        let root = crate::test_data::root();
        if !root.join("toc.bin").exists() { eprintln!("skipped: no extracted/"); return; }
        let mut total = 0usize;
        for i in 0..19 {
            let dir = root.join(format!("levels/{i:02}"));
            let core_index = std::fs::read(dir.join("core_index.bin")).unwrap();
            let core_data = crate::test_data::core_data(i).unwrap();
            let gameplay = crate::test_data::gameplay(i).unwrap();
            let core = crate::level::parse_level_core(&core_index, core_data.len()).unwrap();
            let classes = crate::tie::parse_level_ties(&core, &core_data).unwrap();
            let insts = crate::tie::parse_tie_instances(&gameplay).unwrap();
            let bank = crate::tfrag_light::parse_light_bank(&gameplay).unwrap();
            let mut sum = [0u64; 3];
            for inst in &insts {
                let c = classes.iter().find(|c| c.o_class == inst.o_class).expect("instance class resolves");
                let cols = light_tie_instance(&c.class, inst, &bank, None);
                for col in cols {
                    assert!(col[..3].iter().all(|&x| x <= 243));
                    for k in 0..3 { sum[k] += col[k] as u64; }
                }
            }
            let n = (insts.len() * SLOTS).max(1) as u64;
            eprintln!("level {i:02}: {} tie instances, mean slot colour {:?}", insts.len(), sum.map(|s| s / n));
            total += insts.len();
        }
        assert_eq!(total, 44_712);
    }
}
