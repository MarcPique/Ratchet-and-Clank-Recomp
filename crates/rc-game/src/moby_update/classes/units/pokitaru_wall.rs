//! **Pokitaru's spline wall, class 361** (level11 `0x2f3350`, 1 placed; census U375): every tick it names its spline
//! (pvar+0x00, spline 57: a ring round the island at z 223) as the wall the hero's capsule pass keeps Ratchet off
//! (`0x14162a`, [`crate::hero::physics::spline_wall`]), while Ratchet is above z 194 and not in state 0x32. Read from
//! the level11 decomp.
//!
//! | address | what | port |
//! |---|---|---|
//! | state 0 | update distance +0x30 = 0xff, → 1; pvar+0x00 = −1 → `DeleteMoby` | [`update`] |
//! | state 1 | Ratchet's z (0x13f3d8) > 194 and his state (0x1413d4) ≠ 0x32 → 0x14162a = pvar+0x00 (u16) | [`update`] (`HeroFields::wall_spline`) |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature as c;
use crate::moby_update::services::World;

pub const REFERENCE_LEVEL: u32 = 11;
pub const UPDATE_FN: u32 = 0x2f_3350;
pub const CLASSES: [i16; 1] = [361];
/// Ratchet's height above which the wall holds (the literal 194.0).
pub const MIN_Z: f32 = 194.0;

/// Level11 `0x2f3350` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 4 { return; }
    let spline = c::pi32(w, id, 0);
    match w.m(id).state {
        0 => {
            let m = w.mm(id);
            m.update_dist = 0xff;
            m.state = 1;
            if spline == -1 { w.delete_moby(id); }
        }
        1 if MIN_Z < w.hero.pos[2].to_f32() && w.hero.state != 0x32 => w.hero_fields_mut().wall_spline = Some(spline as i16),
        _ => {}
    }
}
