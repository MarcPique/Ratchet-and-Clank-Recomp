//! **Batalia's story NPCs** (level 08; read from the level08 decomp):
//!
//! * **1130 "Commando"** (`0x302ce8`, census U297, one instance): talk block at +0x20. The first talk sets global
//!   flag 0x2c and moves him to cuboid +0x180; there his talk's node 3 unlocks Orxon (planet 10) with its banner, sets
//!   his mission, the checkpoint at Ratchet, saves; then he is gone.
//! * **1144 "Deserter"** (`0x305270`, U298, one instance): within 8 (XY) his mission is set; then, while Ratchet stands
//!   on no moby and has been on the ground for `ticks(30)` (or a talk node is up), his talk runs; node 4 unlocks Gaspar
//!   (planet 9) and saves; then he is gone.
//! * **1283 "Water Worker"** (`0x3065d8`, U299, one instance): the turret mission's host. His talk's node 2 sets the
//!   checkpoint at cuboid +0x48 and puts Ratchet on the turret +0x50 (its +0xbc = 1, hero state 0x32); node 3 played
//!   plays the level's item movie 1 and then gives the Metal Detector (item 27) with his mission and a save; node 4
//!   played is the death reload.
//!
//! **System or not.** Per-class code from the NPC template (`story_npc`, the look-at layout rows), the talk system,
//! the checkpoint record, `story`, cinematic.
//!
//! ## 1130 coverage (level08 `0x302ce8`)
//!
//! | address | what | status |
//! |---|---|---|
//! | top | `0x302c00` (the big-head scene manipulator) | `manip::scene_big_head` |
//! | top | drawn within gp−0x4a28 (24) of the camera: the shadow probe, +0x7f = 24·13 >> 4 | [`commando_update`] |
//! | state 0 | update distance 0xff; seq ≠ 1 → blend 1 (`ticks(10)`); planet 10 unlocked and his mission done → `DeleteMoby`; radius (+0x2c) 3; the name; flag 0x13d3b4 clear → 1; set → at cuboid +0x180 (centre, Euler), → 2; `NpcTalkRegister(+0x20)` | [`commando_update`] |
//! | state 1 | `NpcTalkUpdate(+0x20)` → at cuboid +0x180, → 2, flag 0x13d3b4 = 1 | [`commando_update`] |
//! | state 2 | at a sequence's end: `randi(2) + 1` ≠ seq → blend to another `randi(2) + 1` over `ticks(20)`; `NpcTalkUpdate` → 3, `PlaceAfterScene(2.2)` | [`commando_update`] |
//! | state 3 | game mode ≠ 2: auto (+0x28) = 1; node played (+0x24) 3: `UnlockPlanet(10)`, `ShowPlanetBanner(10)`, `SetMissionDone(+0xb0)`, the checkpoint at Ratchet, `memcard_Save`, `DeleteMoby`; else → 2 | [`commando_update`] |
//! | tail | the look-at (records +0x60 list 0 / +0xe0 list 1, glance +0x160, s32 +0x188 / +0x18c, eye 1, pitch × 1, yaw 0.6 / 0.4, gate seq 0); the big-head cheat | [`talking_npc::look_at_layout`] (the cheat in `look_springs`) |
//!
//! ## 1144 coverage (level08 `0x305270`)
//!
//! | address | what | status |
//! |---|---|---|
//! | top | `0x305138` (the big-head scene manipulator); drawn within 30: the shadow probe, +0x7f = 0x18 | the probe: [`deserter_update`]; the cheat: `manip::scene_big_head` |
//! | state 0 | seq ≠ 1 → blend 1 (`ticks(20)`); planet 9 unlocked and his mission done → `DeleteMoby`; the name, radius 4, → 1; +0x64 ≥ 0 → `NpcTalkRegister(+0x20)` | [`deserter_update`] |
//! | state 1 | within 8 (XY): `SetMissionDone(+0xb0)`, → 2 | [`deserter_update`] |
//! | state 2 | the end-of-sequence pick (as 1130); standing on no moby (0x13f64c = 0) and (on the ground past `ticks(30)` (0x13f650) or the node (+0x56) ≠ 0): `NpcTalkUpdate(+0x20)` and `PlaceAfterScene(2.2)`; node played 4: `UnlockPlanet(9)`, `memcard_Save`, `DeleteMoby` | [`deserter_update`] |
//! | tail | the look-at (records +0x80 / +0x100, glance +0x180, s32 +0x194 / +0x198, yaw 0.5 / 0.5, gate seq 1) | [`talking_npc::look_at_layout`] |
//!
//! ## 1283 coverage (level08 `0x3065d8`)
//!
//! | address | what | status |
//! |---|---|---|
//! | top | `0x3063f0` (the big-head scene manipulator) | `manip::scene_big_head` |
//! | state 0 | update distance 0xff; mission done → `DeleteMoby`; the name, → 1, `NpcTalkRegister` | [`water_worker_update`] |
//! | state 1 | `NpcTalkUpdate` → `PlaceAfterScene(2.7)`; node 2: the checkpoint at cuboid +0x48, → 2, the turret +0x50: +0xbc = 1 and `SetState(0x32, 1)` (`0x2e1698`); node played 3: the item movie 1 (`0x2a1880`), → 5; node played 4: the death sequence (`0x222c50` = `0x2319b0`), node played := −1 | [`water_worker_update`] (`cinematic::item_movie`, `HeroCall::Death`) |
//! | state 5 | `SetMissionDone(+0xb0)`, `GiveItem(27, 1)`, `memcard_Save`, `DeleteMoby` | [`water_worker_update`] |
//! | tail | the look-at (records +0x60 / +0xe0, glance +0x160, s32 +0x174 / +0x178, yaw 0.7 / 0.3, k 0.03 when seen) | [`talking_npc::look_at_layout`] |
//! | `0x306b30(host, k)` | the turret 440's call at the battle's end: → 1, auto (+0x08) = 1, the node (+0x36) 4 (the turret lost, k 0) or 3 (won, k 1); k −1 → the death sequence | [`host_talk`] |

