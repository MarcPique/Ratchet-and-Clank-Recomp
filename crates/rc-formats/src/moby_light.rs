//! Moby instance rotation and vertex lighting, bit-exact on PS2 float arithmetic
//! ([`crate::tfrag_light::ps2`]). Spec: docs/plan/moby_skinning_lighting.md §2, §4, §5 and
//! docs/plan/moby_render_notes.md. Three game routines are ported:
//!
//! 1. [`rotation_rows`]: `fun_0020def8` (boot 0x20def8) runs VU0 program 28259 at 0xd18
//!    (`vcallms 0x1a3`) on moby+0x40 (the instance's Euler angles) and stores the rows vf20..vf22 at
//!    moby+0xc0/0xd0/0xe0. The program starts from the identity and, for each non-zero angle
//!    (MAC zero flags via `fmor`), applies Rx, then Ry, then Rz to the rows, so row i = R·e_i with
//!    R = Rz·Ry·Rx. Sine/cosine come from a 9th-order odd polynomial after folding the angle into
//!    [-pi/2, pi/2] (`L4` at 0xe20).
//! 2. [`moby_lights`]: MobyProc (`fun_00211808`, 0x211d88..0x212140) builds the job's light block: the
//!    light direction(s) of the selected bank set taken into model space (`L = -R^T dir`), colours,
//!    back-light factors K and the ambient from moby+0x3c.
//! 3. [`light_vertex`]: VU0 program 104691 (entry 0, single-matrix loop) per vertex, plus the EE pack
//!    in `fun_001ee650` (multiplier bytes, halfword saturation).
//!
//! The point-light merge (MobyProc 0x212150) is the engine's per-frame job (rc-engine `moby_render.rs`, the
//! mobys' third light); this load-time pass leaves the third light's row and colour 0, which is also what the
//! job holds when none is in range.

use crate::moby::MobySubmesh;
use crate::tfrag_light::{ps2, DirLightSet, LightBank, NormalTable};

/// Four VU lanes as raw f32 bits.
pub type V4 = [u32; 4];

const ZERO: V4 = [0; 4];
const PI_2: u32 = 0x3fc9_0fda; // 1.57079625, I at 28259 0xe20
const PI: u32 = 0x4049_0fda; // 3.1415925, 0xe28
const NEG_PI: u32 = 0xc049_0fda; // 0xe30
/// Sine polynomial coefficients for x^3, x^5, x^7, x^9 (0xed0..0xee8 / 0xef8..0xf10).
const SIN_C: [u32; 4] = [0xbe2a_aaa4, 0x3c08_873e, 0xb94f_b21d, 0x362e_9c14];

fn lanes(f: impl Fn(usize) -> u32) -> V4 { [f(0), f(1), f(2), f(3)] }
/// `mulax ACC, a, v.x; madday ACC, b, v.y; maddz r, c, v.z` = `(a*v.x + b*v.y) + c*v.z` per lane.
fn mat3(a: V4, b: V4, c: V4, v: [u32; 3]) -> V4 {
    lanes(|k| ps2::add(ps2::add(ps2::mul(a[k], v[0]), ps2::mul(b[k], v[1])), ps2::mul(c[k], v[2])))
}
fn is_zero(x: u32) -> bool { x & 0x7f80_0000 == 0 }

/// `L4` of VU0 28259: (sin a, cos a) as the VU computes them (`vf01.x`, `vf02.x`).
pub fn vu0_sin_cos(a: u32) -> (u32, u32) {
    // I-register values reach the upper instruction one pair after their LOI (`:i`) load.
    let b = ps2::add(a, PI_2); // addi.x vf11, vf01, I
    let (pi, npi) = (ps2::add(0, PI), ps2::add(0, NEG_PI)); // addi.x vf05/vf06, vf00, I
    let fold = |x: u32| ps2::max(ps2::min(x, ps2::sub(pi, x)), ps2::sub(npi, x));
    let poly = |x: u32| {
        let x2 = ps2::mul(x, x);
        let x3 = ps2::mul(x, x2);
        let x5 = ps2::mul(x3, x2);
        let x7 = ps2::mul(x5, x2);
        let x9 = ps2::mul(x7, x2);
        let acc = ps2::mul(x, ps2::ONE); // mulaw.x ACC, vf01, vf00 (w = 1)
        let acc = ps2::add(acc, ps2::mul(x3, SIN_C[0]));
        let acc = ps2::add(acc, ps2::mul(x5, SIN_C[1]));
        let acc = ps2::add(acc, ps2::mul(x7, SIN_C[2]));
        ps2::add(acc, ps2::mul(x9, SIN_C[3]))
    };
    (poly(fold(a)), poly(fold(b)))
}

