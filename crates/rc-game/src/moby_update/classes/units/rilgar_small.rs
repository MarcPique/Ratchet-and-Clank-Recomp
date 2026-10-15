//! Rilgar's small level classes (level 05; read from the level05 decomp and disassembly; the names are descriptive [L]):
//!
//! * **1099, the scene-hidden prop** (`0x319c28`, 1 placed; census U216): hidden while a scene plays (game mode 2),
//!   shown otherwise.
//! * **984, the bobbing marker** (`0x318b98`, 1 placed; U214): 2.3 times its class scale, bobbing ±1 about its placed
//!   height at 50°/s from a random phase, hidden during scenes.
//! * **1550, the scene FX driver** (`0x31d5c8`, 1 placed; U220): in scenes 10 and 11 the infobot's thrusters on actor 3
//!   (the pattern of Eudora's 1549, [`super::eudora_scene_fx`]).
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x319c28` | game mode (0x15f5c4) 2 → +0x31 = 0, mode \|= 1; else +0x31 = 1, mode &= ~1 | [`hidden_update`] |
//! | `0x318b98` | scale = class scale · 2.3 (gp−0x4cf8); state 0: → 1, +0x00 = `random_angle_radians`, +0x04 = z; state 1: hidden in scenes as 1099; +0x00 = `fast_add_rotations(+0x00, 50°·dt)`, z = +0x04 + sin(+0x00) | [`bob_update`] |
//! | `0x31d5c8` | state 0: → 1, +0x30 = 0xff; state 1: game mode 2 and the scene (0x16cc90) 10 or 11 → `0x28da08(actor 3)` (L01 `0x278450`) | [`scene_fx_update`] (`cutscene_fx::infobot_thrusters`) |

use crate::moby_runtime::MobyId;
use crate::moby_update::classes::cutscene_fx::infobot_thrusters;
use crate::moby_update::creature::{self as c, DT};
use crate::moby_update::services::World;

pub const REFERENCE_LEVEL: u32 = 5;
pub const HIDDEN_FN: u32 = 0x31_9c28;
pub const HIDDEN_CLASSES: [i16; 1] = [1099];
pub const BOB_FN: u32 = 0x31_8b98;
pub const BOB_CLASSES: [i16; 1] = [984];
pub const SCENE_FX_FN: u32 = 0x31_d5c8;
pub const SCENE_FX_CLASSES: [i16; 1] = [1550];
/// gp−0x4cf8: the marker's scale.
pub const BOB_SCALE: f32 = f32::from_bits(0x4013_3333);
const BOB_RATE: f32 = 0.872_664_63;

fn scene_hide(w: &mut World, id: MobyId) {
    let hide = w.svc.game_mode == 2;
    let m = w.mm(id);
    if hide {
        m.visible = 0;
        m.mode |= 1;
    } else {
        m.visible = 1;
        m.mode &= !1;
    }
}

/// Level05 `0x319c28` (module doc).
pub fn hidden_update(w: &mut World, id: MobyId) { scene_hide(w, id) }

/// Level05 `0x318b98` (module doc).
pub fn bob_update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 8 { w.mm(id).pvars.resize(8, 0); }
    let s = super::class_scale(w, w.m(id).o_class) * BOB_SCALE;
    w.mm(id).scale = s;
    match w.m(id).state {
        0 => {
            w.mm(id).state = 1;
            let a = w.rng.rand_angle();
            c::set_pf(w, id, 0, a);
            let z = w.m(id).position[2];
            c::set_pf(w, id, 4, z);
        }
        1 => {
            scene_hide(w, id);
            let ph = c::add_rot(c::pf(w, id, 0), DT * BOB_RATE);
            c::set_pf(w, id, 0, ph);
            let z = c::pf(w, id, 4) + ph.sin();
            w.mm(id).position[2] = z;
        }
        _ => {}
    }
}

/// Level05 `0x31d5c8` (module doc).
pub fn scene_fx_update(w: &mut World, id: MobyId) {
    match w.m(id).state {
        0 => {
            let m = w.mm(id);
            m.state = 1;
            m.update_dist = 0xff;
        }
        1 => {
            if w.svc.game_mode != 2 { return; }
            let Some(scene) = w.svc.cinematic.scene.clone() else { return };
            if !(10..=11).contains(&scene.id) { return; }
            if let Some(a) = scene.actors.get(3) { infobot_thrusters(w, a); }
        }
        _ => {}
    }
}
