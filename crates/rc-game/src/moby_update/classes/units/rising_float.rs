//! Kalebo's rising floats, class 650 (level 16, 29 instances): level16 0x2d5cb8 (census unit U500). While its timer
//! (+0x08, set by another moby) runs the float rises 3 units over its rest height, easing there and back
//! (`0x270830`); the tick the timer ends it plays class sound 1, at most once every 4 ticks for all floats (the
//! level word `$gp` 0x161a88 holds the last tick). Read from the level16 decomp. The name is descriptive [L].
//! Native `f32`.
//!
//! | address | what | port |
//! |---|---|---|
//! | state 0 | +0x00 = z (the rest height), → 1 | [`update`] |
//! | state 1 | `FastDecTimer__FRi(+0x08)`: running (0) → target rest + 3 (`$gp` 0x161a84); just ended (2) → target rest, and when `|0x161a88 − tick| ≥ 4` (0x221110): 0x161a88 = tick, `PlayClassSound(1, 0)`; idle (1) → target rest | [`update`] |
//! | | `0x270830(target, 10·dt², 10·dt², 10·dt, &z, &+0x04)` | `creature::turn::spring` |
//! | | no particle, light, save flag, other moby | n/a |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, turn, DT, DT2};
use crate::moby_update::services::World;

pub const UPDATE_FN: u32 = 0x2d_5cb8;
pub const REFERENCE_LEVEL: u32 = 16;
pub const CLASSES: [i16; 1] = [650];
/// `$gp` 0x161a84: the rise.
pub const RISE: f32 = 3.0;
/// The level word holding the tick of the last sound (`$gp` 0x161a88).
pub const LAST_SOUND: u32 = 0x16_1a88;

/// Level16 0x2d5cb8 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x0c { return; }
    match w.m(id).state {
        0 => {
            w.mm(id).state = 1;
            let z = w.m(id).position[2];
            c::set_pf(w, id, 0, z);
        }
        1 => {
            let rest = c::pf(w, id, 0);
            let target = match c::dec_timer_pvar_i32(w, id, 8) {
                0 => rest + RISE,
                2 => {
                    let tick = w.counter as i32;
                    let last = w.svc.units.word(LAST_SOUND) as i32;
                    if (last.wrapping_sub(tick)).wrapping_abs() >= 4 {
                        w.svc.units.set_word(LAST_SOUND, tick as u32);
                        w.play_sound(1, 0, id);
                    }
                    rest
                }
                _ => rest,
            };
            let (mut z, mut v) = (w.m(id).position[2], c::pf(w, id, 4));
            turn::spring(target, DT2 * 10.0, DT2 * 10.0, DT * 10.0, &mut z, &mut v);
            w.mm(id).position[2] = z;
            c::set_pf(w, id, 4, v);
        }
        _ => {}
    }
}
