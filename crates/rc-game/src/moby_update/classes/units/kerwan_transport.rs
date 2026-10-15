//! Kerwan's flying transports: the **called platforms 816** (level03 `0x2d3198`, 3 placed: #919, #920, #921) and the
//! **two-way shuttles 1012** (level03 `0x2dccc0`, 2 placed: #961, #962), both flown along the level's splines by the
//! same follower (`0x2d3918`, [`follow`]): toward the path's current point (the next one within two steps), the
//! speed springing toward the path's end (4.5·dt² both ways, at most 18·dt), the pitch to the move (±15°), the yaw to
//! the point and a roll of the yaw's turn rate × the speed × −50, each turned with an acceleration limit; the class
//! sound 0 of class 816 loops while it flies. After the states both bob (0.2, 120°/s) and wobble (0.0785, 70°/s /
//! 100°/s; the wobble writes the pitch and roll the follower turns: the game's order), then publish the tick's move
//! for whoever stands on them (`CarryRiders(+0x20, position − old, old Euler, Euler)`, `0x24ee28`: the platform block
//! the hero's carry reads; every placed 816 / 1012 has mode 0x20 and its block at +0x20). Read from the level03 decomp and
//! disassembly (the follower's arguments, the boarding test, the prompt). Native `f32`.
//!
//! **The called platform 816** (pvars 0xf8): up to five boarding cuboids (+0x80[k]); Ratchet in one (or a class's
//! request +0x74 = k) calls the platform: it appears at the start of the come path +0x94[k] (else the ride path +0xa8[k])
//! with collision and flies it to the boarding point, where it waits; standing on it shows "△" (message 0xbc9), and
//! △ walks Ratchet onto its centre and flies him along the ride path +0xa8[k] (his jumps locked and his walk braked
//! while on it); at the end it waits until he is 15 away and flies the return path +0xbc[k] back out of sight (draw
//! distance 0, no collision), or without a return path until he is 5 away (xy) and waits there for the next call. A
//! later visit's load deletes a platform whose save bits are set. **The shuttle 1012** (pvars 0xb8): hidden until
//! Ratchet enters its cuboid +0xa0 (or always with +0x8c), then flies to the end of the two paths +0x90 / +0x94 he is
//! nearer (from the other end); standing on it shows its destination (the message of +0x98 / +0x9c by the other end)
//! and △ flies him to the other end; it follows him when he walks to the other end, once he stepped off and is 2 away.
//!
//! ## Coverage: the follower `0x2d3918(m, path, &voice)`
//! | address | what | port |
//! |---|---|---|
//! | | target = point +0x64 of the path (`0x1b05b0` = level03's spline table); \|target − pos\| < 2·18·dt → +0x64 + 1, done when it equals the count | [`follow`] |
//! | | `FUN_0024a3b8(dist(pos, last point), 4.5·dt², 4.5·dt², 18·dt, &0, +0x60)` (= `0x270830`, the spring) | [`follow`] (`turn::spring`) |
//! | | move = unit(target − pos)·+0x60 (`FastVecNormalize`), pos += move; pitch −`FastArcTan`(\|move.xy\|, move.z) in ±0x3e860a92 | [`follow`] |
//! | | `0x24a848` (= `0x270cc0`) ×3: pitch (180°·dt², 4π·dt, +0x44, +0x6c), yaw to the target (45°·dt², +0x48, +0x68), roll +0x68·+0x60·−50 (45°·dt², +0x40, +0x70) | [`follow`] (`turn::turn_toward`) |
//! | | not done: `SoundIsAlive(m, voice)` false → voice = `PlayClassSoundByClass(0, 4, m, 816)` (`0x27a4d8` = `0x2a16c0`); done: the voice released when still this moby's (`release_voice_slot`), −1 | [`follow`] |
//!
//! ## Coverage: 816 `0x2d3198`
//! | address | what | port |
//! |---|---|---|
//! | | `0x254a10(m, 0x2d30d8(m))` (= `0x27b438`): the talked word = class 816 in states 2..6 | [`update_816`] (`interact::set_talked`) |
//! | | gp−0x50fc ≠ 0: the five come / ride / return paths drawn as type-41 dots (0x800000ff / 0x8000ff00 / 0x80ff0000); the word is 0 on the disc | not ported (a debug draw that never runs) |
//! | state 0 | come path +0x94 (else ride path +0xa8) of slot 0: position its point 0, yaw to point 1; draw distance 0, update distance 0xff, +0x74 = −1, → 1, collision off (+0x94 = 0); the collected byte `0x1bb784[uid]` or the death bit → `DeleteMoby` | [`update_816`] |
//! | state 1 | +0x7c ≠ 0: per slot k < 5 with a cuboid +0x80[k] (or +0x74 = k): `PointInCuboid(0x13f3d0, +0x80[k])` or +0x74 = k → collision on (class +0x10), position / yaw at the come (else ride) path's start, +0xbc = k, → 2, draw distance 0x80, +0x64 = 1, +0x74 = −1, +0x60 = 18·dt | [`update_816`] (`triggers::point_in_cuboid`) |
//! | state 2 | come path −1 or [`follow`] done → 3, +0xe4 = −1 | [`update_816`] |
//! | state 3 | the pitch turned to 0; Ratchet on it (0x13f64c = m, 0x13f65e = 0): `try_set_help_message(8, 0xbc9)` (`0x2d3100`, both branches); △ and the prompt owner 0x15f594 = 8 → `0x2236b8(yaw, pos, 0)` (= `0x249580`, the walk to the centre), → 4, +0x64 = 1 | [`update_816`] (`HeroCall::WalkTo`) |
//! | state 4 | `0x223708` (= `0x2495d0`) = 0 (not walking) → 5 | [`update_816`] |
//! | state 5 | Ratchet on it (`0x2d3140`) → 0x13f544 = 0x13f542 = 4; [`follow`] the ride path done → 6 | [`update_816`] (`HeroFields::{jump_lockout, edge_brake}`) |
//! | state 6 | the pitch turned to 0; a return path: Ratchet beyond 15 → 7, +0x64 = 1; none: beyond 5 (xy) → 1 | [`update_816`] |
//! | state 7 | [`follow`] the return path done → draw distance 0, → 1, collision off | [`update_816`] |
//! | `0x2d3c40` | `0x250fe0(0.2, 120°·dt, m, +0xec, +0xe8)` (= `0x277a00`), `0x251060(0.0785, 70°·dt, 100°·dt, m, +0xf0, +0xf4)` (= `0x277a80`) | [`tail`] (`units::bob`, `units::wobble`) |
//!
//! ## Coverage: 1012 `0x2dccc0`
//! | address | what | port |
//! |---|---|---|
//! | | +0x90 or +0x94 = −1 → nothing; `0x254a10(m, 0x2dcb30(m))`: the talked word = class 1012, state ≠ 0 and +0x7c = 1; `FastDecTimer(+0x88)` | [`update_1012`] |
//! | state 0 | k = the low byte of +0x80: +0xbc = k, position = point 0 of path +0x90[k]; +0xa4 = −1, +0x88 = 0, → 1, update / draw distance 0xff | [`update_1012`] |
//! | state 1 | near = Ratchet nearer path +0x90's point 0 than +0x94's (3-D); +0x7c = 0: +0x8c ≠ 0 or Ratchet in cuboid +0xa0 → collision on, +0xbc = near, mode &= ~0x41, position = point 0 of path +0x90[near], → 3, +0x7c = 1, +0x64 = 1, +0x84 = 0; else collision off, mode \|= 0x41 | [`update_1012`] |
//! | | +0x7c ≠ 0, not ready (+0x84) or Ratchet not on it (`0x2dcb70`): near ≠ the platform's nearer end → 3, +0xbc ^= 1, +0x64 = 1, +0x84 = 0 | [`update_1012`] |
//! | | ready and on it: `0x2dcbc8(m, +0x98[+0xbc ^ 1])`: `try_set_help_message(8, 3022 / 3023 / 3019 / 3021 for 1..4, else −1)`, slot 12 created (`queue_animation_update(12, 0, …)`, handle +0xa4) or kept up 10 ticks; △ and owner 8 → the walk (`0x249580`), +0xbc ^= 1, +0x64 = 1, +0x84 = 0, → 2 | [`update_1012`], [`prompt_1012`] (`Interact::class_shown`) |
//! | state 2 | not walking → 3 | [`update_1012`] |
//! | state 3 | Ratchet on it → 0x13f544 = 0x13f542 = 4; [`follow`] path +0x90[+0xbc] done → 1, +0x88 = `ticks(15)` | [`update_1012`] |
//! | `0x2dd0f0`, tail | the bob (0.2, 120°/s, +0xac, +0xa8) and wobble (0.0785, 70°/s / 100°/s, +0xb0, +0xb4); Ratchet not on it, beyond 2 (xy) and +0x88 = 0 → +0x84 = 1 | [`tail`], [`update_1012`] |
//!
//! **Not the game's [L]:** a path index of −1 where the game would read the spline table at −1 (816's ride / return
//! paths unchecked) counts as done; the shuttle's slot-12 handle +0xa4 is not kept (the request is per tick).

