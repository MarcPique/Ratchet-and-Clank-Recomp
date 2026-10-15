//! Blarg flyers, class 660: `BlargFlyerUpdate` level01 0x2f4428 and the shared flyer driver
//! `FlyerPathDriver` 0x2f5168 (path flight on a Kochanek–Bartels / Hermite spline, the exhaust emitters).
//! Spec: `docs/plan/moby_update_catalogue.md` "In the port: Blarg flyers". Native `f32` (standard floats and
//! `std` trig; no PS2 float model), draws on the one shared `rand` stream in the game's order.
//!
//! **Update (0x2f4428)** per tick:
//! 1. `MobyGetHitMessage(m, 0x800000, 0)`, `+0xa4 = 0xff`;
//! 2. state 0: pvar+0x170 = scale, +0x174 = speed +0xfc (the bases);
//! 3. the distance factor `f = clamp(c·d + (1 − c·20), 0.5, 1)`, `c = −(1 − 0.5)/(200 − 20)` (gp−0x510c/−0x5108/
//!    −0x5104 = 0.5 / 200 / 20), `d` = |pos − camera 0x167240|: scale = +0x170·f, speed +0xfc = +0x174·f;
//! 4. a hit: the kill (below); else, unless state 0x65 (dead): the driver, then mode |= 0x1000 (targetable),
//!    pvar+0x2c = 100, +0x2b = 1, and the altitude fade alpha +0x23: z ≥ 175 → 0, 125 < z < 175 →
//!    `trunc((175 − z)·128/50)`, z ≤ 125 → 0x80.
//!
//! **Driver (0x2f5168)**, pvar layout (P = pvar):
//!
//! | P+ | use |
//! |---|---|
//! | 0x60 | path: s32 current point, +0x64 s8 loop flag (≥ 0: wraps over `count`), +0x65 init byte, +0x6c distance, +0x70 spline pointer (port: unused), **+0x74 spline index** (`0x1b0930`), +0x78 yaw to the point |
//! | 0x90 / 0xa0 | tangents m0 / m1 of the current segment (`FUN_0028b8c8`) |
//! | 0xb0 / 0xc0 | segment points p0 / p1 (w = the segment's length) |
//! | 0xd0 / 0xe0 | this tick's / last tick's point on the curve |
//! | 0xf0 | t in the segment; 0xf4 / 0xf8 tension / bias; 0xfc speed (units per tick) |
//! | 0x100 / 0x104 | roll spring gain / velocity; 0x108 / 0x10c pitch spring gain / velocity |
//! | 0x110 | yaw time constant; 0x114 / 0x118 roll from the yaw step |
//! | 0x11c s16 | start delay (state 3); 0x11e s16 loop-sound handle |
//! | 0x120 u8 | flags: bit 0 type-10 sparks at the joints, bits 1–4 the linked emitters, 0x20 debug lines, 0x40 no arc-length correction |
//! | 0x121 u8 | spark countdown (7 ticks); 0x122 s16 yaw offset (degrees) |
//! | 0x124 / 0x128 / 0x12c | speed bob amplitude / rate (degrees per tick) / phase |
//! | 0x130 / 0x134 | lateral / vertical offset from the path (`FUN_002f5040`) |
//! | 0x138.. | sound timers (classes 0x33/0x3c/0x40/0x44/0x4a/0x46a/0x473/0x474 only), 0x13a hit timer (levels 3, 9) |
//! | 0x13c + 4k | moby link of flag bit k (0x140 = bit 1 …; runtime moby index through the spawn remap) |
//!
//! * **State 0** (the load pass): the spline's closing point is dropped when it is within 0.5 of the first
//!   (`count−−`, shared spline), update distance 0xff, sound handle −1, the offsets > 500 become
//!   `randf(−1, 1)·v/1000` (one draw each), `FUN_0028b410` (per point w = chord to the next, z rounded through
//!   +0.5 −0.5), the nearest point `FUN_0028b510` when P+0x60 < 1 (a looping path's last point → 0), the
//!   segment set-up (t = 1, tangent at the point, p0 = p1 = the point), rotation (0, −atan(z, |xy|) and
//!   atan(y, x) of the tangent), position = the point (+ lateral offset), spark countdown 7, the bob phase
//!   (degrees > 2π → radians; −1 with an amplitude → `rand_angle`), the arc lengths `FUN_0028bb90` (per point w),
//!   the emitters placed, then state 3 (delay P+0x11c > 0) or 1.
//! * **State 1** (flight): when t ≥ 1 the next segment (point + 1, m0 = m1, m1 = tangent at the new end, p0 =
//!   p1, p1 = the next point); speed `v = P.fc + (P.124·K·sin(phase))·K`, `dt = v / p0.w`, phase +=
//!   `P.128·K·π/180`; the curve point at t + dt (`FUN_0028ba80`), re-evaluated once with dt scaled by
//!   v / |step| (flag 0x40 clear) so the flyer moves v per tick; rotation: culled by the view (`FUN_00275690`:
//!   `FastBSphereCheck(draw distance, bsphere/1024)`) → (0, 0, heading), else yaw eases to the heading (+ the
//!   offset) by `1/P.110` per tick, pitch and roll through `SpringTurn` 0x26cef0; t += dt; position = last
//!   tick's curve point (+ lateral offset when seen); the joints: each set flag bit k takes the next joint list
//!   j (0, 1, … over the set bits), `FUN_002645a8(m, j)`, and bit k ≥ 1 puts the linked emitter (class 27)
//!   there with P+0x80 = unit(joint − position) and P+0x70 |= 0x1000, bit 0 spawns two type-10 sparks (`particles::type10`) while the
//!   countdown is 0; the countdown runs 7 → 0.
//! * **State 2**: back to 0 once a spline index is set. **State 3**: the delay, then state 1.
//!
//! The kill on a hit 0x800000 is ported: state 0x65, `SpawnBeamExplosion` [`KILL_BEAM`], the links and itself
//! deleted, the first kill's skill point 0x13d408 / level sound 1 / banner 0x53d6 (`story::award_skill_point`) and the
//! wreck 1510 (`FUN_0030be70`, `classes::burning_wreck`).
//!
//! **Not ported (counted in `FxStats::unported`):** the group synchronisation (group ≥ 0); the level-3/9 combat
//! block (hit 0x210000, the 0x13a hide timer); the class-specific sounds; the debug lines. `0x161b00`
//! (gp−0x5100, "flyers paused", 0 on Novalis) is taken as 0.

