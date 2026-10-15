//! **Kalebo's grind skill point, class 1561** (level16 `0x2e7208`, 1 placed; census U565): grinding into its first
//! cuboid arms it, and reaching its second cuboid without leaving the rail on the way (state 0x42 disarms it) earns
//! skill point 0x13d421. Read from the level16 decomp.
//!
//! **Pvars**: +0x00 / +0x04 the cuboids, +0x08 armed.
//!
//! | address | what | port |
//! |---|---|---|
//! | armed 0 | Ratchet (0x13f3d0) in cuboid +0x00 (`PointInCuboid` 0x25b258 = L01 0x274820) and grinding (group 0x1413dc = 0xf) → armed | [`update`] |
//! | armed 1 | Ratchet in state 0x42 → 0; else in cuboid +0x04 and the skill point byte 0x13d421 clear → set, `PlayLevelSoundAtMoby(1, 0, 0)`, `ShowBanner(0x53d6, −1)` | [`update`] (`story::award_skill_point`) |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature as c;
use crate::moby_update::services::World;
use crate::moby_update::story::{award_skill_point, skill_index};

pub const REFERENCE_LEVEL: u32 = 16;
pub const UPDATE_FN: u32 = 0x2e_7208;
pub const CLASSES: [i16; 1] = [1561];
/// The grind group and the state that disarms it.
pub const GRINDING: i32 = 0xf;
pub const DISARM: i32 = 0x42;

/// Level16 `0x2e7208` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0xc { return; }
    let p = w.hero_point();
    match c::pi32(w, id, 8) {
        0 => {
            if w.in_cuboid(p, c::pi32(w, id, 0)) && w.hero.group == GRINDING { c::set_pi32(w, id, 8, 1); }
        }
        1 => {
            if w.hero.state == DISARM {
                c::set_pi32(w, id, 8, 0);
            } else if w.in_cuboid(p, c::pi32(w, id, 4)) {
                award_skill_point(w, skill_index(0x13_d421));
            }
        }
        _ => {}
    }
}
