//! **The tilting platforms, class 1178** (level11 `0x30eb90`, 4 placed; census U388): platforms that tip about their
//! own y axis under Ratchet's weight and carry him with the tilt. A spring pulls the tilt back level and damps it;
//! while Ratchet stands on one, the side he stands on (his x in the platform's frame) pushes it over, harder against
//! the swing, never less than 5°/s². Tipped past 40° it throws him off (`SetState(6, 1)`) when he is on foot. Read
//! from the level11 decomp, the words gp−0x4c98..−0x4c7c. Native `f32`.
//!
//! **Pvars** (0x80): +0x08 the platform block's offset (0x20), +0x20 the block (+0x5c its flags), +0x60 the tilt's
//! angular velocity.
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x30eb90` | the old Euler kept; block flags +0x5c = 1; v += 10°·dt²·(−2/π)·tilt + 10°·dt²·(−100)·v | [`update`] |
//! | | Ratchet on it (0x13f64c) and grounded (0x13f65e = 0): f = 90°·dt²·0.125·x, x = (Ratchet − position)·row 0 (the transposed rows 0x1fa2d8, `fun_001f9d20`); against both v and the tilt → 2f; f ± 5°·dt² by its sign; v += f; \|tilt\| (0x221128) > 40° and the movement group 0x1413dc < 2 → `SetState(6, 1)` (0x24db50 = L01 0x23cf98) | [`update`] (`HeroCall::SetState`) |
//! | | tilt (+0x44) = `fast_add_rotations(tilt, v)`; `CarryRiders(block, 0, old, new)` (0x285cf8 = L01 0x2755f8) | [`update`] (`triggers::carry_riders`) |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, DT2};
use crate::moby_update::services::{HeroCall, World};
use crate::moby_update::triggers;

pub const REFERENCE_LEVEL: u32 = 11;
pub const UPDATE_FN: u32 = 0x30_eb90;
pub const CLASSES: [i16; 1] = [1178];
const BLOCK: usize = 0x20;
const FLAGS: usize = 0x5c;
const VEL: usize = 0x60;
const DEG: f32 = 0.017_453_292;
/// gp−0x4c98..−0x4c7c (level11 0x161f68..0x161f84).
const PUSH_SCALE: f32 = 0.125;
const SPRING_K: f32 = f32::from_bits(0xbf22_f983);
const DAMP_K: f32 = -100.0;
const MIN_PUSH: f32 = 5.0;
const PUSH: f32 = 90.0;
const SPRING: f32 = 10.0;
const DAMP: f32 = 10.0;
const THROW: f32 = 40.0;

/// Level11 `0x30eb90` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < VEL + 4 { w.mm(id).pvars.resize(0x80, 0); }
    let old = w.m(id).rotation;
    c::set_pi32(w, id, FLAGS, 1);
    let tilt = w.m(id).rotation[1];
    let mut v = c::pf(w, id, VEL);
    v += SPRING * DEG * DT2 * SPRING_K * tilt + DAMP * DEG * DT2 * DAMP_K * v;
    if w.hero.ground_moby == Some(id) && w.hero.air_ticks == 0 {
        let (h, m) = (super::hero_pos(w), w.m(id));
        let d = [h[0] - m.position[0], h[1] - m.position[1], h[2] - m.position[2]];
        let x = d[0] * m.rows[0][0] + d[1] * m.rows[0][1] + d[2] * m.rows[0][2];
        let mut f = PUSH * DEG * DT2 * PUSH_SCALE * x;
        if v * f < 0.0 && tilt * f < 0.0 { f += f; }
        if f < 0.0 { f -= MIN_PUSH * DEG * DT2; } else { f += MIN_PUSH * DEG * DT2; }
        v += f;
        if THROW * DEG < tilt.abs() && (w.hero.group as u32) < 2 {
            w.hero_fields_mut().call(HeroCall::SetState { id: 6, play: true });
        }
    }
    c::set_pf(w, id, VEL, v);
    let m = w.mm(id);
    m.rotation[1] = c::add_rot(tilt, v);
    let new = m.rotation;
    triggers::carry_riders(&mut m.pvars, BLOCK, [0.0; 4], old, new);
}
