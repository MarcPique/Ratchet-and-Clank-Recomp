//! **Kalebo's arena troopers, class 541** (level16 `0x2cddb8` with its tick `0x2ce9f0`, mover `0x2ced20`, group tick
//! `0x2cee70` and group glow `0x2cef60`; 29 placed; census U545; the name is descriptive [L]). Asleep and hidden until
//! their arena trigger 654 wakes them ([`super::kalebo_arena`]: state 2), each glides to the first point of its path
//! and works it by kind (pvar +0xc0), carrying a weapon moby on its joint 0:
//! * **0, the grenadier** (weapon 0x35f): walks between points 1 and 0 of its path (toggling), and at each turns to
//!   lob a grenade ([`super::kalebo_grenade`]) at Ratchet (found by a region search half the time) or at its point's
//!   cuboid;
//! * **1, the sweeper** (weapon 0x374): runs the path end to end and back, a damage sphere at its joint that hits
//!   whoever it touches (a hurt sound when it is Ratchet and he is not invulnerable), turning at each point;
//! * **2, the flamer** (weapon 0x36c, with its flames): like the grenadier but sprays fire (`creature::flame`, the
//!   Aridia flamer's emitter) the length of its animation.
//!
//! Hits (mask 0x330000) take their damage off its health 2 and make it flinch toward Ratchet (state 9, then it picks up
//! where it was); at 0 it bursts into three pieces with the death explosion and its death bits, and comes back down at
//! its home (a second, from 3 above) as many times as its lives (+0xc8: its bolts shared among them), then is gone with
//! its weapon. Turned into a chicken (its damage record's +0x0e set to 2 by the Morph-o-Ray) it counts the same as a
//! death. The group's troopers glow (`0x2781d0` quads, FX 0xb, pulsing between two colours on a shared phase) and the
//! big-head cheat swells their heads. Read from the level16 decomp, its words gp−0x5238..−0x521c and the table
//! 0x1d3130. Native `f32`.
//!
//! **Pvars** (0x160): +0x20 the damage record (health; +0x24 s16 2, +0x28 1, +0x2e the script byte), +0x60 the flash
//! record, +0x70 the target record (`0x274df8`: +0xb4 its kind, 2 = none), +0xc0 the kind, +0xc4 the path, +0xc8 the
//! lives, +0xcc the float group, +0xd0 the points' cuboids, +0xd8 the target region, +0xdc the point, +0xe0 home, +0xf0
//! the timer, +0xf8 the nearest float 650, +0xfc the speed, +0x100 the turn velocity, +0x104 the weapon (moby + 1),
//! +0x108 the sweep's direction, +0x10c the flinch heading, +0x110 / +0x114 the state and sequence to resume, +0x118
//! the group tick, +0x11c the glow phase, +0x120 the flames (`creature::flame`), +0x140 the big-head node.
//!
//! | address | what | port |
//! |---|---|---|
//! | every tick | `0x2ce9f0` ([`pre`]: not in state 1); `0x25e9f0(2.5, m, 2, +0x140)` (the big-head cheat) | [`update`] (`manip::big_head`) |
//! | `0x2ce9f0` | the script byte 2 → 1, health 2, the timer `ticks(60)`, `SetDeathBits(m, 0, −1)`, untargetable, a life gone: none left → the weapon and the moby deleted, else → 0xb at home | [`pre`] |
//! | | else: the group tick (`0x2cee70`, once a tick for the group: each 541's +0x118 = the tick, its phase += 360°·dt; `RegisterDrawCallback(0x2cef60)`); `MobyGetHitMessage(m, 0x330000, 0)` → `0x255db0` (col 4); not out5 1 and not in 10 / 11: health −= damage, ≤ 0 → 10; else (not in 9: keep the state and sequence) the flinch heading at Ratchet, → 9, sequence 8 (`MobyAnimBlend(m, 8, 2, 1)`), flash 0x78; +0xa4 = 0xff; the flash; the weapon at joint 0 with the joint's rows (normalised), `MobyBuildMatrix`; the flash again | [`pre`] (`damage::resolve`, `flash`) |
//! | state 0 | +0x2e = 1; lives > 0: +0xb4 /= lives; → 1, hidden; `randi(2)` → mirrored; the record +0x28 1, +0x24 2, health 2; the phase `random_angle_radians`; the float (`0x2d5db0`: the nearest 650 of group +0xcc within 37); home = position; the weapon (0x35f / 0x374 / 0x36c with its flames cleared) keeping its rows, draw distance 0x40, update 0, drawn | [`update`] |
//! | state 2 | the float's hum (`0x2d5e80`); `0x2ced20(yaw, m, point 0)` arrived → timer `ticks(30)`, point 1, direction 1; kind 0 → 3, 1 → 5, else 7 | [`update`] ([`go`]) |
//! | state 3 | the timer out, `0x2ced20(heading to point, m, point)` arrived → timer `ticks(30)`, → 4, sequence 0x15 (`ticks(30)`); `randi(2)`: the region search (range 12, region +0xd8), else target kind 2 | [`update`] |
//! | state 4 | the aim: no target (kind 2) → the point's cuboid centre, else the target; key 15 crossed with a weapon: g = 10·dt², speed 6·dt (gp−0x5234), the aim on the ground (`GroundHeight(0.5)`), from the weapon's joint 0: dir = the flat direction · speed, z = the lob's vertical speed (`0x256528`), a grenade (fuse `ticks(300)`); else wrapped → point ^= 1, → 3, sequence 0x17; then facing the aim (key < 15) or the other point | [`update`] |
//! | state 5 | sequence 10: wrapped → `MobyAnimBlend(11, 0, 1)`; else the step to the point (at most 20·dt, gp−0x5230), the joint (list 0) damage sphere `0x2551e8(0.333, 1, 1, m, j, 1, 0, 1, 0)`: the collision output's moby (0x174258, read as the first listed [L]) Ratchet and he not invulnerable → sound 6; arrived (< 0.0001) → sequence 9, → 6, timer `ticks(60)`, direction +1 at point 0, −1 at the last | [`update`] (`attack::sphere_hit`) |
//! | state 6 | the turn toward point + direction (4π·dt², 8π·dt); the timer out → the point advances, sequence 10, → 5 | [`update`] |
//! | state 7 | as 3 for the flamer: arrived → → 8, sequence 1 | [`update`] |
//! | state 8 | facing the target (no target: none); the weapon's joint 0: the flame along its flat heading (range 3, gp−0x522c), the flames' hits (template damage 1, flags 0x10001, type 5 / 1, the class); wrapped → point ^= 1, → 7, sequence 3 | [`update`] (`flame::emit` / `flame::hits`) |
//! | state 9 | moving to the point with the flinch heading; wrapped → the kept state and sequence | [`update`] |
//! | state 10 | health 2, timer `ticks(60)`; `BreakFxB` 0x655 / 0x656 / 0x657; `0x25a988(0.5, 10, m, position + 1.2 z, −1)`; `SetDeathBits(m, 0, −1)`; sound 7; a life gone (as above) | [`update`] (`fx::death_explosion`) |
//! | state 0xb | the timer out → speed 0, → 2, sequence 9, health 2, targetable; z = home z + 3·dt·timer | [`update`] |
//! | `0x2cef60` | each 541 of the group not deleted: a glow quad of 0.2 at position + 0.2 z, colour `FastTweenColor((sin(phase) + 1)/2, 0x80804040, 0x40802020)` | [`glow_quads`] |

