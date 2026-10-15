//! **Blarg's shuttle, class 1109** (level06 `0x302578`, census U237, one instance): the station's transit shuttle.
//! Ratchet stands on it and presses △ (the context prompt); it flies one of five routes along three splines each (its
//! path, the camera's path, the camera's look path) with the script camera following, while scripted bursts go off
//! along the way; the last route (entered through cuboid +0xf0) blows a group of blocks away, lands by the infobot,
//! which Ratchet walks to: scene 12, the infobot gone, **planet 5 (Rilgar) unlocked**, the mission done, the
//! checkpoint, movie 5, then (Hoverboard not owned) the planet banner and the save, or scene 5 first. Read from the
//! level06 decomp and disassembly (`0x302578` and its helpers `0x302fd8` / `0x303160` / `0x303748` / `0x303b68` /
//! `0x304028`; the lost stack / register arguments from the disassembly); the level data from the overlay (gp
//! 0x166c00). Native `f32`.
//!
//! **System or not.** Per-class code (a cluster of one), on the shared pieces: the context prompt (`interact`), the
//! script camera (`cinematic::{camera_script, camera_targets, script_springs, camera_script2}`), `CarryRiders`
//! (`triggers::carry_riders`), the story layer (`cinematic::{start_scene, start_movie, unlock_planet,
//! show_planet_banner, set_mission_done, save}`, `story` flags), the infobot ([`super::super::infobot::show`]), the
//! bobbing blocks' drift ([`super::bob_block`]), the flash 1898 ([`super::veldin_shots::flash_new`], the same code),
//! the map's alternate side (`MapState::alt`, gp−0x6e08).
//!
//! **Pvar block** (0x118; the paths / cuboids / group / moby words come with the instance): +0x20 the riders' carry
//! block, +0x60 / +0x74 / +0x88 the five routes' shuttle / camera / look paths, +0x9c the route the prompt offers, +0xa0
//! / +0xa4 / +0xa8 the three stops' cuboids, +0xac Ratchet's seat cuboid, +0xb0 / +0xb4 the turnaround angle and
//! rate, +0xbc the side (stop) it is at, +0xc0 the route flown, +0xc4 / +0xc8 the scale at its start / end, +0xcc the
//! group blown away, +0xd0.. the bursts' cuboids (by the point w), +0xf0 the last route's trigger cuboid, +0xf4 the
//! route's progress (0..1), +0xf8 / +0xfc missions, +0x100 / +0x104 / +0x108 the three stops' walk-in cuboids, +0x10c
//! the infobot, +0x110 leaving, +0x114 the last route.
//!
//! ## Coverage (`0x302578`)
//! | address | what | port |
//! |---|---|---|
//! | entry | scale = class scale·2.2 (gp−0x4b08); gp−0x4b00 set → the flash 1898 at 0x13f420 (`0x30bef8(·, ticks(25), 0x80808080, 0x101040)`), cleared | [`update`] (gp−0x4b00: no writer on level 6 [L], a unit word) |
//! | 0 | map side 0; the mission (+0xf8) done → the group +0xcc deleted (`0x304028`), else flag 0x13d3ac = 0; update 0xff, draw 0x80; Ratchet's light / ambient; → 1; blend 2; +0xbc = +0xc0 = 0 | [`update`] |
//! | 1 | not standing on it on the ground (the help box busy with text or voice on, Ratchet's ground moby not it, or airborne) and no forced route (gp−0x4b00): leaving → go; else at stop 0 in +0x100 → to stop 2 (cuboid +0xa8); at 2 in +0x104 → stop 0 (+0xa0); not at 1 in +0x108 → stop 1 (+0xa4), map side 1, flag 0x13d3a9 = 1 | [`update`] |
//! | 1 | on it: the prompt (`0x302fd8`); △ pressed with the prompt (owner 10) and a route (+0x9c ≥ 0): leaving, map side 0; route 0 → +0xbc = 1, music (4, 8), 2.2 → 0.6; 1 → +0xbc = 0, music (0, 10), 0.6 → 2.2; 2 → +0xbc 0, 0.6 → 2.2; 3 → +0xbc 2, 2.2; 4 → +0xbc 0, 2.2 | [`update`] |
//! | 1 | go: → 3, `PromptRelease(10)`, blend 0, `FadeToBlack(ticks(10))`, +0xb0 = 0; `CameraScript(the camera path's start, (0, −pitch, yaw) toward the look path's start, 2, ticks(300), 0)`, the springs (`0x312b40(0.001, 1, 1, 0.001, 1, 1)`), the targets; `HeroTeleport(cuboid +0xac, 0x72, 0)`; `PlayClassSound(2, 0)` | [`update`] (`cinematic::{fade_to_black, script_springs}`) |
//! | 2 | Ratchet in +0xf0: the last route (2, +0x114 = 1), leaving, → 1 | [`update`] |
//! | 3 | the flight (`0x303160`) done: → 4, +0xf4 = 0; at the stop +0xbc's cuboid, rows from its Euler, class scale; the map side; `HeroTeleport(position − 2·row 0 + row 2, (0, 0, yaw + π), 0, 0)`; the last route: flag 0x13d3ac = 1, the infobot shown (`0x2e6148`), → 6, blend 2 over 30; `CameraScript2(0)` | [`update`] |
//! | 4 | → 1, blend 2 over 30 | [`update`] |
//! | 6 | Ratchet within 3.5 (xy) of the infobot: `HeroTeleport(its position, its Euler + π, 0x72, 1)`, music (0, 10), scene 12, the infobot deleted, `UnlockPlanet(5)`, → 8, `SetMissionDone(+0xf8)`, the checkpoint there | [`update`] |
//! | 7 | out of mode 2: the Hoverboard (item 30) not owned → `ShowPlanetBanner(5)`, → 1, the body's idle (`0x239528`), save; owned → scene 5, → 9 | [`update`] (`HeroCall::BodyIdle` [L: the level-6 copy leaves body 2 alone]) |
//! | 8 | out of mode 2: movie 5 (`DialogStreamUpdate`), → 7 | [`update`] |
//! | 9 | out of mode 2: `ShowPlanetBanner(5)`, save, → 1 | [`update`] |
//! | tail | `CarryRiders(+0x20, position − old, old rotation, rotation)` | [`update`] (`triggers::carry_riders`) |
//!
//! ## Coverage (the helpers)
//! | address | what | port |
//! |---|---|---|
//! | 0x302fd8 | the route by the stop: 1 → 1; 2 → 4; 0 → mission +0xf8 not done → 0, else +0xfc not done → 3, else −1; with 0x141c00 = 0, route 0 and the help text or voice on: `Help_Request(0x177a, 0x53)`, route −1; else `try_set_help_message(10, 0x1782 / 0x1785 / 0x1786 / 0x1784)` | [`prompt`] |
//! | 0x303160 | progress `Approach(1, 1/ticks(1200))`; the camera on its path (lerp), the shuttle on its (the fraction of the camera path's index [L: the game's]); its yaw / pitch blended between the segments, pitch ·cos(+0xb0), yaw + +0xb0; the camera's look from the look path; scale from +0xc4 to +0xc8; past 0.8 the turnaround (`0x270cc0(π, 4π·dt², …, 2π·dt)`); the camera path's point w ≥ 0 (or the previous one's): both set to −1, w < 8 → `0x29aec0(0)` (the level def 2 + 0, `World::play_level_def`) and a burst (`0x303748(6, m, cuboid +0xd0[w], its first row, 6)`), w = 10 → `0x29aec0(1)` and the blast (`0x303b68`); done at 1 | [`flight`] |
//! | 0x303748 | the death burst (`0x273f50(2, 13, m, p, −1)`); n type-11 spark pairs (`SPARK_A` / `SPARK_B`, 400000·s, 10·dt·s); 5n type-02 pieces (`randf(0, 30)` / `randf(0, 10)`·dt speeds, sizes `randf(0.05, 0.1)`·s / `randf(0.025, 0.05)`·s, colours tween 0x600040ff..0x6000ffff / 0x30002080..0x30008080, phases `randf(20, 40)`, `randf(60, 72)`, `randf(203.2, 254)`, def 0x1000e) | [`burst`] |
//! | 0x303b68 | the flash 1898 at (426, 189.75, 173.07); each member of the group +0xcc a burst (30, …, 1); then each drifts out from the group's centre (30·dt, more for the smaller), spin `rand_vec(15°·dt, 5°·dt)` (`0x300f70`: state 2, `bob_block`), 20 sparks (type 02, def 0x10019); the last member's +0xbc = 30 | [`blast`] |
//! | 0x304028 | the group +0xcc's members deleted | [`delete_group`] |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::fx;
use crate::moby_update::creature::turn::turn_toward;
use crate::moby_update::creature::{self as c, add, add_rot, atan, dist2, set_len3, sub, sub_rot, DT, DT2};
use crate::moby_update::services::{pf as to_pf, pv, HeroCall, World};
use crate::moby_update::story;

