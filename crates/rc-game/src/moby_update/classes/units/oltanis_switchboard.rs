//! Oltanis's state holders, class 1397 (level 14, 6 instances): level14 0x3061d8 (census unit U456). The class only
//! initialises and advances fields another moby reads: state 0 writes pvar +0x28 = 4, +0x3e = 0, +0x20 = 0, +0x24 = 0
//! and → 1; state 2 (set from outside) writes +0x3e = 5 and → 3. Read from the level14 disassembly (22 words). The
//! name is descriptive [L].
//!
//! | address | what | port |
//! |---|---|---|
//! | 0x3061d8 | state 0: u8 +0x28 = 4, s16 +0x3e = 0, i32 +0x20 = 0, s16 +0x24 = 0, → 1 | [`update`] |
//! | | state 2: s16 +0x3e = 5, → 3; other states: nothing | [`update`] |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature as c;
use crate::moby_update::services::World;

pub const UPDATE_FN: u32 = 0x30_61d8;
pub const REFERENCE_LEVEL: u32 = 14;
pub const CLASSES: [i16; 1] = [1397];

/// Level14 0x3061d8 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x40 { return; }
    match w.m(id).state {
        0 => {
            c::set_pu8(w, id, 0x28, 4);
            c::set_pi16(w, id, 0x3e, 0);
            c::set_pi32(w, id, 0x20, 0);
            c::set_pi16(w, id, 0x24, 0);
            w.mm(id).state = 1;
        }
        2 => {
            c::set_pi16(w, id, 0x3e, 5);
            w.mm(id).state = 3;
        }
        _ => {}
    }
}
