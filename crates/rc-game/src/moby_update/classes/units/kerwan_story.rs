//! **Kerwan's item givers** (level 03; read from the level03 decomp): two talkers on the shared talk system whose
//! talk's node 2 hands over an item.
//!
//! * **890 "Helga"** (`0x2da870`, census U142, one instance): the fitness instructor. Her talk block sits at +0x80.
//!   Every talk ends with her mission and a checkpoint (cuboid +0xcc); node 2 (the course run) gives the Swingshot
//!   (item 12, banner 3016), removes the moby +0xd0 for good and resets the course moby +0xd4 (class 1012, its +0x7c /
//!   +0x8c / +0xa0), then saves. She keeps the course moby's "open" word +0x8c at 1 while her talk runs.
//! * **909 the Heli-Pack giver** (`0x2db558`, U145, one instance): after node 2 he gives the Heli-Pack (item 2,
//!   banner 3015), opens the moby +0x144 (class 997, state 0 → 1, made permanent), sets the checkpoint at cuboid
//!   +0x14c, saves and puts Ratchet on that cuboid. He turns round to face Ratchet the first time (a talked flag).
//!
//! **System or not.** Per-class code on the shared systems (talk, look-at layout rows, checkpoint, `story`). The
//! visit-state record `0x273f50` (= level01 `0x29b0a0`) both call on the course moby is the shared visit system
//! (`crate::moby_update::visit`).
//!
//! ## 890 coverage (level03 `0x2da870`)
//!
//! | address | what | status |
//! |---|---|---|
//! | top | `0x2da710`: in a scene with the big-head cheat, the scene camera's offset 0x15f5a0 = −0.2 / 0.2 at scene ticks 0x1ea..0x265 / 0x2ee..0x320 of scene 2 and the manipulator (2.3) on her scene actors | the manipulator: `manip::scene_big_head`; 0x15f5a0: n/a (no reader: `crate::cheats` module doc) |
//! | state 0 | Swingshot owned (0x13d4cc): the course moby (+0xd4, class 0x3f4) +0x8c = 1 and its visit record, `DeleteMoby`; else her mission done → the same +0x8c = 1; light copied from Ratchet's moby, the name, +0xc8 = 5, `NpcTalkRegister(+0x80)`, the shadow slab, → 1 | [`helga_update`] (the visit record: `crate::moby_update::visit`) |
//! | state 1 | `NpcTalkUpdate(+0x80)` → `PlaceAfterScene(2.3)`, → 2, the course moby's +0x8c = 1 | [`helga_update`] |
//! | state 2, mode 2 | the scene's node (talk +0x36) 2: the course moby's +0x7c = 0, +0x8c = 0, +0xa0 = +0xd8 | [`helga_update`] |
//! | state 2, after | node played (talk +0x04) ≠ 2: `SetMissionDone(+0xb0)`, the course moby's +0x8c = 1, the checkpoint at cuboid +0xcc; → 1; node 2: `GiveItem(12, 1)`, `ShowBanner(3016, −1)`, the moby +0xd0 made permanent (killed / collected bytes) and deleted, the course moby reset (+0x7c, +0x8c = 0, +0xa0 = +0xd8), `memcard_Save`, `DeleteMoby` | [`helga_update`] |
//! | tail | the look-at (records +0xe0 list 0 / +0x160 list 1, glance +0x1e0, s32 timers +0x1f0 / +0x1f4, eye 1, pitch × 1, yaw 0.5 on list 0 only) | [`talking_npc::look_at_layout`] (`yaw_b` 0: list 1's target is never written) |
//! | tail | the big-head cheat (the head record's scale 2.75) | `talking_npc::look_springs` |
//!
//! ## 909 coverage (level03 `0x2db558`)
//!
//! | address | what | status |
//! |---|---|---|
//! | top | `0x2db480` (the big-head scene manipulator) | `manip::scene_big_head` |
//! | state 0 | not talked to (0x13d6fc: talk slot 20's word) → yaw += π, +0x140 = 0; else +0x140 = 1; mission done → the course moby +0x148 (class 0x3f4) +0x8c = 1; light from Ratchet's moby; → 1; `NpcTalkRegister`; the shadow slab; Heli-Pack owned (0x13d4c2) → talk node −1 | [`al_update`] |
//! | state 1 | Heli-Pack owned: the moby +0x144 (class 0x3e5) in state 0 → 1 and made permanent; owned with the mission done → no talk | [`al_update`] |
//! | | `NpcTalkUpdate` → (+0x140 = 0: yaw += π, +0x140 = 1), `PlaceAfterScene(2.7)`, anim speed 0, → 2 | [`al_update`] |
//! | state 2 | game mode ≠ 2: anim speed 1; node ≠ 2: `SetMissionDone(+0xb0)`, the course moby's +0x8c = 1, the checkpoint at cuboid +0x14c; → 1; node 2: `GiveItem(2, 1)`, `ShowBanner(3015, −1)`, the moby +0x144 opened, the checkpoint at cuboid +0x14c, `memcard_Save`, `HeroTeleport(cuboid +0x14c, 0, 1)` | [`al_update`] |
//! | tail | the look-at (records +0x40 / +0xc0, glance +0x150, s32 timers +0x164 / +0x168, eye 1, pitch × 1, yaw 0.5 / 0.5) | [`talking_npc::look_at_layout`] |
//! | tail | the big-head cheat (the head record's scale 2.75) | `talking_npc::look_springs` |

