//! Infobots, class 750: `InfobotUpdate` level01 0x2fbf80 with its helpers 0x2fbb20 (path flight), 0x2fcd88 (glow
//! init) and 0x2fce68 (glow). The same 3592-byte function in 14 overlays (clusters.tsv 2dc87352: levels 0, 1, 3–8,
//! 10, 12–15, 17). Spec: docs/plan/hero_gameplay.md §6, docs/plan/menus.md §6. Native `f32`.
//!
//! An infobot is the planet-coordinates pickup: it waits (or flies a path to Ratchet), and when he reaches it the
//! planet is unlocked and its story sequence plays: **scene → holofilm (PSS movie) → scene**, then the save and the
//! planet banner.
//!
//! **Pvars** (0xd0 bytes, s32 words unless noted): +0x00 shown at start (0: hidden and frozen until something else
//! wakes it), +0x04 the planet it unlocks, +0x08 a checkpoint cuboid (−1), +0x0c / +0x0d / +0x0e u8 scene, movie,
//! scene (0xff: none), +0x10.. the glow block ([`gold_bolt::glow`]), +0x50 an attach cuboid, +0x54 the moby it rides
//! (class 822, state +0xbc = 6 releases it), +0x58 the flight spline after the ride, +0x5c f32 orbit angle, +0x60
//! f32 speed, +0x64 / +0x68 / +0x6c f32 the Euler x / y / z spring velocities, +0x70..+0x8c eight paths to fly
//! towards Ratchet (−1 ends the list), +0xa4 f32 the end orbit's radius, +0xa8 the current path, +0xac f32 the
//! resting yaw, +0xb0 a hidden-until-touched flag (state 2 instead of 1), +0xc0 vec4 the home position.
//!
//! **States** (+0x20):
//! * **0** init: update distance 0xff; the resting yaw and home kept. +0x00 = 0: hidden and frozen (mode |= 0x41),
//!   state 1 (2 with +0xb0); else state 2 with the glow init. +0x50 ≠ −1: state 5 (glow init). Else, with a first
//!   path: every path's segment lengths recomputed (w of points 0..n−3), placed on path 0's first point, state 3.
//! * **1**: nothing (the Novalis infobot, the scene actors' stand-in: +0x00 = 0, no path).
//! * **2** (waiting): Euler springs to (0, 0, resting yaw); the glow; planet already unlocked → deleted; Ratchet
//!   within 1 (3 when hidden with +0xb0, or for planet 12) with health → 8.
//! * **3** (at a path's start): the glow; planet unlocked → deleted; no path left → 2; turns to Ratchet; Ratchet
//!   within 8 → 4 and its talk slot's "talked" flag set (`FUN_0027b438(infobot, 1)`).
//! * **4** (flying the path, 0x2fbb20): 2 units ahead of the nearest point, speed ±16·dt² up to 16·dt (braking to
//!   0.1·dt near the end), banking; at the end (within 1, slow) → 3 with the next path.
//! * **5** (riding a moby): orbits its attach cuboid (radius 1, 3 up, π/s); the ride's moby 822 in state 6 → 6 on
//!   the flight spline. **6** flies the spline like 4 (to 0 speed at its end) → **7**: orbits the end point (radius
//!   growing to 1), Ratchet within 2 with health → 8.
//! * **8** (collected): hidden; `UnlockPlanet(+0x04)`, `SetMissionDone(+0xb0)`, the checkpoint (+0x08); scene +0x0c
//!   → **9**: when no scene runs, movie +0x0d → **10**: when none runs, scene +0x0e → **0xb**: when none runs,
//!   `memcard_Save`, `ShowPlanetBanner(+0x04)`, deleted.
//!
//! The riding placement (after every state but 8..0xb): with an attach cuboid and a class-822 moby, position = the
//! cuboid's centre + the orbit offset; a ride that is a carrier moves the cuboid's centre itself each tick with its
//! carry first (`FUN_002752c0` in place on the level's cuboid record: `triggers::carried`, G-CLS-024).
//!
//! **Novalis.** The one infobot (instance 874) has +0x00 = 0, no path and no scene bytes: it stays in state 1. The
//! Novalis Infobot the player gets is the talk system's: the Water Pump Worker 774 sells it for 500 bolts (talk
//! node kind 1, item 37, purchase + chain), plays scene 1 → movie 2 → scene 2 and unlocks Aridia (`UnlockPlanet(2)`,
//! `ShowPlanetBanner(2)`: `classes::talking_npc`); the mission's reward is the mission NPC's (`classes::mission_npc`:
//! scene 3 → movie 3 → scene 4, Kerwan).