use crate::moby_runtime::MobyId;
use crate::moby_update::classes::units::{bob, class_collision, wobble};
use crate::moby_update::creature::{self as c, turn, DT, DT2};
use crate::moby_update::interact::{self, owner};
use crate::moby_update::services::{HeroCall, World};
use crate::moby_update::triggers;

pub const REFERENCE_LEVEL: u32 = 3;
pub const PLATFORM_FN: u32 = 0x2d_3198;
pub const SHUTTLE_FN: u32 = 0x2d_ccc0;
pub const PLATFORM: i16 = 816;
pub const SHUTTLE: i16 = 1012;
pub const PLATFORM_CLASSES: [i16; 1] = [PLATFORM];
pub const SHUTTLE_CLASSES: [i16; 1] = [SHUTTLE];

/// The follower's pvars (both classes).
mod pv {
    pub const SPEED: usize = 0x60;
    pub const INDEX: usize = 0x64;
    pub const YAW_V: usize = 0x68;
    pub const PITCH_V: usize = 0x6c;
    pub const ROLL_V: usize = 0x70;
    pub const VOICE: usize = 0x78;
}

/// 816's pvars.
mod p816 {
    pub const REQUEST: usize = 0x74;
    pub const ENABLED: usize = 0x7c;
    pub const CUBOIDS: usize = 0x80;
    pub const COME: usize = 0x94;
    pub const RIDE: usize = 0xa8;
    pub const RETURN: usize = 0xbc;
    pub const E4: usize = 0xe4;
    pub const BOB_PREV: usize = 0xe8;
    pub const BOB_PHASE: usize = 0xec;
    pub const WOBBLE_A: usize = 0xf0;
    pub const WOBBLE_B: usize = 0xf4;
    pub const SIZE: usize = 0xf8;
}

