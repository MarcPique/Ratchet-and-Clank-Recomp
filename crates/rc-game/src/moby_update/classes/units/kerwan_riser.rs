//! Kerwan's riser, class 997 (level03 `0x2dca30`, 1 placed: #960 at (303.4, 244, 34.25), the only copy): a prop that
//! stays put until its own save bits say it is done (the level's "collected" byte of its uid, or its persistent death
//! bit), then rises 4 units at 1 unit/s and stays there. On a later visit the load pass finds the bits set and it rises
//! again from its placement. Read from the level03 disassembly (64 words).
//!
//! **Pvars** (0x10): +0x00 the start position.
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x2dca30` state 0 | pvar +0x00 = the position (+0x10); the collected byte `0x1bb784[uid]` (level03's copy of level01's 0x1bbb04) ≠ 0, or the death bit `0x14c190 + level·0x100 + (uid >> 5)·4` bit `uid & 31` → state 1 | [`update`] (`SaveBits::collected` / `SaveBits::death`) |
//! | state 1 | z += dt (0x15ed6c); start z + 4 ≤ z → state 2 | [`update`] |
//! | state 2 | z = start z + 4 | [`update`] |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, DT};
use crate::moby_update::services::World;

pub const UPDATE_FN: u32 = 0x2d_ca30;
pub const REFERENCE_LEVEL: u32 = 3;
pub const CLASSES: [i16; 1] = [997];
/// The pvar of the start position.
const START: usize = 0x00;
/// The rise.
const RISE: f32 = 4.0;

/// Level03 `0x2dca30` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < START + 0x10 { w.mm(id).pvars.resize(START + 0x10, 0); }
    match w.m(id).state {
        0 => {
            let p = w.m(id).position;
            c::set_pv4(w, id, START, p);
            let uid = w.m(id).spawn_id;
            let collected = w.svc.save.collected.get(&uid).is_some_and(|&b| b != 0);
            if collected || w.svc.save.death.contains(&(w.svc.level, uid)) { w.mm(id).state = 1; }
        }
        1 => {
            let top = c::pf(w, id, START + 8) + RISE;
            let m = w.mm(id);
            m.position[2] += DT;
            if top <= m.position[2] { m.state = 2; }
        }
        2 => {
            let top = c::pf(w, id, START + 8) + RISE;
            w.mm(id).position[2] = top;
        }
        _ => {}
    }
}