#![allow(clippy::neg_cmp_op_on_partial_ord)] // the angle wraps test `!(s < π)` as the game's `c.lt.s` does.

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::services::{pvar as p, World};

/// The flyer update address in the level01 class table.
pub const UPDATE_FN: u32 = 0x2f4428;

/// Classes that run [`update`] (the driver's other users are not placed on Novalis).
pub const CLASSES: [i16; 1] = [660];

/// `FUN_002f5168`'s one class exception: 0x336 skips the view test.
const NO_VIEW_TEST: i16 = 0x336;
/// The dead state.
const DEAD: u8 = 0x65;

const PI: f32 = std::f32::consts::PI;
pub(crate) const TWO_PI: f32 = 6.283_185_5;
/// π/180 as the code loads it (0x3c8efa35).
pub(crate) const DEG: f32 = 0.017_453_292;
/// `0x15ed60`: the NTSC speed scale (1.0).
pub(crate) const K: f32 = 1.0;
/// `0x15ed6c` dt and `0x15ed70` dt².
pub(crate) const DT: f32 = 1.0 / 60.0;
pub(crate) const DT2: f32 = 1.0 / 3600.0;
/// gp−0x510c / −0x5108 / −0x5104: the distance factor's minimum, far and near distances.
const SCALE_MIN: f32 = 0.5;
const SCALE_FAR: f32 = 200.0;
const SCALE_NEAR: f32 = 20.0;

// Pvar offsets.
const P_IDX: usize = 0x60;
const P_LOOP: usize = 0x64;
const P_INIT: usize = 0x65;
const P_SPLINE: usize = 0x74;
const P_M0: usize = 0x90;
const P_M1: usize = 0xa0;
const P_P0: usize = 0xb0;
const P_P1: usize = 0xc0;
const P_CUR: usize = 0xd0;
const P_PREV: usize = 0xe0;
const P_T: usize = 0xf0;
const P_SPEED: usize = 0xfc;
const P_FLAGS: usize = 0x120;
const P_SPARK: usize = 0x121;
const P_SCALE0: usize = 0x170;
const P_SPEED0: usize = 0x174;

pub(crate) fn sub3(a: [f32; 4], b: [f32; 4]) -> [f32; 3] { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }
pub(crate) fn len3(v: [f32; 3]) -> f32 { (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt() }
pub(crate) fn dist(a: [f32; 4], b: [f32; 4]) -> f32 { len3(sub3(a, b)) }
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]] }

/// `fast_add_rotations` 0x221ff8: `a + b` wrapped once into [−π, π) (the lower test on the unwrapped sum).
pub(crate) fn add_rot(a: f32, b: f32) -> f32 { wrap_once(a + b) }
/// `fast_subtract_rotations` 0x222040: `a − b`, wrapped the same way.
pub(crate) fn sub_rot(a: f32, b: f32) -> f32 { wrap_once(a - b) }
fn wrap_once(s: f32) -> f32 {
    let below = s < -PI;
    if !(s < PI) { return (s - PI) - PI; }
    if below { return (s + PI) + PI; }
    s
}
/// `FUN_002731d0`: `x` into [−π, π) through the fraction of `(x + π)/2π` (`modf`, truncating).
pub(crate) fn wrap_frac(x: f32) -> f32 {
    let t = (x + PI) / TWO_PI;
    (t - t.trunc()) * TWO_PI - PI
}
/// `FastArcTan(a, b)` 0x2217c0 = atan2(b, a).
pub(crate) fn atan(a: f32, b: f32) -> f32 { if a == 0.0 && b == 0.0 { 0.0 } else { b.atan2(a) } }

