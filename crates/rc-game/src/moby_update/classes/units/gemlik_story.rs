//! **Gemlik's story director** (class 1353, level13 `0x30b628`, census U440, one instance): the arrival scene, Qwark's
//! ambush, the fight with his creature, his escape and the unlock of Oltanis (planet 14). A state machine over the
//! global flags 0x13d3ec..0x13d3ee (docs/plan/level_scripting.md §4). Read from the level13 decomp.
//!
//! **Pvars** (s32): +0x00 the trigger cuboid, +0x08 the placement cuboid, +0x0c a moby deleted once the ambush is past,
//! +0x10 the vehicle (class 69), +0x14 the creature (class 388), +0x20 the placed ship (class 533).
//!
//! **System or not.** Per-class code on the cinematic layer, `story`, the travel lane's ship block (the landing spot).
//!
//! | address | what | status |
//! |---|---|---|
//! | state 0 | game mode 0 → 1 | [`update`] |
//! | state 1 | flag 0x13d3ec clear: set, → 2, `DialogStreamStart(0)` (the arrival); set: → 3 | [`update`] |
//! | state 2 | game mode ≠ 2 → 3 | [`update`] |
//! | state 3 | → 4 | [`update`] |
//! | state 4 | flag 0x13d3ed set → 6; P[0] = −1 → 7; Ratchet in cuboid P[0]: flag set, the creature P[5] hidden (mode \|= 0x41), → 5, `DialogStreamStart(1)`; any state left: the moby P[3] deleted | [`update`] |
//! | state 5 | game mode ≠ 2: the creature shown (mode &= ~0x41); with P[2] and P[4]: the vehicle at cuboid P[2]'s centre, `HeroTeleport(cuboid P[2], 0, 1)`, 0x1413f5 = 1 (Ratchet hidden), the vehicle's +0x7c = 0.99, the fade 0x15f3fc = 0.99, the vehicle → state 2; → 6 | [`update`] (`HeroFields::hero_hidden`) |
//! | state 6 | → 7 | [`update`] |
//! | state 7 | flag 0x13d3ee set → 0xc; P[5] = −1 → 0xd; the creature (class 0x184) alive → wait; else: the vehicle mode \|= 2, `HeroTeleport(cuboid P[2], 0x72, 1)`, flag set, the ship P[8] hidden, `MusicRequestTrack(0, 5)`, → 8, `DialogStreamStart(2)` | [`update`] |
//! | state 8 | game mode ≠ 2: → 9, `DialogStreamUpdate(14)` | [`update`] |
//! | state 9 | game mode ≠ 2: `DialogStreamStart(3)`, → 10 | [`update`] |
//! | state 10 | game mode ≠ 2: the vehicle mode &= ~2; `UnlockPlanet(14)`; the ship P[8] shown and adopted (`0x298cb0` = level01 `0x2a2360`), the landing spot 0x13e090 = (464.66, 580.68, 316.72), yaw 0x13e0a8 = 2.77; the letterbox off; → 0xb | [`update`] (`ShipGlobals::landing_spot`, `travel::ship::adopt`) |
//! | state 0xb | the vehicle (class 0x45) alive → wait; else `memcard_Save`, → 0xc (P[4] = −1: → 0xc at once) | [`update`] |
//! | state 0xc | → 0xd | [`update`] |
//! | state 0xd | P[4] = −1 → `DeleteMoby`; the vehicle's mission loaded done (0x15fc88[+0xb0] = −1): the ship P[8] deleted, → 0xe | [`update`] (`MissionState::mission_slot`) |
//! | state 0xe | mode \|= 2 | [`update`] |

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::services::{pvar as p, World};
use crate::moby_update::story;

pub const REFERENCE_LEVEL: u32 = 13;
pub const UPDATE_FN: u32 = 0x30_b628;
pub const CLASSES: [i16; 1] = [1353];

const FLAG_ARRIVAL: usize = story::flag_index(0x13_d3ec);
const FLAG_AMBUSH: usize = story::flag_index(0x13_d3ed);
const FLAG_ESCAPE: usize = story::flag_index(0x13_d3ee);
/// The creature (class 388) and the vehicle (class 69).
const CREATURE: i16 = 0x184;
const VEHICLE: i16 = 0x45;
/// 0x13e090 / 0x13e0a8: the landing spot after the escape.
const LANDING: [f32; 3] = [464.66, 580.68, 316.72];
const LANDING_YAW: f32 = 2.77;

fn pv(w: &World, id: MobyId, k: usize) -> i32 { p::i32(&w.m(id).pvars, 4 * k) }
fn moby(w: &World, id: MobyId, k: usize) -> Option<MobyId> { story::link(w, pv(w, id, k)) }

