//! Quartu's cogs, class 937 (level 15, 48 instances): level15 0x2e46b0 (census unit U477). A cog turns (yaw) while
//! the moby it is linked to moves: that moby in state 2 turns it one way and in state 4 the other, the direction
//! flipped by that moby's pvar word +0x10. Read from the level15 decomp and disassembly. The name is descriptive [L].
//! Native `f32`.
//!
//! **Pvar block**: +0x00 i32 the linked moby (index, −1 none). The rate is the level's `$gp` word 0x161ec0, −180 °/s.
//!
//! | address | what it does | port |
//! |---|---|---|
//! | 0x2e46b0 | P[0] = −1: nothing | [`update`] |
//! | | linked state 2: rate = −180°·dt, negated when its +0x10 ≠ 0; state 4: negated when its +0x10 = 0; other states: nothing | [`update`] |
//! | | yaw (+0x48) = `fast_add_rotations(yaw, rate)` | [`update`] |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, DT};
use crate::moby_update::services::{pvar, World};

pub const UPDATE_FN: u32 = 0x2e_46b0;
pub const REFERENCE_LEVEL: u32 = 15;
pub const CLASSES: [i16; 1] = [937];
/// `$gp` 0x161ec0 (°/s).
pub const RATE: f32 = -180.0;

/// Level15 0x2e46b0 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 4 { return; }
    let l = c::pi32(w, id, 0);
    if l == -1 { return; }
    let Some(m) = w.table.mobys.get(l as usize) else { return };
    let flag = if m.pvars.len() >= 0x14 { pvar::i32(&m.pvars, 0x10) } else { 0 };
    let base = RATE * 0.017_453_292 * DT;
    let rate = match m.state {
        2 => if flag != 0 { -base } else { base },
        4 => if flag == 0 { -base } else { base },
        _ => return,
    };
    let me = w.mm(id);
    me.rotation[2] = c::add_rot(me.rotation[2], rate);
}
