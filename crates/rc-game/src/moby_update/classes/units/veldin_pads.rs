//! Two level-18 devices of the boss 1422's arena (the names are descriptive [L]):
//!
//! * **U554: class 583, the boss's pads** (level18 `0x2d6600`, 4 placed in group 13, with the boss's calls `0x2d6b58`,
//!   `0x2d6c80`, `0x2d6d50`, `0x2d6dc0`). Each pad builds itself from three pieces on the ground (a base 1771 and two
//!   doors 1776); armed by the boss its doors slide open and it rises; it closes again when its arming runs out, when
//!   the level's scene 4 or later runs, or when Ratchet does his state 0x22 on the ground; the pads standing on a moby
//!   are deleted with their pieces in scene 3.
//! * **U555: class 586, the button / the countdown** (level18 `0x2d7b20`, 2 placed; the boss's calls `0x2d7f68`,
//!   `0x2d7fd0`, `0x2d8050`). With +0x60 (#209, the boss's) it is the countdown: the boss starts it (state 2: the timer
//!   +0x6c, the level sound 0x12, the on-screen countdown `0x2d8098`) and when it runs out the arena word 0x162390 is
//!   set (the boss's state 0x15 then goes to 5); Ratchet's state 0x22 on it stops it (3). Without +0x60 (#210) it is a
//!   plain floor button (4 → 5, the command byte, the level-18 flag 127) with a help hint (message 0x2b02, record 0x71)
//!   while Ratchet stands near it.
//!
//! **583 pvars** (0x90): +0x00 the ground point, +0x10 the ground z, +0x14 / +0x18 the doors 1776, +0x1c the base
//! 1771, +0x20 the doors' opening, +0x24 its target (1.5 armed, 0 shut), +0x28 the spring velocity, +0x2c the arming
//! timer, +0x30 armed for good, +0x34 the height offset (−2.8 down, −1 up), +0x38 stands on a moby.
//! **586 pvars** (0x80, mode 0x20): +0x20 the creature record (+0x3e its flags: bit 3 set at the init), +0x60 the
//! countdown kind, +0x64 the placed z, +0x68 the spring velocity, +0x6c the countdown (ticks), +0x70 the sound's voice.
//!
//! The level18 words: gp−0x5268 2.0 (583's scale factor), gp−0x5264 −1.0 / gp−0x5260 −2.8 (its up / down offset);
//! gp−0x51f0 2.0 (586's scale factor), gp−0x51ec −2.0 / gp−0x51e8 −3.0 (its two heights), gp−0x51e4 260 / −0x51e0 80
//! (the countdown's centre), gp−0x51dc / −0x51d8 0x80c0c0c0 / 0x804040ff (its colours), gp−0x51d4.. −5, 20, −50, 52
//! (its frame), gp−0x51c4 0x60 (the frame's alpha).
//!
//! ## Coverage
//!
//! **583** `0x2d6600` (every tick: scale = class scale · 2):
//!
//! | address | what it does | ported / not |
//! |---|---|---|
//! | 0 | collision off; z + 1, z = `GroundHeight(0.5, pos, 0)`; +0x00 = position; the probe hit a moby (0x174858) → +0x38 = 1 | [`pad_update`] (`World::coll_line`) |
//! | | `CreateMoby(0x6eb)`: update distance 0x40, drawn, draw distance 0x56, scale ·2, position +0x00, the pad's Euler, `0x2530e0` (matrix) → +0x1c | [`pad_update`] |
//! | | two `CreateMoby(0x6f0)`: update 0x40, drawn, draw distance 0x56 / 0x40, scale ·2, the pad's Euler (the second + π), matrix → +0x14 / +0x18 | [`pad_update`] |
//! | | state 1, +0x10 = z, +0x34 = −2.8, collision on (class +0x10) | [`pad_update`] |
//! | 1 | game mode 2, +0x38 set, scene 3 → the three pieces and the pad deleted | [`pad_update`] |
//! | | +0x24 ≠ 0: (+0x30 = 0 and `FastDecTimer(+0x2c)` = 2) or (game mode 2 and the scene past 3) → `PlayClassSound(0, 0)`, +0x24 = 0; Ratchet on the ground in state 0x22 → the same | [`pad_update`] |
//! | | +0x24 = 0 and +0x34 = −2.8, or +0x24 ≠ 0 and +0x20 ≠ +0x24 → `0x25dcb0(+0x24, 4·dt², 4·dt², 4·dt, &+0x20, &+0x28)`; else `0x25dcb0(+0x24 ? −1 : −2.8, 16·dt², 4·dt², 8·dt, &+0x34, &+0x28)` | [`pad_update`] (`turn::spring`) |
//! | | each door: position = its row 1 at length −+0x20 + its row 2 at −0.2·+0x20 + the ground point, matrix; z = +0x10 + +0x34 | [`pad_update`] |
//! | `0x2d6b58(from, to, group)` | the 583 of the group whose bearing from `from` differs most from `to`'s (`FastDiffRots`; none: 0) | [`pad_farthest_turn`] |
//! | `0x2d6c80(p, group)` | the nearest 583 of the group (3-D, from 255) | [`pad_nearest`] |
//! | `0x2d6d50(group)` | every 583 of the group: +0x24 = 1.5, +0x30 = 1 | [`pads_arm`] |
//! | `0x2d6dc0(pad)` | +0x2c = 5, +0x24 = 1.5 | [`pad_arm`] |
//!
//! **586** `0x2d7b20`:
//!
//! | address | what it does | ported / not |
//! |---|---|---|
//! | 0 | +0x64 = z; +0x3e \|= 8; +0x60 ≠ 0 → 1, scale = class scale · 2; else 4 | [`button_update`] |
//! | 1, 3 | `0x25dcb0(+0x64 − 3, 20·dt², 40·dt², 20·dt, &z, &+0x68)` | [`button_update`] |
//! | 2 | `0x25dcb0(+0x64 − 2, 5·dt², 10·dt², 20·dt, …)`; Ratchet on it (0x13f64c, grounded) in state 0x22 → 3, `PlayClassSound(0, 0)`, its voice +0x70 released (owner-guarded), −1 | [`button_update`] |
//! | | else Ratchet not in 0x72 → `RegisterDrawCallback(0x2d8098)`; `FastDecTimer(+0x6c)` out → 0x162390 = 1 | [`button_update`] (`Callback::UnitFrame`) |
//! | 4 | Ratchet on it in state 0x22 → 5, +0xbc = 1, z −= 1.5, level 18 → flag 0x13d407 (127) = 1 | [`button_update`] (`interact::set_global_flag`) |
//! | | within 2.5 (xy) and 0x15717c's bit of the level clear: help record 0x71 (0x141cf0): count 0 → 1, time / mask bumped; else `ticks(18)·60 < ticks(play) − 600·time` or time 0 → `Help_Request(0x2b02, 0x71)`, else the time raised | [`button_update`] (0x15717c: no writer found, read as 0 [L]) |
//! | 5 | `Approach(+0x64 − 1.5, 0.15, &z)` done → 6 | [`button_update`] |
//! | `0x2d7f68(m, t)` | state ≠ 2 → 2, +0x6c = t; the voice not alive → +0x70 = `0x28d878(0x12, 0, m)` (the level sound def 0x12) | [`countdown_start`] (`World::play_level_def`) |
//! | `0x2d7fd0(m)` | state 2 → 3, the voice released, −1 | [`countdown_stop`] |
//! | `0x2d8050(m)` | state 3 → 1, 1; state 2: +0x6c = 0 → −1, else 0; else 0 | [`countdown_poll`] |
//! | `0x2d8098` (draw) | `DrawUIFrame(75, 100, 210, 312, 0x60)`; colour `FastTweenColor(clamp(3·sin(2π·(frame % 60)/60) + 0.5, 0, 1), 0x80c0c0c0, 0x804040ff)`; text "0M:SS:Tr" (+0x6c over `ticks(3600)` / `ticks(600)` / `ticks(60)` / `ticks(6)`, r = `randi(10)`), `font_print_center_large(260, 80, …, 8)` | [`countdown_draw`] (the rand and the text here; the 2-D draw: `rc-engine` scene_render) |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, turn, DT, DT2};
use crate::moby_update::scheduler::group_ids;
use crate::moby_update::services::{pv as pf4, World};