use super::gold_bolt::{glow_init, item_glow};
use crate::cinematic;
use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::creature::turn::spring_turn;
use crate::moby_update::creature::{add_rot, atan, sub_rot};
use crate::moby_update::interact::GameWrite;
use crate::moby_update::services::{pvar as p, World};
use std::f32::consts::{FRAC_PI_2, FRAC_PI_3, FRAC_PI_4, FRAC_PI_6, PI};

/// The update address in the level01 class table.
pub const UPDATE_FN: u32 = 0x2fbf80;
/// Classes that run [`update`].
pub const CLASSES: [i16; 1] = [750];
/// The overlays whose class table runs this exact function for 750 (clusters.tsv 2dc87352).
pub const LEVELS: [u32; 14] = [0, 1, 3, 4, 5, 6, 7, 8, 10, 12, 13, 14, 15, 17];
/// The moby class an infobot rides (`0x336`).
pub const RIDE_CLASS: i16 = 0x336;

const DT: f32 = 1.0 / 60.0;
const DT2: f32 = DT * DT;
/// `fGpffffb080`: the braking floor, × dt.
const MIN_SPEED: f32 = 0.1;

/// Pvar offsets.
pub mod pv {
    pub const SHOWN: usize = 0x00;
    pub const PLANET: usize = 0x04;
    pub const CHECKPOINT: usize = 0x08;
    pub const SCENE_A: usize = 0x0c;
    pub const MOVIE: usize = 0x0d;
    pub const SCENE_B: usize = 0x0e;
    pub const GLOW: usize = 0x10;
    pub const ATTACH: usize = 0x50;
    pub const RIDE: usize = 0x54;
    pub const FLIGHT: usize = 0x58;
    pub const ORBIT: usize = 0x5c;
    pub const SPEED: usize = 0x60;
    pub const VEL_X: usize = 0x64;
    pub const VEL_Y: usize = 0x68;
    pub const VEL_Z: usize = 0x6c;
    pub const PATHS: usize = 0x70;
    pub const RADIUS: usize = 0xa4;
    pub const PATH: usize = 0xa8;
    pub const REST_YAW: usize = 0xac;
    pub const TOUCH: usize = 0xb0;
    pub const HOME: usize = 0xc0;
    pub const LEN: usize = 0xd0;
}

fn f3(v: [f32; 4]) -> [f32; 3] { [v[0], v[1], v[2]] }
fn dist3(a: [f32; 3], b: [f32; 3]) -> f32 { ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt() }
fn dist2(a: [f32; 3], b: [f32; 3]) -> f32 { ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt() }
fn hero(w: &World) -> [f32; 3] { crate::hero::physics::to_f32x3(w.hero.pos) }
fn pi32(w: &World, id: MobyId, o: usize) -> i32 { p::i32(&w.m(id).pvars, o) }
fn pff(w: &World, id: MobyId, o: usize) -> f32 { p::ff(&w.m(id).pvars, o) }
fn set_pff(w: &mut World, id: MobyId, o: usize, x: f32) { p::set_ff(&mut w.mm(id).pvars, o, x) }
fn byte(w: &World, id: MobyId, o: usize) -> u8 { p::u8(&w.m(id).pvars, o) }

/// Spline `i` of `0x1b0930` as points (x, y, z, w = segment length).
fn spline(w: &World, i: i32) -> Option<Vec<[f32; 4]>> {
    let s = w.svc.splines.get(usize::try_from(i).ok()?)?;
    Some(s.iter().map(|q| q.map(f32::from_bits)).collect())
}