pub const REFERENCE_LEVEL: u32 = 6;
pub const UPDATE_FN: u32 = 0x30_2578;
pub const CLASSES: [i16; 1] = [1109];

pub mod pvo {
    pub const CARRY: usize = 0x20;
    pub const PATH: usize = 0x60;
    pub const CAM_PATH: usize = 0x74;
    pub const LOOK_PATH: usize = 0x88;
    pub const OFFER: usize = 0x9c;
    pub const STOPS: usize = 0xa0;
    pub const SEAT: usize = 0xac;
    pub const TURN: usize = 0xb0;
    pub const TURN_V: usize = 0xb4;
    pub const SIDE: usize = 0xbc;
    pub const ROUTE: usize = 0xc0;
    pub const SCALE0: usize = 0xc4;
    pub const SCALE1: usize = 0xc8;
    pub const GROUP: usize = 0xcc;
    pub const BURSTS: usize = 0xd0;
    pub const LAST_TRIGGER: usize = 0xf0;
    pub const T: usize = 0xf4;
    pub const MISSION: usize = 0xf8;
    pub const MISSION2: usize = 0xfc;
    pub const WALK_IN: usize = 0x100;
    pub const INFOBOT: usize = 0x10c;
    pub const LEAVING: usize = 0x110;
    pub const LAST: usize = 0x114;
    pub const LEN: usize = 0x118;
}