/// 583's update in the level18 class table.
pub const PAD_FN: u32 = 0x2d_6600;
/// 586's.
pub const BUTTON_FN: u32 = 0x2d_7b20;
/// 586's countdown draw (the port's row for `Callback::UnitFrame`).
pub const COUNTDOWN_FN: u32 = 0x2d_8098;
pub const REFERENCE_LEVEL: u32 = 18;
pub const PAD_CLASSES: [i16; 1] = [583];
pub const BUTTON_CLASSES: [i16; 1] = [586];
pub const PAD: i16 = 0x247;
pub const BASE_PIECE: i16 = 0x6eb;
pub const DOOR: i16 = 0x6f0;
/// The countdown's run-out word (gp−0x4870).
pub const RUN_OUT_WORD: u32 = 0x16_2390;
/// Ratchet's state the pads and the button react to.
pub const PRESS: i32 = 0x22;
/// The countdown's level sound def.
pub const COUNTDOWN_SOUND: i32 = 0x12;
/// The plain button's hint.
pub const HINT_MSG: i32 = 0x2b02;
pub const HINT_REC: usize = 0x71;
/// The plain button's flag on level 18 (0x13d407).
pub const BUTTON_FLAG: usize = 127;

pub mod pad {
    pub const GROUND: usize = 0x00;
    pub const BASE_Z: usize = 0x10;
    pub const DOOR_A: usize = 0x14;
    pub const DOOR_B: usize = 0x18;
    pub const BASE: usize = 0x1c;
    pub const OPEN: usize = 0x20;
    pub const TARGET: usize = 0x24;
    pub const VEL: usize = 0x28;
    pub const TIMER: usize = 0x2c;
    pub const HELD: usize = 0x30;
    pub const LIFT: usize = 0x34;
    pub const ON_MOBY: usize = 0x38;
    pub const SIZE: usize = 0x3c;
}

