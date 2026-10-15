//! **The jets in Kalebo's first scene, class 1812** (level16 `0x2e7e00`, 1 placed; census U567): while scene 0 plays, it
//! gives the scene's actor 3 two jet glows: type-23 puffs at its joint lists 6 and 7, moving 0.075 along the lines from
//! lists 4 and 5. Level 16's copies of the scene globals sit 0x600 below level 18's (the scene 0x16cc90, the actors
//! 0x16cdd8: actor 3 is 0x16cde4), the pattern of Veldin's 1799 ([`super::veldin_scene_jet`]). Read from the level16
//! decomp. Native `f32`.
//!
//! | address | what | port |
//! |---|---|---|
//! | state 0 | → 1, +0x30 = 0xff | [`update`] |
//! | state 1 | game mode 2 (0x15f5c4) and scene 0x16cc90 = 0: actor 3 (0x16cde4); its joint lists 6 / 4 / 7 / 5 (`0x24aea0` = L01 `0x2645a8`); for (6, 4) and (7, 5): dir = `FastVecNormalize(0.075, a − b)`; `0x25ef60(20000, 1000, a, a, dir)` (L01 `0x278810`) | [`update`] (`creature::fx::jet_puffs`) |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, fx};
use crate::moby_update::services::World;

pub const REFERENCE_LEVEL: u32 = 16;
pub const UPDATE_FN: u32 = 0x2e_7e00;
pub const CLASSES: [i16; 1] = [1812];
/// The actor and the joint list pairs (jet, behind it).
const ACTOR: usize = 3;
const JETS: [(usize, usize); 2] = [(6, 4), (7, 5)];
/// 0x3d99999a.
const SPEED: f32 = f32::from_bits(0x3d99_999a);

/// Level16 `0x2e7e00` (module doc).
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
            if scene.id != 0 { return; }
            let Some(a) = scene.actors.get(ACTOR) else { return };
            for (j, b) in JETS {
                let (p, q) = (a.joint_point(j), a.joint_point(b));
                let d = c::set_len3(c::sub(p, q), SPEED);
                fx::jet_puffs(w, 20000.0, 1000.0, p, p, d);
            }
        }
        _ => {}
    }
}
