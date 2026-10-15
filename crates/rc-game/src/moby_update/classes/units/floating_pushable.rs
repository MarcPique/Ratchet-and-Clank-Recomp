//! Novalis' floating pushables, class 695 (level 01, 9 created instances): level01 `FloatingPushableUpdate` 0x2f8268
//! (census U60), with its spring 0x2f81c8. A float that sways back toward its placed point on two damped springs,
//! is pushed out to 1 unit from Ratchet when he comes closer at its height, moves over the ground as a radius-2 body
//! (`walker::move_ground`), stays within 10·dt of its placed point, and rides the water surface (at most 3 units/s up
//! or down). Read from the level01 decomp (0x2f8268, 0x2f81c8) and the overlay's data (gp−0x5010 .. −0x5000).
//! Native `f32`.
//!
//! **Pvar block**: +0x00 the placed point, +0x10 / +0x14 the x / y spring velocities.
//!
//! | address | what | port |
//! |---|---|---|
//! | 0x2f8268 | no pvar block → nothing | [`update`] |
//! | state 0 | +0x00 = position; → 1 | [`update`] |
//! | state 1 | `0x2f81c8(home.x, 10·dt², 0.1, 3, &x, &+0x10)` and the same on y (gp−0x5010 / −0x500c) | [`update`] ([`spring`]) |
//! | 0x2f81c8(t, k, damp, range, &x, &v) | v ·= (1 − damp)·[0x15ed60] − [0x15ed60] + 1; a = min((x − t)², range²)·k / range²; v −= a (x > t) or += a; x += v (the trailing `fabs` is unused) | [`spring`] |
//! | | `VecDistance2`(position, Ratchet) < 1 (gp−0x5008) and \|z − Ratchet z\| < 0.7 → xy = Ratchet + (cos, sin)(`atan`(position − Ratchet))·1 | [`update`] |
//! | | `0x26d270(0.1, 2 (gp−0x5000), 600, π/2, m, &old, &position, 3)` (`walker::move_ground`); then x / y = its result, z = the z at the tick's start | [`update`] |
//! | | `VecDistance`(home, position) > 10·dt (gp−0x5004) → position = home + unit(position − home)·10·dt | [`update`] |
//! | | `SetWaterLevel(position, 0)` (0x26ed38; no water → its own z) − z, clamped to ±3·dt, added to z | [`update`] (`water::WaterWorld::water_height`) |
//! | | no sound, particle, hit, flag | n/a |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, walker, DT, DT2, SPEED};
use crate::moby_update::services::World;

/// The update in the level01 class table.
pub const UPDATE_FN: u32 = 0x2f_8268;
pub const REFERENCE_LEVEL: u32 = 1;
pub const CLASSES: [i16; 1] = [695];
/// gp−0x5010 .. −0x5000: the spring rate (· dt²), its damping, the push radius, the leash (· dt), the body radius.
pub const K: f32 = 10.0;
pub const DAMP: f32 = 0.1;
pub const PUSH_R: f32 = 1.0;
pub const LEASH: f32 = 10.0;
pub const BODY_R: f32 = 2.0;

/// 0x2f81c8(t, k, damp, range, &x, &v) (module doc).
pub fn spring(t: f32, k: f32, damp: f32, range: f32, x: &mut f32, v: &mut f32) {
    let r2 = range * range;
    *v *= ((1.0 - damp) - 1.0) * SPEED + 1.0;
    let d2 = ((*x - t) * (*x - t)).min(r2);
    let a = (d2 * k) / r2;
    if t < *x { *v -= a } else { *v += a }
    *x += *v;
}

/// Level01 0x2f8268 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x18 { return; }
    let start = w.m(id).position;
    match w.m(id).state {
        0 => {
            c::set_pv4(w, id, 0, start);
            w.mm(id).state = 1;
        }
        1 => {
            let mut old = start;
            let home = c::pv4(w, id, 0);
            for (k, o) in [(0usize, 0x10usize), (1, 0x14)] {
                let mut x = w.m(id).position[k];
                let mut v = c::pf(w, id, o);
                spring(home[k], K * DT2, DAMP, 3.0, &mut x, &mut v);
                w.mm(id).position[k] = x;
                c::set_pf(w, id, o, v);
            }
            let h = super::hero_pos(w);
            let p = w.m(id).position;
            if c::dist2(p, h) < PUSH_R && (p[2] - h[2]).abs() < 0.7 {
                let a = c::atan(p[0] - h[0], p[1] - h[1]);
                let m = w.mm(id);
                m.position[0] = a.cos() * PUSH_R + h[0];
                m.position[1] = a.sin() * PUSH_R + h[1];
            }
            let mut to = w.m(id).position;
            walker::move_ground(w, id, 0.1, BODY_R, 600.0, std::f32::consts::FRAC_PI_2, &mut old, &mut to, 3);
            let m = w.mm(id);
            m.position[0] = old[0];
            m.position[1] = old[1];
            m.position[2] = start[2];
            let p = m.position;
            if LEASH * DT < c::dist3(home, p) {
                m.position = c::add(home, c::set_len3(c::sub(p, home), LEASH * DT));
            }
            let p = w.m(id).position;
            let wl = w.svc.water.water_height([p[0], p[1], p[2]]).unwrap_or(p[2]);
            let lim = DT * 3.0;
            let dz = (wl - p[2]).clamp(-lim, lim);
            w.mm(id).position[2] += dz;
        }
        _ => {}
    }
}