pub mod button {
    pub const FLAGS: usize = 0x3e;
    pub const KIND: usize = 0x60;
    pub const HOME_Z: usize = 0x64;
    pub const VEL: usize = 0x68;
    pub const COUNT: usize = 0x6c;
    pub const VOICE: usize = 0x70;
    pub const SIZE: usize = 0x74;
}

const PAD_SCALE: f32 = 2.0;
const PAD_UP: f32 = -1.0;
const PAD_DOWN: f32 = -2.8;
const BUTTON_SCALE: f32 = 2.0;
const BUTTON_UP: f32 = -2.0;
const BUTTON_DOWN: f32 = -3.0;
/// The armed doors' opening.
pub const ARMED: f32 = 1.5;

fn link(w: &World, id: MobyId, o: usize) -> Option<MobyId> {
    usize::try_from(c::pi32(w, id, o) - 1).ok().filter(|&m| m < w.table.mobys.len())
}
fn set_link(w: &mut World, id: MobyId, o: usize, m: Option<MobyId>) { c::set_pi32(w, id, o, m.map_or(0, |m| m as i32 + 1)); }

fn scaled_class(w: &World, o: i16, k: f32) -> f32 { super::class_scale(w, o) * k }

fn on_it(w: &World, id: MobyId) -> bool { w.hero.air_ticks == 0 && w.hero.ground_moby == Some(id) }

// ---------------------------------------------------------------------------------------------------------------
// 583

/// Level18 0x2d6600 (module doc).
pub fn pad_update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pad::SIZE { return; }
    let s = scaled_class(w, w.m(id).o_class, PAD_SCALE);
    w.mm(id).scale = s;
    match w.m(id).state {
        0 => {
            w.mm(id).has_collision = false;
            w.mm(id).position[2] += 1.0;
            let p = w.m(id).position;
            let a = [p[0], p[1], p[2] + 0.5, p[3]];
            let b = [p[0], p[1], 0.01, p[3]];
            let hit = w.coll_line(pf4(a), pf4(b), 2, None);
            w.mm(id).position[2] = hit.as_ref().map_or(0.0, |o| o.point[2]);
            let ground = w.m(id).position;
            c::set_pv4(w, id, pad::GROUND, ground);
            if hit.is_some_and(|o| o.moby.is_some()) { c::set_pi32(w, id, pad::ON_MOBY, 1); }
            let rot = w.m(id).rotation;
            let base = w.create_moby(BASE_PIECE);
            if let Some(b) = base {
                let sc = scaled_class(w, BASE_PIECE, PAD_SCALE);
                let m = w.mm(b);
                m.update_dist = 0x40;
                m.visible = 1;
                m.draw_dist = 0x56;
                m.scale = sc;
                m.position = ground;
                m.rotation = rot;
                w.build_matrix(b);
            }
            set_link(w, id, pad::BASE, base);
            for (o, dd, turn_half) in [(pad::DOOR_A, 0x56, false), (pad::DOOR_B, 0x40, true)] {
                let d = w.create_moby(DOOR);
                set_link(w, id, o, d);
                let Some(d) = d else { continue };
                let sc = scaled_class(w, DOOR, PAD_SCALE);
                let m = w.mm(d);
                m.update_dist = 0x40;
                m.draw_dist = dd;
                m.visible = 1;
                m.scale = sc;
                m.rotation = rot;
                if turn_half { m.rotation[2] = c::add_rot(m.rotation[2], f32::from_bits(0x4049_0fd0)); }
                w.build_matrix(d);
            }
            w.mm(id).state = 1;
            let z = w.m(id).position[2];
            c::set_pf(w, id, pad::BASE_Z, z);
            c::set_pf(w, id, pad::LIFT, PAD_DOWN);
            let col = super::class_collision(w, w.m(id).o_class);
            w.mm(id).has_collision = col;
        }
        1 => pad_armed(w, id),
        _ => {}
    }
}

