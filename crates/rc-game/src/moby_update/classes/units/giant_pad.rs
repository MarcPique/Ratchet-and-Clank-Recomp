//! **Giant Clank's pad** (class 1451 on Quartu, level15 0x2ed068; class 1899 on Veldin 2, level18 0x2fb868: the same
//! code, census U500): the pad Ratchet stands on to climb into Giant Clank (`SwitchCharacter(2, 0x5a, giant)`,
//! `crate::hero::bodies`) and, on Quartu, to climb out (the level's leave copy 0x208ca8). Read from the level15 decomp.
//!
//! **Pvar block** (s32): +0x60 Giant Clank's moby (class 0x1a3), +0x68 (−1 written while nobody uses the pad).
//!
//! | address | what | port |
//! |---|---|---|
//! | state 0 | +0x60 = −1 → `DeleteMoby(self)`; Giant Clank hidden (+0x34 \|= 1), not drawn (+0x31 = 0), no collision (+0x94 = 0); → 3 on level 18 with its mission +0xb0 done, else → 1 | [`update`] |
//! | state 1 | → 2 | [`update`] |
//! | state 2 | Ratchet on the pad (0x13f64c = self), grounded, game mode ≠ 2, state ≠ 0x1d: on foot `try_set_help_message(4, 0x3aa4)`; as Giant Clank off level 18 `(4, 0x3aa5)`; △ (0x13cae4 & 0x10) with the prompt owner 4: | [`update`] |
//! | | Giant Clank on level 15: his moby without collision, not drawn, hidden; leave (0x208ca8); his anim cut to 0 (`fun_00212ed8`); the scene 9 (0x286788), `MusicRequestTrack(0, 5)` | [`update`] (`bodies::queue_leave`, `cinematic::start_scene`) |
//! | | on foot: his moby drawn (+0x31 = 1), collision on (+0x94 = class +0x10), shown; `SwitchCharacter(2, 0x5a, giant)`; level 15: the scene 8, the hero's position / Euler into 0x141050 / 0x141060, `MusicRequestTrack(2, 4)`; level 18: the scene 6, the pose saved, → 3 | [`update`] (`bodies::queue_switch`, `HeroFields::save_entry_pose`) |
//! | otherwise | +0x68 = −1 | [`update`] |
//! | | no sound, particle, light, HUD element of its own | n/a |

use crate::moby_runtime::MobyId;
use crate::moby_update::services::{pvar as p, World};

/// The update in the level15 class table (level18's 0x2fb868 for class 1899 is the same code).
pub const UPDATE_FN: u32 = 0x2e_d068;
pub const REFERENCE_LEVEL: u32 = 15;
pub const CLASSES: [i16; 2] = [1451, 1899];

/// The prompt owner and messages (`try_set_help_message(4, …)`).
const OWNER: i32 = 4;
const MSG_IN: i32 = 0x3aa4;
const MSG_OUT: i32 = 0x3aa5;

fn music(w: &mut World, track: i16, stinger: i16) {
    if let Some(s) = w.sound.as_deref_mut() { s.music_request(track, stinger); }
}

pub fn update(w: &mut World, id: MobyId) {
    use crate::hero::bodies::body;
    let giant = p::i32(&w.m(id).pvars, 0x60);
    let level = w.svc.level;
    match w.m(id).state {
        0 => {
            if giant == -1 {
                w.delete_moby(id);
                return;
            }
            let g = w.mm(giant as usize);
            g.visible = 0;
            g.has_collision = false;
            g.mode |= crate::moby_runtime::mode::HIDDEN;
            let done = crate::moby_update::classes::units::hints::mission_done(w, w.m(id).mission as i32);
            w.mm(id).state = if level == 18 && done { 3 } else { 1 };
        }
        1 => w.mm(id).state = 2,
        2 => {
            let h = w.hero;
            if h.ground_moby == Some(id) && h.air_ticks == 0 && w.svc.game_mode != 2 && h.state != 0x1d {
                let mode = h.mode;
                if mode == body::RATCHET {
                    w.svc.interact.try_prompt(OWNER, MSG_IN);
                } else if mode == body::GIANT && level != 18 {
                    w.svc.interact.try_prompt(OWNER, MSG_OUT);
                }
                if w.hero.loop_in.pad.pressed & crate::pad::button::TRIANGLE == 0 || w.svc.interact.prompt.owner != OWNER { return; }
                let gi = giant as usize;
                if mode == body::GIANT && level == 15 {
                    let g = w.mm(gi);
                    g.has_collision = false;
                    g.visible = 0;
                    g.mode |= crate::moby_runtime::mode::HIDDEN;
                    crate::hero::bodies::queue_leave(w);
                    crate::moby_update::creature::hard_cut(w, gi, 0, 0);
                    crate::cinematic::start_scene(w, 9, false);
                    music(w, 0, 5);
                    return;
                }
                if mode != body::RATCHET { return; }
                let col = w.classes.info(w.m(gi).o_class).is_some_and(|i| i.has_collision);
                let g = w.mm(gi);
                g.visible = 1;
                g.has_collision = col;
                g.mode &= !crate::moby_runtime::mode::HIDDEN;
                crate::hero::bodies::queue_switch(w, body::GIANT, crate::hero::bodies::giant::IDLE, gi);
                match level {
                    15 => {
                        crate::cinematic::start_scene(w, 8, false);
                        w.hero_fields_mut().save_entry_pose = true;
                        music(w, 2, 4);
                    }
                    18 => {
                        crate::cinematic::start_scene(w, 6, false);
                        w.hero_fields_mut().save_entry_pose = true;
                        w.mm(id).state = 3;
                    }
                    _ => {}
                }
                return;
            }
            p::set_i32(&mut w.mm(id).pvars, 0x68, -1);
        }
        _ => p::set_i32(&mut w.mm(id).pvars, 0x68, -1),
    }
}