/// gp−0x4b0c / −0x4b08: the small and large scales.
const SMALL: f32 = f32::from_bits(0x3f19_999a);
const LARGE: f32 = f32::from_bits(0x400c_cccd);
/// gp−0x4b00 / −0x4afc (unit words: a forced last route, a flash; nothing on level 6 writes them [L]).
const FORCE_ROUTE: u32 = 0x16_2100;
const FLASH_REQUEST: u32 = 0x16_2104;
/// gp−0x4af8 / −0x4af4 / −0x4af0: the flash's ticks and colours.
const FLASH_TICKS: i32 = 25;
const FLASH_C1: u32 = 0x8080_8080;
const FLASH_C2: u32 = 0x0010_1040;
/// The prompt's owner id and messages; the help request of the first ride.
const PROMPT_OWNER: i32 = 10;
const PROMPTS: [i32; 5] = [0x1782, 0x1785, 0x1786, 0x1784, 0x1785];
const HELP: (i32, i32) = (0x177a, 0x53);
/// The story flags 0x13d3a9 (station B seen) and 0x13d3ac (the last ride taken).
const FLAG_SIDE_B: usize = story::flag_index(0x13_d3a9);
const FLAG_LAST: usize = story::flag_index(0x13_d3ac);
/// The Hoverboard.
const HOVERBOARD: usize = 30;
/// The blast's flash point.
const BLAST_AT: [f32; 4] = [426.0, 189.75, f32::from_bits(0x432d_11ec), 0.0];

fn pf(w: &World, id: MobyId, o: usize) -> f32 { c::pf(w, id, o) }
fn set(w: &mut World, id: MobyId, o: usize, v: f32) { c::set_pf(w, id, o, v) }
fn pi(w: &World, id: MobyId, o: usize) -> i32 { c::pi32(w, id, o) }
fn seti(w: &mut World, id: MobyId, o: usize, v: i32) { c::set_pi32(w, id, o, v) }
fn music(w: &mut World, track: i16, stinger: i16) { if let Some(s) = w.sound.as_deref_mut() { s.music_request(track, stinger); } }

/// Path `i` of the level (empty for −1 / missing).
fn path(w: &World, i: i32) -> Vec<[f32; 4]> {
    usize::try_from(i).ok().and_then(|i| w.svc.splines.get(i)).map_or(Vec::new(), |p| p.iter().map(|q| q.map(f32::from_bits)).collect())
}

fn point(p: &[[f32; 4]], i: i32) -> [f32; 4] { usize::try_from(i).ok().and_then(|i| p.get(i)).copied().unwrap_or([0.0; 4]) }

fn lerp(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] { std::array::from_fn(|k| a[k] + (b[k] - a[k]) * t) }

/// The moby at cuboid `c`'s centre with its Euler.
fn place(w: &mut World, id: MobyId, c: i32) {
    if let Some((p, r)) = story::cuboid(w, c) {
        let m = w.mm(id);
        m.position = [p[0], p[1], p[2], m.position[3]];
        m.rotation = [r[0], r[1], r[2], m.rotation[3]];
    }
}

fn blend(w: &mut World, id: MobyId, seq: u8, ticks: i32) {
    if w.m(id).anim.seq_b != seq { w.anim_blend(id, seq, 0, ticks); }
}