/// Rotation rows (moby+0xc0, 0xd0, 0xe0) for Euler angles `rot` (radians, as f32), exactly as
/// VU0 28259 at 0xd18 builds them. Row i is the image of model axis i: world = sum_i v_i * row_i.
pub fn rotation_rows(rot: [f32; 3]) -> [V4; 3] {
    let one = ps2::ONE;
    let r = rot.map(f32::to_bits);
    // add.xyz vf31, vf01, vf00: the MAC zero flags of this add decide which steps run.
    let a = [ps2::add(r[0], 0), ps2::add(r[1], 0), ps2::add(r[2], 0)];
    let (mut r0, mut r1, mut r2): (V4, V4, V4) = ([one, 0, 0, 0], [0, one, 0, 0], [0, 0, one, 0]);
    // Rows' = Y0 * row.x + Y1 * row.y + Y2 * row.z (`L5`), using the rows as they were before the step.
    let apply = |y0: V4, y1: V4, y2: V4, rows: [V4; 3]| rows.map(|row| mat3(y0, y1, y2, [row[0], row[1], row[2]]));
    if !is_zero(a[0]) {
        let (s, c) = vu0_sin_cos(r[0]); // bal L4 with vf01.x = the loaded angle itself
        r1[1] = ps2::add(0, c);
        r1[2] = ps2::add(0, s);
        r2[2] = ps2::add(0, c);
        r2[1] = ps2::sub(0, s);
    }
    if !is_zero(a[1]) {
        let (s, c) = vu0_sin_cos(ps2::add(0, a[1]));
        let y0 = [ps2::add(0, c), 0, ps2::sub(0, s), 0];
        let y1 = [0, one, 0, 0];
        let y2 = [ps2::add(0, s), 0, ps2::add(0, c), 0];
        [r0, r1, r2] = apply(y0, y1, y2, [r0, r1, r2]);
    }
    if !is_zero(a[2]) {
        let (s, c) = vu0_sin_cos(ps2::add(0, a[2]));
        let z0 = [ps2::add(0, c), ps2::add(0, s), 0, 0];
        let z1 = [ps2::sub(0, s), ps2::add(0, c), 0, 0];
        let z2 = [0, 0, one, 0];
        [r0, r1, r2] = apply(z0, z1, z2, [r0, r1, r2]);
    }
    [r0, r1, r2]
}

/// Rows for an instance: [`rotation_rows`], then mode bit 0x8000 negates row 1's xyz
/// (`vsub.xyz vf21, vf0, vf21` in `fun_0020def8`). Mode 0x100 (keep the stored rows) is a run-time rule (`rc_game` scheduler, `KEEP_ROWS`).
pub fn instance_rows(rot: [f32; 3], mode: u16) -> [V4; 3] {
    let mut rows = rotation_rows(rot);
    if mode & 0x8000 != 0 {
        for x in &mut rows[1][..3] { *x = ps2::sub(0, *x); }
    }
    rows
}

/// Per-moby light constants as `fun_001ee650` loads them into VU0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MobyLights {
    /// vf25..vf27 (job 0x40/0x50/0x60): row j = (L_0[j], L_1[j], L_2[j], 0), L_k the model-space
    /// "to light" vector of light k.
    pub rows: [V4; 3],
    /// vf28..vf30 (job 0x70/0x80/0x90): light colours, w = 0 (1.0 adds 128 to a colour byte).
    pub colors: [V4; 3],
    /// VU0[0xfb]: (-|K_0|, -|K_1|, -|K_2|, 1), K = the colours' original w (back-light factor).
    pub neg_k: V4,
    /// vf31 (job 0xb0): `0x47800000 | byte` per lane = 65536 + {r, g, b, a} / 128.
    pub ambient: V4,
}

