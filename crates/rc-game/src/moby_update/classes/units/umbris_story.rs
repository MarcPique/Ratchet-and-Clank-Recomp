//! **Umbris' story director** (class 436, level07 `0x2f5ba0`, census U248, one instance): the arrival scene, the
//! Snagglebeast's lair (scenes 1 / 2, the fight), then the exit (scene 3, movie 8, scene 4), Batalia (planet 8)
//! unlocked, a save made as if on Batalia and the trip there (`0x2a29a0(8)`). A state machine over the global flags
//! 0x13d3b0..0x13d3b2, the same shape as Gemlik's 1353. Read from the level07 decomp.
//!
//! **Pvars** (s32): +0x00 / +0x04 the trigger cuboids, +0x08 a moby deleted past the lair's trigger, +0x0c the beast,
//! +0x10 a moby hidden past the fight and deleted at the exit, +0x14 a moby hidden with the beast, +0x18 / +0x1c the
//! placement cuboids.
//!
//! **System or not.** Per-class code on the cinematic layer, `story` (flags, the level exit), the map mask.
//!
//! | address | what | status |
//! |---|---|---|
//! | state 0 | flag 0x29 (0x13d3b1) set: moby P[2] deleted; game mode 0 → 1 | [`update`] |
//! | state 1 | flag 0x28 (0x13d3b0) clear: set, → 2, `DialogStreamStart(0)`; set → 3 | [`update`] |
//! | state 2 | game mode ≠ 2 → 3 | [`update`] |
//! | state 3 | → 4 | [`update`] |
//! | state 4 | flag 0x29 set → 7; P[0] = −1 → 8; Ratchet in cuboid P[0]: flag 0x29, P[3] and P[5] hidden (mode \|= 0x41), → 5, `DialogStreamStart(1)`; any state left: P[2] deleted | [`update`] |
//! | state 5 | game mode ≠ 2: P[3] hidden, → 6, `DialogStreamStart(2)` | [`update`] |
//! | state 6 | game mode ≠ 2: `HeroTeleport(cuboid P[6], 0, 1)`, → 7 | [`update`] |
//! | state 7 | P[3] and P[5] shown (mode &= ~0x41), `MusicRequestTrack(2, 4)`, → 8 | [`update`] |
//! | state 8 | flag 0x2a (0x13d3b2) set → 0xc; the beast P[3] alive → wait; then P[1] = −1 → 0xd; Ratchet in cuboid P[1]: `HeroTeleport(cuboid P[7], 0x72, 1)`, flag 0x2a, → 9, `DialogStreamStart(3)`; any state left: P[4] hidden | [`update`] |
//! | state 9 | game mode ≠ 2: → 10, `DialogStreamUpdate(8)` | [`update`] |
//! | state 10 | game mode ≠ 2: → 0xb, `DialogStreamStart(4)`, and on into 0xb (which then waits: the scene's game mode 2) | [`update`] (`cinematic::start_scene` stores mode 2 at once, as `DialogStreamStart`) |
//! | state 0xb | game mode ≠ 2: `HeroTeleport(cuboid P[7], 0, 1)`; P[4] moved to Ratchet + clamp(Ratchet − it, 0.9) and deleted; the map mask's rectangle x 135..221, y 12..72 revealed (`0x270248`); `UnlockPlanet(8)`; `memcard_Save(0, 8)`; `0x2a29a0(8)`; → 0xc | [`update`] (`Mask::set`; the tile refresh `0x271508`: n/a, the port composes the page from the mask) |
//! | state 0xc | P[4] alive → `DeleteMoby` (with no argument: its own pointer [L]); → 0xd | [`update`] |
//! | state 0xd | `DeleteMoby` | [`update`] |

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::creature as c;
use crate::moby_update::services::{pvar as p, World};
use crate::moby_update::story;

pub const REFERENCE_LEVEL: u32 = 7;
pub const UPDATE_FN: u32 = 0x2f_5ba0;
pub const CLASSES: [i16; 1] = [436];

const FLAG_ARRIVAL: usize = story::flag_index(0x13_d3b0);
const FLAG_LAIR: usize = story::flag_index(0x13_d3b1);
const FLAG_EXIT: usize = story::flag_index(0x13_d3b2);
/// Batalia.
const NEXT: i32 = 8;
/// `0x270248(0x87, 0xc, 0x56, 0x3c)`: the map rectangle revealed at the exit (x, y, width, height).
const REVEAL: (usize, usize, usize, usize) = (0x87, 0xc, 0x56, 0x3c);