/// The route's start and its scales (the cases of `0x302578`'s route switch).
fn start_route(w: &mut World, id: MobyId, route: i32) {
    let (side, s0, s1, tune) = match route {
        0 => (1, LARGE, SMALL, Some((4, 8))),
        1 => (0, SMALL, LARGE, Some((0, 10))),
        2 => (0, SMALL, LARGE, None),
        3 => (2, LARGE, LARGE, None),
        4 => (0, LARGE, LARGE, None),
        _ => return,
    };
    seti(w, id, pvo::ROUTE, route);
    seti(w, id, pvo::SIDE, side);
    set(w, id, pvo::SCALE0, s0);
    set(w, id, pvo::SCALE1, s1);
    if let Some((t, s)) = tune { music(w, t, s); }
}

/// Level06 `0x302578` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    story::pvars(w, id, pvo::LEN);
    let (old_pos, old_rot) = (w.m(id).position, w.m(id).rotation);
    let cs = super::class_scale(w, w.m(id).o_class);
    w.mm(id).scale = cs * LARGE;
    if w.svc.units.word(FLASH_REQUEST) != 0 {
        w.svc.units.set_word(FLASH_REQUEST, 0);
        let p = crate::hero::physics::to_f32x3(w.hero.pos);
        let t = w.ticks(FLASH_TICKS);
        super::veldin_shots::flash_new(w, [p[0], p[1], p[2], 0.0], t, FLASH_C1, FLASH_C2);
    }
    match w.m(id).state {
        0 => {
            w.svc.map.alt = false;
            if story::mission_done(w, pi(w, id, pvo::MISSION)) { delete_group(w, id); } else { story::set_flag(w, FLAG_LAST, 0); }
            let (l, a) = w.hero_moby.map_or((0, [0; 4]), |h| (w.m(h).light, w.m(h).ambient));
            {
                let m = w.mm(id);
                m.update_dist = 0xff;
                m.draw_dist = 0x80;
                m.state = 1;
                m.light = l;
                m.ambient = a;
            }
            blend(w, id, 2, 0);
            seti(w, id, pvo::SIDE, 0);
            seti(w, id, pvo::ROUTE, 0);
        }
        1 => stop(w, id),
        2 => {
            if story::hero_in(w, pi(w, id, pvo::LAST_TRIGGER)) {
                start_route(w, id, 2);
                seti(w, id, pvo::LEAVING, 1);
                seti(w, id, pvo::LAST, 1);
                w.mm(id).state = 1;
            }
        }
        3 => {
            let route = pi(w, id, pvo::ROUTE);
            if flight(w, id, route) {
                w.mm(id).state = 4;
                set(w, id, pvo::T, 0.0);
                let side = pi(w, id, pvo::SIDE);
                place(w, id, pi(w, id, pvo::STOPS + 4 * side as usize));
                w.build_matrix(id);
                w.mm(id).scale = cs;
                w.svc.map.alt = side == 1;
                if side == 1 && story::flag(w, FLAG_SIDE_B) == 0 { story::set_flag(w, FLAG_SIDE_B, 1); }
                let m = w.m(id);
                let back = set_len3(m.rows[0], -2.0);
                let up = m.rows[2];
                let p = add(add(m.position, back), up);
                let e = [0.0, 0.0, add_rot(m.rotation[2], std::f32::consts::PI)];
                crate::cinematic::hero_teleport(w, [p[0], p[1], p[2]], e, 0, false);
                if pi(w, id, pvo::LAST) != 0 {
                    story::set_flag(w, FLAG_LAST, 1);
                    if let Some(b) = story::link(w, pi(w, id, pvo::INFOBOT)) { crate::moby_update::classes::infobot::show(w, b); }
                    seti(w, id, pvo::LAST, 0);
                    w.mm(id).state = 6;
                    let t = w.ticks(0x1e);
                    blend(w, id, 2, t);
                }
                crate::cinematic::camera_script2(w, 0);
            }
        }
        4 => {
            w.mm(id).state = 1;
            let t = w.ticks(0x1e);
            blend(w, id, 2, t);
        }
        6 => {
            if let Some(b) = story::link(w, pi(w, id, pvo::INFOBOT)) {
                let bp = w.m(b).position;
                if dist2(story::hero4(w), bp) < 3.5 {
                    let r = w.m(b).rotation;
                    let e = [r[0], r[1], add_rot(r[2], f32::from_bits(0x4049_0fd0))];
                    let p = [bp[0], bp[1], bp[2]];
                    crate::cinematic::hero_teleport(w, p, e, 0x72, true);
                    music(w, 0, 10);
                    crate::cinematic::start_scene(w, 0xc, false);
                    w.delete_moby(b);
                    crate::cinematic::unlock_planet(w, 5);
                    w.mm(id).state = 8;
                    let mission = pi(w, id, pvo::MISSION) as u8;
                    crate::cinematic::set_mission_done(w, mission);
                    crate::moby_update::classes::checkpoint::record(w, crate::moby_update::classes::checkpoint::Record { pos: p, rot: e });
                }
            }
        }
        7 => {
            if w.svc.game_mode != 2 {
                if !super::hints::owned(w, HOVERBOARD) {
                    crate::cinematic::show_planet_banner(w, 5);
                    w.mm(id).state = 1;
                    w.hero_fields_mut().call(HeroCall::BodyIdle);
                    crate::cinematic::save(w);
                } else {
                    crate::cinematic::start_scene(w, 5, false);
                    w.mm(id).state = 9;
                }
            }
        }
        8 if w.svc.game_mode != 2 => {
            crate::cinematic::start_movie(w, 5);
            w.mm(id).state = 7;
        }
        9 if w.svc.game_mode != 2 => {
            crate::cinematic::show_planet_banner(w, 5);
            crate::cinematic::save(w);
            w.mm(id).state = 1;
        }
        _ => {}
    }
    let (p, r) = (w.m(id).position, w.m(id).rotation);
    let delta = sub(p, old_pos);
    crate::moby_update::triggers::carry_riders(&mut w.mm(id).pvars, pvo::CARRY, delta, old_rot, r);
}