use super::GlowQuad;
use crate::moby_runtime::{mode, MobyId, MobyTable};
use crate::moby_update::classes::crate_::set_death_bits;
use crate::moby_update::classes::draw_callbacks::Callback;
use crate::moby_update::creature::{self as c, attack, damage, flame, flash, fx, ground, knock, target, turn, DT, DT2};
use crate::moby_update::services::{pf, pv as v4, pvar as p, HitTemplate, Services, World};
use crate::ps2v::Pf;

pub const REFERENCE_LEVEL: u32 = 16;
pub const UPDATE_FN: u32 = 0x2c_ddb8;
/// The group glow (draw only).
pub const GLOW_FN: u32 = 0x2c_ef60;
pub const CLASS: i16 = 0x21d;
pub const CLASSES: [i16; 1] = [CLASS];
/// The weapons by kind.
pub const WEAPONS: [i16; 3] = [0x35f, 0x374, 0x36c];
/// The classes whose joints it reads: its own and its weapons'.
pub const JOINTS: [i16; 4] = [CLASS, 0x35f, 0x374, 0x36c];
/// The float the trooper rides up on.
pub const FLOAT: i16 = 0x28a;
/// gp−0x5234 (throw speed ×dt), −0x5230 (sweep speed ×dt), −0x522c (flame range), −0x5224 (glow spin, °/s).
const THROW: f32 = 6.0;
const SWEEP: f32 = 20.0;
const FLAME_RANGE: f32 = 3.0;
const SPIN: f32 = 360.0;
/// gp−0x5220 / −0x521c: the glow's colours.
const GLOW: (u32, u32) = (0x8080_4040, 0x4080_2020);
/// The float's hum: the last tick it played (gp−0x5178, level16 0x161a88).
const HUM_TICK: u32 = 0x16_1a88;