/// `SpringTurn(cur, target, acc, damp, max, &vel)` 0x26cef0: `d = wrap(target − cur)`, `x = clamp(d/(π/20), −1, 1)`,
/// `vel += acc·x − damp·vel`, `|vel| ≤ max` (when max ≠ 0) and `≤ |d|`; returns `wrap(cur + vel)`.
pub(crate) fn spring_turn(cur: f32, target: f32, acc: f32, damp: f32, max: f32, vel: &mut f32) -> f32 {
    let d = sub_rot(target, cur);
    let x = (d / f32::from_bits(0x3e20_d97c)).clamp(-1.0, 1.0);
    *vel += acc * x - damp * *vel;
    if max != 0.0 {
        if max < *vel { *vel = max } else if *vel < -max { *vel = -max }
    }
    let a = d.abs();
    if a < *vel { *vel = a } else if *vel < -a { *vel = -a }
    add_rot(cur, *vel)
}

// ---------------------------------------------------------------------------------------------------
// The path (pvar+0x60) on `Services::splines`

/// The flyer's spline index and loop flag, with the spline's point count (`*0x1b0930[i]`).
#[derive(Clone, Copy)]
pub(crate) struct Path {
    pub(crate) spline: usize,
    pub(crate) looped: bool,
    pub(crate) count: i32,
}

impl Path {
    pub(crate) fn of(w: &World, id: MobyId) -> Option<Path> {
        let pv = &w.m(id).pvars;
        let s = usize::try_from(p::i32(pv, P_SPLINE)).ok()?;
        let count = w.svc.splines.get(s)?.len() as i32;
        Some(Path { spline: s, looped: (pv[P_LOOP] as i8) >= 0, count })
    }
    fn on(pts: &[[u32; 4]], looped: bool) -> Path { Path { spline: 0, looped, count: pts.len() as i32 } }
    pub(crate) fn pts<'a>(&self, w: &'a World) -> &'a [[u32; 4]] { &w.svc.splines[self.spline] }
    /// The last index the path addresses: `count − 1` when it loops, else 0.
    fn max(&self) -> i32 { if self.looped { self.count - 1 } else { 0 } }
    /// `FUN_0028b7b0(path, i)`: the point `i` wrapped (i < 0 → max + 1 + i; i > max → i mod (max + 1)).
    fn ref_index(&self, i: i32) -> usize {
        let m = self.max();
        let k = if i < 0 { m + 1 + i } else if i <= m { i } else { i.rem_euclid(m + 1) };
        k.max(0) as usize
    }
    /// `FUN_0028b828(path, i)`: the index `i` wrapped (i < 0 → max + i, the game's one-off).
    pub(crate) fn wrap_index(&self, i: i32) -> i32 {
        let m = self.max();
        if i < 0 { m + i } else if i <= m { i } else { i.rem_euclid(m + 1) }
    }
    /// `FUN_0028b878(path)`: the point after `idx`, wrapped.
    pub(crate) fn next_index(&self, idx: i32) -> i32 {
        let (m, i) = (self.max(), idx + 1);
        if m < i { if m + 1 == 0 { 0 } else { i.rem_euclid(m + 1) } } else { i }
    }
    pub(crate) fn pt(&self, pts: &[[u32; 4]], i: usize) -> [f32; 4] { pts.get(i).map(|q| q.map(f32::from_bits)).unwrap_or([0.0; 4]) }
    fn set_w(&self, w: &mut World, i: usize, x: f32) {
        if let Some(q) = w.svc.splines[self.spline].get_mut(i) { q[3] = x.to_bits(); }
    }
    /// `FUN_0028b8c8(T, B, out, path, k)`: the Kochanek–Bartels tangent at point `idx + k`: `a = (p − p₋)·(1−T)(1+B)/2`,
    /// `b = (p₊ − p)·(1−T)(1−B)/2`, `a + b` limited to 1.1 × the shorter neighbouring chord. (w: 0.)
    pub(crate) fn tangent(&self, pts: &[[u32; 4]], idx: i32, k: i32, tension: f32, bias: f32) -> [f32; 4] {
        let i = idx + k;
        let (pm, p0, pp) = (self.pt(pts, self.ref_index(i - 1)), self.pt(pts, self.ref_index(i)), self.pt(pts, self.ref_index(i + 1)));
        let s_b = ((1.0 - tension) * (1.0 - bias)) * 0.5;
        let s_a = ((1.0 - tension) * (bias + 1.0)) * 0.5;
        let (d1, d2) = (dist(pm, p0), dist(p0, pp));
        let lim = if d1 < d2 { d1 } else { d2 } * 1.1;
        let (a, b) = (sub3(p0, pm), sub3(pp, p0));
        let v = [a[0] * s_a + b[0] * s_b, a[1] * s_a + b[1] * s_b, a[2] * s_a + b[2] * s_b];
        let l = len3(v);
        let v = if lim < l { v.map(|x| x * (lim / l)) } else { v };
        [v[0], v[1], v[2], 0.0]
    }
}