/// MobyProc's light block for a moby with rotation rows `rows` (unit, from [`instance_rows`]),
/// light word moby+0x38..0x3b (`[set0, set1, fade, _]`), ambient moby+0x3c..0x3e and alpha
/// (`moby[0x23] * fade >> 7`, 0x80 when fully visible).
pub fn moby_lights(rows: &[V4; 3], bank: &LightBank, light_word: u32, ambient: [u8; 3], alpha: u8) -> MobyLights {
    // vf12..vf14: (-r0.c, -r1.c, -r2.c) for component c = x, y, z (0x211d88..0x211da8).
    let neg = |c: usize| -> V4 { [ps2::sub(0, rows[0][c]), ps2::sub(0, rows[1][c]), ps2::sub(0, rows[2][c]), 0] };
    let (n0, n1, n2) = (neg(0), neg(1), neg(2));
    let to_model = |d: V4| mat3(n0, n1, n2, [d[0], d[1], d[2]]);
    let [i0, i1, fade, _] = light_word.to_le_bytes();
    // The bank index is scaled by 0x40 into the 16-set scratchpad copy; sets past 15 would read the
    // point lights, which this port does not hold (use a zero set, as the game's unused sets are).
    let set = |i: u8| bank.sets.get(i as usize).copied().unwrap_or_default();
    let bits = |v: [f32; 4]| v.map(f32::to_bits);
    let (la, lb, ca, cb);
    if fade == 0 {
        let s: DirLightSet = set(i0);
        la = to_model(bits(s.dir_a));
        lb = to_model(bits(s.dir_b));
        (ca, cb) = (bits(s.color_a), bits(s.color_b));
    } else {
        // 0x212000: t = itof12(fade << 4), w = itof12(0x1000 - (fade << 4)); dirs xyz and colours xyzw
        // blended, then L = (-R^T d) * rsqrt(|d|^2) with |d|^2 = (x^2 + y^2) + 1 * z^2.
        let t = ps2::itof12((fade as i32) << 4);
        let w = ps2::itof12(0x1000 - ((fade as i32) << 4));
        let (s0, s1) = (set(i0), set(i1));
        let blend = |a: [f32; 4], b: [f32; 4], n: usize| -> V4 {
            let (a, b) = (bits(a), bits(b));
            // Lanes past `n` are not written (`vmulz.xyz` / `vadd.xyz`): set 0's value stays.
            lanes(|k| if k < n { ps2::add(ps2::mul(a[k], w), ps2::mul(b[k], t)) } else { a[k] })
        };
        let dir = |d: V4| {
            let sq = lanes(|k| ps2::mul(d[k], d[k]));
            let q = ps2::rsqrt(ps2::ONE, ps2::add(ps2::add(sq[0], sq[1]), ps2::mul(ps2::ONE, sq[2])));
            let l = to_model(d);
            [ps2::mul(l[0], q), ps2::mul(l[1], q), ps2::mul(l[2], q), l[3]]
        };
        la = dir(blend(s0.dir_a, s1.dir_a, 3));
        lb = dir(blend(s0.dir_b, s1.dir_b, 3));
        ca = blend(s0.color_a, s1.color_a, 4);
        cb = blend(s0.color_b, s1.color_b, 4);
    }
    // Job rows: lane x = light 0, lane y = light 1, lane z = light 2 (point merge: 0 here).
    let rows_out = [[la[0], lb[0], 0, 0], [la[1], lb[1], 0, 0], [la[2], lb[2], 0, 0]];
    let k = [ca[3], cb[3], 0, 0];
    // lqc2 vf1 = K; vabs; vsub vf1 = vf0 - |K| (w: 1 - 0).
    let neg_k = lanes(|i| ps2::sub(if i == 3 { ps2::ONE } else { 0 }, k[i] & !ps2::SIGN));
    let clear_w = |c: V4| [c[0], c[1], c[2], 0];
    let [r, g, b] = ambient;
    MobyLights {
        rows: rows_out,
        colors: [clear_w(ca), clear_w(cb), ZERO],
        neg_k,
        ambient: [r, g, b, alpha].map(|x| 0x4780_0000 | x as u32),
    }
}

