//! **Hoven's story NPCs** (level 12; read from the level12 decomp):
//!
//! * **282** (`0x2e6c48`, census U394, one instance; talk block +0x20): talkable while global flag 1 is clear; after
//!   his talk: flag 1 set, banner 12007 for `ticks(300)`, flag 0x61 = 1, his mission, the checkpoint at cuboid
//!   +0x170, a save. Gone once flag 1 is set. He carries a prop (class 0x11f) on his joint 2.
//! * **328 the Hydro-Pack giver** (`0x2eb570`, U398, one instance; talk block +0x20): talkable until the Hydro-Pack
//!   (item 4) is owned. Every talk sets his mission and the checkpoint at cuboid +0x160; node 2 gives the Hydro-Pack
//!   (not equipped), banner 12006, the checkpoint again and a save. Before he was first talked to (talk slot 70) he
//!   holds a pose (seq 1) with two props (classes 0x4c1, 0x196) at his spot.
//!
//! * **1404 the scene triggers** (`0x3093c8`, U411, two instances): once per global flag 0x5c + P[1], Ratchet in
//!   cuboid P[0] starts scene P[2] and is put back where he stands when it ends; gone once the flag is set.
//! * **1557 the scene thrusters** (`0x3094a0`, U445, one instance): updated always (+0x30 0xff); in a scene (game
//!   mode 2) 1 or 7, the thrusters (`0x27bf50` = L01 `0x278450`) on actor 2 (scene 1) or 0 (scene 7).
//!
//! **System or not.** Per-class code from the NPC template (look-at layout rows, `story_npc::attach_child`: the
//! level's child-on-joint helper `0x27b9c0`), the talk system, `story`.
//!
//! ## 282 coverage (level12 `0x2e6c48`)
//!
//! | address | what | status |
//! |---|---|---|
//! | top | `0x2e6b60` (the big-head scene manipulator) | `manip::scene_big_head` |
//! | state 0 | flag 1 (0x13d389) set → `DeleteMoby`; → 1, `NpcTalkRegister(+0x20)`, the shadow slab (`0x272fe0`) | [`merchant_update`] |
//! | state 1 | flag 1 clear: `NpcTalkUpdate(+0x20)` → `PlaceAfterScene(2.5)`, anim speed 0, → 2 | [`merchant_update`] |
//! | state 2 | game mode ≠ 2: anim speed 1, flag 1 = 1, `ShowBanner(12007, ticks(300))`, flag 0x61 (0x13d3e9) = 1, `SetMissionDone(+0xb0)`, the checkpoint at cuboid +0x170 (−1: none), `memcard_Save`, → 1 | [`merchant_update`] |
//! | tail | the look-at (records +0x60 list 1 / +0xe0 list 0, glance +0x160, s32 +0x178 / +0x17c, pitch × 1, yaw 0 / 0.7) | [`talking_npc::look_at_layout`] |
//! | tail | the prop +0x180: none → `CreateMoby(0x11f)` at his position / rotation, his draw distance, drawn, its matrix; else `0x27b9c0(m, prop, 2)` | [`merchant_update`] (`story_npc::attach_child`) |
//!
//! ## 328 coverage (level12 `0x2eb570`)
//!
//! | address | what | status |
//! |---|---|---|
//! | top | `0x2eb498` (the big-head scene manipulator) | `manip::scene_big_head` |
//! | top | talk slot 70 not talked to (0x13da1c): seq 1 (`MobySetSequence(1, 0, 2)`) unless on it; talked: seq ≠ 0 → seq 0 | [`hydro_update`] |
//! | state 0 | → 1, `NpcTalkRegister(+0x20)`, radius 1.7, the shadow slab; Hydro-Pack owned (0x13d4c4) → node −1 | [`hydro_update`] |
//! | state 1 | not owned: `NpcTalkUpdate` → `PlaceAfterScene(2.52)`, anim speed 0, → 2 | [`hydro_update`] |
//! | state 2 | game mode ≠ 2: anim speed 1, `SetMissionDone(+0xb0)`, the checkpoint at cuboid +0x160, → 1; node played 2: `GiveItem(4, 0)`, `ShowBanner(12006, −1)`, the checkpoint again, `memcard_Save` | [`hydro_update`] |
//! | tail | the look-at targets (records +0x60 list 1 / +0xe0 list 0, glance +0x170, s32 +0x184 / +0x188, yaw 0.5 / 0.5) | [`talking_npc::look_at_layout`] |
//! | tail | talked: the props +0x168 / +0x164 deleted; not talked: prop +0x164 (`CreateMoby(0x4c1)` at his spot, its matrix, mode 0, seq 1 (`MobySetSequence(1, 0, 2)`)) and +0x168 (`CreateMoby(0x196)`, his draw distance, drawn, his spot, its matrix); both hidden / undrawn in game mode 2, shown / drawn otherwise | [`hydro_update`] |