/// Pvar offsets (module doc).
pub mod pv {
    pub const D: usize = 0x20;
    pub const FLASH: usize = 0x60;
    pub const TARGET: usize = 0x70;
    pub const KIND: usize = 0xc0;
    pub const PATH: usize = 0xc4;
    pub const LIVES: usize = 0xc8;
    pub const FLOATS: usize = 0xcc;
    pub const CUBOIDS: usize = 0xd0;
    pub const REGION: usize = 0xd8;
    pub const POINT: usize = 0xdc;
    pub const HOME: usize = 0xe0;
    pub const TIMER: usize = 0xf0;
    pub const FLOAT: usize = 0xf8;
    pub const SPEED: usize = 0xfc;
    pub const TURN_V: usize = 0x100;
    pub const WEAPON: usize = 0x104;
    pub const DIR: usize = 0x108;
    pub const FLINCH: usize = 0x10c;
    pub const RESUME: usize = 0x110;
    pub const RESUME_SEQ: usize = 0x114;
    pub const GROUP_TICK: usize = 0x118;
    pub const PHASE: usize = 0x11c;
    pub const FLAMES: usize = 0x120;
    pub const HEAD: usize = 0x140;
    pub const SIZE: usize = 0x160;
}

fn moby_ref(w: &World, id: MobyId, o: usize) -> Option<MobyId> {
    let v = c::pi32(w, id, o);
    (v > 0).then(|| (v - 1) as MobyId).filter(|&m| m < w.table.mobys.len())
}

fn path(w: &World, id: MobyId) -> Vec<c::V> {
    usize::try_from(c::pi32(w, id, pv::PATH)).ok().and_then(|i| w.svc.splines.get(i)).map(|s| s.iter().map(|q| q.map(f32::from_bits)).collect()).unwrap_or_default()
}

fn point(w: &World, id: MobyId, k: i32) -> c::V { path(w, id).get(usize::try_from(k).unwrap_or(0)).copied().unwrap_or(w.m(id).position) }

fn blend(w: &mut World, id: MobyId, seq: u8, frame: i32, n: i32) {
    let t = w.ticks(n);
    c::blend_to(w, id, seq, frame, t);
}

fn blend_unless(w: &mut World, id: MobyId, seq: u8, n: i32) {
    if w.m(id).anim.seq_b != seq { blend(w, id, seq, 0, n); }
}

fn wrapped(w: &World, id: MobyId) -> bool { w.m(id).anim.flags & 2 != 0 }

