//! Gaspar's hazard columns, class 1206 (level 09, 9 created instances): level09 0x305a28 (census U313). An invisible
//! damage column: every tick `count` spheres of radius `r` placed along the moby's z row over `length` hit what they
//! touch with a template of the pvar's damage and flags; the class's loop sound 0 starts on the moby's tick phase
//! (every 8th tick) while it has none. Read from the level09 decomp and disassembly of 0x305a28 (the slot test is an
//! integer compare). Native `f32`.
//!
//! **Pvar block**: +0x00 r, +0x04 the length, +0x08 s32 the count, +0x0c the damage, +0x10 s32 the hit flags, +0x14 s32
//! the voice slot.
//!
//! | address | what | port |
//! |---|---|---|
//! | state 0 | → 1, slot = −1 | [`update`] |
//! | every tick | on its phase ((0x15f5cc & 7) = (address >> 8) & 7) with slot −1: slot = `PlayClassSound(0, 4, m)` (loop); a live or dead slot ≠ −1 is never replayed (`SoundIsAlive` 0x2aa180 is called, its answer cannot change that) | [`update`] (`kalebo_traffic::slot_phase`, `World::play_sound`) |
//! | | step = row 2 · (length − 2r)/count (`matrix_mul_vec3`); p = position + unit(step)·r | [`update`] |
//! | | template (0x2793f0 = L01 0x26e7d8): attacker m, flags +0x10, damage +0x0c, +0x20 = 0, dir 0 (+0x18..+0x1b stale stack: 0 [L]) | [`update`] |
//! | | count × `coll_sphere_mobys(r, p, 1, m, tmpl)` (0x211bd0 = L01 0x214468), p += step; a hit only prints (`STUB_printf`: n/a) | [`update`] (`World::sphere_mobys`) |
//! | | no particle, light, flag | n/a |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature as c;
use crate::moby_update::services::{self as sv, HitTemplate, World};
use crate::ps2v::Pf;

use super::kalebo_traffic::slot_phase;

/// The update in the level09 class table.
pub const UPDATE_FN: u32 = 0x30_5a28;
pub const REFERENCE_LEVEL: u32 = 9;
pub const CLASSES: [i16; 1] = [1206];

/// Level09 0x305a28 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x18 { return; }
    if w.m(id).state == 0 {
        w.mm(id).state = 1;
        c::set_pi32(w, id, 0x14, -1);
    }
    if w.counter & 7 == slot_phase(id) & 7 && c::pi32(w, id, 0x14) == -1 {
        let s = w.play_sound(0, 4, id);
        c::set_pi32(w, id, 0x14, s);
    }
    let (r, len, n) = (c::pf(w, id, 0), c::pf(w, id, 4), c::pi32(w, id, 8));
    let (dmg, flags) = (c::pf(w, id, 0xc), c::pi32(w, id, 0x10) as u32);
    let z = w.m(id).rows[2];
    let k = (len - (r + r)) / n as f32;
    let step = [z[0] * k, z[1] * k, z[2] * k, z[3] * k];
    let mut p = c::add(w.m(id).position, c::set_len3(step, r));
    let tmpl = HitTemplate { dir: [Pf::ZERO; 4], attacker: Some(id), flags, b18: 0, b19: 0, h1a: 0, damage: sv::pf(dmg), w20: 0 };
    for _ in 0..n.max(0) {
        w.sphere_mobys(sv::pf(r), sv::pv(p), 1, Some(id), Some(&tmpl));
        p = c::add(p, step);
    }
}