/// The init's `w[k] = |p[k+1] − p[k]|` for k < n − 2 (written into the shared spline table, as the game does).
fn measure(w: &mut World, i: i32) {
    let Some(s) = usize::try_from(i).ok().and_then(|i| w.svc.splines.get_mut(i)) else { return };
    let n = s.len();
    for k in 0..n.saturating_sub(2) {
        let (a, b) = (s[k].map(f32::from_bits), s[k + 1].map(f32::from_bits));
        s[k][3] = dist3(f3(a), f3(b)).to_bits();
    }
}

fn cuboid(w: &World, i: i32) -> Option<([f32; 3], [f32; 3])> {
    let s = w.svc.volumes.shape(rc_formats::volumes::ShapeKind::Cuboid, i)?;
    Some((s.centre(), s.euler))
}

fn planet_unlocked(w: &World, id: MobyId) -> bool {
    let pl = pi32(w, id, pv::PLANET);
    usize::try_from(pl).ok().and_then(|k| w.svc.interact.game.planet_unlocked.get(k)).is_some_and(|&b| b != 0)
}

/// The glow 0x2fce68 (at the moby's position, only while shown; no sprites in game mode 2).
fn glow(w: &mut World, id: MobyId) {
    if w.m(id).mode & mode::HIDDEN != 0 { return; }
    let c = f3(w.m(id).position);
    let spawn = w.svc.game_mode != 2;
    item_glow(w, id, pv::GLOW, c, 1.0, spawn);
}

/// The Euler x / y springs of the flight (0x2fbb20's tail; `LAB_002fcb8c`): pitch to `speed·20°·cos(a)/(16·dt)`,
/// roll to `−speed·20°·sin(a)/(16·dt)`, a = the move's heading relative to the yaw.
fn bank(w: &mut World, id: MobyId, step: [f32; 3]) {
    let a = sub_rot(atan(step[0], step[1]), w.m(id).rotation[2]);
    let speed = pff(w, id, pv::SPEED);
    let k = 0.349_065_84 / (DT * 16.0);
    let (mut vy, mut vx) = (pff(w, id, pv::VEL_Y), pff(w, id, pv::VEL_X));
    let y = spring_turn(w.m(id).rotation[1], speed * k * a.cos(), DT2 * FRAC_PI_6, DT2 * FRAC_PI_3, DT * FRAC_PI_4, &mut vy);
    let x = spring_turn(w.m(id).rotation[0], -speed * k * a.sin(), DT2 * FRAC_PI_3, DT2 * (2.0 * FRAC_PI_3), DT * FRAC_PI_2, &mut vx);
    set_pff(w, id, pv::VEL_Y, vy);
    set_pff(w, id, pv::VEL_X, vx);
    let m = w.mm(id);
    m.rotation[1] = y;
    m.rotation[0] = x;
}

/// Pitch and roll back to 0 (the waiting states).
fn level_out(w: &mut World, id: MobyId, acc_y: f32, acc_x: f32) {
    let (mut vy, mut vx) = (pff(w, id, pv::VEL_Y), pff(w, id, pv::VEL_X));
    let y = spring_turn(w.m(id).rotation[1], 0.0, acc_y * FRAC_PI_6, acc_y * FRAC_PI_3, DT * FRAC_PI_4, &mut vy);
    let x = spring_turn(w.m(id).rotation[0], 0.0, acc_x * FRAC_PI_3, DT2 * (2.0 * FRAC_PI_3), DT * FRAC_PI_2, &mut vx);
    set_pff(w, id, pv::VEL_Y, vy);
    set_pff(w, id, pv::VEL_X, vx);
    let m = w.mm(id);
    m.rotation[1] = y;
    m.rotation[0] = x;
}

fn turn_yaw(w: &mut World, id: MobyId, target: f32, acc: f32, damp: f32, max: f32) {
    let mut v = pff(w, id, pv::VEL_Z);
    let y = spring_turn(w.m(id).rotation[2], target, acc, damp, max, &mut v);
    set_pff(w, id, pv::VEL_Z, v);
    w.mm(id).rotation[2] = y;
}