/// `0x2ced20(yaw, m, to)`: turn toward `yaw` (4π·dt², 8π·dt) and move toward `to` at a speed springing toward the
/// distance (12·dt², 6·dt); arrived when nearer than 0.05 at a speed below 0.005.
pub fn go(w: &mut World, id: MobyId, yaw: f32, to: c::V) -> bool {
    let (mut a, mut av) = (w.m(id).rotation[2], c::pf(w, id, pv::TURN_V));
    turn::turn_toward(yaw, DT2 * 12.566_371, DT2 * 12.566_371, DT * 25.132_742, &mut a, &mut av);
    w.mm(id).rotation[2] = a;
    c::set_pf(w, id, pv::TURN_V, av);
    let pos = w.m(id).position;
    let d = [to[0] - pos[0], to[1] - pos[1], to[2] - pos[2], to[3] - pos[3]];
    let len = c::len3(d);
    let (mut x, mut s) = (0.0, c::pf(w, id, pv::SPEED));
    turn::spring(len, DT2 * 12.0, DT2 * 12.0, DT * 6.0, &mut x, &mut s);
    c::set_pf(w, id, pv::SPEED, s);
    let step = c::set_len3(d, s);
    w.mm(id).position = c::add(pos, step);
    len < 0.05 && s < 0.005
}

fn heading_to(w: &World, id: MobyId, to: c::V) -> f32 {
    let p = w.m(id).position;
    c::atan(to[0] - p[0], to[1] - p[1])
}

/// The life lost (the death and the Morph-o-Ray): none left → gone with the weapon, else back at home (0xb).
fn lose_life(w: &mut World, id: MobyId) {
    let n = c::pi32(w, id, pv::LIVES) - 1;
    c::set_pi32(w, id, pv::LIVES, n);
    if n == -1 {
        if let Some(wp) = moby_ref(w, id, pv::WEAPON) { w.delete_moby(wp); }
        w.delete_moby(id);
        return;
    }
    w.mm(id).state = 0xb;
    let home = c::pv4(w, id, pv::HOME);
    w.mm(id).position = home;
    let t = w.ticks(60);
    c::set_pi32(w, id, pv::TIMER, t);
}

/// `0x2cee70`: the group's tick (module doc).
fn group_tick(w: &mut World, id: MobyId) {
    let g = w.m(id).group;
    if g == -1 { return; }
    let rate = SPIN * 0.017_453_292 * DT;
    let list = usize::try_from(g).ok().and_then(|g| w.svc.groups.lists.get(g).cloned().flatten()).unwrap_or_default();
    let tick = w.counter as i32;
    for k in list.into_iter().map(usize::from) {
        if k >= w.table.mobys.len() || w.m(k).o_class != CLASS || w.m(k).pvars.len() < pv::SIZE { continue; }
        c::set_pi32(w, k, pv::GROUP_TICK, tick);
        let ph = c::add_rot(c::pf(w, k, pv::PHASE), rate);
        c::set_pf(w, k, pv::PHASE, ph);
    }
    if let Some(r) = super::row(REFERENCE_LEVEL, GLOW_FN) { w.svc.draw_callbacks.register(Callback::UnitGlow(r), id); }
}