/// The flight segment that starts at point `idx` of a processed spline (the state-1 pvars after the segment
/// switch): `[p0 (+0xb0), p1 (+0xc0), m0 (+0x90), m1 (+0xa0)]`. For the comparison tools.
pub fn segment(pts: &[[u32; 4]], looped: bool, idx: i32, tension: f32, bias: f32) -> [[f32; 4]; 4] {
    let path = Path::on(pts, looped);
    [
        path.pt(pts, path.ref_index(idx)),
        path.pt(pts, path.wrap_index(idx + 1).max(0) as usize),
        path.tangent(pts, idx, 0, tension, bias),
        path.tangent(pts, idx, 1, tension, bias),
    ]
}

/// The curve point at `t` of a segment ([`segment`] order), `FUN_0028ba80`.
pub fn curve_point(t: f32, seg: &[[f32; 4]; 4]) -> [f32; 4] { hermite(t, seg[0], seg[1], seg[2], seg[3]) }

/// `FUN_0028ba80(t, out, p0, p1, m0, m1)`: the cubic Hermite point (`h00·p0 + h10·m0 + h11·m1 + h01·p1`; w = p0.w).
pub(crate) fn hermite(t: f32, p0: [f32; 4], p1: [f32; 4], m0: [f32; 4], m1: [f32; 4]) -> [f32; 4] {
    let t2 = t * t;
    let t3 = t2 * t;
    let (h00, h10, h11, h01) = ((t3 + t3 - t2 * 3.0) + 1.0, (t3 - (t2 + t2)) + t, t3 - t2, t2 * 3.0 - (t3 + t3));
    let c = |k: usize| p0[k] * h00 + m0[k] * h10 + m1[k] * h11 + p1[k] * h01;
    [c(0), c(1), c(2), p0[3]]
}

/// `FUN_0028b410(path)` (the spline part): per point `w = |p_i − p_(i+1 mod count)|` and `z = (z + 0.5) − 0.5`,
/// in point order (a point's chord is measured before its own z is rewritten; the last one's uses the rewritten
/// first point).
pub(crate) fn chord_lengths(w: &mut World, path: &Path) {
    for i in 0..path.count.max(0) as usize {
        let (a, b) = (path.pt(path.pts(w), i), path.pt(path.pts(w), (i + 1) % path.count as usize));
        let d = dist(a, b);
        let q = &mut w.svc.splines[path.spline][i];
        q[3] = d.to_bits();
        q[2] = ((f32::from_bits(q[2]) + 0.5) - 0.5).to_bits();
    }
}

/// `FUN_0028bb90(T, B, path, …)` (without the debug lines): per segment i (point i → i + 1, wrapped), the length
/// of the Hermite curve as 20 chords at t = 0.05·k into point i's w.
///
/// The game accumulates `t += 0.05` and loops while `t ≤ 1.0`: on the PS2's truncating adds the 20th sample
/// lands at t = 0.99999946 and is taken; IEEE rounding would reach 1.0000001 after 19 and drop the last chord
/// (5 % of every segment). The port samples t = 0.05·k, k = 1..=20, the PS2's result
/// (`hardware_fidelity_layers.md` "Native reproductions").
pub(crate) fn arc_lengths(w: &mut World, path: &Path, tension: f32, bias: f32) {
    let mut m1 = path.tangent(path.pts(w), 0, 0, tension, bias);
    let mut p1 = path.pt(path.pts(w), 0);
    for i in 0..path.count.max(0) {
        let (m0, p0) = (m1, p1);
        p1 = path.pt(path.pts(w), path.wrap_index(i + 1).max(0) as usize);
        m1 = path.tangent(path.pts(w), i, 1, tension, bias);
        let mut prev = p0;
        let mut len = 0.0f32;
        for k in 1..=20 {
            let q = hermite(0.05 * k as f32, p0, p1, m0, m1);
            len += dist(prev, q);
            prev = q;
        }
        path.set_w(w, i as usize, len);
    }
}

/// `FUN_0028b510(0, pos, spline)`: the first point with the smallest distance to `pos` (the same function as the
/// other overlays' `0x264558` / `0x2a0260`: `crate::path::nearest_at_distance`).
pub(crate) fn nearest(w: &World, path: &Path, pos: [f32; 4]) -> i32 {
    crate::path::nearest_at_distance(&path.pts(w)[..path.count.max(0) as usize], 0.0, pos)
}

/// `FUN_002f5040(m, v)`: the lateral / vertical offset: with `f` = row 0 of the moby's Euler rows,
/// `e = (f − ẑ) × f`, `d = e × f`, `v += e·P.130 + d·P.134`.
fn lateral(w: &mut World, id: MobyId) { lateral_at(w, id, 0x130) }