/// Where the flight aims: 2 units past the nearest point of `pts` (segment length = the first segment's, the paths
/// being evenly spaced), and the end point. `(seg, t)` of the aim, t in segment lengths.
fn aim(pts: &[[f32; 4]], pos: [f32; 3]) -> (usize, f32) {
    let n = pts.len();
    let len0 = pts[0][3];
    let c = crate::spline::nearest(pts, false, 1000.0, 5.0, 0.0, pos).map(|x| x.1).unwrap_or_default();
    let d = c.seg as f32 * len0 + c.t + 2.0;
    let mut seg = (d / len0) as i32;
    let mut t = (d - seg as f32 * len0) / len0;
    if seg >= n as i32 - 1 {
        seg = n as i32 - 2;
        t = 1.0;
    }
    (seg.max(0) as usize, t)
}

fn lerp_point(pts: &[[f32; 4]], seg: usize, t: f32) -> [f32; 3] {
    let (a, b) = (f3(pts[seg]), f3(pts[seg + 1]));
    std::array::from_fn(|k| (b[k] - a[k]) * t + a[k])
}

/// The move toward `target` limited to the speed, then the banking.
fn step_to(w: &mut World, id: MobyId, target: [f32; 3]) {
    let pos = f3(w.m(id).position);
    let d = [target[0] - pos[0], target[1] - pos[1], target[2] - pos[2]];
    let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    let speed = pff(w, id, pv::SPEED);
    let s = if l > speed { speed } else { l };
    let step = if l == 0.0 { [0.0; 3] } else { d.map(|x| x * (s / l)) };
    let m = w.mm(id);
    for (p, d) in m.position.iter_mut().zip(step) { *p += d; }
    bank(w, id, step);
}

/// The speed rule: brake (16·dt² a tick, not below `floor`) within the stopping distance of `end`, else speed up
/// to 16·dt.
fn speed_rule(w: &mut World, id: MobyId, end: [f32; 3], floor: f32) {
    let pos = f3(w.m(id).position);
    let v = pff(w, id, pv::SPEED);
    let acc = DT2 * 16.0;
    let s = if dist3(pos, end) <= v * v / (acc + acc) { (v - acc).max(floor) } else { (v + acc).min(DT * 16.0) };
    set_pff(w, id, pv::SPEED, s);
}

/// 0x2fbb20: one tick of the flight along path `idx`; at the end → `next`.
fn fly_path(w: &mut World, id: MobyId, idx: i32, next: u8) {
    let Some(pts) = spline(w, idx).filter(|s| s.len() >= 2) else { return };
    let pos = f3(w.m(id).position);
    let (seg, t) = aim(&pts, pos);
    let end = f3(pts[pts.len() - 1]);
    speed_rule(w, id, end, MIN_SPEED * DT);
    let target = lerp_point(&pts, seg, t);
    if dist2(end, pos) < 1.0 && pff(w, id, pv::SPEED) <= MIN_SPEED * DT { w.mm(id).state = next; }
    let look = if seg + 1 < pts.len().saturating_sub(8) { f3(pts[seg + 1]) } else { hero(w) };
    turn_yaw(w, id, atan(look[0] - pos[0], look[1] - pos[1]), 0.01, 0.3, 0.1);
    step_to(w, id, target);
}