/// State 1: at a stop (module doc).
fn stop(w: &mut World, id: MobyId) {
    let h = &w.svc.help;
    let busy = (h.bx.text_on || h.bx.voice_on) && !h.idle();
    let off = busy || w.hero.ground_moby != Some(id) || w.hero.air_ticks != 0;
    let forced = w.svc.units.word(FORCE_ROUTE) != 0;
    let mut go = false;
    if off && !forced {
        if pi(w, id, pvo::LEAVING) != 0 {
            go = true;
        } else {
            let side = pi(w, id, pvo::SIDE);
            let walk = |w: &World, k: usize| story::hero_in(w, pi(w, id, pvo::WALK_IN + 4 * k));
            if side == 0 && walk(w, 0) {
                place(w, id, pi(w, id, pvo::STOPS + 8));
                seti(w, id, pvo::SIDE, 2);
                w.svc.map.alt = false;
            } else if side == 2 && walk(w, 1) {
                place(w, id, pi(w, id, pvo::STOPS));
                seti(w, id, pvo::SIDE, 0);
                w.svc.map.alt = false;
            } else if side != 1 && walk(w, 2) {
                place(w, id, pi(w, id, pvo::STOPS + 4));
                seti(w, id, pvo::SIDE, 1);
                w.svc.map.alt = true;
                if story::flag(w, FLAG_SIDE_B) == 0 { story::set_flag(w, FLAG_SIDE_B, 1); }
            }
        }
    } else {
        prompt(w, id);
        let chosen = forced || (w.hero.loop_in.pad.pressed & 0x10 != 0 && if w.svc.interact.prompt.owner == PROMPT_OWNER { pi(w, id, pvo::OFFER) >= 0 } else { false });
        if chosen {
            seti(w, id, pvo::LEAVING, 1);
            w.svc.map.alt = false;
            if forced {
                seti(w, id, pvo::OFFER, 2);
                w.svc.units.set_word(FORCE_ROUTE, 0);
            }
            let r = pi(w, id, pvo::OFFER);
            start_route(w, id, r);
        }
        go = pi(w, id, pvo::LEAVING) != 0;
    }
    if go { depart(w, id); }
}