/// `0x2ce9f0` (module doc).
fn pre(w: &mut World, id: MobyId) {
    if w.m(id).state == 1 { return; }
    if c::pu8(w, id, pv::D + 0xe) == 2 {
        c::set_pu8(w, id, pv::D + 0xe, 1);
        c::set_pf(w, id, pv::D, 2.0);
        let t = w.ticks(60);
        c::set_pi32(w, id, pv::TIMER, t);
        set_death_bits(w, id, 0, -1);
        w.mm(id).mode &= !mode::TARGETABLE;
        lose_life(w, id);
        return;
    }
    if c::pi32(w, id, pv::GROUP_TICK) != w.counter as i32 { group_tick(w, id); }
    let hit = w.get_hit(id, 0x33_0000, false);
    let res = damage::resolve(w, id, hit, pv::D, 0, 4);
    let st = w.m(id).state;
    if res.out5 != 1 && st != 10 && st != 0xb {
        let hp = c::pf(w, id, pv::D) - res.damage;
        c::set_pf(w, id, pv::D, hp);
        if hp <= 0.0 {
            w.mm(id).state = 10;
        } else {
            if st != 9 {
                c::set_pi32(w, id, pv::RESUME, st as i32);
                c::set_pi32(w, id, pv::RESUME_SEQ, w.m(id).anim.seq_b as i32);
            }
            let h = super::hero_pos(w);
            let a = heading_to(w, id, h);
            c::set_pf(w, id, pv::FLINCH, a);
            w.mm(id).state = 9;
            if w.m(id).anim.seq_b != 8 { c::blend_to(w, id, 8, 2, 1); }
            c::set_pu8(w, id, pv::FLASH + 7, 0x78);
            flash::start(w, id, pv::FLASH);
        }
    }
    w.mm(id).hit_slot = 0xff;
    flash::update(w, id, pv::FLASH);
    if let Some(wp) = moby_ref(w, id, pv::WEAPON) {
        let j = w.joint_point(id, 0);
        let mx = w.joint_matrix(id, 0);
        let m = w.mm(wp);
        m.position = j;
        for (row, r) in m.rows.iter_mut().zip(mx.iter()).take(3) { *row = c::set_len3(*r, 1.0); }
        w.build_matrix(wp);
    }
    flash::update(w, id, pv::FLASH);
}

/// `0x2d5e80(float)`: its hum, at most every 4 ticks for all of them, re-armed every 5.
fn float_hum(w: &mut World, id: MobyId) {
    let Some(f) = moby_ref(w, id, pv::FLOAT) else { return };
    if w.m(f).pvars.len() < 0xc { return; }
    if c::pi32(w, f, 8) == 0 {
        let last = w.svc.units.word(HUM_TICK) as i32;
        if 3 < (last - w.counter as i32).abs() {
            w.svc.units.set_word(HUM_TICK, w.counter as u32);
            w.play_sound(0, 0, f);
        }
    }
    let t = w.ticks(5);
    c::set_pi32(w, f, 8, t);
}

/// `0x2d5db0(group, at)`: the nearest float of the group within 37.
fn nearest_float(w: &World, g: i32, at: c::V) -> Option<MobyId> {
    let list = usize::try_from(g).ok().and_then(|g| w.svc.groups.lists.get(g).cloned().flatten())?;
    let mut best = (37.0f32, None);
    for k in list.into_iter().map(usize::from) {
        if k >= w.table.mobys.len() || w.m(k).o_class != FLOAT { continue; }
        let d = c::dist3(at, w.m(k).position);
        if d < best.0 { best = (d, Some(k)); }
    }
    best.1
}

fn search(w: &mut World, id: MobyId) {
    let region = usize::try_from(c::pi32(w, id, pv::REGION)).ok().filter(|&i| w.svc.splines.get(i).is_some_and(|s| !s.is_empty()));
    let t = target::acquire_in(w, id, 12.0, region);
    let o = pv::TARGET;
    c::set_pv4(w, id, o, t.pos);
    c::set_pv4(w, id, o + 0x10, t.rot);
    c::set_pv4(w, id, o + 0x20, t.aim);
    c::set_pv4(w, id, o + 0x30, t.body);
    c::set_pi32(w, id, o + 0x40, t.moby.map_or(0, |m| m as i32 + 1));
    c::set_pi32(w, id, o + 0x44, t.kind as i32);
}

fn no_target(w: &World, id: MobyId) -> bool { c::pi32(w, id, pv::TARGET + 0x44) == 2 }

