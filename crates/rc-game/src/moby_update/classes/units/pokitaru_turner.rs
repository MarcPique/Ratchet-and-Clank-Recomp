//! **The button-turned piece, class 1180** (level11 `0x30f2a8`, 1 placed; census U390): once its floor button (the
//! moby of pvar+0x00, a 1179: [`super::pokitaru_button`]) is pressed it turns 60° about z. Pressed during play it sets
//! flag 91, plays its sound and turns on a spring; already pressed at the load (the button restored in its first
//! ticks) it stands turned at once. Read from the level11 disassembly.
//!
//! **Pvars**: +0x00 the button's moby index, +0x04 the turn's angular velocity, +0x08 the placed yaw.
//!
//! | address | what | port |
//! |---|---|---|
//! | state 0 | +0x08 = yaw (+0x48); update distance 0xff; → 1 | [`update`] |
//! | state 1 | `0x30f270(button)`: class 0x49b in state 2 → the tick counter (0x15f5cc) ≥ 11: flag 91 (0x13d3e3) = 1, → 2, `PlayClassSound(0, 0)`; else → 3, yaw = `fast_add_rotations(+0x08, π/3)` | [`update`] |
//! | state 2 | `0x281ca0(+0x08 + π/3, π/9·dt², π/9·dt², π/4·dt, &yaw, &+0x04)` (L01 0x270cc0) returns 0 → 3 | [`update`] (`turn::turn_toward`) |
//! | state 3 | nothing | n/a |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, turn::turn_toward, DT, DT2};
use crate::moby_update::services::World;

pub const REFERENCE_LEVEL: u32 = 11;
pub const UPDATE_FN: u32 = 0x30_f2a8;
pub const CLASSES: [i16; 1] = [1180];
/// The button class (0x49b).
pub const BUTTON: i16 = 1179;
/// The flag set when the button turns it in play (0x13d3e3).
pub const FLAG: usize = 91;
/// The turn (0x3f860a92 = π/3).
const TURN: f32 = f32::from_bits(0x3f86_0a92);
/// The spring's rate (π/9) and top speed (π/4).
const RATE: f32 = f32::from_bits(0x3eb2_b8c2);
const VMAX: f32 = f32::from_bits(0x3f49_0fdb);

/// `0x30f270(m)`: moby `m` is a 1179 in state 2.
fn pressed(w: &World, idx: i32) -> bool {
    usize::try_from(idx).ok().and_then(|i| w.table.mobys.get(i)).is_some_and(|m| m.o_class == BUTTON && m.state == 2)
}

/// Level11 `0x30f2a8` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0xc { return; }
    match w.m(id).state {
        0 => {
            let yaw = w.m(id).rotation[2];
            c::set_pf(w, id, 8, yaw);
            let m = w.mm(id);
            m.update_dist = 0xff;
            m.state = 1;
        }
        1 => {
            if !pressed(w, c::pi32(w, id, 0)) { return; }
            if 11 <= w.counter as i32 {
                crate::moby_update::interact::set_global_flag(w, FLAG, 1);
                w.mm(id).state = 2;
                w.play_sound(0, 0, id);
            } else {
                w.mm(id).state = 3;
                let yaw = c::add_rot(c::pf(w, id, 8), TURN);
                w.mm(id).rotation[2] = yaw;
            }
        }
        2 => {
            let target = c::add_rot(c::pf(w, id, 8), TURN);
            let (mut a, mut v) = (w.m(id).rotation[2], c::pf(w, id, 4));
            let step = turn_toward(target, RATE * DT2, RATE * DT2, VMAX * DT, &mut a, &mut v);
            w.mm(id).rotation[2] = a;
            c::set_pf(w, id, 4, v);
            if step == 0.0 { w.mm(id).state = 3; }
        }
        _ => {}
    }
}
