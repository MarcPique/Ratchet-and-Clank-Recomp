//! U172 (census 2026-10-02): class 584, Eudora's grabbable blocks (level04 0x2d3580, the only copy: 3 placed). Read from
//! the level04 disassembly (no decomp file). Native `f32`.
//!
//! A carrier (mode 0x20) that reports its move each tick for whatever stands on it, with a pvar record whose ledge bit is
//! set so Ratchet can hang from it. It never moves by itself.
//!
//! | address | what | port |
//! |---|---|---|
//! | every tick | `CarryRiders(P+0x60, position − P+0xa0, Euler, Euler)` (`0x2211b8` = `VecSub`); P+0xa0 = position | [`update`] (`triggers::carry_riders`) |
//! | 0 | the pvar record (P+0x20): +0x00 s32 0, +0x04 s16 0, +0x08 byte 4, +0x1e s16 0xd (bit 0: a ledge, [`triggers::record_ledge_flag`]); Ratchet's light word and ambient (`FUN_00272078`); 1 | [`update`] |

use crate::moby_runtime::MobyId;
use crate::moby_update::services::{pvar as p, World};
use crate::moby_update::triggers;

pub const REFERENCE_LEVEL: u32 = 4;
pub const UPDATE_FN: u32 = 0x2d_3580;
pub const CLASSES: [i16; 1] = [584];

const RECORD: usize = 0x20;
const BLOCK: usize = 0x60;
const LAST: usize = 0xa0;
const SIZE: usize = 0xb0;

/// Level04 0x2d3580 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < SIZE { return; }
    {
        let m = w.mm(id);
        let last = p::v4f(&m.pvars, LAST);
        let pos = m.position;
        let d = [pos[0] - last[0], pos[1] - last[1], pos[2] - last[2], pos[3] - last[3]];
        let r = m.rotation;
        triggers::carry_riders(&mut m.pvars, BLOCK, d, r, r);
        p::set_v4f(&mut m.pvars, LAST, pos);
    }
    if w.m(id).state != 0 { return; }
    {
        let m = w.mm(id);
        p::set_i32(&mut m.pvars, RECORD, 0);
        p::set_i16(&mut m.pvars, RECORD + 4, 0);
        m.pvars[RECORD + 8] = 4;
        p::set_i16(&mut m.pvars, RECORD + 0x1e, 0xd);
    }
    if let Some(h) = w.hero_moby {
        let (l, a) = (w.m(h).light, w.m(h).ambient);
        let m = w.mm(id);
        m.light = l;
        m.ambient = a;
    }
    w.mm(id).state = 1;
}