/// 1012's pvars.
mod p1012 {
    pub const ACTIVE: usize = 0x7c;
    pub const START: usize = 0x80;
    pub const READY: usize = 0x84;
    pub const TIMER: usize = 0x88;
    pub const ALWAYS: usize = 0x8c;
    pub const PATHS: usize = 0x90;
    pub const MESSAGES: usize = 0x98;
    pub const CUBOID: usize = 0xa0;
    pub const HANDLE: usize = 0xa4;
    pub const BOB_PREV: usize = 0xa8;
    pub const BOB_PHASE: usize = 0xac;
    pub const WOBBLE_A: usize = 0xb0;
    pub const WOBBLE_B: usize = 0xb4;
    pub const SIZE: usize = 0xb8;
}

/// The level03 words (gp−0x50f8..−0x50e4, gp−0x5110..−0x5100 / −0x5028..−0x5018: both classes' tails are equal).
mod k {
    pub const SPEED: f32 = 18.0;
    pub const ACCEL: f32 = 4.5;
    pub const PITCH_ACC: f32 = 180.0;
    pub const YAW_ACC: f32 = 45.0;
    pub const ROLL_ACC: f32 = 45.0;
    pub const ROLL: f32 = -50.0;
    pub const PITCH_MAX: f32 = f32::from_bits(0x3e86_0a92);
    pub const DEG: f32 = f32::from_bits(0x3c8e_fa35);
    pub const FOUR_PI: f32 = f32::from_bits(0x4149_0fdb);
    pub const BOB_AMP: f32 = 0.2;
    pub const BOB_RATE: f32 = 120.0;
    pub const WOBBLE_AMP: f32 = f32::from_bits(0x3da0_d97c);
    pub const WOBBLE_A: f32 = 70.0;
    pub const WOBBLE_B: f32 = 100.0;
}