//!
//! ## 1404 coverage (level12 `0x3093c8`)
//!
//! | address | what | status |
//! |---|---|---|
//! | tick | tick > 0 and game mode 0: flag 0x13d3e4 + P[1] set → `DeleteMoby` (no argument: its own pointer [L]); clear and Ratchet in cuboid P[0]: the flag set, the scene-end place = Ratchet's x / y with his ground z (0x13f628) and his Euler (0x16cd26 = 1), `DialogStreamStart(P[2])` | [`scene_trigger_update`] |

use super::story_npc;
use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::classes::talking_npc::{look_at_layout, LookLayout};
use crate::moby_update::interact::{self, talk};
use crate::moby_update::services::{pvar as p, World};
use crate::moby_update::story;

pub const REFERENCE_LEVEL: u32 = 12;
pub const MERCHANT_FN: u32 = 0x2e_6c48;
pub const MERCHANT_CLASSES: [i16; 1] = [282];
pub const HYDRO_FN: u32 = 0x2e_b570;
pub const HYDRO_CLASSES: [i16; 1] = [328];
pub const SCENE_FN: u32 = 0x30_93c8;
pub const SCENE_CLASSES: [i16; 1] = [1404];
pub const THRUSTERS_FN: u32 = 0x30_94a0;
pub const THRUSTERS_CLASSES: [i16; 1] = [1557];
/// 0x13d3e4: the scene triggers' flags (global flag 0x5c + P[1]).
const SCENE_FLAGS: usize = story::flag_index(0x13_d3e4);

const T: usize = 0x20;
const MERCHANT_LOOK: LookLayout = LookLayout { pitch: (0x60, 1), yaw: (0xe0, 0), glance: 0x160, seen: 0x178, glance_timer: 0x17c, gate_seq_b: true, eye: 1.0, pitch_k: 1.0, yaw_a: 0.0, yaw_b: 0.7, short_timers: false, gate_main: 0, gate_alt: 0xff, k_seen: 0.04 };
const HYDRO_LOOK: LookLayout = LookLayout { pitch: (0x60, 1), yaw: (0xe0, 0), glance: 0x170, seen: 0x184, glance_timer: 0x188, gate_seq_b: true, eye: 1.0, pitch_k: 1.0, yaw_a: 0.5, yaw_b: 0.5, short_timers: false, gate_main: 0, gate_alt: 0xff, k_seen: 0.04 };

/// A prop at the NPC's spot (`CreateMoby(class)`, position and Euler copied, its matrix); None when no slot.
fn prop_here(w: &mut World, id: MobyId, class: i16) -> Option<MobyId> {
    let q = w.create_moby(class)?;
    let (pos, rot) = (w.m(id).position, w.m(id).rotation);
    let m = w.mm(q);
    m.position = pos;
    m.rotation = rot;
    w.build_matrix(q);
    Some(q)
}