/// The departure (`LAB_00302a1c`).
fn depart(w: &mut World, id: MobyId) {
    seti(w, id, pvo::LEAVING, 0);
    w.mm(id).state = 3;
    let up = w.svc.interact.prompt_hud;
    w.svc.interact.prompt.release(PROMPT_OWNER, up);
    if w.m(id).anim.seq_b != 0 { w.anim_blend(id, 0, 0, 0); }
    let t = w.ticks(10);
    crate::cinematic::fade_to_black(w, t);
    set(w, id, pvo::TURN, 0.0);
    let route = pi(w, id, pvo::ROUTE) as usize;
    let a = point(&path(w, pi(w, id, pvo::PATH + 4 * route)), 0);
    let b = point(&path(w, pi(w, id, pvo::CAM_PATH + 4 * route)), 0);
    let yaw = atan(b[0] - a[0], b[1] - a[1]);
    let pitch = -atan(dist2(a, b), b[2] - a[2]);
    let e = [0.0, pitch, yaw];
    let at = [a[0], a[1], a[2]];
    let t = w.ticks(300);
    crate::cinematic::camera_script(w, at, e, 2, t, false);
    crate::cinematic::script_springs(w, [f32::from_bits(0x3a83_126f), 1.0, 1.0, f32::from_bits(0x3a83_126f), 1.0, 1.0]);
    crate::cinematic::camera_targets(w, Some(at), Some(e));
    story::teleport_to(w, pi(w, id, pvo::SEAT), 0x72, false);
    w.play_sound(2, 0, id);
}

/// Level06 `0x302fd8` (module doc).
fn prompt(w: &mut World, id: MobyId) {
    let offer = match pi(w, id, pvo::SIDE) {
        1 => 1,
        2 => 4,
        0 => {
            if !story::mission_done(w, pi(w, id, pvo::MISSION)) { 0 } else if !story::mission_done(w, pi(w, id, pvo::MISSION2)) { 3 } else { -1 }
        }
        _ => pi(w, id, pvo::OFFER),
    };
    seti(w, id, pvo::OFFER, offer);
    let h = &w.svc.help;
    // 0x141c00: help record 0x53's count (0x141968 + 8·0x53).
    if h.records.help[HELP.1 as usize].count == 0 && offer == 0 && (h.bx.text_on || h.bx.voice_on) {
        w.svc.help.request(HELP.0, HELP.1);
        seti(w, id, pvo::OFFER, -1);
    } else if let Some(&m) = usize::try_from(offer).ok().and_then(|k| PROMPTS.get(k)) {
        w.svc.interact.try_prompt(PROMPT_OWNER, m);
    }
}

/// Level06 `0x303160(route)`: one tick of the flight; true at its end (module doc).
fn flight(w: &mut World, id: MobyId, route: i32) -> bool {
    let r = route.max(0) as usize;
    let (pi_, ci, li) = (pi(w, id, pvo::PATH + 4 * r), pi(w, id, pvo::CAM_PATH + 4 * r), pi(w, id, pvo::LOOK_PATH + 4 * r));
    let (cam_path, look_path, path_) = (path(w, pi_), path(w, ci), path(w, li));
    let mut t = pf(w, id, pvo::T);
    let n = w.ticks(0x4b0);
    c::turn::approach(1.0, 1.0 / n as f32, &mut t);
    set(w, id, pvo::T, t);
    let fp = t * (cam_path.len() as f32 - 1.0);
    let i = fp as i32;
    let fr = fp - i as f32;
    let ic = (t * (look_path.len() as f32 - 1.0)) as i32;
    let fl_ = t * (path_.len() as f32 - 1.0);
    let frl = fl_ - i as f32;
    let il = fl_ as i32;
    let cam = lerp(point(&cam_path, i), point(&cam_path, i + 1), fr);
    let p = lerp(point(&path_, il), point(&path_, il + 1), frl);
    w.mm(id).position = [p[0], p[1], p[2], w.m(id).position[3]];
    crate::cinematic::camera_targets(w, Some([cam[0], cam[1], cam[2]]), None);
    let turn = pf(w, id, pvo::TURN);
    if il < 1 {
        let (a, b) = (point(&path_, 0), point(&path_, 1));
        let m = w.mm(id);
        m.rotation[1] = -atan(dist2(a, b), b[2] - a[2]);
        m.rotation[2] = atan(b[0] - a[0], b[1] - a[1]);
    } else {
        let (a, b, d) = (point(&path_, il - 1), point(&path_, il), point(&path_, il + 1));
        let y0 = atan(b[0] - a[0], b[1] - a[1]);
        let y1 = atan(d[0] - b[0], d[1] - b[1]);
        let yaw = add_rot(sub_rot(y1, y0) * frl, y0);
        let e0 = atan(dist2(a, b), b[2] - a[2]);
        let e1 = atan(dist2(b, d), d[2] - b[2]);
        let pitch = add_rot(sub_rot(-e1, -e0) * frl, -e0) * turn.cos();
        let m = w.mm(id);
        m.rotation[1] = pitch;
        m.rotation[2] = add_rot(yaw, turn);
    }
    if 0 < i && 0 < ic {
        let (a, b) = (point(&look_path, ic), point(&look_path, ic + 1));
        let y0 = atan(a[0] - cam[0], a[1] - cam[1]);
        let y1 = atan(b[0] - cam[0], b[1] - cam[1]);
        let yaw = add_rot(sub_rot(y1, y0) * frl, y0);
        let e0 = atan(dist2(cam, a), a[2] - cam[2]);
        let e1 = atan(dist2(cam, b), b[2] - cam[2]);
        let pitch = add_rot(sub_rot(-e1, -e0) * frl, -e0);
        crate::cinematic::camera_targets(w, None, Some([0.0, pitch, yaw]));
    }
    let cs = super::class_scale(w, w.m(id).o_class);
    let (s0, s1) = (pf(w, id, pvo::SCALE0), pf(w, id, pvo::SCALE1));
    w.mm(id).scale = cs * (s0 - (s0 - s1) * t);
    if 0.8 < t {
        let (mut a, mut v) = (pf(w, id, pvo::TURN), pf(w, id, pvo::TURN_V));
        turn_toward(std::f32::consts::PI, DT2 * 12.566_371, DT2 * 12.566_371, DT * std::f32::consts::TAU, &mut a, &mut v);
        set(w, id, pvo::TURN, a);
        set(w, id, pvo::TURN_V, v);
    }
    let k = point(&cam_path, i)[3] as i32;
    let prev = if i == 0 { -1 } else { point(&cam_path, i - 1)[3] as i32 };
    if 0 <= k || 0 <= prev {
        if let Ok(pi_u) = usize::try_from(pi_) {
            if let Some(sp) = w.svc.splines.get_mut(pi_u) {
                let minus1 = (-1.0f32).to_bits();
                if let Some(q) = usize::try_from(i).ok().and_then(|i| sp.get_mut(i)) { q[3] = minus1; }
                if let Some(q) = usize::try_from(i - 1).ok().and_then(|i| sp.get_mut(i)) { q[3] = minus1; }
            }
        }
        if k < 8 && prev < 8 {
            w.play_level_def(0, 0, id);
            let c = pi(w, id, (pvo::BURSTS as i32 + 4 * k) as usize);
            if let Some(s) = w.svc.volumes.shape(rc_formats::volumes::ShapeKind::Cuboid, c) {
                let (centre, row0) = (s.centre(), s.matrix[0]);
                burst(w, 6.0, id, [centre[0], centre[1], centre[2], 0.0], row0, 6);
            }
        } else if k == 10 || prev == 10 {
            w.play_level_def(1, 0, id);
            blast(w, id);
        }
    }
    t == 1.0
}