/// 816's "△" message; 1012's destinations (`0x2dcbc8`, kinds 1..4).
const PROMPT_816: i32 = 0xbc9;
const PROMPTS_1012: [i32; 4] = [3022, 3023, 3019, 3021];

fn pi(w: &World, id: MobyId, o: usize) -> i32 { c::pi32(w, id, o) }
fn spline(w: &World, path: i32) -> Option<usize> { usize::try_from(path).ok().filter(|&p| p < w.svc.splines.len() && !w.svc.splines[p].is_empty()) }
fn point(w: &World, p: usize, i: usize) -> c::V { let s = &w.svc.splines[p]; s[i.min(s.len() - 1)].map(f32::from_bits) }
fn hero(w: &World) -> c::V { crate::moby_update::classes::units::hero_pos(w) }
fn dist_xy(a: c::V, b: c::V) -> f32 { ((a[0] - b[0]) * (a[0] - b[0]) + (a[1] - b[1]) * (a[1] - b[1])).sqrt() }

/// The moby at point 0 of `path`, yawed toward point 1 (816's starts; 1012 sets the position only).
fn to_start(w: &mut World, id: MobyId, path: i32, yaw: bool) {
    let Some(p) = spline(w, path) else { return };
    let (p0, p1) = (point(w, p, 0), point(w, p, 1));
    let m = w.mm(id);
    m.position = p0;
    if yaw { m.rotation[2] = c::atan(p1[0] - p0[0], p1[1] - p0[1]); }
}

/// The collision on (class +0x10) or off (+0x94 = 0).
fn collision(w: &mut World, id: MobyId, on: bool) {
    let o = w.m(id).o_class;
    let has = on && class_collision(w, o);
    w.mm(id).has_collision = has;
}

/// The pitch turned back to 0 (816's states 3 and 6).
fn level_pitch(w: &mut World, id: MobyId) {
    let a = k::PITCH_ACC * k::DEG * DT2;
    let (mut ang, mut v) = (w.m(id).rotation[1], c::pf(w, id, pv::PITCH_V));
    turn::turn_toward(0.0, a, a, DT * k::FOUR_PI, &mut ang, &mut v);
    w.mm(id).rotation[1] = ang;
    c::set_pf(w, id, pv::PITCH_V, v);
}

/// Ratchet's walk onto the centre (`0x249580(yaw, pos, 0)`).
fn walk_on(w: &mut World, id: MobyId) {
    let m = w.m(id);
    let (point, yaw) = ([m.position[0], m.position[1], m.position[2]], m.rotation[2]);
    w.hero_fields_mut().call(HeroCall::WalkTo { point, yaw, release: 0 });
}

