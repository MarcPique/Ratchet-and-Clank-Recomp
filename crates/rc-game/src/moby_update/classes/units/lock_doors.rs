//! The doors the Trespasser locks open ([`super::trespasser_lock`]: a lock in state 4 is open): level02's sliding pair
//! 743 (census U107, `0x2dffc8`) / 744 (U108, `0x2e0138`) and level11's split door 1159 (U365, `0x30e978`, which also
//! opens on a pressed floor switch 830). Read from the level02 / level11 decompiler output. The other levels' lock
//! consumers are their own class ports: L04 1101 / 1102 / 1531 / 1532 (`units::…` U168), 1343 (U171), L08 467 / 472
//! (U289); L18's barrier 1392 (U562) is not ported (G-CLS-001).
//!
//! **743 / 744** (pvars: +0x00 the closed position, +0x10 the link): state 0 keeps the position, state 1; state 1 waits
//! for the link (a moby of class 0x267 (615) or 0x23f (575)) to reach state 4 → state 2 (743: `PlayClassSound(0, 0,
//! m)`); state 2 moves toward the closed position + 2 (743) / − 2 (744) along its row 1 (+0xd0), at most 2·dt a tick
//! (`0x2616f8`); arrived (the step 0) → state 3 (743: `PlayClassSound(1, 0, m)`).
//!
//! **1159** (pvars: +0x00 / +0x04 the spring's travel and velocity, +0x08 the link, +0x0c the size, +0x10 the travel,
//! +0x14 the global flag's offset): state 0 lifts it by +0x0c·1.9, makes its twin (`CreateMoby(0x487)`: +0x31 = 1, +0x32
//! = 0x40, light words and mode copied, at its position, its Euler with x + π, `MobyBuildMatrix`, pvars +0x08 / +0x10 /
//! +0x14 copied, scale = class scale × +0x0c, state 1), its own scale = class scale × +0x0c, state 1; state 1 waits for
//! the link (615 in state 4, or a floor switch 830 (0x33e) with its +0xbc set) → state 2, global flag `0x13d3df + +0x14`
//! = 1, `PlayClassSound(0, 0, m)`; state 2: `Spring(+0x10, dt², dt², 2·dt, &+0x00, &+0x04)` (0x270830) and the moby moves
//! by −(+0x04) along its row 1 (the twin, upside down, the other way).
//!
//! | address | what | port |
//! |---|---|---|
//! | level02 0x2dffc8 | 743 (module doc) | [`update_743`] |
//! | level02 0x2e0138 | 744 (module doc) | [`update_744`] |
//! | level11 0x30e978 | 1159 (module doc) | [`update_1159`] |
//! | sounds, particles, lights, stats | the class sounds above; the global flag of 1159; nothing else | — |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, add_rot};
use crate::moby_update::services::World;
use std::f32::consts::PI;

pub const REFERENCE_LEVEL: u32 = 2;
pub const UPDATE_743: u32 = 0x2d_ffc8;
pub const UPDATE_744: u32 = 0x2e_0138;
pub const CLASSES_743: [i16; 1] = [743];
pub const CLASSES_744: [i16; 1] = [744];
pub const LEVEL_1159: u32 = 11;
pub const UPDATE_1159: u32 = 0x30_e978;
pub const CLASSES_1159: [i16; 1] = [1159];

/// The lock classes a door follows (615 the Trespasser lock, 575).
fn lock_open(w: &World, link: i32) -> bool {
    let Some(m) = usize::try_from(link).ok().and_then(|l| w.table.mobys.get(l)) else { return false };
    (m.o_class == 0x267 || m.o_class == 0x23f) && m.state == 4
}