fn pv_link(w: &World, id: MobyId, o: usize) -> Option<MobyId> { story::link(w, p::i32(&w.m(id).pvars, o) - 1) }
fn set_link(w: &mut World, id: MobyId, o: usize, m: Option<MobyId>) { p::set_i32(&mut w.mm(id).pvars, o, m.map_or(0, |m| m as i32 + 1)); }

/// Level12 `0x2e6c48` (282; module doc).
pub fn merchant_update(w: &mut World, id: MobyId) {
    story::pvars(w, id, 0x184);
    interact::poll_scene_end_at(w, id, T);
    // 0x2e6b60: the actors' big-head cheat on the scene actors of [282] (list 1, 2.75).
    crate::moby_update::manip::scene_big_head(w, &[282], 1, 2.75);
    match w.m(id).state {
        0 => {
            if story::flag(w, 1) != 0 {
                w.delete_moby(id);
                return;
            }
            w.mm(id).state = 1;
            interact::talk_register_at(w, id, T);
            story::shadow_slab(w, id);
        }
        1 => {
            if story::flag(w, 1) == 0 && interact::talk_update_at(w, id, T) {
                interact::place_after_scene(w, id, 2.5);
                w.mm(id).anim.speed = 0.0;
                w.mm(id).state = 2;
            }
        }
        2 if w.svc.game_mode != 2 && w.svc.interact.talker != Some(id) => {
            w.mm(id).anim.speed = 1.0;
            story::set_flag(w, 1, 1);
            let t = w.ticks(300);
            crate::cinematic::show_banner(w, 0x2ee7, t);
            story::set_flag(w, story::flag_index(0x13_d3e9), 1);
            let mission = w.m(id).mission;
            crate::cinematic::set_mission_done(w, mission);
            let cb = p::i32(&w.m(id).pvars, 0x170);
            story::checkpoint_at(w, cb);
            crate::cinematic::save(w);
            w.mm(id).state = 1;
        }
        _ => {}
    }
    look_at_layout(w, id, &MERCHANT_LOOK);
    // The prop on joint 2 (+0x180: index + 1 here, a pointer in the game).
    match pv_link(w, id, 0x180).filter(|&q| story::alive(w, q)) {
        None => {
            let q = prop_here(w, id, 0x11f);
            if let Some(q) = q {
                let (dd, _) = (w.m(id).draw_dist, ());
                let m = w.mm(q);
                m.draw_dist = dd;
                m.visible = 1;
            }
            set_link(w, id, 0x180, q);
        }
        Some(q) => story_npc::attach_child(w, id, q, 2),
    }
}

