//! **Hoven's hover platforms, class 240** (level12 `0x2e3ed8`; census U424; 8 placed). Jet-driven platforms Ratchet
//! rides (`CarryRiders` on the block at pvar +0x60); each tilts toward him while he stands on it (his offset in the
//! platform's yaw frame ÷ 3) and blows two jets out of its back while drawn. A linked moby (+0xfc) still alive, or a
//! group (+0xdc) with live members, holds it (jets only) until they are gone. The kind (+0xe4) picks the motion:
//!
//! * 0 (state 7): along its path (+0xb4) node to node (+0xa4 the step, 1), at the node's w as its speed when not −1
//!   (else +0xd0, default 6; acceleration +0xd4, default 6), its yaw springing toward the travel.
//! * 1 (state 2): bobs between the path's first two points (the phase +0xd8 from a random angle at 70°/s, the fraction
//!   a folded sine), holding its yaw (speed default 4, acceleration 2, then 0).
//! * 2 (state 4): the same bob turned on its side (y −90°, z + 180°; speed / acceleration default 8).
//! * 3 (state 1): nothing (draw distance 0, no collision).
//! * 4 (state 9): hovers 0.4 above the moby +0xf0's height (0.2 with Ratchet on it), speed / acceleration 4.
//!
//! States 3, 5 (the timed side swaps), 6 (a drift rising at 6·dt²; deleted once not drawn) and 8 (a fall spinning by
//! +0xe8 / +0xec, back to 0 within 0.5 of the path's start, deleted below z 0) are entered by no code here.
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x2e3ed8` | the platform | [`update`] |
//! | `0x2e38f0` | Ratchet on it (`HeroOnMoby`) | `World::hero_on_moby` |
//! | `0x2e3948` | the jets (drawn only): from (−1.5, ±0.8, −0.5) in its frame, a type-21 spark (size 10000, out `(−randf(0, 5·dt), ±2·dt, ±2·dt)` in its frame, 0x4f007fff / 0x1fffffff, `rand_range(ticks 8, ticks 17)`, split 1) and a type-23 puff (0.05, 1.01, 1.03, 30000, spin 6, `(−randf(0, dt), ±dt, ±dt)`, 0x404040) each | [`jets`] (`fx::part21`, `fx::part23`) |
//! | `0x2e3e28` | the tilt: Ratchet's offset turned by −yaw, (−y / 3, x / 3) (gp−0x52ec); off him: 0 | [`tilt`] |
//! | `0x2e3c08` | the move: the speed toward +0xd0·dt by +0xd4·dt² (braking: slowing when its stopping distance reaches the target), the velocity along the line to the target; x and y rotation approach the tilt by a fifteenth of the difference (gp−0x52e8), `SpringAngle(yaw, 0.005, 0.3, 110°·dt, +0xf8)` | [`steer`] (`hero::physics::turn_spring`) |
//!
//! Read from the level12 decomp and disassembly (the jets). [L] State 5's angles are stack leftovers in the game
//! (unreachable): 0 here; state 8's ground test is a stub returning 0 in the game, so its explosion never happens.
//! Native `f32`.

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, fx, turn};
use crate::moby_update::scheduler::group_count;
use crate::moby_update::services::{euler_rows, pv, World};
use crate::moby_update::triggers;
use crate::ps2v::Pf;
use std::f32::consts::{FRAC_PI_2, PI};

pub const REFERENCE_LEVEL: u32 = 12;
pub const UPDATE_FN: u32 = 0x2e_3ed8;
pub const CLASSES: [i16; 1] = [240];

const DT: f32 = c::DT;
const DT2: f32 = c::DT2;

/// Pvar offsets (module doc).
pub mod pv {
    pub const FLAGS: usize = 0x3e;
    pub const VEL: usize = 0x40;
    pub const BLOCK: usize = 0x60;
    pub const NODE: usize = 0xa0;
    pub const STEP: usize = 0xa4;
    /// The game's path pointer; here the path + 1 (0 none).
    pub const PATH: usize = 0xb0;
    pub const PATH_IDX: usize = 0xb4;
    pub const SPEED: usize = 0xd0;
    pub const ACCEL: usize = 0xd4;
    pub const PHASE: usize = 0xd8;
    pub const GROUP: usize = 0xdc;
    pub const TIMER: usize = 0xe0;
    pub const KIND: usize = 0xe4;
    pub const SPIN_X: usize = 0xe8;
    pub const SPIN_Y: usize = 0xec;
    pub const UNDER: usize = 0xf0;
    pub const YAW_V: usize = 0xf8;
    pub const LINK: usize = 0xfc;
    pub const SIZE: usize = 0x100;
}

fn path(w: &World, id: MobyId) -> Option<usize> {
    usize::try_from(c::pi32(w, id, pv::PATH) - 1).ok().filter(|&p| p < w.svc.splines.len())
}
fn point(w: &World, p: usize, i: usize) -> c::V { w.svc.splines[p].get(i).map_or([0.0; 4], |q| q.map(f32::from_bits)) }
fn moby(w: &World, i: i32) -> Option<MobyId> { usize::try_from(i).ok().filter(|&m| m < w.table.mobys.len()) }

/// `v` in the frame of `rows` (`fun_001f9cf8`: row-vector times the 3×3).
fn turn_by(rows: [[f32; 4]; 4], v: c::V) -> c::V {
    let mut o = [0.0; 4];
    for (j, x) in o.iter_mut().enumerate().take(3) { *x = rows[0][j] * v[0] + rows[1][j] * v[1] + rows[2][j] * v[2]; }
    o
}

/// `0x2e3948(m)`: the jets (module doc).
fn jets(w: &mut World, id: MobyId) {
    if w.m(id).visible == 0 { return; }
    let rows = w.m(id).rows;
    let pos = c::pos(w, id);
    let mut v = [-w.rng.randf(0.0, DT * 5.0), 0.0, 0.0, 0.0];
    v[1] = w.rng.randf(-(DT + DT), DT + DT);
    v[2] = w.rng.randf(-(DT + DT), DT + DT);
    let v = turn_by(rows, v);
    let p1 = c::add(turn_by(rows, [-1.5, 0.8, -0.5, 0.0]), pos);
    let mut u = [-w.rng.randf(0.0, DT), 0.0, 0.0, 0.0];
    u[1] = w.rng.randf(-DT, DT);
    u[2] = w.rng.randf(-DT, DT);
    let u = turn_by(rows, u);
    let p2 = c::add(turn_by(rows, [-1.5, -0.8, -0.5, 0.0]), pos);
    for p in [p1, p2] {
        let (a, b) = (w.ticks(8), w.ticks(0x11));
        let life = w.rng.rand_range(a, b);
        fx::part21(w, 10000.0, p, v, 0x4f00_7fff, 0x1fff_ffff, life, 1);
        fx::part23(w, [f32::from_bits(0x3d4c_cccd), f32::from_bits(0x3f81_47ae), f32::from_bits(0x3f83_d70a), 30000.0], p, 6, u, 0x40_4040);
    }
}

/// `0x2e3e28(m, angles, on)`: the tilt toward Ratchet (module doc).
fn tilt(w: &World, id: MobyId, on: bool) -> [f32; 2] {
    if !on { return [0.0, 0.0]; }
    let d = c::sub(super::hero_pos(w), c::pos(w, id));
    let r = euler_rows(pv([0.0, 0.0, -c::yaw(w, id), 0.0])).map(|row| row.map(|x| f32::from_bits(x.0)));
    let l = turn_by(r, d);
    [-l[1] / 3.0, l[0] / 3.0]
}

/// `FastVecNormalize(len, out, v)`: `v` set to length `len` (a zero vector stays zero).
fn with_len(v: c::V, len: f32) -> c::V {
    let l = c::len3(v);
    if l == 0.0 { [0.0; 4] } else { let k = len / l; [v[0] * k, v[1] * k, v[2] * k, 0.0] }
}

/// `0x2e3c08(m, angles, target, brake, mode)`: the move (module doc).
fn steer(w: &mut World, id: MobyId, angles: [f32; 3], target: c::V, mode: bool) {
    let v = c::sub(target, c::pos(w, id));
    let dist = c::len3(v);
    let mut sp = c::len3(c::pv4(w, id, pv::VEL));
    let (speed, accel) = (c::pf(w, id, pv::SPEED), c::pf(w, id, pv::ACCEL));
    if !mode {
        let vmax = speed * DT;
        if sp < vmax {
            sp += accel * DT2;
        } else if vmax < sp {
            sp -= accel * DT2;
        }
    } else {
        if sp == 0.0 { sp = DT * 0.01; }
        let a = accel * DT2;
        if a < (sp * sp) / (dist + dist) {
            sp -= a;
        } else if sp < speed * DT {
            sp += a;
        }
    }
    let vel = with_len(v, sp);
    c::set_pv4(w, id, pv::VEL, vel);
    let r = w.m(id).rotation;
    let (mut x, mut y) = (r[0], r[1]);
    turn::approach(angles[0], c::diff_rots(angles[0], x) / 15.0, &mut x);
    turn::approach(angles[1], c::diff_rots(angles[1], y) / 15.0, &mut y);
    let (mut a, mut b) = (Pf::f(r[2]), Pf::f(c::pf(w, id, pv::YAW_V)));
    crate::hero::physics::turn_spring(Pf::f(angles[2]), Pf::f(0.005), Pf::f(f32::from_bits(0x3e99_999a)), Pf::f(DT * 1.919_862_2), &mut a, &mut b, 0);
    c::set_pf(w, id, pv::YAW_V, b.to_f32());
    let m = w.mm(id);
    m.rotation[0] = x;
    m.rotation[1] = y;
    m.rotation[2] = a.to_f32();
    for (i, d) in vel.iter().enumerate().take(3) { m.position[i] += d; }
}

/// The bob's fraction (states 2 / 4): the phase on at 70°/s, its sine folded to 0..1.
fn bob(w: &mut World, id: MobyId) -> f32 {
    let ph = c::add_rot(c::pf(w, id, pv::PHASE), DT * 1.221_730_5);
    c::set_pf(w, id, pv::PHASE, ph);
    let mut f = ph.sin();
    if FRAC_PI_2 < ph && ph < PI { f = 2.0 - f; }
    if -PI < ph && ph < -FRAC_PI_2 { f = 2.0 - f; }
    let mut t = f * 0.5 + 0.5;
    if 1.0 < t { t -= 2.0; }
    t.abs()
}

/// The bob's point (`0x2745a8`: per-lane lerp of the path's first two points).
fn bob_point(w: &World, id: MobyId, t: f32) -> c::V {
    let Some(p) = path(w, id) else { return c::pos(w, id) };
    let (a, b) = (point(w, p, 0), point(w, p, 1));
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t)
}

/// State 0 (module doc).
fn init(w: &mut World, id: MobyId) {
    let coll = super::class_collision(w, w.m(id).o_class);
    let start = |w: &mut World, id: MobyId, speed: f32, accel: f32| {
        w.mm(id).has_collision = coll;
        let pi = c::pi32(w, id, pv::PATH_IDX);
        c::set_pu8(w, id, pv::STEP, 1);
        c::set_pi32(w, id, pv::PATH, if pi < 0 { 0 } else { pi + 1 });
        c::set_pi32(w, id, pv::NODE, 0);
        if c::pf(w, id, pv::SPEED) == 0.0 { c::set_pf(w, id, pv::SPEED, speed); }
        if c::pf(w, id, pv::ACCEL) == 0.0 { c::set_pf(w, id, pv::ACCEL, accel); }
    };
    let fl = c::pi16(w, id, pv::FLAGS) as u16;
    match c::pi32(w, id, pv::KIND) {
        0 => {
            start(w, id, 6.0, 6.0);
            c::set_pi16(w, id, pv::FLAGS, (fl | 9) as i16);
            c::set_pv4(w, id, pv::VEL, [0.0; 4]);
            w.mm(id).state = 7;
        }
        1 => {
            start(w, id, 4.0, 2.0);
            let a = w.rng.rand_angle();
            c::set_pf(w, id, pv::PHASE, a);
            c::set_pi16(w, id, pv::FLAGS, (fl | 9) as i16);
            c::set_pv4(w, id, pv::VEL, [0.0; 4]);
            w.mm(id).state = 2;
        }
        2 => {
            let m = w.mm(id);
            m.rotation[1] = -FRAC_PI_2;
            m.rotation[2] += PI;
            start(w, id, 8.0, 8.0);
            c::set_pv4(w, id, pv::VEL, [0.0; 4]);
            w.mm(id).state = 4;
        }
        3 => {
            let m = w.mm(id);
            m.draw_dist = 0;
            m.state = 1;
            m.has_collision = false;
        }
        4 => {
            w.mm(id).state = 9;
            c::set_pf(w, id, pv::SPEED, 4.0);
            c::set_pf(w, id, pv::ACCEL, 4.0);
            c::set_pi16(w, id, pv::FLAGS, (fl & 0xfffe) as i16);
        }
        _ => {}
    }
}

/// The side swap of states 3 / 5: the timer out → the step 1 ↔ 2, to `next`.
fn swap(w: &mut World, id: MobyId, next: u8) {
    if c::dec_timer_pvar_i32(w, id, pv::TIMER) != 0 {
        let s = if c::pu8(w, id, pv::STEP) == 1 { 2 } else { 1 };
        c::set_pu8(w, id, pv::STEP, s);
        w.mm(id).state = next;
    }
}

/// Level12 `0x2e3ed8` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pv::SIZE { return; }
    let (pos0, rot0) = (c::pos(w, id), w.m(id).rotation);
    if w.m(id).state != 0 {
        let held = moby(w, c::pi32(w, id, pv::LINK)).is_some_and(|m| (w.m(m).state as i8) >= 0);
        let g = c::pi32(w, id, pv::GROUP);
        if held || (g != -1 && group_count(w, g, -1) != 0) {
            jets(w, id);
            return;
        }
    }
    let yaw = c::yaw(w, id);
    match w.m(id).state {
        0 => init(w, id),
        2 => {
            c::set_pf(w, id, pv::ACCEL, 0.0);
            let t = bob(w, id);
            let p = bob_point(w, id, t);
            w.mm(id).position = p;
            let on = w.hero_on_moby(id);
            let [a, b] = tilt(w, id, on);
            steer(w, id, [a, b, yaw], p, true);
        }
        3 => {
            swap(w, id, 2);
            let on = w.hero_on_moby(id);
            let [a, b] = tilt(w, id, on);
            let p = c::pos(w, id);
            steer(w, id, [a, b, yaw], p, true);
        }
        4 => {
            let t = bob(w, id);
            let p = bob_point(w, id, t);
            let m = w.mm(id);
            m.position = p;
            m.rotation[1] = -FRAC_PI_2;
        }
        5 => {
            swap(w, id, 4);
            if c::pu8(w, id, pv::STEP) == 2 { jets(w, id); }
            let p = c::pos(w, id);
            steer(w, id, [0.0, 0.0, yaw], p, true);
        }
        6 => {
            let mut v = c::pv4(w, id, pv::VEL);
            if c::len3(v) < DT * 6.0 { v[2] += DT2 * 6.0; }
            c::set_pv4(w, id, pv::VEL, v);
            let p = c::add(c::pos(w, id), v);
            w.mm(id).position = p;
            if w.m(id).visible == 0 {
                w.delete_moby(id);
                return;
            }
        }
        7 => {
            jets(w, id);
            let Some(p) = path(w, id) else { return };
            let n = w.svc.splines[p].len() as i32;
            if n == 0 { return; }
            let i = (c::pi32(w, id, pv::NODE) + n + c::pu8(w, id, pv::STEP) as i8 as i32).rem_euclid(n) as usize;
            let q = point(w, p, i);
            if c::dist3(c::pos(w, id), q) < 0.5 {
                c::set_pi32(w, id, pv::NODE, i as i32);
                if q[3] != -1.0 { c::set_pf(w, id, pv::SPEED, q[3]); }
            }
            let d = c::sub(q, c::pos(w, id));
            let h = c::atan(d[0], d[1]);
            let on = w.hero_on_moby(id);
            let [a, b] = tilt(w, id, on);
            steer(w, id, [a, b, h], q, false);
        }
        8 => {
            let v = c::pv4(w, id, pv::VEL);
            let p = c::add(c::pos(w, id), v);
            w.mm(id).position = p;
            let mut v = v;
            v[2] -= DT2 * 7.0;
            c::set_pv4(w, id, pv::VEL, v);
            let (sx, sy) = (c::pf(w, id, pv::SPIN_X), c::pf(w, id, pv::SPIN_Y));
            let m = w.mm(id);
            m.rotation[0] = c::add_rot(sx, m.rotation[0]);
            m.rotation[1] = c::add_rot(sy, m.rotation[1]);
            if let Some(pp) = path(w, id) {
                if c::dist2(c::pos(w, id), point(w, pp, 0)) < 0.5 { w.mm(id).state = 0; }
            }
            if c::pos(w, id)[2] < 0.0 {
                w.delete_moby(id);
                return;
            }
        }
        9 => {
            let mz = moby(w, c::pi32(w, id, pv::UNDER)).map_or(0.0, |m| w.m(m).position[2]);
            let p = c::pos(w, id);
            let on = w.hero_on_moby(id);
            let target = [p[0], p[1], mz + if on { 0.2 } else { 0.4 }, p[3]];
            let [a, b] = tilt(w, id, on);
            steer(w, id, [a, b, yaw], target, true);
        }
        _ => {}
    }
    let m = w.mm(id);
    let (d, r) = (c::sub(m.position, pos0), m.rotation);
    triggers::carry_riders(&mut m.pvars, pv::BLOCK, d, rot0, r);
}