use crate::moby_runtime::MobyId;
use crate::moby_update::classes::talking_npc::{look_at_layout, LookLayout};
use crate::moby_update::creature as c;
use crate::moby_update::interact::{self, talk};
use crate::moby_update::services::{pvar as p, HeroCall, World};
use crate::moby_update::story;

pub const REFERENCE_LEVEL: u32 = 8;
pub const COMMANDO_FN: u32 = 0x30_2ce8;
pub const COMMANDO_CLASSES: [i16; 1] = [1130];
pub const DESERTER_FN: u32 = 0x30_5270;
pub const DESERTER_CLASSES: [i16; 1] = [1144];
pub const WORKER_FN: u32 = 0x30_65d8;
pub const WORKER_CLASSES: [i16; 1] = [1283];

/// The commando's and the deserter's talk block.
const T: usize = 0x20;
/// gp−0x4a28: the shadow range of the commando.
const SHADOW_RANGE: i32 = 24;

const COMMANDO_LOOK: LookLayout = LookLayout { pitch: (0x60, 0), yaw: (0xe0, 1), glance: 0x160, seen: 0x188, glance_timer: 0x18c, gate_seq_b: true, eye: 1.0, pitch_k: 1.0, yaw_a: 0.6, yaw_b: 0.4, short_timers: false, gate_main: 0, gate_alt: 0xff, k_seen: 0.04 };
const DESERTER_LOOK: LookLayout = LookLayout { pitch: (0x80, 0), yaw: (0x100, 1), glance: 0x180, seen: 0x194, glance_timer: 0x198, gate_seq_b: true, eye: 1.0, pitch_k: 1.0, yaw_a: 0.5, yaw_b: 0.5, short_timers: false, gate_main: 1, gate_alt: 0xff, k_seen: 0.04 };
const WORKER_LOOK: LookLayout = LookLayout { pitch: (0x60, 0), yaw: (0xe0, 1), glance: 0x160, seen: 0x174, glance_timer: 0x178, gate_seq_b: true, eye: 1.0, pitch_k: 1.0, yaw_a: 0.7, yaw_b: 0.3, short_timers: false, gate_main: 0, gate_alt: 0xff, k_seen: 0.03 };

fn shadow(w: &mut World, id: MobyId, range: f32, b7f: u8) {
    if w.m(id).visible != 0 && c::len3(c::sub(c::pos(w, id), w.camera.map(|x| f32::from_bits(x.0)))) < range {
        crate::shadows::probe_down(w, id);
        w.mm(id).b7f = b7f;
    }
}