/// `InfobotUpdate` (0x2fbf80).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pv::LEN { w.mm(id).pvars.resize(pv::LEN, 0); }
    let mut offset = [0.0f32; 3];
    match w.m(id).state {
        0 => {
            init(w, id);
        }
        2 => {
            let hidden = w.m(id).mode & mode::HIDDEN != 0;
            let r = if (pi32(w, id, pv::TOUCH) != 0 && hidden) || pi32(w, id, pv::PLANET) == 0xc { 3.0 } else { 1.0 };
            let rest = pff(w, id, pv::REST_YAW);
            turn_yaw(w, id, rest, 0.01, 0.3, 0.1);
            level_out(w, id, DT2, DT2);
            glow(w, id);
            if planet_unlocked(w, id) {
                w.delete_moby(id);
                return;
            }
            if dist3(hero(w), f3(w.m(id).position)) < r && w.hero.health != 0 { w.mm(id).state = 8; }
        }
        3 => {
            glow(w, id);
            if planet_unlocked(w, id) {
                w.delete_moby(id);
                return;
            }
            let k = pi32(w, id, pv::PATH).clamp(0, 7) as usize;
            if pi32(w, id, pv::PATHS + 4 * k) == -1 {
                w.mm(id).state = 2;
            } else {
                let (h, pos) = (hero(w), f3(w.m(id).position));
                turn_yaw(w, id, atan(h[0] - pos[0], h[1] - pos[1]), 0.01, 0.3, 0.1);
                level_out(w, id, DT2, DT2);
                if dist3(h, pos) < 8.0 {
                    w.mm(id).state = 4;
                    let g = crate::moby_update::interact::talk_slot(w, id);
                    if g >= 0 {
                        if let Some(t) = w.svc.interact.game.talked.get_mut(g as usize) { *t = 1; }
                        w.svc.interact.writes.push(GameWrite::Talked(g as usize, 1));
                    }
                }
            }
        }
        4 => {
            let k = pi32(w, id, pv::PATH).clamp(0, 7) as usize;
            let path = pi32(w, id, pv::PATHS + 4 * k);
            fly_path(w, id, path, 3);
            glow(w, id);
            if w.m(id).state != 4 {
                let n = pi32(w, id, pv::PATH) + 1;
                p::set_i32(&mut w.mm(id).pvars, pv::PATH, n);
            }
        }
        5 => {
            glow(w, id);
            if planet_unlocked(w, id) {
                w.delete_moby(id);
                return;
            }
            let a = add_rot(pff(w, id, pv::ORBIT), DT * PI);
            set_pff(w, id, pv::ORBIT, a);
            offset = [a.cos(), a.sin(), 3.0];
            w.mm(id).rotation[2] = add_rot(a, FRAC_PI_2);
            if let Some(r) = ride(w, id) {
                let flight = pi32(w, id, pv::FLIGHT);
                if w.m(r).cmd == 6 && flight != -1 {
                    if let Some(pts) = spline(w, flight).filter(|s| s.len() >= 2) {
                        w.mm(id).state = 6;
                        p::set_i32(&mut w.mm(id).pvars, pv::ATTACH, -1);
                        let (p0, p1) = (pts[0], pts[1]);
                        w.mm(id).position = p0;
                        w.mm(id).rotation[2] = atan(p1[0] - p0[0], p1[1] - p0[1]);
                        measure(w, flight);
                        return;
                    }
                }
            }
        }
        6 => {
            glow(w, id);
            flight(w, id);
            return;
        }
        7 => {
            glow(w, id);
            let Some(pts) = spline(w, pi32(w, id, pv::FLIGHT)).filter(|s| !s.is_empty()) else { return };
            let end = f3(pts[pts.len() - 1]);
            let r = (pff(w, id, pv::RADIUS) + DT).min(1.0);
            set_pff(w, id, pv::RADIUS, r);
            let a = add_rot(pff(w, id, pv::ORBIT), DT * PI);
            set_pff(w, id, pv::ORBIT, a);
            turn_yaw(w, id, add_rot(a, FRAC_PI_2), DT * (8.0 * PI), DT * (8.0 * PI), DT * (4.0 * PI));
            let m = w.mm(id);
            m.position = [a.cos() * r + end[0], a.sin() * r + end[1], end[2], m.position[3]];
            if dist3(hero(w), end) < 2.0 && w.hero.health != 0 { w.mm(id).state = 8; }
            level_out(w, id, DT2, DT2);
            return;
        }
        8 => {
            w.mm(id).mode |= mode::HIDDEN | mode::NO_ANIM;
            let planet = pi32(w, id, pv::PLANET);
            cinematic::unlock_planet(w, planet);
            let mission = w.m(id).mission;
            cinematic::set_mission_done(w, mission);
            if let Some((c, e)) = cuboid(w, pi32(w, id, pv::CHECKPOINT)) {
                super::checkpoint::record(w, super::checkpoint::Record { pos: c, rot: e });
            }
            w.mm(id).state = 9;
            let s = byte(w, id, pv::SCENE_A);
            if s != 0xff { cinematic::start_scene(w, s as usize, false); }
        }
        9 => {
            if w.svc.game_mode != 2 {
                w.mm(id).state = 10;
                let m = byte(w, id, pv::MOVIE);
                if m != 0xff { cinematic::start_movie(w, m as i32); }
            }
        }
        10 => {
            if w.svc.game_mode != 2 {
                let s = byte(w, id, pv::SCENE_B);
                if s != 0xff { cinematic::start_scene(w, s as usize, false); }
                w.mm(id).state = 0xb;
            }
        }
        0xb => {
            if w.svc.game_mode == 2 { return; }
            cinematic::save(w);
            let planet = pi32(w, id, pv::PLANET);
            cinematic::show_planet_banner(w, planet);
            w.delete_moby(id);
            return;
        }
        _ => {}
    }
    ride_place(w, id, offset);
}