/// Level12 `0x2eb570` (328; module doc).
pub fn hydro_update(w: &mut World, id: MobyId) {
    story::pvars(w, id, 0x190);
    interact::poll_scene_end_at(w, id, T);
    // 0x2eb498: the actors' big-head cheat on the scene actors of [328] (list 1, 1.9).
    crate::moby_update::manip::scene_big_head(w, &[328], 1, 1.9);
    let talked = w.svc.interact.game.talked.get(70).is_some_and(|&t| t != 0);
    if !talked {
        if w.m(id).anim.seq_b != 1 { w.anim_blend(id, 1, 0, 2); }
    } else if w.m(id).anim.seq_b != 0 {
        w.anim_blend(id, 0, 0, 2);
    }
    match w.m(id).state {
        0 => {
            w.mm(id).state = 1;
            interact::talk_register_at(w, id, T);
            p::set_ff(&mut w.mm(id).pvars, T + talk::RADIUS, f32::from_bits(0x3fd9_999a));
            story::shadow_slab(w, id);
            if w.inventory.owned(4) { p::set_i16(&mut w.mm(id).pvars, T + talk::NODE, -1); }
        }
        1 => {
            if !w.inventory.owned(4) && interact::talk_update_at(w, id, T) {
                interact::place_after_scene(w, id, f32::from_bits(0x4021_47ae));
                w.mm(id).anim.speed = 0.0;
                w.mm(id).state = 2;
            }
        }
        2 if w.svc.game_mode != 2 && w.svc.interact.talker != Some(id) => {
            w.mm(id).anim.speed = 1.0;
            let mission = w.m(id).mission;
            crate::cinematic::set_mission_done(w, mission);
            let cb = p::i32(&w.m(id).pvars, 0x160);
            story::checkpoint_at(w, cb);
            w.mm(id).state = 1;
            if p::i16(&w.m(id).pvars, T + talk::LAST) == 2 {
                interact::give_item(w, 4, false);
                let t = w.ticks(180);
                crate::cinematic::show_banner(w, 0x2ee6, t);
                story::checkpoint_at(w, cb);
                crate::cinematic::save(w);
            }
        }
        _ => {}
    }
    // The props (module doc): +0x164 / +0x168 (index + 1 here).
    if talked {
        for o in [0x168, 0x164] {
            if let Some(q) = pv_link(w, id, o) {
                w.delete_moby(q);
                set_link(w, id, o, None);
            }
        }
    } else {
        let scene = w.svc.game_mode == 2;
        let show = |w: &mut World, q: MobyId| {
            let m = w.mm(q);
            if scene { m.mode |= mode::HIDDEN; m.visible = 0; } else { m.mode &= !mode::HIDDEN; m.visible = 1; }
        };
        match pv_link(w, id, 0x164) {
            None => {
                let q = prop_here(w, id, 0x4c1);
                if let Some(q) = q {
                    let dd = w.m(id).draw_dist;
                    let m = w.mm(q);
                    m.draw_dist = dd;
                    m.visible = 1;
                    m.mode = 0;
                    w.anim_blend(q, 1, 0, 2);
                }
                set_link(w, id, 0x164, q);
            }
            Some(q) => show(w, q),
        }
        match pv_link(w, id, 0x168) {
            None => {
                let q = prop_here(w, id, 0x196);
                if let Some(q) = q {
                    let dd = w.m(id).draw_dist;
                    let m = w.mm(q);
                    m.draw_dist = dd;
                    m.visible = 1;
                }
                set_link(w, id, 0x168, q);
            }
            Some(q) => {
                if let Some(a) = pv_link(w, id, 0x164) { show(w, a); }
                show(w, q);
            }
        }
    }
    look_at_layout(w, id, &HYDRO_LOOK);
}

/// Level12 `0x3093c8` (1404; module doc).
pub fn scene_trigger_update(w: &mut World, id: MobyId) {
    story::pvars(w, id, 0xc);
    if w.counter == 0 || w.svc.game_mode != 0 { return; }
    let flag = (SCENE_FLAGS as i32 + p::i32(&w.m(id).pvars, 4)) as usize;
    if story::flag(w, flag) != 0 {
        w.delete_moby(id);
        return;
    }
    if !story::hero_in(w, p::i32(&w.m(id).pvars, 0)) { return; }
    story::set_flag(w, flag, 1);
    let h = crate::hero::physics::to_f32x3(w.hero.pos);
    let (gz, yaw) = (w.hero.ground_z.to_f32(), w.hero.yaw().to_f32());
    w.svc.interact.scene_end_place = Some(([h[0], h[1], gz], yaw));
    if let Ok(scene) = usize::try_from(p::i32(&w.m(id).pvars, 8)) { crate::cinematic::start_scene(w, scene, false); }
}

/// Level12 `0x3094a0`: the scene thrusters (module doc).
pub fn thrusters_update(w: &mut World, id: MobyId) {
    match w.m(id).state {
        0 => {
            let m = w.mm(id);
            m.state = 1;
            m.update_dist = 0xff;
        }
        1 => {
            if w.svc.game_mode != 2 { return; }
            let Some(scene) = w.svc.cinematic.scene.clone() else { return };
            let k = match scene.id {
                1 => 2,
                7 => 0,
                _ => return,
            };
            if let Some(a) = scene.actors.get(k) { crate::moby_update::classes::cutscene_fx::infobot_thrusters(w, a); }
        }
        _ => {}
    }
}
