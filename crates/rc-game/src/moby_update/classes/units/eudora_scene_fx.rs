//! **Eudora's cutscene FX driver, class 1549** (level04 `0x2e53a0`, 1 placed): level 04's counterpart of Kerwan's 1548
//! ([`super::kerwan_scene_fx`]). While a scene plays (game mode 2) it fires the infobot's thrusters at an actor: scene 0
//! (0x16ca10) actor 3, scene 1 actor 2 (0x16cb58 + 4·k). Read from the level04 decomp. Native `f32`.
//!
//! | address | what | port |
//! |---|---|---|
//! | state 0 | → 1, update distance 0xff | [`update`] |
//! | state 1 | game mode 2 and scene < 2: `FUN_002560e8(actor)` (L01 `0x278450`, the infobot's thrusters) on actor 3 (scene 0) / 2 (scene 1) | [`update`] (`cutscene_fx::infobot_thrusters`) |

use crate::moby_runtime::MobyId;
use crate::moby_update::classes::cutscene_fx::infobot_thrusters;
use crate::moby_update::services::World;

pub const REFERENCE_LEVEL: u32 = 4;
pub const UPDATE_FN: u32 = 0x2e_53a0;
pub const CLASSES: [i16; 1] = [1549];

/// Level04 `0x2e53a0` (module doc).
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
            let k = match scene.id { 0 => 3, 1 => 2, _ => return };
            if let Some(a) = scene.actors.get(k) { infobot_thrusters(w, a); }
        }
        _ => {}
    }
}
