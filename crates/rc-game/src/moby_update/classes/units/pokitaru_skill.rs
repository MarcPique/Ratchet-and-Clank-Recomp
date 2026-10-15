//! **Pokitaru's skill-point watcher, class 1350** (level11 `0x31aa80`, 1 placed; census U401): once every moby of its
//! group (pvar+0x04) is gone, skill point 0x13d41b is earned. Read from the level11 decomp.
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x31aa80` | pvar+0x00 = −1 → `DeleteMoby`; else group pvar+0x04 ≠ −1, `MobyGroupCount(group, −1)` (0x27f058 = L01 0x26e008) = 0 and the skill point byte 0x13d41b clear → set, `PlayLevelSoundAtMoby(1, 0, 0)`, `ShowBanner(0x53d6, −1)` | [`update`] (`story::award_skill_point`) |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature as c;
use crate::moby_update::scheduler::group_count;
use crate::moby_update::services::World;
use crate::moby_update::story::{award_skill_point, skill_index};

pub const REFERENCE_LEVEL: u32 = 11;
pub const UPDATE_FN: u32 = 0x31_aa80;
pub const CLASSES: [i16; 1] = [1350];

/// Level11 `0x31aa80` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 8 { return; }
    if c::pi32(w, id, 0) == -1 {
        w.delete_moby(id);
        return;
    }
    let g = c::pi32(w, id, 4);
    if g != -1 && group_count(w, g, -1) == 0 {
        award_skill_point(w, skill_index(0x13_d41b));
    }
}
