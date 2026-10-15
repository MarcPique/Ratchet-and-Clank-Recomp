//! Rilgar's flaps, class 895 (level 05, 4 created instances): level05 0x3166a0 (census U195). A flap that turns ±90°
//! from its placed yaw at the init (mirrored when its pvar +0x00 is set) and, once its linked moby's command byte is
//! set, swings ±40° back toward it with a spring, playing class sound 0 once. Read from the level05 decomp of
//! 0x3166a0. Native `f32`.
//!
//! **Pvar block**: +0x00 s32 the side (0: left), +0x04 the yaw velocity, +0x08 s32 the linked moby, +0x0c the placed yaw.
//!
//! | address | what | port |
//! |---|---|---|
//! | state 0 | side ≠ 0 → mode \|= 0x8000 (mirror); → 1; +0x0c = yaw; yaw = yaw + π/2 (side 0) or − π/2 (`fast_add` / `fast_subtract_rotations`); then the state-1 check this tick | [`update`] |
//! | state 1 | the linked moby's +0xbc ≠ 0 → `PlayClassSound(0, 0, m)`, → 2 | [`update`] (`World::play_sound`) |
//! | state 2 | target = +0x0c + 40° (side 0) or − 40° (0x3f32b8c2); `0x286078(target, 4π·dt², 4π·dt², 4π·dt, &yaw, &+0x04)` (L01 0x270cc0) | [`update`] (`turn::turn_toward`) |
//! | | no particle, hit, flag | n/a |

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::creature::{self as c, turn, DT, DT2};
use crate::moby_update::services::World;

/// The update in the level05 class table.
pub const UPDATE_FN: u32 = 0x31_66a0;
pub const REFERENCE_LEVEL: u32 = 5;
pub const CLASSES: [i16; 1] = [895];
const SWING: f32 = f32::from_bits(0x3f32_b8c2);
const RATE: f32 = 12.566_371;

/// Level05 0x3166a0 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x10 { return; }
    let side = c::pi32(w, id, 0);
    match w.m(id).state {
        0 => {
            if side != 0 { w.mm(id).mode |= mode::MIRROR; }
            w.mm(id).state = 1;
            let yaw = w.m(id).rotation[2];
            c::set_pf(w, id, 0xc, yaw);
            let q = std::f32::consts::FRAC_PI_2;
            w.mm(id).rotation[2] = if side == 0 { c::add_rot(yaw, q) } else { c::sub_rot(yaw, q) };
        }
        1 => {}
        2 => {
            let base = c::pf(w, id, 0xc);
            let t = if side == 0 { c::add_rot(base, SWING) } else { c::sub_rot(base, SWING) };
            let mut yaw = w.m(id).rotation[2];
            let mut v = c::pf(w, id, 4);
            turn::turn_toward(t, DT2 * RATE, DT2 * RATE, DT * RATE, &mut yaw, &mut v);
            w.mm(id).rotation[2] = yaw;
            c::set_pf(w, id, 4, v);
            return;
        }
        _ => return,
    }
    let link = c::pi32(w, id, 8);
    if usize::try_from(link).ok().and_then(|l| w.table.mobys.get(l)).is_some_and(|m| m.cmd != 0) {
        w.play_sound(0, 0, id);
        w.mm(id).state = 2;
    }
}