fn pv(w: &World, id: MobyId, k: usize) -> i32 { p::i32(&w.m(id).pvars, 4 * k) }
fn moby(w: &World, id: MobyId, k: usize) -> Option<MobyId> { story::link(w, pv(w, id, k)) }
fn hide(w: &mut World, id: MobyId, k: usize) { if let Some(m) = moby(w, id, k) { w.mm(m).mode |= mode::HIDDEN | mode::NO_ANIM; } }
fn show(w: &mut World, id: MobyId, k: usize) { if let Some(m) = moby(w, id, k) { w.mm(m).mode &= !(mode::HIDDEN | mode::NO_ANIM); } }

/// Level07 `0x2f5ba0` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    story::pvars(w, id, 0x20);
    match w.m(id).state {
        0 => {
            if story::flag(w, FLAG_LAIR) != 0 {
                if let Some(m) = moby(w, id, 2) { w.delete_moby(m); }
            }
            if w.svc.game_mode == 0 { w.mm(id).state = 1; }
        }
        1 => {
            if story::flag(w, FLAG_ARRIVAL) == 0 {
                story::set_flag(w, FLAG_ARRIVAL, 1);
                w.mm(id).state = 2;
                crate::cinematic::start_scene(w, 0, false);
            } else {
                w.mm(id).state = 3;
            }
        }
        2 => if w.svc.game_mode != 2 { w.mm(id).state = 3; },
        3 => w.mm(id).state = 4,
        4 => {
            if story::flag(w, FLAG_LAIR) != 0 {
                w.mm(id).state = 7;
            } else if pv(w, id, 0) == -1 {
                w.mm(id).state = 8;
            } else if story::hero_in(w, pv(w, id, 0)) {
                story::set_flag(w, FLAG_LAIR, 1);
                hide(w, id, 3);
                hide(w, id, 5);
                w.mm(id).state = 5;
                crate::cinematic::start_scene(w, 1, false);
            }
            if w.m(id).state != 4 {
                if let Some(m) = moby(w, id, 2) { w.delete_moby(m); }
            }
        }
        5 => {
            if w.svc.game_mode != 2 {
                hide(w, id, 3);
                w.mm(id).state = 6;
                crate::cinematic::start_scene(w, 2, false);
            }
        }
        6 => {
            if w.svc.game_mode != 2 {
                story::teleport_to(w, pv(w, id, 6), 0, true);
                w.mm(id).state = 7;
            }
        }
        7 => {
            show(w, id, 3);
            show(w, id, 5);
            if let Some(s) = w.sound.as_deref_mut() { s.music_request(2, 4); }
            w.mm(id).state = 8;
        }
        8 => {
            if story::flag(w, FLAG_EXIT) != 0 {
                w.mm(id).state = 0xc;
            } else if moby(w, id, 3).is_some_and(|b| story::alive(w, b)) {
                // The beast still alive: wait.
            } else if pv(w, id, 1) == -1 {
                w.mm(id).state = 0xd;
            } else if story::hero_in(w, pv(w, id, 1)) {
                story::teleport_to(w, pv(w, id, 7), 0x72, true);
                story::set_flag(w, FLAG_EXIT, 1);
                w.mm(id).state = 9;
                crate::cinematic::start_scene(w, 3, false);
            }
            if w.m(id).state != 8 { hide(w, id, 4); }
        }
        9 => {
            if w.svc.game_mode != 2 {
                w.mm(id).state = 10;
                crate::cinematic::start_movie(w, 8);
            }
        }
        10 | 0xb => {
            if w.m(id).state == 10 {
                if w.svc.game_mode == 2 { return; }
                w.mm(id).state = 0xb;
                crate::cinematic::start_scene(w, 4, false);
            }
            if w.svc.game_mode == 2 { return; }
            story::teleport_to(w, pv(w, id, 7), 0, true);
            if let Some(m) = moby(w, id, 4) {
                let h = story::hero4(w);
                let d = c::sub(h, c::pos(w, m));
                let l = c::len3(d);
                let d = if l > 0.9 { c::scale(d, 0.9 / l) } else { d };
                w.mm(m).position = c::add(h, d);
                w.delete_moby(m);
            }
            let (x, y, wd, ht) = REVEAL;
            for yy in y..y + ht {
                for xx in x..x + wd { w.svc.map.mask.set(xx, yy, false); }
            }
            crate::cinematic::unlock_planet(w, NEXT);
            crate::cinematic::save_as(w, NEXT);
            story::level_exit(w, NEXT);
            w.mm(id).state = 0xc;
        }
        0xc => {
            if let Some(m) = moby(w, id, 4).filter(|&m| story::alive(w, m)) { w.delete_moby(m); }
            w.mm(id).state = 0xd;
        }
        0xd => w.delete_moby(id),
        _ => {}
    }
}