/// The aim of states 4 / 8: the point's cuboid without a target, else the target's position.
fn aim(w: &World, id: MobyId) -> c::V {
    if no_target(w, id) {
        let k = c::pi32(w, id, pv::POINT);
        let cub = c::pi32(w, id, pv::CUBOIDS + 4 * usize::try_from(k).unwrap_or(0));
        if let Some(s) = w.svc.volumes.shape(rc_formats::volumes::ShapeKind::Cuboid, cub) { return s.matrix[3]; }
        w.m(id).position
    } else {
        c::pv4(w, id, pv::TARGET)
    }
}

/// The arrival of states 3 / 7 at a point: the timer, the next state and sequence, the target half the time.
fn arrive(w: &mut World, id: MobyId, next: u8, seq: u8) {
    let t = w.ticks(30);
    c::set_pi32(w, id, pv::TIMER, t);
    w.mm(id).state = next;
    blend_unless(w, id, seq, 30);
    if w.rng.randi(2) != 0 {
        search(w, id);
    } else {
        c::set_pi32(w, id, pv::TARGET + 0x44, 2);
    }
}

/// Level16 `0x2cddb8` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pv::SIZE { w.mm(id).pvars.resize(pv::SIZE, 0); }
    pre(w, id);
    if w.m(id).state >= 0x80 { return; }
    crate::moby_update::manip::big_head(w, 2.5, id, 2, id, pv::HEAD);
    let st = w.m(id).state;
    match st {
        0 => {
            c::set_pu8(w, id, pv::D + 0xe, 1);
            let lives = c::pi32(w, id, pv::LIVES);
            if 0 < lives { w.mm(id).b4 = (w.m(id).b4 as i32 / lives) as i16; }
            let m = w.mm(id);
            m.state = 1;
            m.visible = 0;
            m.mode |= 1;
            if w.rng.randi(2) != 0 { w.mm(id).mode |= mode::MIRROR; }
            c::set_pu8(w, id, pv::D + 8, 1);
            c::set_pi16(w, id, pv::D + 4, 2);
            c::set_pf(w, id, pv::D, 2.0);
            let a = w.rng.rand_angle();
            c::set_pf(w, id, pv::PHASE, a);
            let pos = w.m(id).position;
            let f = nearest_float(w, c::pi32(w, id, pv::FLOATS), pos);
            c::set_pi32(w, id, pv::FLOAT, f.map_or(0, |f| f as i32 + 1));
            c::set_pv4(w, id, pv::HOME, pos);
            let kind = c::pi32(w, id, pv::KIND);
            let wp = usize::try_from(kind).ok().and_then(|k| WEAPONS.get(k)).and_then(|&cl| w.create_moby(cl));
            if kind == 2 { flame::clear(w, id, pv::FLAMES); }
            c::set_pi32(w, id, pv::WEAPON, wp.map_or(0, |m| m as i32 + 1));
            if let Some(wp) = wp {
                let m = w.mm(wp);
                m.mode |= mode::KEEP_ROWS;
                m.draw_dist = 0x40;
                m.update_dist = 0;
                m.visible = 1;
            }
        }
        2 => {
            float_hum(w, id);
            let (yaw, p0) = (w.m(id).rotation[2], point(w, id, 0));
            if !go(w, id, yaw, p0) { return; }
            let t = w.ticks(30);
            c::set_pi32(w, id, pv::TIMER, t);
            c::set_pi32(w, id, pv::POINT, 1);
            c::set_pi32(w, id, pv::DIR, 1);
            w.mm(id).state = match c::pi32(w, id, pv::KIND) { 0 => 3, 1 => 5, _ => 7 };
        }
        3 | 7 => {
            if c::dec_timer_pvar_i32(w, id, pv::TIMER) == 0 { return; }
            let to = point(w, id, c::pi32(w, id, pv::POINT));
            let yaw = if st == 3 { heading_to(w, id, to) } else { w.m(id).rotation[2] };
            if !go(w, id, yaw, to) { return; }
            if st == 3 { arrive(w, id, 4, 0x15) } else { arrive(w, id, 8, 1) }
        }
        4 => throw(w, id),
        5 => sweep(w, id),
        6 => {
            let next = c::pi32(w, id, pv::POINT) + c::pi32(w, id, pv::DIR);
            let to = point(w, id, next);
            let yaw = heading_to(w, id, to);
            let (mut a, mut v) = (w.m(id).rotation[2], c::pf(w, id, pv::SPEED));
            turn::turn_toward(yaw, DT2 * 12.566_371, DT2 * 12.566_371, DT * 25.132_742, &mut a, &mut v);
            w.mm(id).rotation[2] = a;
            c::set_pf(w, id, pv::SPEED, v);
            if c::dec_timer_pvar_i32(w, id, pv::TIMER) == 0 { return; }
            c::set_pi32(w, id, pv::POINT, next);
            blend_unless(w, id, 10, 10);
            w.mm(id).state = 5;
        }
        8 => spray(w, id),
        9 => {
            let to = point(w, id, c::pi32(w, id, pv::POINT));
            let yaw = c::pf(w, id, pv::FLINCH);
            go(w, id, yaw, to);
            if !wrapped(w, id) { return; }
            w.mm(id).state = c::pi32(w, id, pv::RESUME) as u8;
            let seq = c::pi32(w, id, pv::RESUME_SEQ) as u8;
            if w.m(id).anim.seq_b != seq { blend(w, id, seq, 0, 20); }
        }
        10 => {
            c::set_pf(w, id, pv::D, 2.0);
            let t = w.ticks(60);
            c::set_pi32(w, id, pv::TIMER, t);
            let (pos, rot) = (w.m(id).position, w.m(id).rotation);
            for class in [0x655, 0x656, 0x657] { fx::break_piece(w, id, class, pos, rot, 0, 0); }
            fx::death_explosion(w, 0.5, 10.0, Some(id), [pos[0], pos[1], pos[2] + 1.2, pos[3]], -1);
            set_death_bits(w, id, 0, -1);
            w.play_sound(7, 0, id);
            lose_life(w, id);
        }
        0xb => {
            if c::dec_timer_pvar_i32(w, id, pv::TIMER) != 0 {
                c::set_pf(w, id, pv::SPEED, 0.0);
                w.mm(id).state = 2;
                blend_unless(w, id, 9, 10);
                c::set_pf(w, id, pv::D, 2.0);
                w.mm(id).mode |= mode::TARGETABLE;
            }
            let z = c::pf(w, id, pv::HOME + 8) + DT * 3.0 * c::pi32(w, id, pv::TIMER) as f32;
            w.mm(id).position[2] = z;
        }
        _ => {}
    }
}