fn pad_armed(w: &mut World, id: MobyId) {
    let scene = w.svc.cinematic.scene.as_ref().map(|s| s.id as i32);
    if w.svc.game_mode == 2 && c::pi32(w, id, pad::ON_MOBY) != 0 && scene == Some(3) {
        for o in [pad::DOOR_A, pad::DOOR_B, pad::BASE] {
            if let Some(m) = link(w, id, o) { w.delete_moby(m); }
        }
        // The game goes on writing the deleted pieces' fields this tick; nothing reads them.
        w.delete_moby(id);
        return;
    }
    if c::pf(w, id, pad::TARGET) != 0.0 {
        let timed_out = c::pi32(w, id, pad::HELD) == 0 && c::dec_timer_pvar_i32(w, id, pad::TIMER) == 2;
        if timed_out || (w.svc.game_mode == 2 && scene.is_some_and(|s| 3 < s)) {
            w.play_sound(0, 0, id);
            c::set_pf(w, id, pad::TARGET, 0.0);
        }
        if w.hero.air_ticks == 0 && w.hero.state == PRESS {
            w.play_sound(0, 0, id);
            c::set_pf(w, id, pad::TARGET, 0.0);
        }
    }
    let target = c::pf(w, id, pad::TARGET);
    let (mut x, mut v) = (c::pf(w, id, pad::OPEN), c::pf(w, id, pad::VEL));
    let lift = c::pf(w, id, pad::LIFT);
    let open_spring = if target == 0.0 { lift == PAD_DOWN } else { x != target };
    if open_spring {
        turn::spring(target, 4.0 * DT2, 4.0 * DT2, 4.0 * DT, &mut x, &mut v);
        c::set_pf(w, id, pad::OPEN, x);
    } else {
        let to = if target == 0.0 { PAD_DOWN } else { PAD_UP };
        let mut l = lift;
        turn::spring(to, 16.0 * DT2, 4.0 * DT2, 8.0 * DT, &mut l, &mut v);
        c::set_pf(w, id, pad::LIFT, l);
    }
    c::set_pf(w, id, pad::VEL, v);
    let x = c::pf(w, id, pad::OPEN);
    let ground = c::pv4(w, id, pad::GROUND);
    for o in [pad::DOOR_A, pad::DOOR_B] {
        let Some(d) = link(w, id, o) else { continue };
        let r = w.m(d).rows;
        let p = c::add(c::add(c::set_len3(r[1], -x), c::set_len3(r[2], -x * 0.2)), ground);
        w.mm(d).position = p;
        w.build_matrix(d);
    }
    let z = c::pf(w, id, pad::BASE_Z) + c::pf(w, id, pad::LIFT);
    w.mm(id).position[2] = z;
}

fn pads(w: &World, group: i32) -> Vec<MobyId> {
    let Ok(g) = i8::try_from(group) else { return Vec::new() };
    group_ids(w, g).into_iter().filter(|&m| w.table.mobys.get(m).is_some_and(|o| o.o_class == PAD)).collect()
}

/// `0x2d6b58(from, to, group)` (module doc).
pub fn pad_farthest_turn(w: &World, from: c::V, to: c::V, group: i32) -> Option<MobyId> {
    let mut best = 0.0f32;
    let mut out = None;
    for m in pads(w, group) {
        let a = c::atan(from[0] - to[0], from[1] - to[1]);
        let p = w.m(m).position;
        let b = c::atan(from[0] - p[0], from[1] - p[1]);
        let d = c::diff_rots(a, b);
        if best < d {
            out = Some(m);
            best = d;
        }
    }
    out
}