/// The identity joint matrix (rows 0..2; bind pose / unanimated palette).
pub const IDENTITY: [V4; 3] = [[ps2::ONE, 0, 0, 0], [0, ps2::ONE, 0, 0], [0, 0, ps2::ONE, 0]];

/// Lit colour of one vertex: VU0 104691 single-matrix path with joint matrix rows `m`
/// (the blended palette matrix; [`IDENTITY`] for the bind pose), then the EE pack with the
/// vertex's RGBA multiplier bytes (0x80 = 1.0).
pub fn light_vertex_m(l: &MobyLights, table: &NormalTable, m: &[V4; 3], azimuth: u8, elevation: u8, multiplier: [u8; 4]) -> [u8; 4] {
    let [ca, sa] = table.0[azimuth as usize];
    let [ce, se] = table.0[elevation as usize];
    // vf14 = (cos a, sin a, cos e, sin e); mulz.xy -> (ca*ce, sa*ce); the normal's z is lane w.
    let n = [ps2::mul(ca, ce), ps2::mul(sa, ce), se];
    // vf15 = M0*n.x + M1*n.y + M2*n.w (not normalised).
    let np = mat3(m[0], m[1], m[2], n);
    // vf16 = n'^2; addy.x, addz.x: |n'|^2 = (x^2 + y^2) + z^2; Q = 1 / sqrt.
    let sq = lanes(|k| ps2::mul(np[k], np[k]));
    let q = ps2::rsqrt(ps2::ONE, ps2::add(ps2::add(sq[0], sq[1]), sq[2]));
    // vf19 = d_k = L_k . n' in lane k; vf20 = d * (-|K|); max.
    let d = mat3(l.rows[0], l.rows[1], l.rows[2], [np[0], np[1], np[2]]);
    let f = lanes(|k| ps2::max(d[k], ps2::mul(d[k], l.neg_k[k])));
    // vf20 = C0*f.x + C1*f.y + C2*f.z; mula ACC = vf20 * Q; maddw vf11 = ACC + A * 1.
    let s = mat3(l.colors[0], l.colors[1], l.colors[2], [f[0], f[1], f[2]]);
    let c = lanes(|k| ps2::add(ps2::mul(s[k], q), ps2::mul(l.ambient[k], ps2::ONE)));
    pack(c, multiplier)
}

/// [`light_vertex_m`] in the bind pose.
pub fn light_vertex(l: &MobyLights, table: &NormalTable, azimuth: u8, elevation: u8, multiplier: [u8; 4]) -> [u8; 4] {
    light_vertex_m(l, table, &IDENTITY, azimuth, elevation, multiplier)
}

/// EE pack: `ppach` keeps the low halfword of each lane (the byte count of the 65536 + c/128 float),
/// `pmulth` by the multiplier byte, `pmfhl.sh` saturates to s16, `psrah 7`, `ppacb` keeps the low byte.
fn pack(c: V4, m: [u8; 4]) -> [u8; 4] {
    [0, 1, 2, 3].map(|k| {
        let h = c[k] as u16 as i16 as i32;
        let p = (h * m[k] as i32).clamp(i16::MIN as i32, i16::MAX as i32);
        (p >> 7) as u8
    })
}