/// State 4: the grenadier's throw (module doc).
fn throw(w: &mut World, id: MobyId) {
    let mut at = aim(w, id);
    let wp = moby_ref(w, id, pv::WEAPON);
    if let Some(wp) = wp.filter(|_| ground::passed_frame(w, id, 15.0)) {
        let (speed, g) = (THROW * DT, DT2 * 10.0);
        at[2] = crate::moby_update::services::fl(w.ground_height(pf(0.5), v4(at), 0));
        let from = w.joint_point(wp, 0);
        let mut d = c::sub(at, from);
        d[2] = 0.0;
        let mut d = c::set_len3(d, speed);
        let mut time = 0.0;
        d[2] = knock::lob_up(speed, -g, from, at, &mut time);
        let fuse = w.ticks(300);
        super::kalebo_grenade::spawn(w, g, id, from, d, fuse);
    } else if wrapped(w, id) {
        let k = (c::pi32(w, id, pv::POINT) + 1) & 1;
        c::set_pi32(w, id, pv::POINT, k);
        w.mm(id).state = 3;
        blend_unless(w, id, 0x17, 30);
    }
    let yaw = if ground::key_time(w, id) < 15.0 {
        heading_to(w, id, at)
    } else {
        let k = (c::pi32(w, id, pv::POINT) + 1) & 1;
        heading_to(w, id, point(w, id, k))
    };
    let here = w.m(id).position;
    go(w, id, yaw, here);
}

