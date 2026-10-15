//! Batalia's linked lifts, classes 467 / 472 (level 08, 4 created instances): level08 0x2ea398 (census U283). A moby
//! whose height follows its linked moby's progress (pvar +0x00 of the link) between its placed height and a top, at
//! most 10 units/s; when the link is one of the classes 615 / 575 instead, it waits for that moby's state 4, then sets
//! global flag 0x35 (`0x13d3bd`), plays class sound 0 and rises to the top. Read from the level08 decomp of 0x2ea398.
//! Native `f32`.
//!
//! **Pvar block**: +0x00 the top z, +0x04 the base z, +0x08 s32 the linked moby.
//!
//! | address | what | port |
//! |---|---|---|
//! | state 0 | +0x04 = z; → 1 | [`update`] |
//! | state 1 | link ≥ 0: class 0x267 / 0x23f: its state 4 → `0x13d3bd` = 1 (global flag 0x35), `PlayClassSound(0, 0, m)`, → 2 | [`update`] (`interact::set_global_flag`, `World::play_sound`) |
//! | | other class: `Approach((top − base)·link pvar +0x00 + base, 10·dt, &z)` (0x265d58 = L01 0x270728) | [`update`] (`turn::approach`) |
//! | state 2 | `Approach(top, 10·dt, &z)` | [`update`] |
//! | | no particle, hit | n/a |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, turn::approach, DT};
use crate::moby_update::interact::set_global_flag;
use crate::moby_update::services::{pvar as p, World};

/// The update in the level08 class table.
pub const UPDATE_FN: u32 = 0x2e_a398;
pub const REFERENCE_LEVEL: u32 = 8;
pub const CLASSES: [i16; 2] = [467, 472];
/// The classes whose state 4 trips it.
pub const TRIP_CLASSES: [i16; 2] = [0x267, 0x23f];
/// `0x13d3bd − 0x13d388`.
pub const FLAG: usize = 0x35;

/// Level08 0x2ea398 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0xc { return; }
    match w.m(id).state {
        0 => {
            let z = w.m(id).position[2];
            c::set_pf(w, id, 4, z);
            w.mm(id).state = 1;
        }
        1 => {
            let Some(l) = usize::try_from(c::pi32(w, id, 8)).ok().filter(|&l| l < w.table.mobys.len()) else { return };
            let (cls, st) = (w.m(l).o_class, w.m(l).state);
            if TRIP_CLASSES.contains(&cls) {
                if st == 4 {
                    set_global_flag(w, FLAG, 1);
                    w.play_sound(0, 0, id);
                    w.mm(id).state = 2;
                }
            } else {
                let t = if w.m(l).pvars.len() >= 4 { p::ff(&w.m(l).pvars, 0) } else { 0.0 };
                let (top, base) = (c::pf(w, id, 0), c::pf(w, id, 4));
                let mut z = w.m(id).position[2];
                approach((top - base) * t + base, DT * 10.0, &mut z);
                w.mm(id).position[2] = z;
            }
        }
        2 => {
            let top = c::pf(w, id, 0);
            let mut z = w.m(id).position[2];
            approach(top, DT * 10.0, &mut z);
            w.mm(id).position[2] = z;
        }
        _ => {}
    }
}