/// Lights one LOD list (e.g. `MobyClass::high_lod`) in the bind pose: one RGBA per vertex of each
/// submesh. Duplicate vertices are not re-lit: like the game they take the colour last written to
/// their 9-bit vertex-cache slot, which is carried across the packets of the list.
pub fn light_lod(submeshes: &[MobySubmesh], l: &MobyLights, table: &NormalTable) -> Vec<Vec<[u8; 4]>> {
    let mut cache = [[0u8; 4]; 512];
    submeshes
        .iter()
        .map(|sub| {
            let mult = sub.rgba_multiplier_records();
            let mut out = Vec::with_capacity(sub.vertices.len());
            let n_in = sub.vertices.iter().take_while(|v| !v.duplicate).count();
            for (i, v) in sub.vertices[..n_in].iter().enumerate() {
                out.push(light_vertex(l, table, v.normal_azimuth, v.normal_elevation, mult.get(i).copied().unwrap_or([0x80; 4])));
            }
            for (v, c) in sub.vertices[..n_in].iter().zip(&out) { cache[(v.id & 0x1ff) as usize] = *c; }
            for v in &sub.vertices[n_in..] { out.push(cache[(v.id & 0x1ff) as usize]); }
            out
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(x: f32) -> u32 { x.to_bits() }
    fn v(r: V4) -> [f32; 4] { r.map(f32::from_bits) }
    fn table() -> NormalTable {
        let mut t = [[0u32; 2]; 256];
        for (i, e) in t.iter_mut().enumerate() {
            let a = i as f64 * std::f64::consts::TAU / 256.0;
            *e = [f(a.cos() as f32), f(a.sin() as f32)];
        }
        t[0] = [f(1.0), 0];
        t[64] = [0, f(1.0)];
        t[128] = [f(-1.0), 0];
        t[192] = [0, f(-1.0)];
        NormalTable(t)
    }

    #[test]
    fn sine_polynomial_matches_sin_and_folds() {
        for a in [-3.1f32, -2.0, -0.5, 0.25, 1.0, 1.5, 2.5, 3.1, -5.183544] {
            let (s, c) = vu0_sin_cos(f(a));
            assert!((f32::from_bits(s) - a.sin()).abs() < 2e-4, "sin {a}");
            assert!((f32::from_bits(c) - a.cos()).abs() < 2e-4, "cos {a}");
        }
        // Exact: x = 0.5 evaluated term by term with truncating arithmetic stays below sin(0.5).
        let (s, _) = vu0_sin_cos(f(0.5));
        assert!(f32::from_bits(s) <= 0.5f32.sin() + 1e-7);
    }

    #[test]
    fn rotation_convention_is_rz_ry_rx() {
        // Zero angles: identity, no VU steps.
        assert_eq!(rotation_rows([0.0; 3]), IDENTITY);
        // +90 deg about Z maps model +X to world +Y, model +Y to world -X.
        let r = rotation_rows([0.0, 0.0, std::f32::consts::FRAC_PI_2]);
        let (r0, r1) = (v(r[0]), v(r[1]));
        assert!((r0[0]).abs() < 1e-4 && (r0[1] - 1.0).abs() < 1e-4, "{r0:?}");
        assert!((r1[0] + 1.0).abs() < 1e-4 && r1[1].abs() < 1e-4, "{r1:?}");
        // X then Z: model +Y --Rx(90)--> +Z --Rz(90)--> +Z (unchanged); model +Z --Rx--> -Y --Rz--> +X.
        let r = rotation_rows([std::f32::consts::FRAC_PI_2, 0.0, std::f32::consts::FRAC_PI_2]);
        let (r1, r2) = (v(r[1]), v(r[2]));
        assert!((r1[2] - 1.0).abs() < 1e-4, "{r1:?}");
        assert!((r2[0] - 1.0).abs() < 1e-4, "{r2:?}");
        // Compare against f64 Rz*Ry*Rx for a generic triple.
        let (x, y, z) = (0.3f64, -1.1f64, 2.7f64);
        let rx = [[1.0, 0.0, 0.0], [0.0, x.cos(), -x.sin()], [0.0, x.sin(), x.cos()]];
        let ry = [[y.cos(), 0.0, y.sin()], [0.0, 1.0, 0.0], [-y.sin(), 0.0, y.cos()]];
        let rz = [[z.cos(), -z.sin(), 0.0], [z.sin(), z.cos(), 0.0], [0.0, 0.0, 1.0]];
        let mm = |a: [[f64; 3]; 3], b: [[f64; 3]; 3]| {
            let mut o = [[0.0; 3]; 3];
            for i in 0..3 { for j in 0..3 { o[i][j] = (0..3).map(|k| a[i][k] * b[k][j]).sum(); } }
            o
        };
        let m = mm(rz, mm(ry, rx));
        let r = rotation_rows([x as f32, y as f32, z as f32]);
        for i in 0..3 {
            for j in 0..3 {
                // row i = column i of R.
                assert!((f32::from_bits(r[i][j]) as f64 - m[j][i]).abs() < 3e-4, "R[{j}][{i}]");
            }
        }
        // Mirror bit.
        let m = instance_rows([0.0; 3], 0x8000);
        assert_eq!(v(m[1])[..3], [0.0, -1.0, 0.0]);
    }

    fn bank_with(set: DirLightSet) -> LightBank {
        let mut b = LightBank::default();
        b.sets[1] = set;
        b.count = 2;
        b
    }

    #[test]
    fn lighting_of_up_facing_vertex() {
        let t = table();
        // Light A travels straight down with colour (1.0, 0.5, 0.25), back factor 0.5; light B along +X, colour 0.25.
        let bank = bank_with(DirLightSet {
            color_a: [1.0, 0.5, 0.25, 0.5],
            dir_a: [0.0, 0.0, -1.0, 0.0],
            color_b: [0.25, 0.25, 0.25, 0.0],
            dir_b: [1.0, 0.0, 0.0, 0.0],
        });
        let l = moby_lights(&IDENTITY, &bank, 1, [41, 42, 43], 0x80);
        assert_eq!(v(l.rows[2])[..2], [1.0, 0.0]); // L_A = -dir_a = +Z
        assert_eq!(v(l.rows[0])[..2], [-0.0, -1.0]); // L_B = -X
        assert_eq!(v(l.neg_k), [-0.5, 0.0, 0.0, 1.0]);
        // Up-facing normal (elevation 64): +128/+64/+32 from A, nothing from B.
        assert_eq!(light_vertex(&l, &t, 0, 64, [0x80; 4]), [169, 106, 75, 0x80]);
        // Down-facing: A is behind, d = -1 -> max(-1, 0.5) = 0.5 -> +64/+32/+16.
        assert_eq!(light_vertex(&l, &t, 0, 192, [0x80; 4]), [105, 74, 59, 0x80]);
        // Facing -X: B gives d = 1 -> +32 each; A gives 0.
        assert_eq!(light_vertex(&l, &t, 128, 0, [0x80; 4]), [73, 74, 75, 0x80]);
        // Multiplier 0x40 halves; saturation at 255.
        assert_eq!(light_vertex(&l, &t, 0, 64, [0x40; 4]), [84, 53, 37, 0x40]);
        let hot = moby_lights(&IDENTITY, &bank, 1, [200, 0, 0], 0x80);
        assert_eq!(light_vertex(&hot, &t, 0, 64, [0x80; 4])[0], 255);
    }

    #[test]
    fn rotation_moves_light_into_model_space() {
        let t = table();
        // Sun straight down; the moby is rolled 180 deg about X, so its model +Z points down and a
        // model up-facing normal faces away from the light: only the back term (K = 0.25) remains.
        let bank = bank_with(DirLightSet { color_a: [1.0, 1.0, 1.0, 0.25], dir_a: [0.0, 0.0, -1.0, 0.0], ..Default::default() });
        let rows = rotation_rows([std::f32::consts::PI, 0.0, 0.0]);
        let l = moby_lights(&rows, &bank, 1, [0x40; 3], 0x80);
        let c = light_vertex(&l, &t, 0, 64, [0x80; 4]);
        assert!((95..=96).contains(&c[0]), "{c:?}"); // 64 + 32 * (1 - tiny)
        // Model down-facing normal is world up: full light.
        let c = light_vertex(&l, &t, 0, 192, [0x80; 4]);
        assert!((191..=192).contains(&c[0]), "{c:?}");
        // Cross-fade 50 % with an empty set: colour halves, direction renormalised.
        let l = moby_lights(&IDENTITY, &bank, 0x80_02_01, [0x40; 3], 0x80);
        assert_eq!(light_vertex(&l, &t, 0, 64, [0x80; 4]), [128, 128, 128, 0x80]);
    }

    #[test]
    fn pack_saturates_like_the_ee() {
        assert_eq!(pack([0x4780_0000 + 300, 0x4780_0000 + 255, 0x4780_0000, 0x4780_0080], [0x80; 4]), [255, 255, 0, 0x80]);
        // 0x8000 and above read as a negative halfword: saturates to -32768 >> 7 = -256 -> 0.
        assert_eq!(pack([0x4780_8000, 0, 0, 0], [0x80; 4])[0], 0);
    }
}