/// The NPC on cuboid `c`'s centre and Euler.
fn to_cuboid(w: &mut World, id: MobyId, cb: i32) {
    if let Some((pos, rot)) = story::cuboid(w, cb) {
        let m = w.mm(id);
        m.position = [pos[0], pos[1], pos[2], m.position[3]];
        m.rotation = [rot[0], rot[1], rot[2], m.rotation[3]];
    }
}

/// The end-of-sequence pick of 1130 / 1144: at the end, `randi(2) + 1` ≠ seq → another `randi(2) + 1`, `ticks(20)`.
fn end_pick(w: &mut World, id: MobyId) {
    if w.m(id).anim.flags & 2 == 0 { return; }
    let seq = w.m(id).anim.seq_b as i32;
    if seq == w.rng.randi(2) + 1 { return; }
    let s = w.rng.randi(2) + 1;
    let n = w.ticks(0x14);
    w.anim_blend(id, s as u8, 0, n);
}

fn blend_to(w: &mut World, id: MobyId, s: u8, t: i32) {
    if w.m(id).anim.seq_b != s {
        let n = w.ticks(t);
        w.anim_blend(id, s, 0, n);
    }
}

/// Level08 `0x302ce8` (1130; module doc).
pub fn commando_update(w: &mut World, id: MobyId) {
    story::pvars(w, id, 0x190);
    interact::poll_scene_end_at(w, id, T);
    // 0x302c00: the actors' big-head cheat on the scene actors of [1130] (list 0, 2.75).
    crate::moby_update::manip::scene_big_head(w, &[1130], 0, 2.75);
    shadow(w, id, SHADOW_RANGE as f32, ((SHADOW_RANGE * 0xd) >> 4) as u8);
    match w.m(id).state {
        0 => {
            w.mm(id).update_dist = 0xff;
            blend_to(w, id, 1, 10);
            if story::planet_unlocked(w, 10) && story::mission_done(w, w.m(id).mission as i32) {
                w.delete_moby(id);
                return;
            }
            p::set_ff(&mut w.mm(id).pvars, T + talk::RADIUS, 3.0);
            if story::flag(w, 0x2c) == 0 {
                w.mm(id).state = 1;
            } else {
                to_cuboid(w, id, p::i32(&w.m(id).pvars, 0x180));
                w.mm(id).state = 2;
            }
            interact::talk_register_at(w, id, T);
        }
        1 => {
            if interact::talk_update_at(w, id, T) {
                to_cuboid(w, id, p::i32(&w.m(id).pvars, 0x180));
                w.mm(id).state = 2;
                story::set_flag(w, 0x2c, 1);
            }
        }
        2 => {
            end_pick(w, id);
            if interact::talk_update_at(w, id, T) {
                w.mm(id).state = 3;
                interact::place_after_scene(w, id, f32::from_bits(0x400c_cccd));
            }
        }
        3 if w.svc.game_mode != 2 && w.svc.interact.talker != Some(id) => {
            p::set_u8(&mut w.mm(id).pvars, T + talk::AUTO, 1);
            if p::i16(&w.m(id).pvars, T + talk::LAST) == 3 {
                crate::cinematic::unlock_planet(w, 10);
                crate::cinematic::show_planet_banner(w, 10);
                let mission = w.m(id).mission;
                crate::cinematic::set_mission_done(w, mission);
                checkpoint_here(w);
                crate::cinematic::save(w);
                w.delete_moby(id);
                return;
            }
            w.mm(id).state = 2;
        }
        _ => {}
    }
    look_at_layout(w, id, &COMMANDO_LOOK);
}

/// The checkpoint at Ratchet (0x13f3d0 / 0x13f3e0).
fn checkpoint_here(w: &mut World) {
    let (pos, yaw) = (crate::hero::physics::to_f32x3(w.hero.pos), w.hero.yaw().to_f32());
    crate::moby_update::classes::checkpoint::record(w, crate::moby_update::classes::checkpoint::Record { pos, rot: [0.0, 0.0, yaw] });
}

