//! Pokitaru's rising gates, class 1248 (level 11, 6 created instances) with the two halves 1247 it creates: level11
//! 0x316320 (census U376). At the init the moby doubles its scale and creates its two halves (class 0x4df, no update
//! of their own; platforms: their platform block at +0x20 marked) 4 below it, lying flat (yaw −90°), the second one
//! mirrored. When Ratchet comes within 12 (xy) and is in its cuboid they rise 4 to its height (loop sound 0 on the
//! first half), stand up, and swing open to ±90°, each step on a spring. Read from the level11 decomp of 0x316320 and
//! the overlay's data (gp−0x48f8 / −0x48f4 / −0x48f0). Native `f32`.
//!
//! **Pvar block**: +0x00 / +0x04 s32 the halves, +0x08 the step's t, +0x0c its velocity, +0x10 s32 the cuboid.
//!
//! | address | what | port |
//! |---|---|---|
//! | state 0 | scale +0x2c ·= 2; twice `CreateMoby(0x4df)`: draw distance 0x40, drawn, the light word and ambient of Ratchet's moby (0x1413d0 +0x38), mode = this mode \| 0x20 (the second also \| 0x8000, mirrored), position = this one 4 lower (gp−0x48f8), rotation = this one with x = ∓0° (gp−0x48f4) and y = −90° (gp−0x48f0); its pvar +0x08 = 0x20 (the platform block), block flags +0x5c \|= 1; → 1 | [`update`] (`World::create_moby`, `take_hero_light`) |
//! | state 1 | `vec_distance2`(position, Ratchet) < 12 and Ratchet in the cuboid (0x2851a0 = L01 0x274820) → t = 0, → 2, `PlayClassSound(0, 4, first half)` (loop) | [`update`] |
//! | state 2 | `0x281810(1, 8·dt², 16·dt², 8·dt, &t, &v)` (L01 0x270830); both halves at position + unit(row 0)·0.5, z = z + 4t − 4; their spheres (0x20def8); t ≥ 1 → t = 0, → 3 | [`update`] (`turn::spring`) |
//! | state 3 | the spring (10, 20, 10); both halves' rotation y = (1 − t)·(−90°); t ≥ 1 → t = 0, → 4 | [`update`] |
//! | state 4 | the spring (12, 24, 12); both halves' z = z + 0.25t, rotation x = −t·π/2 / +t·π/2; t ≥ 1 → t = 0, → 5 | [`update`] |
//! | state 5 | open: nothing | n/a |
//! | state 6 | the halves placed open (offset, z + gp−0x48f8, the init rotation): no code sets state 6 (n/a) | n/a |
//! | | no particle, hit, flag | n/a |

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::creature::{self as c, turn, DT, DT2};
use crate::moby_update::services::{pvar as p, World};

/// The update in the level11 class table.
pub const UPDATE_FN: u32 = 0x31_6320;
pub const REFERENCE_LEVEL: u32 = 11;
pub const CLASSES: [i16; 1] = [1248];
/// The halves (no update of their own).
pub const HALF: i16 = 0x4df;
const DEG: f32 = 0.017_453_292;
pub const DROP: f32 = -4.0;
pub const YAW: f32 = -90.0;

fn halves(w: &World, id: MobyId) -> [Option<MobyId>; 2] {
    [0, 4].map(|o| usize::try_from(c::pi32(w, id, o)).ok().filter(|&m| m < w.table.mobys.len()))
}

fn step(w: &mut World, id: MobyId, k: f32) -> f32 {
    let (mut t, mut v) = (c::pf(w, id, 8), c::pf(w, id, 0xc));
    turn::spring(1.0, k * DT2, 2.0 * k * DT2, k * DT, &mut t, &mut v);
    c::set_pf(w, id, 8, t);
    c::set_pf(w, id, 0xc, v);
    t
}

fn done(w: &mut World, id: MobyId, t: f32, next: u8) {
    if 1.0 <= t {
        c::set_pf(w, id, 8, 0.0);
        w.mm(id).state = next;
    }
}

/// Level11 0x316320 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x14 { return; }
    let hs = halves(w, id);
    match w.m(id).state {
        0 => {
            w.mm(id).scale *= 2.0;
            let (pos, rot, md) = (w.m(id).position, w.m(id).rotation, w.m(id).mode);
            let light = w.hero_moby.map(|h| (w.m(h).light, w.m(h).ambient));
            for (k, sign) in [(0usize, -1.0f32), (1, 1.0)] {
                let m = w.create_moby(HALF);
                c::set_pi32(w, id, 4 * k, m.map_or(-1, |m| m as i32));
                let Some(m) = m else { continue };
                let h = w.mm(m);
                h.draw_dist = 0x40;
                h.visible = 1;
                if let Some((l, a)) = light { h.light = l; h.ambient = a; }
                h.mode = md | 0x20 | if k == 1 { mode::MIRROR } else { 0 };
                h.position = pos;
                h.position[2] += DROP;
                h.rotation = rot;
                h.rotation[0] = sign * 0.0 * DEG;
                h.rotation[1] = YAW * DEG;
                if h.pvars.len() >= 0x60 {
                    p::set_i32(&mut h.pvars, 8, 0x20);
                    let f = p::i32(&h.pvars, 0x5c) | 1;
                    p::set_i32(&mut h.pvars, 0x5c, f);
                }
            }
            w.mm(id).state = 1;
        }
        1 => {
            let h = super::hero_pos(w);
            if 12.0 <= c::dist2(w.m(id).position, h) { return; }
            if !w.in_cuboid(w.hero_point(), c::pi32(w, id, 0x10)) { return; }
            c::set_pf(w, id, 8, 0.0);
            w.mm(id).state = 2;
            if let Some(a) = hs[0] { w.play_sound(0, 4, a); }
        }
        2 => {
            let t = step(w, id, 8.0);
            let (pos, r0) = (w.m(id).position, w.m(id).rows[0]);
            let off = c::set_len3(r0, 0.5);
            for m in hs.into_iter().flatten() {
                let mut q = c::add(pos, off);
                q[2] = (pos[2] + t * 4.0) - 4.0;
                w.mm(m).position = q;
                w.build_matrix(m);
            }
            done(w, id, t, 3);
        }
        3 => {
            let t = step(w, id, 10.0);
            let y = (1.0 - t) * YAW * DEG;
            for m in hs.into_iter().flatten() {
                w.mm(m).rotation[1] = y;
                w.build_matrix(m);
            }
            done(w, id, t, 4);
        }
        4 => {
            let t = step(w, id, 12.0);
            let z = w.m(id).position[2] + t * 0.25;
            for (k, m) in hs.into_iter().enumerate() {
                let Some(m) = m else { continue };
                let h = w.mm(m);
                h.position[2] = z;
                h.rotation[0] = if k == 0 { -(t * std::f32::consts::FRAC_PI_2) } else { t * std::f32::consts::FRAC_PI_2 };
                w.build_matrix(m);
            }
            done(w, id, t, 5);
        }
        _ => {}
    }
}
