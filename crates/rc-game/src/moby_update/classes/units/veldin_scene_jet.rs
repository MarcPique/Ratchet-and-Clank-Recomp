//! **The boss's jet in the scenes of Veldin's last level, class 1799** (level18 `0x2fad28`, census U602; one placed,
//! #970). While scenes 0..3 play, it gives the scenes' copy of the boss (actor 4 in scene 1, actor 3 in the others)
//! the jet glow the boss 1422 draws in play (`units::veldin_boss`): type-23 puffs at the actor's joint list 8,
//! moving 0.1 along the line from list 9 to list 8. Read from the level18 decomp. Native `f32`.
//!
//! ## Coverage (`0x2fad28`)
//! | address | what | port |
//! |---|---|---|
//! | state 0 | → 1, +0x30 = 0xff | [`update`] |
//! | state 1 | game mode 2 (0x15f5c4) and scene 0x16d290 < 4: actor 0x16d3d8[3] (scenes 0, 2, 3) / [4] (scene 1); its joint lists 8 / 9 (`0x251ab0` = L01 `0x2645a8`); dir = `FastVecNormalize(0.1, j8 − j9)` (gp−0x4688); `0x266140(600000, 0, j8, j8, dir)` (gp−0x4684 / −0x4680; L01 `0x278810`) | [`update`] (`creature::fx::jet_puffs`) |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, fx};
use crate::moby_update::services::World;

pub const REFERENCE_LEVEL: u32 = 18;
pub const UPDATE_FN: u32 = 0x2f_ad28;
pub const CLASSES: [i16; 1] = [1799];

/// Level18 `0x2fad28` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    match w.m(id).state {
        0 => {
            let m = w.mm(id);
            m.state = 1;
            m.update_dist = 0xff;
        }
        1 => {
            if w.svc.game_mode != 2 { return; }
            let Some(scene) = w.svc.cinematic.scene.clone() else { return };
            let sid = scene.id as i32;
            if !(sid < 4) { return; }
            let k = if sid == 1 { 4 } else { 3 };
            let Some(a) = scene.actors.get(k) else { return };
            let (j8, j9) = (a.joint_point(8), a.joint_point(9));
            let d = c::set_len3(c::sub(j8, j9), 0.1);
            fx::jet_puffs(w, 600000.0, 0.0, j8, j8, d);
        }
        _ => {}
    }
}
