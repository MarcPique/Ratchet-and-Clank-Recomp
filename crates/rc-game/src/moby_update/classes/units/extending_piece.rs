//! Kalebo's extending pieces, class 647 (level 16, 16 instances): level16 0x2d59e0 (census unit U499). Set to state 1
//! by another moby, the piece slides out from its home along its rotation row 0 to the distance +0x14, easing
//! (`0x270830`), then stops (state 2). Read from the level16 decomp. The name is descriptive [L]. Native `f32`.
//!
//! | address | what | port |
//! |---|---|---|
//! | state 0 | home +0x00 = pos, → 2 | [`update`] |
//! | state 1 | pos = home; `0x270830(+0x14, 4·dt², 4·dt², 8·dt, &+0x18, &+0x10)`; pos += `FastVecNormalize(+0x18, rows[0])` (`VecAdd`); at the target → 2 | [`update`] (`creature::turn::spring`) |
//! | | no sound, particle, light, save flag, other moby | n/a |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, turn, DT, DT2};
use crate::moby_update::services::World;

pub const UPDATE_FN: u32 = 0x2d_59e0;
pub const REFERENCE_LEVEL: u32 = 16;
pub const CLASSES: [i16; 1] = [647];

/// Level16 0x2d59e0 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x1c { return; }
    match w.m(id).state {
        0 => {
            let p = c::pos(w, id);
            c::set_pv4(w, id, 0, p);
            w.mm(id).state = 2;
        }
        1 => {
            let home = c::pv4(w, id, 0);
            let target = c::pf(w, id, 0x14);
            let (mut x, mut v) = (c::pf(w, id, 0x18), c::pf(w, id, 0x10));
            turn::spring(target, DT2 * 4.0, DT2 * 4.0, DT * 8.0, &mut x, &mut v);
            c::set_pf(w, id, 0x18, x);
            c::set_pf(w, id, 0x10, v);
            let r = w.m(id).rows[0];
            let d = c::set_len3([r[0], r[1], r[2], 0.0], x);
            w.mm(id).position = c::add(home, d);
            if x == target { w.mm(id).state = 2; }
        }
        _ => {}
    }
}
