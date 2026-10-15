//! Novalis's water currents, class 613 (level 01, 4 placed instances; census U50): level01 `WaterCurrentUpdate`
//! 0x2f3120. A current is a path (`0x1b0930[pvar +0x20]`); while Ratchet swims (movement group 0x11 / 0x12, or just
//! came out of the water) within its radius of the path, the current's velocity grows along the path by 7·dt² a tick
//! up to its top speed and is his platform push (0x13f440: the hero's move adds it), plus an optional pull toward the
//! path; at the path's end the push decays. Only the nearest current pushes (0x141608). Leaving the water by a jump
//! keeps him drifting and locks the next surface jump for 35 ticks. Read from the level01 disassembly. Native `f32`.
//!
//! **Pvar block**: +0x00 the velocity (quad), +0x20 s32 the path (−1 none), +0x24 the top speed, +0x28 the radius
//! (xy), +0x2c s16 the pull flag, +0x2e s16 the init flag.
//!
//! | address | what | port |
//! |---|---|---|
//! | 0x2f3120 | no pvar block, or path −1 → nothing | [`update`] |
//! | | first tick (+0x2e = 0): +0x2e = 1; every point's w = `VecDistance`(point, next) (the last: back to the first) in the shared path table; update distance 0xff | [`update`] (`Services::splines`) |
//! | | Ratchet in group 0x11 / 0x12, or previous group (0x1413e4) 0x11 / 0x12, or state 0x12, or previous state (0x1413e0) 0x12; else nothing | [`update`] |
//! | | the path point nearest to him: `0x272e28(999, 5, 0, path, 0x13f3d0, &p, &seg, &t, open)` (`spline::nearest`; none: the stack's zeros [L]) | [`update`] |
//! | | 2 units further along: `0x2726c8(2, path, &q, &seg, &t, open)` (`spline::advance`), its end flag | [`update`] |
//! | | `VecDistance2`(Ratchet, p) (xy) > radius → nothing | [`update`] |
//! | | group 0x11 / 0x12: 0x141608 < d → nothing; else 0x141608 = d | [`update`] (`HeroFields::current_dist`) |
//! | | not at the end: v += unit(q − p)·7·dt² (0x15ed70); v.z = 0; \|v\| > top → v = unit(v)·top; 0x13f440 = v (quad) | [`update`] (`HeroFields::platform`) |
//! | | (the w lanes of p and q, 0x13f44c's yaw change: taken as 0 [L]) | [`update`] |
//! | | pull (+0x2c ≠ 0): u = p − Ratchet, u.z = 0, \|u\| > 0.4·top → unit(u)·0.4·top; 0x13f440 += u | [`update`] |
//! | | at the end: v = unit(v)·(\|v\| − 0.004999995·[0x15ed60]·\|v\|); 0x13f440 = v | [`update`] |
//! | | group 0x12 and previous state 0x12 and the state timer 0x13f4e8 < `ticks(3)`: speed 0x13f4e4 = min(speed, 1.5·dt (gp−0x7e94 = 0x15ed6c)); momentum 0x13f4a0 = 0 (`0x221170`) | [`update`] (`HeroFields::speed` / `momentum`) |
//! | | not in group 0x11 / 0x12: previous group 0x12 or the group before (0x1413f0) 0x12, and group 4 / 5: v = unit(v)·(\|v\| − 0.014999986·[0x15ed60]·\|v\|); 0x13f440 = v; 0x13f528 < `ticks(0x23)` → `ticks(0x23)` | [`update`] (`HeroFields::jump_lock`) |
//! | | no sound, particle, hit, flag | n/a |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, DT, DT2, SPEED};
use crate::moby_update::services::World;

/// The update in the level01 class table.
pub const UPDATE_FN: u32 = 0x2f_3120;
pub const REFERENCE_LEVEL: u32 = 1;
pub const CLASSES: [i16; 1] = [613];

pub mod pv {
    pub const VEL: usize = 0x00;
    pub const PATH: usize = 0x20;
    pub const TOP: usize = 0x24;
    pub const RADIUS: usize = 0x28;
    pub const PULL: usize = 0x2c;
    pub const INIT: usize = 0x2e;
    pub const SIZE: usize = 0x30;
}