/// Level06 `0x303748(size, m, pos, vel, n)` (module doc).
fn burst(w: &mut World, size: f32, id: MobyId, p: [f32; 4], vel: [f32; 4], n: i32) {
    fx::death_explosion(w, 2.0, 13.0, Some(id), p, -1);
    for _ in 0..n.max(0) {
        let a = w.rng.randi(6) as usize;
        let b = w.rng.randi(6) as usize;
        let (t15, t20) = (w.ticks(0xf), w.ticks(0x14));
        let life = w.rng.rand_range(t15, t20);
        let (t25, t30) = (w.ticks(0x19), w.ticks(0x1e));
        let t1 = w.rng.rand_range(t25, t30);
        w.part11(to_pf(size * 400_000.0), to_pf(DT * 10.0 * size), pv(p), pv(vel), fx::SPARK_A[a], fx::SPARK_B[b], life, t1, 0, 0);
    }
    for _ in 0..(n * 5).max(0) {
        let q = w.rng.rand_vec(0.0, 1.0);
        let at = add([q[0], q[1], q[2], 0.0], p);
        let r = w.rng.rand_vec(0.0, 0.9);
        let d1 = add([r[0], r[1], r[2], 0.0], vel);
        let s1 = w.rng.randf(0.0, 30.0);
        let mut v1 = c::scale(d1, s1 * DT);
        let r = w.rng.rand_vec(0.0, 0.9);
        let d2 = add([r[0], r[1], r[2], 0.0], vel);
        let s2 = w.rng.randf(0.0, 10.0);
        let mut v2 = c::scale(d2, s2 * DT);
        v1[3] = w.rng.randf(0.05, 0.1) * size;
        v2[3] = w.rng.randf(0.025, 0.05) * size;
        let f = w.rng.randf(0.0, 1.0);
        let c1 = crate::hud::tween_color(f, 0x6000_40ff, 0x6000_ffff);
        let f = w.rng.randf(0.0, 1.0);
        let c2 = crate::hud::tween_color(f, 0x3000_2080, 0x3000_8080);
        let t0 = w.rng.randf(20.0, 40.0) as i32;
        let t1 = w.rng.randf(60.0, 72.0) as i32;
        let t2 = w.rng.randf(254.0 * 0.8, 254.0) as i32;
        fx::part02(w, &crate::particles::type02::Spawn { pos: at, v1, v2, c1, c2, t: [t0, t1, t2], def: 0x1_000e });
    }
}

