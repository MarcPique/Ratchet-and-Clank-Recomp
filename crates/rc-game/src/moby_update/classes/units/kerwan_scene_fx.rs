//! **Kerwan's cutscene FX driver, class 1548** (level03 `0x2e01d0`, 1 placed: #988 by the landing pad): level 03's
//! counterpart of level 00's 1545 ([`super::veldin_scene_fx`]) and level 01's `CutsceneFxUpdate` 1546
//! ([`super::super::cutscene_fx`]), with Kerwan's scenes. While a scene plays (game mode 2) it fires the infobot's
//! thrusters at its actor (scene 4: actor 2; scene 10: actor 0) and, in scene 3 between ticks 900 and 10000, puffs two
//! grey smokes a tick from actor 3's joint lists 0 and 1. Read from the level03 disassembly (the decompile lost the
//! spawns' arguments). Native `f32`; the draws at the game's points.
//!
//! ## Coverage (`0x2e01d0`)
//! | address | what | port |
//! |---|---|---|
//! | state 0 | → 1, update distance 0xff | [`update`] |
//! | state 1 | not game mode 2 (0x15f5c4) → nothing | [`update`] |
//! | | scene 0x16c990 = 4 or 10: `FUN_00251a30(actor)` (L01 `0x278450`, the infobot's thrusters) on actor 2 (scene 4) / 0 (scene 10) (0x16cad8 + 4·k) | [`update`] (`cutscene_fx::infobot_thrusters`) |
//! | | scene 3, `ticks(900)` ≤ tick (0x16c994) ≤ `ticks(10000)`, actor 3 (0x16cae4): its joint lists 0 and 1 points (`0x23e338` = `0x2645a8`); per point `PartType22Spawn(randf(0.25, 0.5)·209920, p, 0, c1, 0x007f7f7f, ticks(rand_range(45, 60)))`, c1 0x7f7f7f7f (list 0) / 0x407f7f7f (list 1) | [`update`], [`smoke`] |

use crate::moby_runtime::MobyId;
use crate::moby_update::classes::cutscene_fx::infobot_thrusters;
use crate::moby_update::creature::projectile;
use crate::moby_update::services::World;
use crate::particles::type22;

pub const REFERENCE_LEVEL: u32 = 3;
pub const UPDATE_FN: u32 = 0x2e_01d0;
pub const CLASSES: [i16; 1] = [1548];
/// The smoke's size scale (0x484d1400).
const SIZE: f32 = f32::from_bits(0x484d_1400);

/// Level03 `0x2e01d0` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    match w.m(id).state {
        0 => {
            let m = w.mm(id);
            m.state = 1;
            m.update_dist = 0xff;
            return;
        }
        1 => {}
        _ => return,
    }
    if w.svc.game_mode != 2 { return; }
    let Some(scene) = w.svc.cinematic.scene.clone() else { return };
    let (sid, tick) = (scene.id as i32, scene.tick);
    if sid == 4 || sid == 10 {
        let k = if sid == 4 { 2 } else { 0 };
        if let Some(a) = scene.actors.get(k) { infobot_thrusters(w, a); }
    }
    if sid != 3 || tick < w.ticks(900) || w.ticks(10000) < tick { return; }
    let Some(a) = scene.actors.get(3) else { return };
    let (p0, p1) = (a.joint_point(0), a.joint_point(1));
    smoke(w, p0, 0x7f7f_7f7f);
    smoke(w, p1, 0x407f_7f7f);
}

/// One smoke puff of scene 3 (module table), its draws in the game's order.
fn smoke(w: &mut World, pos: [f32; 4], c1: u32) {
    let size = w.rng.randf(0.25, 0.5) * SIZE;
    let r = w.rng.rand_range(45, 60);
    let life = w.ticks(r);
    projectile::part22(w, &type22::Spawn { size, pos, vel: [0.0; 4], c1, c2: 0x007f_7f7f, life });
}