/// Level08 `0x305270` (1144; module doc).
pub fn deserter_update(w: &mut World, id: MobyId) {
    story::pvars(w, id, 0x19c);
    interact::poll_scene_end_at(w, id, T);
    // 0x305138: the actors' big-head cheat on the scene actors of [1144] (list 0, 2.75).
    crate::moby_update::manip::scene_big_head(w, &[1144], 0, 2.75);
    shadow(w, id, 30.0, 0x18);
    match w.m(id).state {
        0 => {
            blend_to(w, id, 1, 0x14);
            if story::planet_unlocked(w, 9) && story::mission_done(w, w.m(id).mission as i32) {
                w.delete_moby(id);
                return;
            }
            p::set_ff(&mut w.mm(id).pvars, T + talk::RADIUS, 4.0);
            w.mm(id).state = 1;
            if p::i32(&w.m(id).pvars, 0x64) >= 0 { interact::talk_register_at(w, id, T); }
        }
        1 => {
            if c::dist2(c::pos(w, id), story::hero4(w)) < 8.0 {
                let mission = w.m(id).mission;
                crate::cinematic::set_mission_done(w, mission);
                w.mm(id).state = 2;
            }
        }
        2 => {
            end_pick(w, id);
            if w.hero.ground_moby.is_none() && (w.ticks(0x1e) < w.hero.grounded_ticks || p::i16(&w.m(id).pvars, T + talk::NODE) != 0) {
                interact::talk_update_at(w, id, T);
                interact::place_after_scene(w, id, f32::from_bits(0x400c_cccd));
            }
            if p::i16(&w.m(id).pvars, T + talk::LAST) == 4 {
                crate::cinematic::unlock_planet(w, 9);
                crate::cinematic::save(w);
                w.delete_moby(id);
                return;
            }
        }
        _ => {}
    }
    look_at_layout(w, id, &DESERTER_LOOK);
}

/// Level08 `0x3065d8` (1283; module doc).
pub fn water_worker_update(w: &mut World, id: MobyId) {
    story::pvars(w, id, 0x17c);
    interact::poll_scene_end(w, id);
    // 0x3063f0: the actors' big-head cheat on the scene actors of [1283, 0x306] (list 0, 2.3).
    crate::moby_update::manip::scene_big_head(w, &[1283, 0x306], 0, 2.3);
    match w.m(id).state {
        0 => {
            w.mm(id).update_dist = 0xff;
            if story::mission_done(w, w.m(id).mission as i32) {
                w.delete_moby(id);
                return;
            }
            w.mm(id).state = 1;
            interact::talk_register(w, id);
        }
        1 => {
            if interact::talk_update(w, id) { interact::place_after_scene(w, id, f32::from_bits(0x402c_cccd)); }
            if p::i16(&w.m(id).pvars, talk::NODE) == 2 {
                let cb = p::i32(&w.m(id).pvars, 0x48);
                story::checkpoint_at(w, cb);
                w.mm(id).state = 2;
                if let Some(t) = story::link(w, p::i32(&w.m(id).pvars, 0x50)) {
                    // 0x2e1698: the turret's command, Ratchet mounted (hero state 0x32).
                    w.mm(t).cmd = 1;
                    crate::cinematic::hero_state(w, 0x32, true);
                }
            }
            if p::i16(&w.m(id).pvars, talk::LAST) == 3 {
                crate::cinematic::item_movie(w, 1);
                w.mm(id).state = 5;
            }
            if p::i16(&w.m(id).pvars, talk::LAST) == 4 {
                w.hero_fields_mut().call(HeroCall::Death);
                p::set_i16(&mut w.mm(id).pvars, talk::LAST, -1);
            }
        }
        5 => {
            let mission = w.m(id).mission;
            crate::cinematic::set_mission_done(w, mission);
            interact::give_item(w, 27, true);
            crate::cinematic::save(w);
            w.delete_moby(id);
            return;
        }
        _ => {}
    }
    look_at_layout(w, id, &WORKER_LOOK);
}

/// `0x306b30(host, k)` (the 1283 table): the turret 440 hands the battle's end to the host, node 3 (won) or 4 (lost).
pub fn host_talk(w: &mut World, host: MobyId, won: bool) {
    w.mm(host).state = 1;
    if w.m(host).pvars.len() < 0x38 { return; }
    let pv = &mut w.mm(host).pvars;
    p::set_u8(pv, talk::AUTO, 1);
    p::set_i16(pv, talk::NODE, if won { 3 } else { 4 });
}