/// [`lateral`] with the two offsets at pvar `o`, `o + 4` (level03's copy `0x29c8a8` reads +0x140 / +0x144).
pub(crate) fn lateral_at(w: &mut World, id: MobyId, o: usize) {
    let m = w.m(id);
    let (a, b) = (p::ff(&m.pvars, o), p::ff(&m.pvars, o + 4));
    if a == 0.0 && b == 0.0 { return; }
    let r0 = rc_formats::moby_light::rotation_rows([m.rotation[0], m.rotation[1], m.rotation[2]])[0].map(f32::from_bits);
    let f = [r0[0], r0[1], r0[2]];
    let u = [f[0], f[1], f[2] - 1.0];
    let e = cross(u, f);
    let d = cross(e, f);
    let pos = &mut w.mm(id).position;
    for k in 0..3 { pos[k] += e[k] * a; }
    for k in 0..3 { pos[k] += d[k] * b; }
}

/// `FUN_00275690(dd, m)`: `FastBSphereCheck((f32)draw distance, bsphere · 1/1024) == −1`. No view (the load
/// pass: the game's view is all zero before the first render) culls.
pub(crate) fn culled(w: &World, id: MobyId) -> bool {
    let m = w.m(id);
    match w.view {
        None => true,
        Some(v) => v.culled(m.draw_dist as f32, m.bsphere.map(|x| x * (1.0 / 1024.0))),
    }
}

/// The linked emitter placement of flag bit `bit` (≥ 1): link `P+0x13c + 4·bit`; the emitter moby (with a pvar
/// block) gets P+0x70 |= 0x1000, position = the joint, P+0x80..0x8b = `dir` (the game stores the whole quadword;
/// its w is a stack lane the type-6 velocity never reads, so P+0x8c is left as it is).
fn place_emitter(w: &mut World, id: MobyId, bit: usize, joint: [f32; 4], dir: [f32; 3]) {
    let link = p::i32(&w.m(id).pvars, 0x13c + 4 * bit);
    let Ok(e) = usize::try_from(link) else { return };
    let Some(em) = w.table.mobys.get_mut(e) else { return };
    if em.pvars.len() < 0x90 { return; }
    let f = p::u32(&em.pvars, 0x70) | 0x1000;
    p::set_u32(&mut em.pvars, 0x70, f);
    em.position = joint;
    for (k, x) in dir.iter().enumerate() { p::set_ff(&mut em.pvars, 0x80 + 4 * k, *x); }
}

/// `FastVecNormalize(1.0, v, v)` 0x221410 (a zero vector stays zero).
fn unit(v: [f32; 3]) -> [f32; 3] {
    let l = len3(v);
    if l == 0.0 { return v; }
    [v[0] / l, v[1] / l, v[2] / l]
}

/// `BlargFlyerUpdate` (0x2f4428).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x178 { return; }
    let hit = w.get_hit(id, 0x80_0000, false);
    w.mm(id).hit_slot = 0xff;
    if w.m(id).state == 0 {
        let m = w.mm(id);
        let (s, v) = (m.scale, p::ff(&m.pvars, P_SPEED));
        p::set_ff(&mut m.pvars, P_SCALE0, s);
        p::set_ff(&mut m.pvars, P_SPEED0, v);
    }
    let cam = [w.camera[0].to_f32(), w.camera[1].to_f32(), w.camera[2].to_f32(), 0.0];
    let d = dist(w.m(id).position, cam);
    let c = -((1.0 - SCALE_MIN) / (SCALE_FAR - SCALE_NEAR));
    let f = (c * d + (1.0 - c * SCALE_NEAR)).clamp(SCALE_MIN, 1.0);
    {
        let m = w.mm(id);
        m.scale = p::ff(&m.pvars, P_SCALE0) * f;
        let v = p::ff(&m.pvars, P_SPEED0) * f;
        p::set_ff(&mut m.pvars, P_SPEED, v);
    }
    if hit.is_some() {
        if w.m(id).state != DEAD { kill(w, id, f); }
        return;
    }
    if w.m(id).state == DEAD { return; }
    driver(w, id);
    let m = w.mm(id);
    m.mode |= mode::TARGETABLE;
    m.pvars[0x2c] = 100;
    m.pvars[0x2b] = 1;
    let z = m.position[2];
    if 175.0 <= z {
        m.alpha = 0;
    } else if 125.0 < z {
        m.alpha = (((175.0 - z) * 128.0) / 50.0) as i32 as u8;
    } else {
        m.alpha = 0x80;
    }
}

/// `SpawnBeamExplosion(0, 0, 4, 2, 100000, 3, 15, ship, P+0xd0 − P+0xe0, pos, 20, 3, 4, 1, 1, 1, −1)`: the Blarg ships'
/// kill blast, the same call in the flyer 660 (`0x2f45d4`) and the gunship 688 (`crate::moby_update::classes::gunship`).
pub const KILL_BEAM: crate::moby_update::creature::fx::Beam = crate::moby_update::creature::fx::Beam {
    damage_r: 0.0, damage: 0.0, flash: 4.0, flash2: 2.0, flash_dist: 100000.0, scale: 3.0, light: 15.0, streaks: 20, sparks: 3, puffs: 4, debris: 1, sound: 1, shake: true,
};