fn slide(w: &mut World, id: MobyId, dist: f32, sounds: bool) {
    match w.m(id).state {
        0 => {
            if w.m(id).pvars.len() < 0x14 { return; }
            let p = w.m(id).position;
            c::set_pv4(w, id, 0, p);
            w.mm(id).state = 1;
        }
        1 => {
            if w.m(id).pvars.len() < 0x14 { return; }
            let link = c::pi32(w, id, 0x10);
            if link < 0 || !lock_open(w, link) { return; }
            w.mm(id).state = 2;
            if sounds { w.play_sound(0, 0, id); }
        }
        2 => {
            let dt = crate::moby_update::services::DT.to_f32();
            let r = w.m(id).rows[1];
            let l = (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt();
            let dir = if l == 0.0 { [0.0; 3] } else { [r[0] / l * dist, r[1] / l * dist, r[2] / l * dist] };
            let home = c::pv4(w, id, 0);
            let goal = [home[0] + dir[0], home[1] + dir[1], home[2] + dir[2]];
            let pos = w.m(id).position;
            let mut v = [goal[0] - pos[0], goal[1] - pos[1], goal[2] - pos[2]];
            let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            let max = dt + dt;
            if max < n { v = v.map(|x| x * (max / n)); }
            let m = w.mm(id);
            for (p, d) in m.position.iter_mut().zip(v) { *p += d; }
            if v.iter().all(|&x| x == 0.0) {
                if sounds { w.play_sound(1, 0, id); }
                w.mm(id).state = 3;
            }
        }
        _ => {}
    }
}

/// Level02 0x2dffc8 (743): slides 2 along its row 1, with sounds.
pub fn update_743(w: &mut World, id: MobyId) { slide(w, id, 2.0, true); }

/// Level02 0x2e0138 (744): slides 2 the other way, silent.
pub fn update_744(w: &mut World, id: MobyId) { slide(w, id, -2.0, false); }

/// Level11 0x30e978 (1159).
pub fn update_1159(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x18 { return; }
    match w.m(id).state {
        0 => {
            let size = c::pf(w, id, 0xc);
            w.mm(id).position[2] += size * 1.9;
            let o_class = w.m(id).o_class;
            let scale = crate::moby_update::classes::units::class_scale(w, o_class) * size;
            if let Some(t) = w.create_moby(0x487) {
                let (light, ambient, mode, pos, rot) = { let m = w.m(id); (m.light, m.ambient, m.mode, m.position, m.rotation) };
                let (link, travel, flag) = (c::pi32(w, id, 8), c::pi32(w, id, 0x10), c::pi32(w, id, 0x14));
                let m = w.mm(t);
                m.visible = 1;
                m.draw_dist = 0x40;
                (m.light, m.ambient, m.mode) = (light, ambient, mode);
                m.position = pos;
                m.rotation = [add_rot(rot[0], PI), rot[1], rot[2], rot[3]];
                if m.pvars.len() < 0x18 { m.pvars.resize(0x18, 0); }
                w.build_matrix(t);
                c::set_pi32(w, t, 8, link);
                c::set_pi32(w, t, 0x10, travel);
                c::set_pi32(w, t, 0x14, flag);
                w.mm(t).state = 1;
                w.mm(t).scale = scale;
            }
            w.mm(id).scale = scale;
            w.mm(id).state = 1;
        }
        1 => {
            let link = c::pi32(w, id, 8);
            let Some(l) = usize::try_from(link).ok().and_then(|l| w.table.mobys.get(l)) else { return };
            let open = (l.o_class == 0x267 && l.state == 4) || (l.o_class == 0x33e && l.cmd != 0);
            if !open { return; }
            w.mm(id).state = 2;
            let k = c::pi32(w, id, 0x14);
            if let Ok(k) = usize::try_from(k + 0x57) { crate::moby_update::interact::set_global_flag(w, k, 1); }
            w.play_sound(0, 0, id);
        }
        2 => {
            let dt = crate::moby_update::services::DT.to_f32();
            let (mut x, mut v) = (c::pf(w, id, 0), c::pf(w, id, 4));
            let target = c::pf(w, id, 0x10);
            crate::moby_update::creature::turn::spring(target, dt * dt, dt * dt, dt + dt, &mut x, &mut v);
            c::set_pf(w, id, 0, x);
            c::set_pf(w, id, 4, v);
            let r = w.m(id).rows[1];
            let l = (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt();
            if l != 0.0 {
                let m = w.mm(id);
                for (p, d) in m.position.iter_mut().zip(r) { *p += d / l * -v; }
            }
        }
        _ => {}
    }
}