/// State 5: the sweeper's run (module doc).
fn sweep(w: &mut World, id: MobyId) {
    if w.m(id).anim.seq_b == 10 {
        if wrapped(w, id) { c::blend_to(w, id, 11, 0, 1); }
        return;
    }
    let k = c::pi32(w, id, pv::POINT);
    let to = point(w, id, k);
    let pos = w.m(id).position;
    let d = c::clamp_len3(c::sub(to, pos), SWEEP * DT);
    w.mm(id).position = c::add(pos, d);
    let j = w.joint_point(id, 0);
    let t = attack::sphere_template(w, id, 1.0, 1.0, 1, 0, 1);
    let hit = w.sphere_mobys_list(pf(f32::from_bits(0x3eaa_7efa)), v4(j), 0, Some(id), Some(&t));
    if hit.first().is_some_and(|&m| Some(m) == w.hero_moby) && w.hero.f510 == 0 { w.play_sound(6, 0, id); }
    if 0.0001 <= c::len3(d) { return; }
    blend_unless(w, id, 9, 20);
    w.mm(id).state = 6;
    let t = w.ticks(60);
    c::set_pi32(w, id, pv::TIMER, t);
    let last = path(w, id).len() as i32 - 1;
    if k == 0 {
        c::set_pi32(w, id, pv::DIR, 1);
    } else if k == last {
        c::set_pi32(w, id, pv::DIR, -1);
    }
}

/// State 8: the flamer's spray (module doc).
fn spray(w: &mut World, id: MobyId) {
    if !no_target(w, id) {
        let pos = w.m(id).position;
        let t = c::pv4(w, id, pv::TARGET);
        let yaw = heading_to(w, id, t);
        go(w, id, yaw, pos);
    }
    if let Some(wp) = moby_ref(w, id, pv::WEAPON) {
        let j = w.joint_point(wp, 0);
        let pos = w.m(id).position;
        let a = c::atan(j[0] - pos[0], j[1] - pos[1]);
        let (ca, sa) = c::cs(a);
        flame::emit(w, id, pv::FLAMES, FLAME_RANGE, j, [ca, sa, 0.0, 0.0]);
        let t = HitTemplate { dir: [Pf::f(ca), Pf::f(sa), Pf::ONE, Pf::b(0x45af_df66)], attacker: Some(id), flags: 0x1_0001, b18: 5, b19: 1, h1a: CLASS as u16, damage: Pf::ONE, w20: 1 };
        flame::hits(w, id, pv::FLAMES, &t);
    }
    if wrapped(w, id) {
        let k = (c::pi32(w, id, pv::POINT) + 1) & 1;
        c::set_pi32(w, id, pv::POINT, k);
        w.mm(id).state = 7;
        blend_unless(w, id, 3, 30);
    }
}

/// `0x2cef60`: the group's glows (module doc; draw only).
pub fn glow_quads(table: &MobyTable, svc: &Services, id: MobyId) -> Vec<GlowQuad> {
    let Some(me) = table.mobys.get(id) else { return Vec::new() };
    let Some(Some(list)) = usize::try_from(me.group).ok().and_then(|g| svc.groups.lists.get(g)) else { return Vec::new() };
    list.iter()
        .filter_map(|&e| table.mobys.get((e & 0x7fff) as usize))
        .filter(|m| m.o_class == CLASS && m.state != 0xfe && m.state != 0xfd && m.pvars.len() >= pv::SIZE)
        .map(|m| {
            let s = p::ff(&m.pvars, pv::PHASE).sin();
            let rgba = crate::particles::tween_color(((s + 1.0) * 0.5).to_bits(), GLOW.0, GLOW.1);
            GlowQuad { size: 0.2, pull: 0.0, point: [m.position[0], m.position[1], m.position[2] + 0.2], rgba }
        })
        .collect()
}