/// The hit branch: state 0x65, the blast ([`KILL_BEAM`]), the linked mobys (P+0x140..0x14c) and the flyer deleted.
/// The first kill's skill point 0x13d408 with the level sound 1 and banner 0x53d6 (`story::award_skill_point`), then
/// the wreck 1510 (`0x30be70(f, m, P+0xd0 − P+0xe0)`: `classes::burning_wreck`).
fn kill(w: &mut World, id: MobyId, f: f32) {
    crate::moby_update::story::award_skill_point(w, 0);
    w.mm(id).state = DEAD;
    let pos = crate::moby_update::creature::pos(w, id);
    crate::moby_update::creature::fx::beam_explosion(w, &KILL_BEAM, Some(id), pos);
    let dir = crate::moby_update::creature::sub(p::v4f(&w.m(id).pvars, 0xd0), p::v4f(&w.m(id).pvars, 0xe0));
    super::burning_wreck::spawn(w, f, id, dir);
    for k in 0..4 {
        let link = p::i32(&w.m(id).pvars, 0x140 + 4 * k);
        if let Ok(e) = usize::try_from(link) {
            if e < w.table.mobys.len() { w.delete_moby(e); }
        }
    }
    w.delete_moby(id);
}

/// `FlyerPathDriver` (0x2f5168); also the dropship 666 and the gunship 688 fly with it.
pub fn driver(w: &mut World, id: MobyId) {
    let culled = w.m(id).o_class != NO_VIEW_TEST && culled(w, id);
    let st = w.m(id).state;
    if w.m(id).group >= 0 && st != 0 && st != 3 {
        w.svc.unported("flyer: group synchronisation");
    }
    match st {
        0 => init(w, id),
        1 => fly(w, id, culled),
        2 => {
            if p::i32(&w.m(id).pvars, P_SPLINE) == -1 { return; }
            let m = w.mm(id);
            m.state = 0;
            p::set_i16(&mut m.pvars, 0x11e, -1);
        }
        3 => {
            let mut t = p::i16(&w.m(id).pvars, 0x11c);
            if crate::moby_update::services::fast_dec_timer_s16(&mut t) == 0 {
                p::set_i16(&mut w.mm(id).pvars, 0x11c, t);
                return;
            }
            let m = w.mm(id);
            p::set_i16(&mut m.pvars, 0x11c, 0);
            m.state = 1;
        }
        _ => {}
    }
}

/// State 0 (module doc).
fn init(w: &mut World, id: MobyId) {
    let spline = p::i32(&w.m(id).pvars, P_SPLINE);
    if let Some(s) = usize::try_from(spline).ok().filter(|&s| s < w.svc.splines.len()) {
        let pts = &w.svc.splines[s];
        if let (Some(a), Some(b)) = (pts.first(), pts.last()) {
            if dist(a.map(f32::from_bits), b.map(f32::from_bits)) < 0.5 { w.svc.splines[s].pop(); }
        }
    }
    {
        let m = w.mm(id);
        m.update_dist = 0xff;
        p::set_i16(&mut m.pvars, 0x11e, -1);
    }
    for o in [0x130, 0x134] {
        let v = p::ff(&w.m(id).pvars, o);
        if 500.0 < v {
            let r = w.rng.randf(-1.0, 1.0);
            p::set_ff(&mut w.mm(id).pvars, o, (r * v) / 1000.0);
        }
    }
    if spline == -1 {
        w.mm(id).state = 2;
        return;
    }
    let Some(path) = Path::of(w, id) else {
        // A spline index past the level's table: the game would read garbage; stay parked.
        w.svc.unported("flyer: spline index out of range");
        w.mm(id).state = 2;
        return;
    };
    // FUN_0028b410: init byte, +0x68, the chords.
    {
        let m = w.mm(id);
        m.pvars[P_INIT] = 0;
        p::set_i32(&mut m.pvars, 0x68, 0);
    }
    chord_lengths(w, &path);
    let pos = w.m(id).position;
    if p::i32(&w.m(id).pvars, P_IDX) < 1 {
        let mut k = nearest(w, &path, pos);
        if path.looped && k == path.count - 1 { k = 0; }
        p::set_i32(&mut w.mm(id).pvars, P_IDX, k);
    }
    let idx = p::i32(&w.m(id).pvars, P_IDX);
    let q = path.pt(path.pts(w), idx.max(0) as usize);
    {
        let m = w.mm(id);
        p::set_ff(&mut m.pvars, 0x6c, dist(pos, q));
        p::set_ff(&mut m.pvars, 0x78, atan(q[0] - pos[0], q[1] - pos[1]));
        m.pvars[P_INIT] = 2;
        p::set_i32(&mut m.pvars, 0x68, 0);
        p::set_i32(&mut m.pvars, 0x7c, 0);
        p::set_v4f(&mut m.pvars, P_PREV, q);
        p::set_ff(&mut m.pvars, P_T, 1.0);
    }
    let (tension, bias) = (p::ff(&w.m(id).pvars, 0xf4), p::ff(&w.m(id).pvars, 0xf8));
    let m1 = path.tangent(path.pts(w), idx, 0, tension, bias);
    {
        let m = w.mm(id);
        p::set_v4f(&mut m.pvars, P_M1, m1);
        p::set_v4f(&mut m.pvars, P_P0, q);
        p::set_v4f(&mut m.pvars, P_P1, q);
        m.rotation[0] = 0.0;
        m.rotation[1] = -atan((m1[0] * m1[0] + m1[1] * m1[1]).sqrt(), m1[2]);
        m.rotation[2] = atan(m1[0], m1[1]);
        p::set_ff(&mut m.pvars, 0x104, 0.0);
        p::set_ff(&mut m.pvars, 0x10c, 0.0);
        m.position = q;
    }
    lateral(w, id);
    w.mm(id).pvars[P_SPARK] = 7;
    let (amp, ph) = (p::ff(&w.m(id).pvars, 0x124), p::ff(&w.m(id).pvars, 0x12c));
    if TWO_PI < ph {
        p::set_ff(&mut w.mm(id).pvars, 0x12c, ph * DEG);
    } else if amp != 0.0 && ph == -1.0 {
        let a = w.rng.rand_angle();
        p::set_ff(&mut w.mm(id).pvars, 0x12c, a);
    }
    {
        let m = w.mm(id);
        let p1 = p::v4f(&m.pvars, P_P1);
        p::set_v4f(&mut m.pvars, P_CUR, p1);
    }
    arc_lengths(w, &path, tension, bias);
    let flags = w.m(id).pvars[P_FLAGS];
    if flags & 0x1e != 0 && flags & 1 == 0 {
        for u in 0..4 {
            if flags & (2 << u) == 0 { continue; }
            let j = w.joint_point(id, u);
            let pos = w.m(id).position;
            place_emitter(w, id, u + 1, j, unit(sub3(j, pos)));
        }
    }
    if p::i16(&w.m(id).pvars, 0x11c) < 1 {
        if matches!(w.svc.level, 3 | 9) { p::set_i16(&mut w.mm(id).pvars, 0x13a, 0); }
        w.mm(id).state = 1;
    } else {
        w.mm(id).state = 3;
    }
}