use crate::moby_runtime::MobyId;
use crate::moby_update::classes::talking_npc::{look_at_layout, LookLayout};
use crate::moby_update::creature as c;
use crate::moby_update::interact::{self, talk};
use crate::moby_update::services::{pvar as p, World};
use crate::moby_update::story;

pub const REFERENCE_LEVEL: u32 = 3;
pub const HELGA_FN: u32 = 0x2d_a870;
pub const HELGA_CLASSES: [i16; 1] = [890];
pub const AL_FN: u32 = 0x2d_b558;
pub const AL_CLASSES: [i16; 1] = [909];

/// The fitness course moby both drive (class 1012).
const COURSE: i16 = 0x3f4;
/// The moby the Heli-Pack giver opens (class 997).
const GATE: i16 = 0x3e5;
/// Helga's talk block.
const HT: usize = 0x80;

const HELGA_LOOK: LookLayout = LookLayout { pitch: (0xe0, 0), yaw: (0x160, 1), glance: 0x1e0, seen: 0x1f0, glance_timer: 0x1f4, gate_seq_b: true, eye: 1.0, pitch_k: 1.0, yaw_a: 0.5, yaw_b: 0.0, short_timers: false, gate_main: 0, gate_alt: 0xff, k_seen: 0.04 };
const AL_LOOK: LookLayout = LookLayout { pitch: (0x40, 0), yaw: (0xc0, 1), glance: 0x150, seen: 0x164, glance_timer: 0x168, gate_seq_b: true, eye: 1.0, pitch_k: 1.0, yaw_a: 0.5, yaw_b: 0.5, short_timers: false, gate_main: 0, gate_alt: 0xff, k_seen: 0.04 };

/// The course moby's "open" word +0x8c = 1 and its visit record (`0x273f50(+0x8c, 4, npc, 2, 0x1ba720)`).
fn course_open(w: &mut World, id: MobyId, link: usize, visit: bool) {
    let Some(m) = story::link_of(w, p::i32(&w.m(id).pvars, link), COURSE) else { return };
    story::pvars(w, m, 0xa4);
    p::set_i32(&mut w.mm(m).pvars, 0x8c, 1);
    // `0x273f50(course +0x8c, 4, npc, 2, 0x1ba720)`: the course stays open after a death reload.
    if visit { crate::moby_update::visit::record_pvar(w, id, m, 0x8c, 4); }
}

/// The course moby reset (+0x7c = 0, +0x8c = 0, +0xa0 = the NPC's +0xd8).
fn course_reset(w: &mut World, id: MobyId) {
    let Some(m) = story::link_of(w, p::i32(&w.m(id).pvars, 0xd4), COURSE) else { return };
    let v = p::i32(&w.m(id).pvars, 0xd8);
    story::pvars(w, m, 0xa4);
    let pv = &mut w.mm(m).pvars;
    p::set_i32(pv, 0x7c, 0);
    p::set_i32(pv, 0x8c, 0);
    p::set_i32(pv, 0xa0, v);
}