/// `0x2495d0` ≠ 0: Ratchet still walks to the point or stands at it.
fn walking(w: &World) -> bool { matches!(w.hero.state, 0x65..=0x67) }

/// Ratchet riding: his jumps locked and his walk braked (0x13f542 / 0x13f544 = 4).
fn lock_hero(w: &mut World) {
    let f = w.hero_fields_mut();
    f.jump_lockout = 4;
    f.edge_brake = 4;
}

/// The follower `0x2d3918(m, path, &voice)` (module table): true once the path's last point was reached.
pub fn follow(w: &mut World, id: MobyId, path: i32) -> bool {
    let Some(p) = spline(w, path) else { return true };
    let n = w.svc.splines[p].len();
    let i = pi(w, id, pv::INDEX).max(0) as usize;
    let target = point(w, p, i);
    let pos = c::pos(w, id);
    let d = c::sub(target, pos);
    let mut done = false;
    let step = k::SPEED * DT;
    if c::len3(d) < step + step {
        c::set_pi32(w, id, pv::INDEX, (i + 1) as i32);
        done = i + 1 == n;
    }
    let last = point(w, p, n - 1);
    let dist = c::dist3(pos, last);
    let (mut x, mut v) = (0.0, c::pf(w, id, pv::SPEED));
    let acc = k::ACCEL * DT2;
    turn::spring(dist, acc, acc, k::SPEED * DT, &mut x, &mut v);
    c::set_pf(w, id, pv::SPEED, v);
    let mv = c::set_len3(d, v);
    let pos = c::add(pos, mv);
    w.mm(id).position = pos;
    let hl = (mv[0] * mv[0] + mv[1] * mv[1]).sqrt();
    let pitch = (-c::atan(hl, mv[2])).clamp(-k::PITCH_MAX, k::PITCH_MAX);
    let vmax = DT * k::FOUR_PI;
    let rot = |w: &World, id: MobyId, axis: usize, vel: usize| (w.m(id).rotation[axis], c::pf(w, id, vel));
    let turn_axis = |w: &mut World, axis: usize, vel: usize, target: f32, acc: f32| {
        let (mut a, mut v) = rot(w, id, axis, vel);
        turn::turn_toward(target, acc, acc, vmax, &mut a, &mut v);
        w.mm(id).rotation[axis] = a;
        c::set_pf(w, id, vel, v);
    };
    turn_axis(w, 1, pv::PITCH_V, pitch, k::PITCH_ACC * k::DEG * DT2);
    let yaw = c::atan(target[0] - pos[0], target[1] - pos[1]);
    turn_axis(w, 2, pv::YAW_V, yaw, k::YAW_ACC * k::DEG * DT2);
    let roll = c::pf(w, id, pv::YAW_V) * c::pf(w, id, pv::SPEED) * k::ROLL;
    turn_axis(w, 0, pv::ROLL_V, roll, k::ROLL_ACC * k::DEG * DT2);
    let voice = pi(w, id, pv::VOICE);
    if !done {
        if !w.sound_alive(voice, id) {
            let s = w.play_sound_as(0, 4, id, PLATFORM);
            c::set_pi32(w, id, pv::VOICE, s);
        }
    } else {
        if voice != -1 && w.sound_owner(voice) == Some(id) && w.sound_alive(voice, id) { w.release_sound(voice, id); }
        c::set_pi32(w, id, pv::VOICE, -1);
    }
    done
}

