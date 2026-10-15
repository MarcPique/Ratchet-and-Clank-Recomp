//! The Veldin help director, class 1413 (census U32; level00 `0x2e0988`, one instance, no geometry). Read from the
//! level00 decomp; all of it is the director's own code (no other level has a copy). Records: `H[n]` = help record n
//! (0x141968 + 8n), `M[n]` = move record n (0x141848 + 8n).
//!
//! **Pvars** (P, s32): +0x00 / +0x04 / +0x08 / +0x10 cuboids, +0x14..+0x20 four moby indices (the nanotech crates the
//! first-aid hints watch; −1 none).
//!
//! | address | what | port |
//! |---|---|---|
//! | state 0 | update distance 0xff, → 1 (and the tests below run the same tick) | [`update`] |
//! | `H[3]` count 0 | `Help_Request(3, 3)` (the welcome) | [`update`] |
//! | `H[0x74]` 0, in cuboid +0x10 | `Help_Request(0x4e2a, 0x74)` | [`update`] |
//! | `H[9]` 0, in cuboid +0x00, `M[1]` 0 | `Help_Request(5, 9)` | [`update`] |
//! | `H[8]` 0, `M[0]` 0, in cuboid +0x04 | `Help_Request(4, 8)` | [`update`] |
//! | `H[0]` 0, in cuboid +0x08 | `Help_Request(0, 0)` | [`update`] |
//! | `H[1]` and `H[2]` 0: each of the four mobys (≠ −1) deleted (state 0xfe / 0xfd) | health 0x1415f8 < 4 → `Help_Request(1, 1)`, else `(2, 2)` | [`update`] |
//! | the idle / group tests | none: `Help_Request` refuses while a box is up | [`crate::help::Help::request`] |
//! | no sound, particle, stat bump, flag, save, other moby written | | n/a |

use super::hints::{gone, in_cuboid, pvi, request};
use crate::moby_runtime::MobyId;
use crate::moby_update::services::World;

pub const UPDATE_FN: u32 = 0x2e_0988;
pub const REFERENCE_LEVEL: u32 = 0;
pub const CLASSES: [i16; 1] = [1413];

/// Pvar offsets.
pub mod pv {
    pub const HOUSE: usize = 0x00;
    pub const SHIP: usize = 0x04;
    pub const BOLTS: usize = 0x08;
    pub const INFO: usize = 0x10;
    pub const CRATES: usize = 0x14;
}

/// Level00 0x2e0988 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).state == 0 {
        let m = w.mm(id);
        m.update_dist = 0xff;
        m.state = 1;
    }
    let (h, mv) = (|w: &World, r: usize| w.svc.help.records.help[r].count, |w: &World, r: usize| w.svc.help.records.moves[r].count);
    if h(w, 3) == 0 { request(w, 3, 3); }
    if h(w, 0x74) == 0 && in_cuboid(w, id, pv::INFO) { request(w, 0x4e2a, 0x74); }
    if h(w, 9) == 0 && in_cuboid(w, id, pv::HOUSE) && mv(w, 1) == 0 { request(w, 5, 9); }
    if h(w, 8) == 0 && mv(w, 0) == 0 && in_cuboid(w, id, pv::SHIP) { request(w, 4, 8); }
    if h(w, 0) == 0 && in_cuboid(w, id, pv::BOLTS) { request(w, 0, 0); }
    if h(w, 1) == 0 && h(w, 2) == 0 {
        for k in 0..4 {
            let c = pvi(w, id, pv::CRATES + 4 * k);
            if c != -1 && gone(w, c) {
                if w.hero.health < 4 { request(w, 1, 1) } else { request(w, 2, 2) }
            }
        }
    }
}