/// Level13 `0x30b628` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    story::pvars(w, id, 0x24);
    let mode2 = w.svc.game_mode == 2;
    match w.m(id).state {
        0 => if w.svc.game_mode == 0 { w.mm(id).state = 1; },
        1 => {
            if story::flag(w, FLAG_ARRIVAL) == 0 {
                story::set_flag(w, FLAG_ARRIVAL, 1);
                w.mm(id).state = 2;
                crate::cinematic::start_scene(w, 0, false);
            } else {
                w.mm(id).state = 3;
            }
        }
        2 => if !mode2 { w.mm(id).state = 3; },
        3 => w.mm(id).state = 4,
        4 => {
            if story::flag(w, FLAG_AMBUSH) != 0 {
                w.mm(id).state = 6;
            } else if pv(w, id, 0) == -1 {
                w.mm(id).state = 7;
            } else if story::hero_in(w, pv(w, id, 0)) {
                story::set_flag(w, FLAG_AMBUSH, 1);
                if let Some(c) = moby(w, id, 5) { w.mm(c).mode |= mode::HIDDEN | mode::NO_ANIM; }
                w.mm(id).state = 5;
                crate::cinematic::start_scene(w, 1, false);
            }
            if w.m(id).state != 4 {
                if let Some(m) = moby(w, id, 3) { w.delete_moby(m); }
            }
        }
        5 => {
            if mode2 { return; }
            if let Some(c) = moby(w, id, 5) { w.mm(c).mode &= !(mode::HIDDEN | mode::NO_ANIM); }
            if let (Some((centre, rot)), Some(v)) = (story::cuboid(w, pv(w, id, 2)), moby(w, id, 4)) {
                let m = w.mm(v);
                m.position = [centre[0], centre[1], centre[2], m.position[3]];
                crate::cinematic::hero_teleport(w, centre, rot, 0, true);
                // 0x1413f5 = 1: Ratchet hidden (he rides the vehicle).
                w.hero_fields_mut().hero_hidden = Some(1);
                story::pvars(w, v, 0x80);
                p::set_ff(&mut w.mm(v).pvars, 0x7c, f32::from_bits(0x3f7d_70a4));
                crate::cinematic::set_fade(w, f32::from_bits(0x3f7d_70a4));
                w.mm(v).state = 2;
            }
            w.mm(id).state = 6;
        }
        6 => w.mm(id).state = 7,
        7 => {
            if story::flag(w, FLAG_ESCAPE) != 0 {
                w.mm(id).state = 0xc;
                return;
            }
            if pv(w, id, 5) == -1 {
                w.mm(id).state = 0xd;
                return;
            }
            if let Some(c) = moby(w, id, 5) {
                if w.m(c).o_class == CREATURE && story::alive(w, c) { return; }
            }
            if let Some(v) = moby(w, id, 4) { w.mm(v).mode |= mode::NO_UPDATE; }
            story::teleport_to(w, pv(w, id, 2), 0x72, true);
            story::set_flag(w, FLAG_ESCAPE, 1);
            if let Some(s) = moby(w, id, 8) { w.mm(s).mode |= mode::HIDDEN | mode::NO_ANIM; }
            if let Some(s) = w.sound.as_deref_mut() { s.music_request(0, 5); }
            w.mm(id).state = 8;
            crate::cinematic::start_scene(w, 2, false);
        }
        8 => {
            if !mode2 {
                w.mm(id).state = 9;
                crate::cinematic::start_movie(w, 0xe);
            }
        }
        9 => {
            if !mode2 {
                crate::cinematic::start_scene(w, 3, false);
                w.mm(id).state = 10;
            }
        }
        10 => {
            if mode2 { return; }
            if let Some(v) = moby(w, id, 4) { w.mm(v).mode &= !mode::NO_UPDATE; }
            crate::cinematic::unlock_planet(w, 0xe);
            if let Some(s) = moby(w, id, 8) {
                w.mm(s).mode &= !(mode::HIDDEN | mode::NO_ANIM);
                crate::travel::ship::adopt(w, s);
                w.svc.travel.landing_spot = Some(([LANDING[0], LANDING[1], LANDING[2], 0.0], [0.0, 0.0, LANDING_YAW, 0.0]));
            }
            crate::cinematic::letterbox(w, false);
            w.mm(id).state = 0xb;
        }
        0xb => {
            if pv(w, id, 4) == -1 {
                w.mm(id).state = 0xc;
                return;
            }
            if let Some(v) = moby(w, id, 4) {
                if w.m(v).o_class == VEHICLE && story::alive(w, v) { return; }
            }
            crate::cinematic::save(w);
            w.mm(id).state = 0xc;
        }
        0xc => w.mm(id).state = 0xd,
        0xd => {
            match moby(w, id, 4) {
                None => w.delete_moby(id),
                Some(v) => {
                    if w.missions.mission_slot(w.m(v).mission) == 0xff {
                        if let Some(s) = moby(w, id, 8) { w.delete_moby(s); }
                        w.mm(id).state = 0xe;
                    }
                }
            }
        }
        0xe => w.mm(id).mode |= mode::NO_UPDATE,
        _ => {}
    }
}