/// `0x2d6c80(p, group)` (module doc).
pub fn pad_nearest(w: &World, p: c::V, group: i32) -> Option<MobyId> {
    let mut best = 255.0f32;
    let mut out = None;
    for m in pads(w, group) {
        let d = c::dist3(p, w.m(m).position);
        if d < best {
            out = Some(m);
            best = d;
        }
    }
    out
}

/// `0x2d6d50(group)`: every pad of the group armed for good.
pub fn pads_arm(w: &mut World, group: i32) {
    for m in pads(w, group) {
        if w.m(m).pvars.len() < pad::SIZE { continue; }
        c::set_pf(w, m, pad::TARGET, ARMED);
        c::set_pi32(w, m, pad::HELD, 1);
    }
}

/// `0x2d6dc0(pad)`: armed for 5 ticks.
pub fn pad_arm(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pad::SIZE { return; }
    c::set_pi32(w, id, pad::TIMER, 5);
    c::set_pf(w, id, pad::TARGET, ARMED);
}

// ---------------------------------------------------------------------------------------------------------------
// 586

fn release_voice(w: &mut World, id: MobyId) {
    let v = c::pi32(w, id, button::VOICE);
    if v != -1 { w.release_sound(v, id); }
    c::set_pi32(w, id, button::VOICE, -1);
}

fn spring_z(w: &mut World, id: MobyId, to: f32, a: f32, d: f32, vmax: f32) {
    let (mut z, mut v) = (w.m(id).position[2], c::pf(w, id, button::VEL));
    turn::spring(to, a, d, vmax, &mut z, &mut v);
    w.mm(id).position[2] = z;
    c::set_pf(w, id, button::VEL, v);
}

/// Level18 0x2d7b20 (module doc).
pub fn button_update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < button::SIZE { return; }
    let home = c::pf(w, id, button::HOME_Z);
    match w.m(id).state {
        0 => {
            let z = w.m(id).position[2];
            c::set_pf(w, id, button::HOME_Z, z);
            let f = c::pi16(w, id, button::FLAGS) | 8;
            c::set_pi16(w, id, button::FLAGS, f);
            if c::pi32(w, id, button::KIND) != 0 {
                w.mm(id).state = 1;
                let s = scaled_class(w, w.m(id).o_class, BUTTON_SCALE);
                w.mm(id).scale = s;
            } else {
                w.mm(id).state = 4;
            }
        }
        1 | 3 => spring_z(w, id, home + BUTTON_DOWN, 20.0 * DT2, 40.0 * DT2, 20.0 * DT),
        2 => {
            spring_z(w, id, home + BUTTON_UP, 5.0 * DT2, 10.0 * DT2, 20.0 * DT);
            if on_it(w, id) && w.hero.state == PRESS {
                w.mm(id).state = 3;
                w.play_sound(0, 0, id);
                release_voice(w, id);
            } else if w.hero.state != 0x72 {
                if let Some(r) = super::row(REFERENCE_LEVEL, COUNTDOWN_FN) {
                    w.svc.draw_callbacks.register(crate::moby_update::classes::draw_callbacks::Callback::UnitFrame(r), id);
                }
                if c::dec_timer_pvar_i32(w, id, button::COUNT) != 0 { w.svc.units.set_word(RUN_OUT_WORD, 1); }
            }
        }
        4 => {
            if on_it(w, id) && w.hero.state == PRESS {
                w.mm(id).state = 5;
                w.mm(id).cmd = 1;
                w.mm(id).position[2] -= 1.5;
                if w.svc.level == 18 { crate::moby_update::interact::set_global_flag(w, BUTTON_FLAG, 1); }
            }
            if c::dist2(w.m(id).position, super::hero_pos(w)) < 2.5 { hint(w); }
        }
        5 => {
            let mut z = w.m(id).position[2];
            let r = turn::approach(home - 1.5, f32::from_bits(0x3e19_999a), &mut z);
            w.mm(id).position[2] = z;
            if r == 0.0 { w.mm(id).state = 6; }
        }
        _ => {}
    }
}