fn len3(v: [f32; 4]) -> f32 { (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt() }

/// Level01 0x2f3120 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pv::SIZE { return; }
    let path = c::pi32(w, id, pv::PATH);
    let Some(path) = usize::try_from(path).ok().filter(|&p| p < w.svc.splines.len()) else { return };
    if crate::moby_update::services::pvar::i16(&w.m(id).pvars, pv::INIT) == 0 {
        crate::moby_update::services::pvar::set_i16(&mut w.mm(id).pvars, pv::INIT, 1);
        let pts = &mut w.svc.splines[path];
        let n = pts.len();
        let f = |q: [u32; 4]| [f32::from_bits(q[0]), f32::from_bits(q[1]), f32::from_bits(q[2]), 0.0];
        for k in 0..n {
            let (a, b) = (f(pts[k]), f(pts[(k + 1) % n]));
            pts[k][3] = c::dist3(a, b).to_bits();
        }
        w.mm(id).update_dist = 0xff;
    }
    let h = w.hero;
    let (group, state) = (h.group, h.state);
    let swims = group == 0x11 || group == 0x12;
    if !(swims || h.prev_group == 0x11 || h.prev_group == 0x12 || state == 0x12 || h.prev_state == 0x12) { return; }
    let pts: Vec<[f32; 4]> = w.svc.splines[path].iter().map(|q| q.map(f32::from_bits)).collect();
    let hp = super::hero_pos(w);
    let (p, mut cur) = crate::spline::nearest(&pts, false, 999.0, 5.0, 0.0, [hp[0], hp[1], hp[2]]).unwrap_or(([0.0; 3], Default::default()));
    let (q, end) = crate::spline::advance(&pts, false, 2.0, &mut cur);
    let p4 = [p[0], p[1], p[2], 0.0];
    let d = c::dist2(hp, p4);
    if c::pf(w, id, pv::RADIUS) < d { return; }
    let top = c::pf(w, id, pv::TOP);
    if swims {
        if w.hero_fields().current_dist < d { return; }
        w.hero_fields_mut().current_dist = d;
        let mut v = c::pv4(w, id, pv::VEL);
        if !end {
            let a = c::set_len3([q[0] - p[0], q[1] - p[1], q[2] - p[2], 0.0], DT2 * 7.0);
            for k in 0..4 { v[k] += a[k]; }
            v[2] = 0.0;
            if top < len3(v) { v = c::set_len3(v, top); }
            c::set_pv4(w, id, pv::VEL, v);
            let mut push = v;
            if crate::moby_update::services::pvar::i16(&w.m(id).pvars, pv::PULL) != 0 {
                // The w lane (0x13f44c, the push's yaw change): 0 [L: the w the path functions leave in their outputs
                // is not traced; any other value would turn Ratchet every tick].
                let mut u = [p[0] - hp[0], p[1] - hp[1], p[2] - hp[2], 0.0];
                u[2] = 0.0;
                let lim = top * f32::from_bits(0x3ecc_cccd);
                if lim < len3(u) { u = c::set_len3(u, lim); }
                for k in 0..4 { push[k] += u[k]; }
            }
            w.hero_fields_mut().platform = push;
        } else {
            let l = len3(v);
            v = c::set_len3(v, SPEED * f32::from_bits(0xbba3_d700) * l + l);
            c::set_pv4(w, id, pv::VEL, v);
            w.hero_fields_mut().platform = v;
        }
        if group == 0x12 && h.prev_state == 0x12 && h.timer < w.ticks(3) {
            let f = w.hero_fields_mut();
            let cap = DT * 1.5;
            if cap < f.speed { f.speed = cap; }
            f.momentum = [0.0; 4];
        }
    } else if (h.prev_group == 0x12 || h.prev_prev_group == 0x12) && (group == 4 || group == 5) {
        let mut v = c::pv4(w, id, pv::VEL);
        let l = len3(v);
        v = c::set_len3(v, SPEED * f32::from_bits(0xbc75_c280) * l + l);
        c::set_pv4(w, id, pv::VEL, v);
        let t = w.ticks(0x23);
        let f = w.hero_fields_mut();
        f.platform = v;
        if f.jump_lock < t as i16 { f.jump_lock = t as i16; }
    }
}