/// The bob and the wobble after the states (`0x2d3c40` / `0x2dd0f0`), then `CarryRiders(+0x20, position − old, old
/// Euler, Euler)` (`0x24ee28`) with the pose the update started from: Ratchet standing on it rides its move.
fn tail(w: &mut World, id: MobyId, prev: usize, phase: usize, wa: usize, wb: usize, old: ([f32; 4], [f32; 4])) {
    bob(w, id, k::BOB_AMP, k::BOB_RATE * k::DEG * DT, phase, prev);
    wobble(w, id, k::WOBBLE_AMP, k::WOBBLE_A * k::DEG * DT, k::WOBBLE_B * k::DEG * DT, wa, wb);
    let m = w.mm(id);
    let disp = std::array::from_fn(|i| m.position[i] - old.0[i]);
    let rot = m.rotation;
    crate::moby_update::triggers::carry_riders(&mut m.pvars, CARRY_BLOCK, disp, old.1, rot);
}

/// The platform block both classes carry their riders with (pvar +0x08 = 0x20 on every placed 816 / 1012).
const CARRY_BLOCK: usize = 0x20;

/// The position and Euler an update starts from (moby +0x10 / +0x40, kept before the states).
fn pose(w: &World, id: MobyId) -> ([f32; 4], [f32; 4]) { (w.m(id).position, w.m(id).rotation) }

/// 816's path of slot `slot` in the table at `base`.
fn slot_path(w: &World, id: MobyId, base: usize, slot: usize) -> i32 { pi(w, id, base + 4 * slot.min(4)) }

/// Level03 `0x2d3198`, the called platform 816 (module table).
pub fn update_816(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < p816::SIZE { w.mm(id).pvars.resize(p816::SIZE, 0); }
    let old = pose(w, id);
    let moving = (2..=6).contains(&w.m(id).state);
    interact::set_talked(w, id, moving as u32);
    let slot = w.m(id).cmd as usize;
    let state = w.m(id).state;
    match state {
        0 => {
            let path = match pi(w, id, p816::COME) { -1 => pi(w, id, p816::RIDE), p => p };
            if path != -1 { to_start(w, id, path, true); }
            let m = w.mm(id);
            m.draw_dist = 0;
            m.update_dist = 0xff;
            m.state = 1;
            c::set_pi32(w, id, p816::REQUEST, -1);
            collision(w, id, false);
            let uid = w.m(id).spawn_id;
            if w.svc.save.collected.get(&uid).is_some_and(|&b| b != 0) || w.svc.save.death.contains(&(w.svc.level, uid)) {
                w.delete_moby(id);
                return;
            }
        }
        1 => {
            if pi(w, id, p816::ENABLED) == 0 { return tail_816(w, id, old); }
            let at = hero(w);
            for k in 0..5 {
                let cuboid = slot_path(w, id, p816::CUBOIDS, k);
                let forced = pi(w, id, p816::REQUEST) == k as i32;
                if cuboid == -1 && !forced { continue; }
                let inside = triggers::point_in_cuboid(&w.svc.volumes, [at[0], at[1], at[2]], cuboid);
                if !inside && !forced { continue; }
                collision(w, id, true);
                let path = match slot_path(w, id, p816::COME, k) { -1 => slot_path(w, id, p816::RIDE, k), p => p };
                if path != -1 { to_start(w, id, path, true); }
                let m = w.mm(id);
                m.cmd = k as u8;
                m.state = 2;
                m.draw_dist = 0x80;
                c::set_pi32(w, id, pv::INDEX, 1);
                c::set_pi32(w, id, p816::REQUEST, -1);
                c::set_pf(w, id, pv::SPEED, k::SPEED * DT);
                break;
            }
        }
        2 => {
            let path = slot_path(w, id, p816::COME, slot);
            if path == -1 || follow(w, id, path) {
                w.mm(id).state = 3;
                c::set_pi32(w, id, p816::E4, -1);
            }
        }
        3 => {
            level_pitch(w, id);
            if w.hero.ground_moby != Some(id) || w.hero.air_ticks != 0 { return tail_816(w, id, old); }
            w.svc.interact.try_prompt(owner::TRANSPORT, PROMPT_816);
            if w.svc.interact.triangle() && w.svc.interact.prompt.owner == owner::TRANSPORT {
                walk_on(w, id);
                w.mm(id).state = 4;
                c::set_pi32(w, id, pv::INDEX, 1);
            }
        }
        4 => {
            if !walking(w) { w.mm(id).state = 5; }
        }
        5 => {
            if w.hero_on_moby(id) { lock_hero(w); }
            if follow(w, id, slot_path(w, id, p816::RIDE, slot)) { w.mm(id).state = 6; }
        }
        6 => {
            level_pitch(w, id);
            let pos = c::pos(w, id);
            if slot_path(w, id, p816::RETURN, slot) != -1 {
                if 15.0 < c::dist3(pos, hero(w)) {
                    w.mm(id).state = 7;
                    c::set_pi32(w, id, pv::INDEX, 1);
                }
            } else if 5.0 < dist_xy(pos, hero(w)) {
                w.mm(id).state = 1;
            }
        }
        7 if follow(w, id, slot_path(w, id, p816::RETURN, slot)) => {
            let m = w.mm(id);
            m.draw_dist = 0;
            m.state = 1;
            collision(w, id, false);
        }
        _ => {}
    }
    tail_816(w, id, old);
}

