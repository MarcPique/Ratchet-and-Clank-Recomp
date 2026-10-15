//! **The arc slots and their keeper, class 1331** (level14 `0x3039e0`; census U514, one placed on Oltanis and on
//! Quartu). Three shared arcs (level data 0x162148..0x1621d8 and 0x1edd40..0x1ee5e0) that the zapper bots 28 and the
//! turrets 211 throw at their targets: a class registers itself (`0x303b30`), hands its arc's start frame and end every
//! tick (`0x303d98`) and lets go (`0x303c48`); the keeper moves the arcs and draws them.
//!
//! * **Register** (`0x303b30(m, lights, follow)`): already holding a slot, or the first free one: owner m, state 1,
//!   the phases and scroll 0, no lights; the light bits (1: one at the start, 2: one at the end) and whether the
//!   sparks follow the owner (+0xd0 / +0x1f0 of its pvars: type 69 modes 1 / 2) kept. All taken: 0.
//! * **Aim** (`0x303d98(m, frame, end)`): the owner's slot: state 1 → 2, 4 → 3; the start frame (rows and start
//!   point, 0x40) and the end copied.
//! * **Let go** (`0x303c48(m)`): its slot freed (state and owner 0) and its lights.
//! * **The keeper** (`0x3039e0`): at the start all slots cleared, update distance 0xff. Every tick each slot: waiting
//!   (1) with its owner deleted → freed; not aimed since the last tick (4) → freed with its lights (`0x303d00`);
//!   aimed (2 / 3) → moved (`0x303e58`) and marked 4. Any moved: the draw (`0x3046f8`).
//! * **Move** (`0x303e58`): 20 points a twentieth of the start–end distance apart along the arc (both lines reset
//!   the first time); the phases += 10°, −46°, +3°; the core line's first 19 points swing up and down
//!   (`(0.1 + 0.3·i/20)·sin(phase 0 + 36°·i)`), the glow line's (y `0.3·cos(phase 1 + 57°·i)`, z
//!   `0.5·sin(phase 2 + 12°·i)·sin(phase 1 + 57°·i)` + `randf_sym(0, 0.2)`); the arc's frame: along start → end,
//!   level sideways; its tip = the core's 19th point through it. Sparks (type 69): one off the start
//!   (`randf(0.01, 0.05)` at `randf_sym(20°, 40°)` turned by the start frame), one in three a big one 0.3 along the
//!   frame's row 0 (120000, `ticks(4)`, following +0xd0); one off the tip (a wasted `randf`, then a random vector
//!   0.01..0.05), one in three a big one there (following +0x1f0). The lights (radius 5, colour (1, 1, 2)) at the
//!   start 0.25 down and at the tip 0.5 up.
//! * **Draw** (`0x3046f8` / `0x304f90` / `0x304890`): each aimed slot: its scroll −= 0.2 a frame (wrapped by 8 at
//!   −8); at the start and at the tip a white glow (FX 0xb, size `randf(0.3, 0.325)` / `randf(0.2, 0.225)`, jittered
//!   by one `randf_sym(0, 0.05)` each axis) under a red additive one three times its size; then each line as
//!   camera-facing ribbons through the arc's frame (18 segments): FX +0x04 + 0x28 grey and FX +0x00 + 0x28 red
//!   0x7f4040, half widths 0.6 / 1.2 narrowing to 0.05 / 0.1 (`2t − t²`, t = i / 20; a zapper's 0.3 / 0.6), core
//!   alphas 0x80 / 0x40, glow 0x30 / 0x10, the first segment's start clear, all additive.
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x3039e0` / `0x303d00` | the keeper | [`update`] |
//! | `0x303b30` / `0x303d98` / `0x303c48` | register, aim, let go | [`register`], [`aim`], [`release`] |
//! | `0x303e58` | move | [`step`] |
//! | `0x3046f8` / `0x304f90` / `0x304890` | the draw | [`frame`], [`fx_quad_groups`] |
//!
//! Read from the level14 decomp and disassembly. [L] The strips' frame translation (the start point) is a quad copy
//! the decompiler dropped; the draw is built in the frame part from the camera it sees. Native `f32`.

use super::{FxQuad, FxQuads};
use crate::moby_runtime::{MobyId, MobyTable};
use crate::moby_update::classes::draw_callbacks::Callback;
use crate::moby_update::classes::flyer::wrap_frac;
use crate::moby_update::creature::{self as c, fx};
use crate::moby_update::services::{Services, World};
use crate::point_lights::PointLight;

pub const REFERENCE_LEVEL: u32 = 14;
pub const UPDATE_FN: u32 = 0x30_39e0;
pub const DRAW_FN: u32 = 0x30_46f8;
pub const CLASSES: [i16; 1] = [1331];
/// The zapper bots (their narrower ribbons).
const ZAPPER: i16 = 0x1c;

/// The three arc slots (module doc).
#[derive(Clone, Debug, Default)]
pub struct Arcs {
    state: [i32; 3],
    owner: [Option<MobyId>; 3],
    phase: [[f32; 3]; 3],
    light: [[i32; 2]; 3],
    lights: [u32; 3],
    follow: [i32; 3],
    scroll: [f32; 3],
    /// 0x1edd40: the owner's start frame (rows, start point).
    frame: [[[f32; 4]; 4]; 3],
    /// 0x1ede00: the end.
    end: [[f32; 4]; 3],
    /// 0x1ede30: the tip (the core line's 19th point in the world).
    tip: [[f32; 4]; 3],
    /// 0x1ede60: the core (2k) and glow (2k + 1) lines, local.
    lines: [[[f32; 4]; 20]; 6],
    /// This frame's quads.
    quads: Vec<FxQuads>,
}

fn arcs<'a>(w: &'a mut World<'_>) -> &'a mut Arcs { &mut w.svc.units.oltanis_arcs }

/// `0x303b30(m, lights, follow)` (module doc): whether it holds a slot.
pub fn register(w: &mut World, m: MobyId, lights: u32, follow: i32) -> bool {
    let a = arcs(w);
    if a.owner.contains(&Some(m)) { return true; }
    let Some(k) = a.owner.iter().position(|o| o.is_none()) else { return false };
    a.owner[k] = Some(m);
    a.light[k] = [-1, -1];
    a.lights[k] = lights;
    a.follow[k] = follow;
    a.state[k] = 1;
    a.phase[k] = [0.0; 3];
    a.scroll[k] = 0.0;
    true
}

/// `0x303d98(m, frame, end)` (module doc).
pub fn aim(w: &mut World, m: MobyId, frame: [[f32; 4]; 4], end: [f32; 4]) -> bool {
    let a = arcs(w);
    let Some(k) = a.owner.iter().position(|&o| o == Some(m)) else { return false };
    if a.state[k] == 1 { a.state[k] = 2; } else if a.state[k] == 4 { a.state[k] = 3; }
    a.frame[k] = frame;
    a.end[k] = end;
    true
}

fn free(w: &mut World, k: usize) {
    let lights = {
        let a = arcs(w);
        a.state[k] = 0;
        a.owner[k] = None;
        std::mem::replace(&mut a.light[k], [-1, -1])
    };
    for l in lights {
        if let Ok(i) = usize::try_from(l) { w.svc.point_lights.free(i); }
    }
}

/// `0x303c48(m)` (module doc).
pub fn release(w: &mut World, m: MobyId) -> bool {
    let Some(k) = arcs(w).owner.iter().position(|&o| o == Some(m)) else { return false };
    free(w, k);
    true
}

/// Level14 `0x3039e0` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    match w.m(id).state {
        0 => {
            for k in 0..3 {
                let a = arcs(w);
                a.state[k] = 0;
                a.owner[k] = None;
            }
            let m = w.mm(id);
            m.update_dist = 0xff;
            m.state = 1;
        }
        1 => {
            let mut moved = 0;
            for k in 0..3 {
                let (st, owner) = (arcs(w).state[k], arcs(w).owner[k]);
                match st {
                    1 => {
                        if owner.is_some_and(|o| (w.m(o).state as i8) < 0) {
                            let a = arcs(w);
                            a.state[k] = 0;
                            a.owner[k] = None;
                        }
                    }
                    4 => free(w, k),
                    0 => {}
                    _ => {
                        moved += 1;
                        if let Some(o) = owner { step(w, o, k); }
                        arcs(w).state[k] = 4;
                    }
                }
            }
            if moved != 0 {
                if let Some(row) = super::row(REFERENCE_LEVEL, DRAW_FN) {
                    w.svc.draw_callbacks.register(Callback::UnitFrame(row), id);
                    w.svc.draw_callbacks.register(Callback::UnitQuads(row), id);
                }
            }
        }
        _ => {}
    }
}

fn xyz(v: [f32; 4]) -> [f32; 3] { [v[0], v[1], v[2]] }
/// `FastVecCross(out, a, b)` 0x2212d0 = b × a.
fn fast_cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [b[1] * a[2] - b[2] * a[1], b[2] * a[0] - b[0] * a[2], b[0] * a[1] - b[1] * a[0]] }
fn unit(v: [f32; 3], l: f32) -> [f32; 3] {
    let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if n == 0.0 { [0.0; 3] } else { v.map(|x| x * l / n) }
}
/// The arc's frame: along start → end, level sideways (`FastVecCross` with the gravity (0, 0, −1)), origin the start.
fn arc_frame(start: [f32; 4], end: [f32; 4]) -> [[f32; 3]; 4] {
    let d = unit([end[0] - start[0], end[1] - start[1], end[2] - start[2]], 1.0);
    let s = unit(fast_cross(d, [0.0, 0.0, -1.0]), -1.0);
    let u = fast_cross(s, d);
    [d, s, u, xyz(start)]
}
fn through(f: &[[f32; 3]; 4], p: [f32; 4]) -> [f32; 3] { std::array::from_fn(|k| f[0][k] * p[0] + f[1][k] * p[1] + f[2][k] * p[2] + f[3][k]) }

/// `0x303e58(owner, slot)` (module doc).
pub fn step(w: &mut World, owner: MobyId, k: usize) {
    let (start, end) = (arcs(w).frame[k][3], arcs(w).end[k]);
    let seg = c::len3(c::sub(end, start)) / 20.0;
    {
        let a = arcs(w);
        if a.state[k] == 2 {
            a.state[k] = 3;
            for i in 0..20 {
                a.lines[2 * k][i] = [seg * i as f32, 0.0, 0.0, 1.0];
                a.lines[2 * k + 1][i] = [seg * i as f32, 0.0, 0.0, 1.0];
            }
        }
        let ph = &mut a.phase[k];
        ph[0] = c::add_rot(ph[0], f32::from_bits(0x3e32_b8c2));
        ph[1] = c::sub_rot(ph[1], f32::from_bits(0x3f4d_87ac));
        ph[2] = c::add_rot(ph[2], f32::from_bits(0x3d56_7770));
        let twenty = 20.0f32;
        for i in 0..0x13 {
            let fi = i as f32;
            let amp = (fi / twenty) * 0.3 + 0.1;
            let s = wrap_frac(ph[0] + fi * (360.0 / (twenty * 0.5)) * 0.017_453_292).sin();
            a.lines[2 * k][i] = [seg * fi, 0.0, amp * s, 1.0];
        }
    }
    let f = arc_frame(start, end);
    let tip18 = arcs(w).lines[2 * k][18];
    let t = through(&f, tip18);
    let tip = [t[0], t[1], t[2], 1.0];
    arcs(w).tip[k] = tip;
    let ph = arcs(w).phase[k];
    for i in 0..0x13 {
        let fi = i as f32;
        let u = wrap_frac(ph[1] + fi * 0.994_837_64);
        let v = wrap_frac(ph[2] + fi * 0.20944);
        let z = 0.0 + v.sin() * 0.5 * u.sin() + w.rng.randf_sym(0.0, f32::from_bits(0x3e4c_cccd));
        arcs(w).lines[2 * k + 1][i] = [seg * fi, u.cos() * 0.3, z, 1.0];
    }
    // The sparks.
    let m = arcs(w).frame[k];
    let follow = arcs(w).follow[k] == 1;
    let l = w.rng.randf(f32::from_bits(0x3c23_d70a), f32::from_bits(0x3d4c_cccd));
    let ang = w.rng.randf_sym(f32::from_bits(0x3eb2_b8c2), f32::from_bits(0x3f32_b8c2));
    let v = [ang.cos() * l, ang.sin() * l, 0.0];
    let vel: [f32; 4] = std::array::from_fn(|i| if i == 3 { 1.0 } else { m[0][i] * v[0] + m[1][i] * v[1] + m[2][i] * v[2] });
    fx::part69(w, start, vel, 0x7f, Some(owner));
    let big = |w: &mut World, at: [f32; 4], mode: i16| {
        if w.rng.randi(3) != 0 { return; }
        let Some(i) = fx::part69(w, at, [0.0; 4], 0x7f, Some(owner)) else { return };
        let t = w.ticks(4);
        if let Some(r) = fx::rec_mut(w, i) {
            use crate::particles::rec;
            rec::set_ff(r, 0xc, 120_000.0);
            rec::set_i16(r, 10, t as i16);
            rec::set_u32(r, 0x38, 0x7f7f7f);
            rec::set_ff(r, 0x30, 1.0 / t as f32);
            if follow { rec::set_i16(r, 0x36, mode); }
        }
    };
    big(w, c::add(start, c::scale([m[0][0], m[0][1], m[0][2], 0.0], 0.3)), 1);
    w.rng.randf(f32::from_bits(0x3c23_d70a), f32::from_bits(0x3d4c_cccd));
    let v = fx::rand_vec_ab(w, f32::from_bits(0x3c23_d70a), f32::from_bits(0x3d4c_cccd));
    fx::part69(w, tip, v, 0x7f, Some(owner));
    big(w, tip, 2);
    // The lights.
    let bits = arcs(w).lights[k];
    for (b, at) in [(1u32, [start[0], start[1], start[2] - 0.25]), (2, [tip[0], tip[1], tip[2] + 0.5])] {
        if bits & b == 0 { continue; }
        let j = (b - 1) as usize;
        let l = PointLight { color: [1.0, 1.0, 2.0], intensity: 0.0, pos: at, radius: 5.0 };
        let slot = arcs(w).light[k][j];
        if slot == -1 {
            let load = f32::from_bits(w.svc.frame_load[1].0);
            let got = w.svc.point_lights.alloc(l, load).map_or(-1, |i| i as i32);
            arcs(w).light[k][j] = got;
        } else if let Ok(i) = usize::try_from(slot) {
            w.svc.point_lights.set(i, l);
        }
    }
}

const GLOW_CORNERS: [[f32; 2]; 4] = [[-1.0, 1.0], [-1.0, -1.0], [1.0, 1.0], [1.0, -1.0]];
const GLOW_ST: [[f32; 2]; 4] = [[0.0, 0.0], [0.0, 1.0], [1.0, 0.0], [1.0, 1.0]];

/// A camera-facing quad of the corners (0, y, z)·size at `p` (+ `jit`): `d` to the camera, `side = −unit(d × g)`,
/// `up = side × d`.
pub fn billboard(cam: [f32; 3], p: [f32; 3], size: f32, jit: [f32; 3], rgba: u32) -> FxQuad {
    let d = unit([cam[0] - p[0], cam[1] - p[1], cam[2] - p[2]], 1.0);
    let s = unit(fast_cross(d, [0.0, 0.0, -1.0]), -1.0);
    let u = fast_cross(s, d);
    let corners = GLOW_CORNERS.map(|[y, z]| std::array::from_fn(|k| (s[k] * y + u[k] * z) * size + p[k] + jit[k]));
    FxQuad { corners, st: GLOW_ST, rgba: [rgba; 4] }
}

/// `0x304890(texA, texB, line, alphaA, alphaB, slot)`: one line's two ribbons through the arc's frame.
fn ribbons(a: &Arcs, k: usize, line: usize, alphas: (u32, u32), zapper: bool, cam: [f32; 3]) -> [Vec<FxQuad>; 2] {
    let f = arc_frame(a.frame[k][3], a.end[k]);
    let pts: Vec<[f32; 3]> = a.lines[line].iter().map(|&p| through(&f, p)).collect();
    let scroll = a.scroll[k];
    let w0 = if zapper { 0.3f32 } else { 0.6 };
    let side = |p0: [f32; 3], p1: [f32; 3]| -> [f32; 3] {
        let d = unit([p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]], 1.0);
        unit(fast_cross([cam[0] - p0[0], cam[1] - p0[1], cam[2] - p0[2]], d), 1.0)
    };
    let off = |p: [f32; 3], s: [f32; 3], h: f32| -> [f32; 3] { std::array::from_fn(|i| p[i] + s[i] * h) };
    let (ca, cb) = (alphas.0 << 24 | 0x7f_7f7f, alphas.1 << 24 | 0x7f_4040);
    let sta = [[scroll, 0.0], [scroll, 1.0], [scroll + 1.0, 0.0], [scroll + 1.0, 1.0]];
    let stb = [[1.0, 0.0], [1.0, 1.0], [0.0, 0.0], [0.0, 1.0]];
    let (mut qa, mut qb) = (Vec::new(), Vec::new());
    let sp = side(pts[0], pts[1]);
    let (mut wa, mut wb) = (w0, w0 * 2.0);
    let (mut a0, mut a1) = (off(pts[0], sp, wa), off(pts[0], sp, -wa));
    let (mut b0, mut b1) = (off(pts[0], sp, wb), off(pts[0], sp, -wb));
    for i in 1..0x13 {
        let s = side(pts[i], pts[i + 1]);
        let (n0, n1, m0, m1) = (off(pts[i], s, wa), off(pts[i], s, -wa), off(pts[i], s, wb), off(pts[i], s, -wb));
        let (mut cola, mut colb) = ([ca; 4], [cb; 4]);
        if i == 1 {
            cola[0] = 0xff_ffff;
            cola[1] = 0xff_ffff;
            colb[0] = 0x7f_4040;
            colb[1] = 0x7f_4040;
        }
        qa.push(FxQuad { corners: [a0, a1, n0, n1], st: sta, rgba: cola });
        qb.push(FxQuad { corners: [b0, b1, m0, m1], st: stb, rgba: colb });
        (a0, a1, b0, b1) = (n0, n1, m0, m1);
        let t = i as f32 / 20.0;
        let e = 2.0 * t - t * t;
        wa = w0 + (0.05 - w0) * e;
        wb = wa + wa;
    }
    [qa, qb]
}

/// The draw's frame part (module doc): the scrolls, the glows' draws, every quad.
pub fn frame(w: &mut World, id: MobyId) {
    let cam = crate::hero::physics::to_f32x3(w.camera);
    let (tex_a, tex_b) = (c::pi32(w, id, 4) + 0x28, c::pi32(w, id, 0) + 0x28);
    let mut out = Vec::new();
    for k in 0..3 {
        let (owner, st) = (arcs(w).owner[k], arcs(w).state[k]);
        let Some(owner) = owner.filter(|_| 1 < st) else { continue };
        let s = arcs(w).scroll[k] - c::SPEED * 0.2;
        arcs(w).scroll[k] = if s <= -8.0 { s + 8.0 } else { s };
        // 0x304f90: the glows.
        let size = w.rng.randf(f32::from_bits(0x3e99_999a), f32::from_bits(0x3ea6_6666));
        let jit = [0; 3].map(|_| w.rng.randf_sym(0.0, f32::from_bits(0x3d4c_cccd)));
        let (start, tip) = (xyz(arcs(w).frame[k][3]), xyz(arcs(w).tip[k]));
        let mut white = vec![billboard(cam, start, size, jit, 0xffff_ffff)];
        let mut red = vec![billboard(cam, start, size * 3.0, [0.0; 3], 0x607f_4040)];
        let size = w.rng.randf(f32::from_bits(0x3e4c_cccd), f32::from_bits(0x3e66_6666));
        white.push(billboard(cam, tip, size, jit, 0xffff_ffff));
        red.push(billboard(cam, tip, size * 3.0, [0.0; 3], 0x607f_4040));
        out.push(FxQuads { fx: 0xb, additive: false, subtract: false, quads: white });
        out.push(FxQuads { fx: 0xb, additive: true, subtract: false, quads: red });
        let zapper = w.m(owner).o_class == ZAPPER;
        let a = &w.svc.units.oltanis_arcs;
        let [c0, c1] = ribbons(a, k, 2 * k, (0x80, 0x40), zapper, cam);
        let [g0, g1] = ribbons(a, k, 2 * k + 1, (0x30, 0x10), zapper, cam);
        for (fx, quads) in [(tex_a, c0), (tex_b, c1), (tex_a, g0), (tex_b, g1)] {
            out.push(FxQuads { fx: fx.max(0) as usize, additive: true, subtract: false, quads });
        }
    }
    arcs(w).quads = out;
}

/// Level14 `0x3046f8`: the arcs' quads (module doc).
pub fn fx_quad_groups(_table: &MobyTable, svc: &Services, _id: MobyId) -> Vec<FxQuads> { svc.units.oltanis_arcs.quads.clone() }