fn init(w: &mut World, id: MobyId) {
    w.mm(id).update_dist = 0xff;
    let (pos, yaw) = (w.m(id).position, w.m(id).rotation[2]);
    set_pff(w, id, pv::REST_YAW, yaw);
    p::set_v4f(&mut w.mm(id).pvars, pv::HOME, pos);
    if pi32(w, id, pv::SHOWN) == 0 {
        w.mm(id).state = if pi32(w, id, pv::TOUCH) == 0 { 1 } else { 2 };
        w.mm(id).mode |= mode::HIDDEN | mode::NO_ANIM;
    } else {
        w.mm(id).state = 2;
        glow_init(w, id, pv::GLOW);
    }
    if pi32(w, id, pv::ATTACH) != -1 {
        w.mm(id).state = 5;
        glow_init(w, id, pv::GLOW);
        ride_place(w, id, [0.0; 3]);
        return;
    }
    if pi32(w, id, pv::PATHS) != -1 {
        for k in 0..8 {
            let s = pi32(w, id, pv::PATHS + 4 * k);
            if s != -1 { measure(w, s); }
        }
        if let Some(p0) = spline(w, pi32(w, id, pv::PATHS)).and_then(|s| s.first().copied()) { w.mm(id).position = p0; }
        p::set_i32(&mut w.mm(id).pvars, pv::PATH, 0);
        w.mm(id).state = 3;
    }
}

/// Level06 `0x2e6148` (the copy's `InfobotUpdate` + 0x1140): an infobot shown by another class (Blarg's shuttle 1109
/// at its last stop): state 2, mode &= ~0x41, the glow init (`0x2e5e10` = level01 `0x2fcd88`).
pub fn show(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pv::LEN { return; }
    let m = w.mm(id);
    m.state = 2;
    m.mode &= !(mode::HIDDEN | mode::NO_ANIM);
    glow_init(w, id, pv::GLOW);
}

/// The ride moby (+0x54) when it is a class-822 moby.
fn ride(w: &World, id: MobyId) -> Option<MobyId> {
    let r = usize::try_from(pi32(w, id, pv::RIDE)).ok()?;
    (r < w.table.mobys.len() && w.m(r).o_class == RIDE_CLASS).then_some(r)
}

/// `LAB_002fccb0`: on an attach cuboid of a class-822 ride: position = the cuboid's centre + `offset`.
fn ride_place(w: &mut World, id: MobyId, offset: [f32; 3]) {
    let c = pi32(w, id, pv::ATTACH);
    if c == -1 { return; }
    let Some(r) = ride(w, id) else { return };
    let Some((mut centre, _)) = cuboid(w, c) else { return };
    // `FUN_002752c0(infobot, ride, centre, 0, centre, &rot)`: a carrier moves the cuboid itself (its centre, +0x30 of
    // the level's cuboid record) with this tick's carry; the rotation result goes to a stack temporary.
    if let Some(k) = crate::moby_update::triggers::carrier(w.m(r)) {
        centre = crate::moby_update::triggers::carried(&k, centre, [0.0; 3]).0;
        let v = std::sync::Arc::make_mut(&mut w.svc.volumes);
        if let Some(s) = usize::try_from(c).ok().and_then(|i| v.cuboids.get_mut(i)) { s.matrix[3][..3].copy_from_slice(&centre); }
    }
    let m = w.mm(id);
    m.position = [centre[0] + offset[0], centre[1] + offset[1], centre[2] + offset[2], m.position[3]];
}