fn tail_816(w: &mut World, id: MobyId, old: ([f32; 4], [f32; 4])) { tail(w, id, p816::BOB_PREV, p816::BOB_PHASE, p816::WOBBLE_A, p816::WOBBLE_B, old); }

/// `0x2dcbc8(m, kind)`: the destination's message in the prompt (owner 8), and the prompt element raised now.
pub fn prompt_1012(w: &mut World, kind: i32) {
    let msg = usize::try_from(kind - 1).ok().and_then(|i| PROMPTS_1012.get(i)).copied().unwrap_or(-1);
    w.svc.interact.try_prompt(owner::TRANSPORT, msg);
    w.svc.interact.class_shown = true;
}

/// Level03 `0x2dccc0`, the shuttle 1012 (module table).
pub fn update_1012(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < p1012::SIZE { w.mm(id).pvars.resize(p1012::SIZE, 0); }
    let old = pose(w, id);
    let (a, b) = (pi(w, id, p1012::PATHS), pi(w, id, p1012::PATHS + 4));
    if a == -1 || b == -1 { return; }
    let active = w.m(id).state != 0 && pi(w, id, p1012::ACTIVE) == 1;
    interact::set_talked(w, id, active as u32);
    let t = pi(w, id, p1012::TIMER);
    if t > 0 { c::set_pi32(w, id, p1012::TIMER, t - 1); }
    let path_of = |w: &World, id: MobyId, k: u8| pi(w, id, p1012::PATHS + 4 * (k as usize & 1));
    match w.m(id).state {
        0 => {
            let k = pi(w, id, p1012::START) as u8;
            w.mm(id).cmd = k;
            let path = pi(w, id, p1012::PATHS + 4 * k as usize);
            to_start(w, id, path, false);
            c::set_pi32(w, id, p1012::HANDLE, -1);
            c::set_pi32(w, id, p1012::TIMER, 0);
            let m = w.mm(id);
            m.state = 1;
            m.update_dist = 0xff;
            m.draw_dist = 0xff;
        }
        1 => {
            let (pa, pb) = match (spline(w, a), spline(w, b)) { (Some(pa), Some(pb)) => (point(w, pa, 0), point(w, pb, 0)), _ => return };
            let (h, pos) = (hero(w), c::pos(w, id));
            let near = c::dist3(h, pa) < c::dist3(h, pb);
            let own = c::dist3(pos, pa) < c::dist3(pos, pb);
            if pi(w, id, p1012::ACTIVE) == 0 {
                let go = pi(w, id, p1012::ALWAYS) != 0 || triggers::point_in_cuboid(&w.svc.volumes, [h[0], h[1], h[2]], pi(w, id, p1012::CUBOID));
                if !go {
                    collision(w, id, false);
                    w.mm(id).mode |= 0x41;
                } else {
                    collision(w, id, true);
                    let m = w.mm(id);
                    m.cmd = near as u8;
                    m.mode &= !0x41;
                    let path = path_of(w, id, near as u8);
                    to_start(w, id, path, false);
                    w.mm(id).state = 3;
                    c::set_pi32(w, id, p1012::ACTIVE, 1);
                    c::set_pi32(w, id, pv::INDEX, 1);
                    c::set_pi32(w, id, p1012::READY, 0);
                }
            } else if pi(w, id, p1012::READY) == 0 || !w.hero_on_moby(id) {
                if near != own {
                    let m = w.mm(id);
                    m.state = 3;
                    m.cmd ^= 1;
                    c::set_pi32(w, id, pv::INDEX, 1);
                    c::set_pi32(w, id, p1012::READY, 0);
                }
            } else {
                let other = (w.m(id).cmd ^ 1) as usize & 1;
                let kind = pi(w, id, p1012::MESSAGES + 4 * other);
                prompt_1012(w, kind);
                if w.svc.interact.triangle() && w.svc.interact.prompt.owner == owner::TRANSPORT {
                    walk_on(w, id);
                    let m = w.mm(id);
                    m.cmd ^= 1;
                    m.state = 2;
                    c::set_pi32(w, id, pv::INDEX, 1);
                    c::set_pi32(w, id, p1012::READY, 0);
                }
            }
        }
        2 => {
            if !walking(w) { w.mm(id).state = 3; }
        }
        3 => {
            if w.hero_on_moby(id) { lock_hero(w); }
            let path = path_of(w, id, w.m(id).cmd);
            if follow(w, id, path) {
                w.mm(id).state = 1;
                let t = w.ticks(15);
                c::set_pi32(w, id, p1012::TIMER, t);
            }
        }
        _ => {}
    }
    tail(w, id, p1012::BOB_PREV, p1012::BOB_PHASE, p1012::WOBBLE_A, p1012::WOBBLE_B, old);
    let pos = c::pos(w, id);
    if !w.hero_on_moby(id) && 2.0 < dist_xy(pos, hero(w)) && pi(w, id, p1012::TIMER) == 0 { c::set_pi32(w, id, p1012::READY, 1); }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moby_runtime::{Moby, MobyTable};

    /// The update ends in `CarryRiders` (0x24ee28): a parked platform's bob is published in its block (+0x20), so the
    /// hero's platform carry moves a rider with it.
    #[test]
    fn a_platform_publishes_its_move_for_its_riders() {
        let mut pv = vec![0u8; p816::SIZE];
        pv[8..12].copy_from_slice(&(CARRY_BLOCK as i32).to_le_bytes());
        let m = Moby { o_class: 816, scale: 1.0, state: 1, mode: 0x20, position: [100.0, 100.0, 50.0, 1.0], pvars: pv, hit_slot: 0xff, ..Moby::default() };
        let mut t = MobyTable::new(vec![m], 8);
        let hero = crate::hero::Hero::new();
        let mut rng = crate::rng::Rng::new();
        let classes = crate::moby_update::ClassTable::default();
        let mut svc = crate::moby_update::Services::new();
        let mut w = World::new(&mut t, &hero, &mut rng, &classes, &mut svc, 0);
        for _ in 0..10 {
            let before = w.m(0).position;
            update_816(&mut w, 0);
            let after = w.m(0).position;
            let d = crate::moby_update::triggers::platform_delta(w.m(0)).expect("a carrier");
            assert_eq!(&d.displacement[..3], &[after[0] - before[0], after[1] - before[1], after[2] - before[2]]);
        }
        assert!(crate::moby_update::triggers::platform_delta(w.m(0)).is_some_and(|d| d.displacement[2] != 0.0), "the bob moves it");
    }
}