/// Level06 `0x303b68` (module doc).
fn blast(w: &mut World, id: MobyId) {
    let t = w.ticks(FLASH_TICKS);
    super::veldin_shots::flash_new(w, BLAST_AT, t, FLASH_C1, FLASH_C2);
    let g = pi(w, id, pvo::GROUP);
    let Some(list) = usize::try_from(g).ok().and_then(|g| w.svc.groups.lists.get(g).cloned().flatten()) else { return };
    let members: Vec<MobyId> = list.iter().map(|&e| (e & 0x7fff) as usize).filter(|&m| m < w.table.mobys.len()).collect();
    if members.is_empty() { return; }
    let mut sum = [0.0f32; 4];
    let mut rmax = 0.0f32;
    for &m in &members {
        let r = w.rng.rand_vec(1.0, 2.0);
        let p = w.m(m).position;
        burst(w, 30.0, id, p, [r[0], r[1], r[2], 0.0], 1);
        sum = add(sum, p);
        let rad = w.m(m).bsphere[3];
        if rmax < rad { rmax = rad; }
    }
    let centre = c::scale(sum, 1.0 / members.len() as f32);
    for &m in &members {
        let p = w.m(m).position;
        let r = w.m(m).bsphere[3];
        let mut v = set_len3(sub(p, centre), -((r - rmax) / rmax) * DT * 30.0 + DT * 30.0);
        v[3] = 6.0;
        let s = w.rng.rand_vec(DT * 0.261_799_4, DT * 0.087_266_46);
        super::bob_block::drift(w, m, v, [s[0], s[1], s[2], 0.0]);
        for _ in 0..20 {
            let q = w.rng.rand_vec(0.0, r * (1.0 / 2048.0));
            let at = add([q[0], q[1], q[2], 0.0], p);
            let q = w.rng.rand_vec(0.0, 40.0 * DT);
            let mut v1 = [q[0], q[1], q[2], 0.0];
            let mut v2 = [0.0f32; 4];
            v1[3] = w.rng.randf(2.0, 1.0);
            v2[3] = w.rng.randf(2.0, 1.0);
            let f = w.rng.randf(0.0, 1.0);
            let c1 = crate::hud::tween_color(f, 0x8040_80ff, 0x8040_ffff);
            let f = w.rng.randf(0.0, 1.0);
            let c2 = crate::hud::tween_color(f, 0x8040_80ff, 0x8040_ffff);
            let f = w.rng.randf(0.0, 1.0);
            let t0 = (f + 1.0) as i32;
            let f = w.rng.randf(-0.2, 0.2);
            let t1 = (240.0 * (f + 1.0)) as i32;
            let f = w.rng.randf(-0.2, 0.2);
            let t2 = (240.0 * (f + 1.0)) as i32;
            spark02(w, &crate::particles::type02::Spawn { pos: at, v1, v2, c1, c2, t: [t0, t1, t2], def: 0x1_0019 });
        }
    }
    if let Some(&last) = members.last() { w.mm(last).cmd = 0x1e; }
}

/// `PartType02Spawn` with the record's byte 9 = `trunc(4) − 0x20` (the blast's sparks).
fn spark02(w: &mut World, a: &crate::particles::type02::Spawn) {
    let Some(sys) = w.particles.as_deref_mut() else {
        fx::part02(w, a);
        return;
    };
    *w.svc.fx.part_spawns.entry(2).or_default() += 1;
    match crate::particles::type02::spawn(sys, w.rng, a) {
        Some(i) => sys.pool.recs[i][9] = 4u8.wrapping_sub(0x20),
        None => w.svc.fx.part_failed += 1,
    }
}

/// Level06 `0x304028`: the group's members deleted.
fn delete_group(w: &mut World, id: MobyId) {
    let g = pi(w, id, pvo::GROUP);
    let Some(list) = usize::try_from(g).ok().and_then(|g| w.svc.groups.lists.get(g).cloned().flatten()) else { return };
    for e in list {
        let m = (e & 0x7fff) as usize;
        if m < w.table.mobys.len() { w.delete_moby(m); }
    }
}