/// Level03 `0x2da870` (890; module doc).
pub fn helga_update(w: &mut World, id: MobyId) {
    story::pvars(w, id, 0x1f8);
    interact::poll_scene_end_at(w, id, HT);
    // 0x2da710: the actors' big-head cheat on the scene actors of [890] (list 0, 2.3).
    crate::moby_update::manip::scene_big_head(w, &[890], 0, 2.3);
    match w.m(id).state {
        0 => {
            if w.inventory.owned(12) {
                course_open(w, id, 0xd4, true);
                w.delete_moby(id);
                return;
            }
            if story::mission_done(w, w.m(id).mission as i32) { course_open(w, id, 0xd4, true); }
            w.mm(id).state = 1;
            story::copy_hero_light(w, id);
            p::set_i32(&mut w.mm(id).pvars, 0xc8, 5);
            interact::talk_register_at(w, id, HT);
            story::shadow_slab(w, id);
        }
        1 => {
            if interact::talk_update_at(w, id, HT) {
                interact::place_after_scene(w, id, f32::from_bits(0x4013_3333));
                w.mm(id).state = 2;
                course_open(w, id, 0xd4, true);
            }
        }
        2 => {
            if w.svc.game_mode == 2 || w.svc.interact.talker == Some(id) {
                if p::i16(&w.m(id).pvars, HT + talk::NODE) == 2 { course_reset(w, id); }
            } else {
                let last = p::i16(&w.m(id).pvars, HT + talk::LAST);
                if last != 2 {
                    let mission = w.m(id).mission;
                    crate::cinematic::set_mission_done(w, mission);
                    course_open(w, id, 0xd4, true);
                    let cb = p::i32(&w.m(id).pvars, 0xcc);
                    story::checkpoint_at(w, cb);
                }
                w.mm(id).state = 1;
                if last == 2 {
                    interact::give_item(w, 12, true);
                    let t = w.ticks(180);
                    crate::cinematic::show_banner(w, 0xbc8, t);
                    if let Some(m) = story::link(w, p::i32(&w.m(id).pvars, 0xd0)) {
                        story::kill_record(w, m);
                        w.delete_moby(m);
                    }
                    course_reset(w, id);
                    crate::cinematic::save(w);
                    w.delete_moby(id);
                    return;
                }
            }
        }
        _ => {}
    }
    look_at_layout(w, id, &HELGA_LOOK);
}

/// The moby +0x144 (class 0x3e5) in state 0 → 1 and made permanent.
fn open_gate(w: &mut World, id: MobyId) {
    let Some(m) = story::link_of(w, p::i32(&w.m(id).pvars, 0x144), GATE) else { return };
    if w.m(m).state != 0 { return; }
    w.mm(m).state = 1;
    story::kill_record(w, m);
}

/// Level03 `0x2db558` (909; module doc).
pub fn al_update(w: &mut World, id: MobyId) {
    story::pvars(w, id, 0x170);
    interact::poll_scene_end(w, id);
    // 0x2db480: the actors' big-head cheat on the scene actors of [909] (list 1, 2.0).
    crate::moby_update::manip::scene_big_head(w, &[909], 1, 2.0);
    match w.m(id).state {
        0 => {
            let talked = w.svc.interact.game.talked.get(20).is_some_and(|&t| t != 0);
            if !talked {
                let y = c::add_rot(c::yaw(w, id), std::f32::consts::PI);
                c::set_yaw(w, id, y);
                p::set_i32(&mut w.mm(id).pvars, 0x140, 0);
            } else {
                p::set_i32(&mut w.mm(id).pvars, 0x140, 1);
            }
            if story::mission_done(w, w.m(id).mission as i32) { course_open(w, id, 0x148, false); }
            w.mm(id).state = 1;
            story::copy_hero_light(w, id);
            interact::talk_register(w, id);
            story::shadow_slab(w, id);
            if w.inventory.owned(2) { p::set_i16(&mut w.mm(id).pvars, talk::NODE, -1); }
        }
        1 => {
            let owned = w.inventory.owned(2);
            if owned { open_gate(w, id); }
            if !(owned && story::mission_done(w, w.m(id).mission as i32)) && interact::talk_update(w, id) {
                if p::i32(&w.m(id).pvars, 0x140) == 0 {
                    let y = c::add_rot(c::yaw(w, id), std::f32::consts::PI);
                    c::set_yaw(w, id, y);
                    p::set_i32(&mut w.mm(id).pvars, 0x140, 1);
                }
                interact::place_after_scene(w, id, f32::from_bits(0x402c_cccd));
                w.mm(id).anim.speed = 0.0;
                w.mm(id).state = 2;
            }
        }
        2 if w.svc.game_mode != 2 && w.svc.interact.talker != Some(id) => {
            w.mm(id).anim.speed = 1.0;
            let last = p::i16(&w.m(id).pvars, talk::LAST);
            let cb = p::i32(&w.m(id).pvars, 0x14c);
            if last != 2 {
                let mission = w.m(id).mission;
                crate::cinematic::set_mission_done(w, mission);
                course_open(w, id, 0x148, false);
                story::checkpoint_at(w, cb);
            }
            w.mm(id).state = 1;
            if last == 2 {
                interact::give_item(w, 2, true);
                let t = w.ticks(180);
                crate::cinematic::show_banner(w, 0xbc7, t);
                open_gate(w, id);
                story::checkpoint_at(w, cb);
                crate::cinematic::save(w);
                story::teleport_to(w, cb, 0, true);
            }
        }
        _ => {}
    }
    look_at_layout(w, id, &AL_LOOK);
}