/// The plain button's help hint (record 0x71, message 0x2b02).
fn hint(w: &mut World) {
    let (level, play) = (w.svc.level as i32, w.svc.help.play_time);
    let tk = |n: i32| crate::hud::scale_ticks(n);
    let r = &mut w.svc.help.records.help[HINT_REC];
    if r.count == 0 {
        r.count = 1;
        let t = tk(play) / 600;
        if (r.time as i32) < t { r.time = t as u16; }
        r.mask |= (1u32 << (level as u32 & 31)) | 0x8000_0000;
        return;
    }
    let since = tk(play) - 600 * r.time as i32;
    if ((tk(0x12) as f32 * 60.0) as i32) < since || r.time as i32 * 600 == 0 {
        w.svc.help.request(HINT_MSG, HINT_REC as i32);
    } else {
        let t = tk(play) / 600;
        if (r.time as i32) < t { r.time = t as u16; }
    }
}

/// `0x2d7f68(m, t)` (module doc).
pub fn countdown_start(w: &mut World, id: MobyId, t: i32) {
    if w.m(id).pvars.len() < button::SIZE { return; }
    if w.m(id).state != 2 {
        w.mm(id).state = 2;
        c::set_pi32(w, id, button::COUNT, t);
    }
    let v = c::pi32(w, id, button::VOICE);
    if !w.sound_alive(v, id) {
        let s = w.play_level_def(COUNTDOWN_SOUND, 0, id);
        c::set_pi32(w, id, button::VOICE, s);
    }
}

/// `0x2d7fd0(m)` (module doc).
pub fn countdown_stop(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < button::SIZE || w.m(id).state != 2 { return; }
    w.mm(id).state = 3;
    release_voice(w, id);
}

/// `0x2d8050(m)` (module doc).
pub fn countdown_poll(w: &mut World, id: MobyId) -> i32 {
    match w.m(id).state {
        3 => {
            w.mm(id).state = 1;
            1
        }
        2 if w.m(id).pvars.len() >= button::SIZE && c::pi32(w, id, button::COUNT) == 0 => -1,
        _ => 0,
    }
}

/// One frame of the countdown's draw (`0x2d8098`), for the engine's 2-D layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CountdownDraw {
    /// `DrawUIFrame(top, bottom, left, right, alpha)`.
    pub frame: [i32; 5],
    /// `font_print_center_large(x, y, rgba, text, 8)`.
    pub x: i32,
    pub y: i32,
    pub rgba: u32,
    pub text: [u8; 8],
}

/// The countdown's frame position and colours (module doc).
pub const COUNTDOWN_CENTRE: (i32, i32) = (260, 80);
pub const COUNTDOWN_FRAME: [i32; 5] = [80 - 5, 80 + 20, 260 - 50, 260 + 52, 0x60];
pub const COUNTDOWN_RGBA: [u32; 2] = [0x80c0_c0c0, 0x8040_40ff];

/// `0x2d8098` run by the frame's callbacks (`draw_callbacks::run_frame`): its `randi(10)` and the text of this frame.
pub fn countdown_draw(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < button::SIZE { return; }
    let n = c::pi32(w, id, button::COUNT);
    push_countdown(w, n, COUNTDOWN_CENTRE, COUNTDOWN_RGBA);
}

/// The countdown text of `n` ticks at `centre` (its frame −5 / +20 by −50 / +52 about it, alpha 0x60), colours
/// pulsing between `rgba`: the code of Veldin's `0x2d8098` and Blarg's escape `0x3021d8` (their own centres).
pub fn push_countdown(w: &mut World, n: i32, centre: (i32, i32), rgba2: [u32; 2]) {
    let k = (w.counter % 60) as f32;
    let s = ((k / 60.0) * f32::from_bits(0x40c9_0fd0)).sin() * 3.0 + 0.5;
    let t = s.clamp(0.0, 1.0);
    let rgba = crate::hud::tween_color(t, rgba2[0], rgba2[1]);
    let (t3600, t600, t60, t6) = (w.ticks(0xe10), w.ticks(600), w.ticks(0x3c), w.ticks(6));
    let d = |x: i32| b'0'.wrapping_add(x as u8);
    let r = w.rng.randi(10);
    let text = [b'0', d(n / t3600), b':', d((n % t3600) / t600), d((n % t600) / t60), b':', d((n % t60) / t6), d(r)];
    let frame = [centre.1 - 5, centre.1 + 20, centre.0 - 50, centre.0 + 52, 0x60];
    w.svc.draw_callbacks.texts.push(CountdownDraw { frame, x: centre.0, y: centre.1, rgba, text });
}
