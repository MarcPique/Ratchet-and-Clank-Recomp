//! Quartu's one-way belts, class 1250 (level 15, 18 instances): level15 0x2e73c0 (census unit U479). Like the
//! conveyor belts of levels 02 / 12 (`super::conveyor`, a separate function with a reversal cycle) the belt does not
//! move: it carries whoever rides it (its platform block at pvar +0x60, flags +0x9c) by a velocity across its heading
//! at 2.5 units/s, `CarryRiders` every tick. Read from the level15 decomp and disassembly. Native `f32`.
//!
//! | address | what | port |
//! |---|---|---|
//! | 0x2e73c0 | no pvar block: nothing | [`update`] |
//! | state 0 | velocity (+0x40) = 0 (`0x1f89d8`), block flags +0x9c \|= 4 (a moving floor: `hero::platform`), → 3 | [`update`] |
//! | state 3 | anim speed +0x58 = 1.3888889 (0x3fb1c71d); velocity = 2.5·dt·(cos, sin, 0) of yaw + π/2 (`fast_add_rotations`, `fast_cos` / `fast_sin`) | [`update`] |
//! | every state | `CarryRiders(+0x60, +0x40, +0x40, +0x40)` (L01 0x2755f8) | `triggers::carry_riders` |
//! | | no sound, particle, light, save flag, other moby | n/a |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, DT};
use crate::moby_update::services::World;
use crate::moby_update::triggers;

pub const UPDATE_FN: u32 = 0x2e_73c0;
pub const REFERENCE_LEVEL: u32 = 15;
pub const CLASSES: [i16; 1] = [1250];
pub const VEL: usize = 0x40;
pub const BLOCK: usize = 0x60;
pub const SPEED: f32 = 2.5;

/// Level15 0x2e73c0 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < BLOCK + 0x40 { return; }
    match w.m(id).state {
        0 => {
            c::set_pv4(w, id, VEL, [0.0; 4]);
            let f = c::pi32(w, id, BLOCK + 0x3c) | 4;
            c::set_pi32(w, id, BLOCK + 0x3c, f);
            w.mm(id).state = 3;
        }
        3 => {
            w.mm(id).anim.speed = f32::from_bits(0x3fb1_c71d);
            let a = c::add_rot(w.m(id).rotation[2], std::f32::consts::FRAC_PI_2);
            let v = [a.cos() * DT * SPEED, a.sin() * DT * SPEED, 0.0, c::pv4(w, id, VEL)[3]];
            c::set_pv4(w, id, VEL, v);
        }
        _ => {}
    }
    let (v, r) = (c::pv4(w, id, VEL), w.m(id).rotation);
    triggers::carry_riders(&mut w.mm(id).pvars, BLOCK, v, r, r);
}
