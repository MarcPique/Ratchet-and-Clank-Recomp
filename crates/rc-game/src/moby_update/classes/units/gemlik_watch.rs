//! Gemlik's watchers, class 1577 (level 13, 4 created instances): level13 0x30be60 (census U436). A hidden counter that
//! deletes itself once fewer than its required number of four linked mobys are still alive (the moby it gates, e.g.
//! a barrier, is the 1577 itself). Read from the level13 decomp of 0x30be60. Native.
//!
//! **Pvar block**: +0x00 s32 the required count, +0x10..+0x1f four s32 moby indices (−1 none), +0x30..+0x3f their
//! classes as seen at the init (−1 gone).
//!
//! | address | what | port |
//! |---|---|---|
//! | state 0 | → 1; each link: deleted (state 0xfe / 0xfd) → class −1, else its class (the game reads the moby at index −1 too: [L] −1); required 0 → state 2 (inert) | [`update`] |
//! | state 1 | each link: −1, class −1, a different class now, or deleted → class −1; else counted; count < required → `DeleteMoby(m)` | [`update`] |
//! | | no sound, particle, flag | n/a |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature as c;
use crate::moby_update::services::World;

/// The update in the level13 class table.
pub const UPDATE_FN: u32 = 0x30_be60;
pub const REFERENCE_LEVEL: u32 = 13;
pub const CLASSES: [i16; 1] = [1577];

fn live(w: &World, idx: i32) -> Option<i16> {
    let m = w.table.mobys.get(usize::try_from(idx).ok()?)?;
    (m.state != 0xfe && m.state != 0xfd).then_some(m.o_class)
}

/// Level13 0x30be60 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x40 { return; }
    match w.m(id).state {
        0 => {
            w.mm(id).state = 1;
            for k in 0..4 {
                let idx = c::pi32(w, id, 0x10 + 4 * k);
                let cls = live(w, idx).map_or(-1, |c| c as i32);
                c::set_pi32(w, id, 0x30 + 4 * k, cls);
            }
            if c::pi32(w, id, 0) == 0 { w.mm(id).state = 2; }
        }
        1 => {
            let mut n = 0;
            for k in 0..4 {
                let (idx, cls) = (c::pi32(w, id, 0x10 + 4 * k), c::pi32(w, id, 0x30 + 4 * k));
                let alive = idx != -1 && cls != -1 && live(w, idx).is_some_and(|c| c as i32 == cls);
                if alive { n += 1; } else { c::set_pi32(w, id, 0x30 + 4 * k, -1); }
            }
            if n < c::pi32(w, id, 0) { w.delete_moby(id); }
        }
        _ => {}
    }
}