/// State 1 (module doc).
fn fly(w: &mut World, id: MobyId, culled: bool) {
    let Some(path) = Path::of(w, id) else { return };
    let (tension, bias) = (p::ff(&w.m(id).pvars, 0xf4), p::ff(&w.m(id).pvars, 0xf8));
    let t = p::ff(&w.m(id).pvars, P_T);
    if 1.0 <= t {
        p::set_ff(&mut w.mm(id).pvars, P_T, t - 1.0);
        // FUN_0028b878: the next point.
        let idx = path.next_index(p::i32(&w.m(id).pvars, P_IDX));
        p::set_i32(&mut w.mm(id).pvars, P_IDX, idx);
        let m1 = path.tangent(path.pts(w), idx, 1, tension, bias);
        let next = path.pt(path.pts(w), path.wrap_index(idx + 1).max(0) as usize);
        let m = w.mm(id);
        let (a0, c0) = (p::v4f(&m.pvars, P_M1), p::v4f(&m.pvars, P_P1));
        p::set_v4f(&mut m.pvars, P_M0, a0);
        p::set_v4f(&mut m.pvars, P_M1, m1);
        p::set_v4f(&mut m.pvars, P_P0, c0);
        p::set_v4f(&mut m.pvars, P_P1, next);
    }
    let pv = w.m(id).pvars.clone();
    let phase = p::ff(&pv, 0x12c);
    let speed = p::ff(&pv, P_SPEED) + ((p::ff(&pv, 0x124) * K) * phase.sin()) * K;
    let (p0, p1, m0, m1) = (p::v4f(&pv, P_P0), p::v4f(&pv, P_P1), p::v4f(&pv, P_M0), p::v4f(&pv, P_M1));
    let mut dt = speed / p0[3];
    let phase = add_rot(phase, (p::ff(&pv, 0x128) * K) * DEG);
    let t = p::ff(&pv, P_T);
    let prev = p::v4f(&pv, P_CUR);
    let mut cur = hermite(t + dt, p0, p1, m0, m1);
    if pv[P_FLAGS] & 0x40 == 0 {
        let l = dist(prev, cur);
        if l != 0.0 {
            dt *= speed / l;
            cur = hermite(t + dt, p0, p1, m0, m1);
        }
    }
    {
        let m = w.mm(id);
        p::set_ff(&mut m.pvars, 0x12c, phase);
        p::set_v4f(&mut m.pvars, P_PREV, prev);
        p::set_v4f(&mut m.pvars, P_CUR, cur);
    }
    let heading = atan(cur[0] - prev[0], cur[1] - prev[1]);
    if culled {
        w.mm(id).rotation = [0.0, 0.0, heading, 0.0];
    } else {
        let off = p::i16(&pv, 0x122) as f32 * DEG;
        let rot = w.m(id).rotation;
        let diff = sub_rot(add_rot(heading, off), rot[2]);
        let step = diff * ((1.0 / p::ff(&pv, 0x110)) * K);
        let yaw = add_rot(step, rot[2]);
        let v = sub3(cur, prev);
        let pitch = atan((v[0] * v[0] + v[1] * v[1]).sqrt(), v[2]);
        let g = p::ff(&pv, 0x108);
        let mut vel = p::ff(&pv, 0x10c);
        let ry = spring_turn(rot[1], -pitch, ((g * 10.0) * DEG) * DT2, ((g * 5.0) * DEG) * DT2, (g * DEG) * DT, &mut vel);
        p::set_ff(&mut w.mm(id).pvars, 0x10c, vel);
        let x = wrap_frac(step * p::ff(&pv, 0x114));
        let tgt = wrap_frac(sub_rot(rot[0], x) * p::ff(&pv, 0x118));
        let g = p::ff(&pv, 0x100);
        let mut vel = p::ff(&pv, 0x104);
        let rx = spring_turn(rot[0], tgt, ((g * 10.0) * DEG) * DT2, ((g * 5.0) * DEG) * DT2, (g * DEG) * DT, &mut vel);
        let m = w.mm(id);
        p::set_ff(&mut m.pvars, 0x104, vel);
        m.rotation[0] = rx;
        m.rotation[1] = ry;
        m.rotation[2] = yaw;
    }
    {
        let m = w.mm(id);
        p::set_ff(&mut m.pvars, P_T, t + dt);
        m.position = prev;
    }
    if !culled { lateral(w, id); }
    let flags = w.m(id).pvars[P_FLAGS];
    if flags & 0x1f != 0 {
        let mut j = 0;
        for u in 0..5 {
            if (flags >> u) & 1 == 0 { continue; }
            let jp = w.joint_point(id, j);
            j += 1;
            let pos = w.m(id).position;
            let v = sub3(jp, pos);
            if flags & 1 == 0 {
                place_emitter(w, id, u, jp, unit(v));
            } else if w.m(id).pvars[P_SPARK] == 0 {
                // Two `PartType10Spawn(joint, v)` sparks (`particles::type10`), v = unit(joint − pos)·randf(0.016,
                // 0.031)·speed: the speed's draw, then the spawn's (its size; also made without a particle system).
                for _ in 0..2 {
                    let sp = w.rng.randf(f32::from_bits(0x3c83_126f), f32::from_bits(0x3cfd_f3b6));
                    let d = unit(v);
                    let vel = [d[0] * sp, d[1] * sp, d[2] * sp, 0.0];
                    *w.svc.fx.part_spawns.entry(crate::particles::type10::TYPE).or_default() += 1;
                    match w.particles.as_deref_mut() {
                        Some(ps) => {
                            if crate::particles::type10::spawn(ps, w.rng, jp, vel).is_none() { w.svc.fx.part_failed += 1; }
                        }
                        None => { w.rng.randf(f32::from_bits(0x476d_e400), f32::from_bits(0x47b0_5e00)); }
                    }
                }
            }
        }
        let m = w.mm(id);
        if m.pvars[P_SPARK] == 0 { m.pvars[P_SPARK] = 7; }
        m.pvars[P_SPARK] = m.pvars[P_SPARK].wrapping_sub(1);
    }
    if matches!(w.svc.level, 3 | 9) { w.svc.unported("flyer: level 3/9 combat block"); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hermite_hits_the_ends_and_the_tangents() {
        let (p0, p1) = ([0.0, 0.0, 0.0, 7.0], [10.0, 0.0, 0.0, 3.0]);
        let (m0, m1) = ([0.0, 10.0, 0.0, 0.0], [0.0, -10.0, 0.0, 0.0]);
        assert_eq!(hermite(0.0, p0, p1, m0, m1), p0);
        let e = hermite(1.0, p0, p1, m0, m1);
        assert!((e[0] - 10.0).abs() < 1e-6 && e[1].abs() < 1e-6 && e[3] == 7.0);
        // dP/dt at 0 is m0.
        let h = 1e-3;
        let d = hermite(h, p0, p1, m0, m1);
        assert!((d[1] / h - 10.0).abs() < 0.05);
    }

    #[test]
    fn spring_turn_limits_the_step_to_the_error() {
        let mut v = 0.0;
        let a = spring_turn(0.0, 0.01, 100.0, 0.0, 0.0, &mut v);
        assert!((a - 0.01).abs() < 1e-7, "a big gain stops at the target");
        let mut v = 0.0;
        let a = spring_turn(3.0, -3.0, 1.0, 0.0, 0.05, &mut v);
        assert!(a > 3.0 && (v - 0.05).abs() < 1e-7, "the short way round (+0.28), clamped by max");
    }

    #[test]
    fn wraps() {
        assert!((add_rot(3.0, 1.0) - (4.0 - 2.0 * PI)).abs() < 1e-6);
        assert!((wrap_frac(0.5) - 0.5).abs() < 1e-6);
        assert!((wrap_frac(4.0) - (4.0 - 2.0 * PI)).abs() < 1e-5);
    }
}