/// State 6: the flight spline after the ride (the path flight with a 0 floor and the end → 7).
fn flight(w: &mut World, id: MobyId) {
    let Some(pts) = spline(w, pi32(w, id, pv::FLIGHT)).filter(|s| s.len() >= 2) else { return };
    let pos = f3(w.m(id).position);
    let n = pts.len();
    let len0 = pts[0][3];
    let c = crate::spline::nearest(&pts, false, 1000.0, 5.0, 0.0, pos).map(|x| x.1).unwrap_or_default();
    let d = c.seg as f32 * len0 + c.t + 2.0;
    let end = f3(pts[n - 1]);
    speed_rule(w, id, end, 0.0);
    let mut seg = (d / len0) as i32;
    let mut t = (d - seg as f32 * len0) / len0;
    if seg >= n as i32 - 1 {
        if pff(w, id, pv::SPEED) <= DT2 * 16.0 {
            w.mm(id).state = 7;
            let m = w.mm(id);
            m.position = pts[n - 1];
            return;
        }
        seg = n as i32 - 2;
        t = 1.0;
    }
    let seg = seg.max(0) as usize;
    let target = lerp_point(&pts, seg, t);
    let dir = [pts[seg + 1][0] - pts[seg][0], pts[seg + 1][1] - pts[seg][1]];
    turn_yaw(w, id, atan(dir[0], dir[1]), 0.01, 0.3, 0.1);
    step_to(w, id, target);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aim_is_two_units_ahead_on_an_even_path() {
        let pts = crate::spline::with_lengths(&[[0.0, 0.0, 0.0], [4.0, 0.0, 0.0], [8.0, 0.0, 0.0], [12.0, 0.0, 0.0]]);
        let (seg, t) = aim(&pts, [1.0, 0.5, 0.0]);
        assert_eq!(seg, 0);
        assert!((t - 0.75).abs() < 1e-5, "{t}");
        // Past the end: the last segment's end.
        assert_eq!(aim(&pts, [11.5, 0.0, 0.0]), (2, 1.0));
    }

    /// The ride's carry moves the attach cuboid (the level's record) and the infobot sits on its centre.
    #[test]
    fn ride_carries_the_attach_cuboid() {
        use crate::moby_runtime::{Moby, MobyTable};
        use crate::moby_update::services::{pvar as p, Services};
        let mut ride = Moby { o_class: RIDE_CLASS, pvars: vec![0; 0x80], ..Moby::default() };
        ride.mode |= 0x20;
        p::set_i32(&mut ride.pvars, 8, 0x20);
        crate::moby_update::triggers::carry_riders(&mut ride.pvars, 0x20, [0.5, 0.0, 0.25, 0.0], [0.0; 4], [0.0; 4]);
        let mut bot = Moby { o_class: 0, pvars: vec![0; 0x100], ..Moby::default() };
        p::set_i32(&mut bot.pvars, pv::ATTACH, 0);
        p::set_i32(&mut bot.pvars, pv::RIDE, 0);
        let mut t = MobyTable::new(vec![ride, bot], 4);
        let hero = crate::hero::Hero::new();
        let mut rng = crate::rng::Rng::new();
        let classes = crate::moby_update::ClassTable::default();
        let mut svc = Services::new();
        let mut cub = rc_formats::volumes::Shape::default();
        for k in 0..4 { cub.matrix[k][k] = 1.0; cub.inverse[k.min(2)][k.min(2)] = 1.0; }
        cub.matrix[3] = [10.0, 20.0, 5.0, 1.0];
        svc.volumes = std::sync::Arc::new(rc_formats::volumes::Volumes { cuboids: vec![cub], ..Default::default() });
        {
            let mut w = World::new(&mut t, &hero, &mut rng, &classes, &mut svc, 1);
            ride_place(&mut w, 1, [1.0, 0.0, 3.0]);
        }
        assert_eq!(svc.volumes.cuboids[0].matrix[3][..3], [10.5, 20.0, 5.25]);
        assert_eq!(t.mobys[1].position[..3], [11.5, 20.0, 8.25]);
    }

    #[test]
    fn levels_are_the_cluster() {
        assert!(LEVELS.contains(&1) && !LEVELS.contains(&2) && !LEVELS.contains(&9));
    }
}
