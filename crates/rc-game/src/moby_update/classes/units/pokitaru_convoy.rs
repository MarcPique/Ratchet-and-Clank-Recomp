//! **Pokitaru's convoys, class 1264** (level11 `0x3172c0`, census U387, four instances), **their cars 1265**
//! (`0x3181f0`, eight made by each convoy) **and the sludge 1524** the cars drop into the sea (`0x31ab08`, made by
//! `0x31ada0`): the jet mission's targets. A convoy head flies a closed path with its eight cars strung behind it
//! 3.5 apart (each car on the path 7 behind the one ahead), its escorts 1319 spaced along it
//! ([`super::ship_fighter`]); once Ratchet flies (hero state 0x32) it switches to its battle path. Hits on any car
//! wear the convoy's health (80); each eighth lost, the hit car's place is taken by the tail car, which breaks off
//! in pieces, and the cars behind close the gap; the last car gone, the convoy bursts and is deleted. Passing a
//! marked point (w = 42) a car starts a second of dropping sludge, which splashes on the sea, spreads and sinks. The
//! jet's HUD counts the convoys left ([`super::pokitaru_jet_hud`]). Read from the level11 decomp and disassembly; the
//! level data from the overlay (gp 0x166c00). Native `f32`.
//!
//! **System or not.** Per-class code (a cluster of one), on the shared pieces: the paths (`Services::splines`), the
//! break pieces (`fx::break_piece_with`), the creature death burst (`fx::death_explosion`), the sea's height
//! ([`crate::water::sea::ocean_z`]), the water particles (types 34, 35, 46, 02).
//!
//! **Convoy pvars** (0xf8; the paths / group words come with the instance): +0x60 the battle path, +0x64 the cars
//! left, +0x68 the head's path point, +0x6c the way to the next, +0x70.. the eight cars (index + 1), +0x90 (s16 ×8)
//! their path points, +0xa0 their fractions, +0xc0 their gaps closing, +0xe0 the health, +0xe4 (s16) the escorts'
//! group, +0xec the first path, +0xf0 (the port: the index of) the path flown, +0xf4 the engine voice. **Car pvars**
//! (0x2c): +0x00 the last position, +0x10 the velocity at the break, +0x20 / +0x24 the spin rates, +0x28 the sludge
//! timer. **Sludge pvars** (0x28): +0x00 the velocity, +0x22 (s16) the undrawn count, +0x24 the surface (the sea).
//!
//! ## Coverage (the convoy, `0x3172c0`)
//! | address | what | port |
//! |---|---|---|
//! | 0 | the escorts placed (`0x3180a0`); the mission done → `DeleteMoby`; update 0xff, draw 0x200; the path +0xec; position = its first point; +0xf4 = −1, health 80, 8 cars, +0x68 = +0x6c = 0, → 1; eight `CreateMoby(0x4f1)`: draw 0x200, update 0xff, drawn, the head's light word / ambient, mode without 0x20, scale = class scale·2, the head's position / Euler; points = the path's count − 100, gaps 0 | [`update`], [`place_escorts`] |
//! | 1 | Ratchet in 0x32 → 2, the battle path (+0x60), +0x68 = 0, points = count − 100, gaps 0 | [`update`] |
//! | 1, 2 | +0x6c += 0.2, past 1 → −1, the next point (mod count); each gap `Approach(0, 5·dt)`, reaching 0 → `PlayClassSound(0, 0, car)`; the head on the path (`0x317678`); the cars (`0x317820`); in 2 the hits (`0x317c98`); the engine voice (class sound 0, flags 4) restarted when gone | [`update`], [`head_step`], [`cars_step`], [`hits`] |
//! | 0x317678 | points i, i+1, i+2: rot.y / rot.z blended between the two segments' (pointing back along the path) by +0x6c; position = lerp(p_i, p_i+1, +0x6c) | [`head_step`] |
//! | 0x317820 | from = head + row 0·3.5; per car: the first point after its own within 7 of `from` (each step the one before it); a point with w = 42 and the car's timer out → `PlayClassSound(2, 0, car)` in game mode 0, timer `ticks(60)`; f = (d₀ − 7)/(d₀ − d₁) (0 within 7); its point / fraction kept; a gap shifts the points by its whole part and f by the rest; position = lerp, Euler blended (−pitch between the segments ahead, yaw back), `MobyBuildMatrix`; from = car + row 0·3.5 | [`cars_step`] |
//! | 0x317c98 | each car's hit (`MobyGetHitMessage(car, −1, 0)`): damage summed, +0xa4 = 0xff, the last hit car; health `Approach(0, sum)`; a hit and (health/80)·8 ≤ cars − 1: help record 0x56's count = 0xffff; cars − 1; the tail breaks off (`0x318488`) at the hit car's place; `PlayClassSound(1, 0, hit car)`; the gaps / points behind shift down (the hit car's gap + its point − the next's, + count below 0) | [`hits`] |
//! | 0x317c98 | no cars: v = row 0·0.095, z + 0.08; `PlayClassSound(2, 0, m)`; `0x273f50(3, 13, m, position, −1)`; `BreakFxB(12·dt², m, 0x602 / 0x603 / 0x784, position, Euler, ticks(90), 0, v, 0)`; `DeleteMoby` | [`hits`] (`fx::death_explosion`, `fx::break_piece_with`) |
//! | 0x3180a0 | the escorts (class 1319) of group +0xe4: step = the path's count / their number; each in turn at point k·step: position, +0x64 = k·step, +0x68 = 0, +0x60 = the path | [`place_escorts`] |
//! | 0x318050 | the radar's list: the head and its cars | [`members`] |
//! | 0x318488 | a car broken off: state 1, no collision; +0x10 = position − +0x00; +0x20 / +0x24 = `randf(20, 60)`·π/180·dt | [`break_off`] |
//! | tail | the engine voice | [`update`] |
//!
//! ## Coverage (the car 1265, `0x3181f0`; the sludge 1524, `0x31ab08`)
//! | address | what | port |
//! |---|---|---|
//! | car 0 | the timer +0x28 running, `randi(12)` = 0, drawn and more than 70 free slots (0x15ffbc): the point rows · (−0.8, 0, −6) + position; v = position − +0x00, z − 2·dt, + `rand_vec(0, 4·dt)`; `0x31ada0(randf(0.5, 1), point, v, &sea z, 0)`; +0x00 = position | [`car_update`], [`sludge_spawn`] |
//! | car 1 | `0x273f50(5, 13, m, position, −1)`; v = row 0·0.095, z + 0.08; `BreakFxB(…, 0x785 / 0x786 / 0x787, …)`; → 2, not drawn, hidden | [`car_update`] |
//! | 0x31ada0 | `CreateMoby(0x5f4)`: state 1, position, rot.x / rot.z random, update 0xff, drawn, draw 0xff, scale = class scale · s; +0x24 the surface, +0x00 the velocity | [`sludge_spawn`] |
//! | sludge | not drawn: +0x22 + 1 past `ticks(15)` → `DeleteMoby`; drawn: +0x22 = 0 | [`sludge_update`] |
//! | sludge 1 | velocity z − 15·dt²; above the surface + 3: `randi(3)` = 0 → the drip (`0x31b098`); below the surface: velocity ·= 0.25, → 2, the splash (`0x31ae68(m, surface, 8, 4)`) | [`sludge_update`], [`drip`], [`splash`] |
//! | sludge 2 | velocity z − 5·dt²; every 4th tick a ring (`0x31ae68(…, 0, 1)`) and a bubble (`PartType34Spawn(30000, surface, position, rand_vec(0, dt) with \|z\|)`, colour `FastTweenColor(randf(0, 1), 0x6000c0c0, 0x6000c000)`); scale `Approach(0, class scale / ticks(60))`, 0 → `DeleteMoby` | [`sludge_update`] (`particles::type34`) |
//! | sludge | position += velocity; the velocity's xy length `Approach(0, 4·dt²)` | [`sludge_update`] |
//! | 0x31ae68 | n drops: a = `rand_angle`, s = `randf(0, 6·dt)`, (cos a·s, sin a·s, `randf(3·dt, 6.5·dt)`), `PartType35Spawn(position, v, randi(2), rand_range(90, 120))`; k rings: position + `rand_vec(1, 1)` at the surface + 0.05, `PartType46Spawn(randf(3, 6), k = 0 ? 1 : −1, p, 0, surface)`, byte 9 = 0x44, +0xa = `trunc(scale(randf(30, 90)))`, colour `FastTweenColor(randf(0, 1), 0x20002080, 0x20008080)` | [`splash`] (`particles::{type35, type46}`; the surface by value [L: the game's pointer reads the sea as it moves]) |
//! | 0x31b098 | `PartType02Spawn(position, rand_vec(0, 2·dt) + velocity (w 1), (0, 0, randf(2, 4)·dt, 2), FastTweenColor(randf, 0x4010c040, 0x4010c080), FastTweenColor(randf, 0x2010c080, 0x2010a0c0), trunc(scale(randf(8, 12))), trunc(scale(randf(40, 60))), trunc(scale(randf(24, 36))), −1)` | [`drip`] (`fx::part02`) |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, add, add_rot, atan, dist2, dist3, fx, set_len3, sub_rot, DT, DT2};
use crate::moby_update::services::{pf as to_pf, World};
use crate::moby_update::{scheduler, story};

pub const REFERENCE_LEVEL: u32 = 11;
pub const UPDATE_FN: u32 = 0x31_72c0;
pub const CLASS: i16 = 0x4f0;
pub const CLASSES: [i16; 1] = [CLASS];
pub const CAR_FN: u32 = 0x31_81f0;
pub const CAR: i16 = 0x4f1;
pub const CAR_CLASSES: [i16; 1] = [CAR];
pub const SLUDGE_FN: u32 = 0x31_ab08;
pub const SLUDGE: i16 = 0x5f4;
pub const SLUDGE_CLASSES: [i16; 1] = [SLUDGE];

pub mod pvo {
    pub const PATH_B: usize = 0x60;
    pub const CARS_N: usize = 0x64;
    pub const POINT: usize = 0x68;
    pub const T: usize = 0x6c;
    pub const CARS: usize = 0x70;
    pub const CAR_POINT: usize = 0x90;
    pub const CAR_F: usize = 0xa0;
    pub const CAR_GAP: usize = 0xc0;
    pub const HEALTH: usize = 0xe0;
    pub const ESCORTS: usize = 0xe4;
    pub const PATH_A: usize = 0xec;
    pub const PATH: usize = 0xf0;
    pub const VOICE: usize = 0xf4;
    pub const LEN: usize = 0xf8;
}

pub mod car_pvo {
    pub const LAST: usize = 0x00;
    pub const VEL: usize = 0x10;
    pub const SPIN_A: usize = 0x20;
    pub const SPIN_B: usize = 0x24;
    pub const TIMER: usize = 0x28;
    pub const LEN: usize = 0x2c;
}

pub mod sludge_pvo {
    pub const VEL: usize = 0x00;
    pub const UNDRAWN: usize = 0x22;
    pub const LEN: usize = 0x28;
}

const CARS: usize = 8;
const HEALTH: f32 = 80.0;
/// gp−0x48e0: the head's rate along the path; gp−0x48dc / −0x48d8: the car gap on the path and the spacing.
const RATE: f32 = 0.2;
const GAP: f32 = 7.0;
const SPACING: f32 = 3.5;
/// gp−0x48d4: the sludge window.
const SLUDGE_TICKS: i32 = 60;
/// gp−0x48d0 / −0x4894: the pieces' push along row 0.
const PUSH: f32 = 0.095;
/// The marked path points (w).
const MARK: f32 = 42.0;
/// gp−0x48b0 (0x162350): the sludge's point (model space); gp−0x48c0 the chance; gp−0x48a0 / −0x489c the scale;
/// gp−0x4898 the scatter.
const SLUDGE_AT: [f32; 3] = [-0.8, 0.0, -6.0];
const SLUDGE_CHANCE: i32 = 12;
/// The pieces of the head and of a car.
const HEAD_PIECES: [i16; 3] = [0x602, 0x603, 0x784];
const CAR_PIECES: [i16; 3] = [0x785, 0x786, 0x787];

fn pf(w: &World, id: MobyId, o: usize) -> f32 { c::pf(w, id, o) }
fn set(w: &mut World, id: MobyId, o: usize, v: f32) { c::set_pf(w, id, o, v) }
fn pi(w: &World, id: MobyId, o: usize) -> i32 { c::pi32(w, id, o) }
fn seti(w: &mut World, id: MobyId, o: usize, v: i32) { c::set_pi32(w, id, o, v) }
fn link(w: &World, id: MobyId, o: usize) -> Option<MobyId> { usize::try_from(pi(w, id, o) - 1).ok().filter(|&m| m < w.table.mobys.len()) }
fn car(w: &World, id: MobyId, k: usize) -> Option<MobyId> { link(w, id, pvo::CARS + 4 * k) }
fn car_point(w: &World, id: MobyId, k: usize) -> i32 { c::pi16(w, id, pvo::CAR_POINT + 2 * k) as i32 }
fn set_car_point(w: &mut World, id: MobyId, k: usize, v: i32) { c::set_pi16(w, id, pvo::CAR_POINT + 2 * k, v as i16) }

/// Path `i` of the level (empty for −1 / missing).
fn path(w: &World, i: i32) -> Vec<[f32; 4]> {
    usize::try_from(i).ok().and_then(|i| w.svc.splines.get(i)).map_or(Vec::new(), |p| p.iter().map(|q| q.map(f32::from_bits)).collect())
}

/// Point `i` of a path, wrapped into it [L: the game reads outside the table for an index it never wraps].
fn at(p: &[[f32; 4]], i: i32) -> [f32; 4] { if p.is_empty() { [0.0; 4] } else { p[i.rem_euclid(p.len() as i32) as usize] } }

fn lerp(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] { std::array::from_fn(|k| a[k] + (b[k] - a[k]) * t) }

fn row0(w: &World, id: MobyId, len: f32) -> [f32; 4] {
    let r = w.m(id).rows[0];
    set_len3([r[0], r[1], r[2], 0.0], len)
}

/// The pieces' push: row 0·0.095, z + 0.08 (the speed 0x15ed60 = 1).
fn push(w: &World, id: MobyId) -> [f32; 4] {
    let r = w.m(id).rows[0];
    [r[0] * PUSH, r[1] * PUSH, r[2] * PUSH + 0.08, 0.0]
}

fn pieces(w: &mut World, id: MobyId, classes: [i16; 3]) {
    let (p, rot) = (w.m(id).position, w.m(id).rotation);
    let v = push(w, id);
    for k in classes {
        let t = w.ticks(0x5a);
        fx::break_piece_with(w, id, k, p, rot, t, 0, v, [0.0; 4], [0.0; 4]);
    }
}

/// The cars' points and gaps reset to the start of path `i` (count − 100).
fn reset_cars(w: &mut World, id: MobyId) {
    let n = path(w, pi(w, id, pvo::PATH)).len() as i32;
    for k in 0..CARS {
        set_car_point(w, id, k, (n as i16).wrapping_sub(100) as i32);
        set(w, id, pvo::CAR_GAP + 4 * k, 0.0);
    }
}

/// Level11 `0x3172c0` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    story::pvars(w, id, pvo::LEN);
    match w.m(id).state {
        0 => {
            let b = pi(w, id, pvo::PATH_B);
            place_escorts(w, id, b);
            let mission = w.m(id).mission;
            if story::mission_done(w, mission as i32) {
                w.delete_moby(id);
                return;
            }
            {
                let m = w.mm(id);
                m.update_dist = 0xff;
                m.draw_dist = 0x200;
            }
            let a = pi(w, id, pvo::PATH_A);
            seti(w, id, pvo::PATH, a);
            let first = at(&path(w, a), 0);
            w.mm(id).position = first;
            seti(w, id, pvo::VOICE, -1);
            set(w, id, pvo::HEALTH, HEALTH);
            seti(w, id, pvo::CARS_N, CARS as i32);
            seti(w, id, pvo::POINT, 0);
            set(w, id, pvo::T, 0.0);
            w.mm(id).state = 1;
            let cs = super::class_scale(w, CAR);
            for k in 0..CARS {
                let made = w.create_moby(CAR);
                seti(w, id, pvo::CARS + 4 * k, made.map_or(0, |m| m as i32 + 1));
                if let Some(m) = made {
                    story::pvars(w, m, car_pvo::LEN);
                    let h = w.m(id).clone();
                    let mo = w.mm(m);
                    mo.draw_dist = 0x200;
                    mo.update_dist = 0xff;
                    mo.visible = 1;
                    mo.light = h.light;
                    mo.ambient = h.ambient;
                    mo.mode = h.mode & !0x20;
                    mo.scale = cs + cs;
                    mo.position = h.position;
                    mo.rotation = h.rotation;
                }
            }
            reset_cars(w, id);
            return;
        }
        1 if w.hero.state == 0x32 => {
            w.mm(id).state = 2;
            let b = pi(w, id, pvo::PATH_B);
            seti(w, id, pvo::POINT, 0);
            seti(w, id, pvo::PATH, b);
            reset_cars(w, id);
        }
        1 | 2 => {}
        _ => return,
    }
    let n = path(w, pi(w, id, pvo::PATH)).len() as i32;
    let t = pf(w, id, pvo::T) + RATE;
    set(w, id, pvo::T, t);
    if 1.0 < t {
        set(w, id, pvo::T, t - 1.0);
        if n != 0 { seti(w, id, pvo::POINT, (pi(w, id, pvo::POINT) + 1) % n); }
    }
    for k in 0..CARS {
        let o = pvo::CAR_GAP + 4 * k;
        let old = pf(w, id, o);
        let mut g = old;
        c::turn::approach(0.0, DT * 5.0, &mut g);
        set(w, id, o, g);
        if old != 0.0 && g == 0.0 {
            if let Some(m) = car(w, id, k) { w.play_sound(0, 0, m); }
        }
    }
    head_step(w, id);
    cars_step(w, id);
    if w.m(id).state == 2 && hits(w, id) { return; }
    let v = pi(w, id, pvo::VOICE);
    if !w.sound_alive(v, id) {
        let s = w.play_sound(0, 4, id);
        seti(w, id, pvo::VOICE, s);
    }
}

/// The Euler (pitch, yaw) between the segments p0→p1 and p1→p2 by `t`, both pointing back along the path.
fn blend_euler(p0: [f32; 4], p1: [f32; 4], p2: [f32; 4], t: f32) -> (f32, f32) {
    let y01 = atan(p0[0] - p1[0], p0[1] - p1[1]);
    let y12 = atan(p1[0] - p2[0], p1[1] - p2[1]);
    let e01 = atan(dist2(p0, p1), p1[2] - p0[2]);
    let e12 = atan(dist2(p1, p2), p2[2] - p1[2]);
    (add_rot(sub_rot(e12, e01) * t, e01), add_rot(sub_rot(y12, y01) * t, y01))
}

/// Level11 `0x317678` (module doc).
fn head_step(w: &mut World, id: MobyId) {
    let p = path(w, pi(w, id, pvo::PATH));
    if p.is_empty() { return; }
    let i = pi(w, id, pvo::POINT);
    let t = pf(w, id, pvo::T);
    let (p0, p1, p2) = (at(&p, i), at(&p, i + 1), at(&p, i + 2));
    let (pitch, yaw) = blend_euler(p0, p1, p2, t);
    let pos = lerp(p0, p1, t);
    let m = w.mm(id);
    m.rotation[1] = pitch;
    m.rotation[2] = yaw;
    m.position = [pos[0], pos[1], pos[2], m.position[3]];
}

/// Level11 `0x317820` (module doc).
fn cars_step(w: &mut World, id: MobyId) {
    let p = path(w, pi(w, id, pvo::PATH));
    if p.is_empty() { return; }
    let n = p.len() as i32;
    let mut from = add(row0(w, id, SPACING), w.m(id).position);
    let cars = pi(w, id, pvo::CARS_N).clamp(0, CARS as i32) as usize;
    for k in 0..cars {
        let s = car_point(w, id, k);
        let mut i6 = (s + 2) % n;
        let mut d0 = dist3(at(&p, s), from);
        let mut i7 = (s + 1) % n;
        let mut i5 = s;
        let d1 = loop {
            let d = dist3(at(&p, i7), from);
            if d <= GAP { break d; }
            let nx = (i7 + 1) % n;
            i6 = (nx + 1) % n;
            d0 = d;
            i5 = i7;
            i7 = nx;
        };
        let Some(m) = car(w, id, k) else { continue };
        if at(&p, i5)[3] == MARK && c::pi32(w, m, car_pvo::TIMER) == 0 {
            if w.svc.game_mode == 0 { w.play_sound(2, 0, m); }
            let t = w.ticks(SLUDGE_TICKS);
            c::set_pi32(w, m, car_pvo::TIMER, t);
        }
        let mut f = if GAP < d0 { (d0 - GAP) / (d0 - d1) } else { 0.0 };
        set_car_point(w, id, k, i5);
        set(w, id, pvo::CAR_F + 4 * k, f);
        let gap = pf(w, id, pvo::CAR_GAP + 4 * k);
        if gap != 0.0 {
            f -= gap;
            let mut whole = f as i32;
            f -= (f as i32) as f32;
            if f < 0.0 {
                whole -= 1;
                f += 1.0;
            }
            i5 = (i5 + whole + n) % n;
            i7 = (i7 + whole + n) % n;
            i6 = (i6 + whole + n) % n;
        }
        let (a, b, cc) = (at(&p, i5), at(&p, i7), at(&p, i6));
        let pos = lerp(a, b, f);
        let y1 = atan(a[0] - b[0], a[1] - b[1]);
        let y2 = atan(b[0] - cc[0], b[1] - cc[1]);
        let e1 = atan(dist2(b, a), a[2] - b[2]);
        let e2 = atan(dist2(cc, b), b[2] - cc[2]);
        {
            let mo = w.mm(m);
            mo.position = [pos[0], pos[1], pos[2], mo.position[3]];
            mo.rotation[1] = add_rot(sub_rot(-e2, -e1) * f, -e1);
            mo.rotation[2] = add_rot(sub_rot(y2, y1) * f, y1);
        }
        w.build_matrix(m);
        from = add(row0(w, m, SPACING), w.m(m).position);
    }
}

/// Level11 `0x317c98`; true when the convoy was destroyed (module doc).
fn hits(w: &mut World, id: MobyId) -> bool {
    let mut sum = 0.0f32;
    let mut last = None;
    let cars = pi(w, id, pvo::CARS_N).clamp(0, CARS as i32) as usize;
    for k in 0..cars {
        let Some(m) = car(w, id, k) else { continue };
        if let Some(h) = w.get_hit(m, u32::MAX, false) {
            sum += h.damage.to_f32();
            w.mm(m).hit_slot = 0xff;
            last = Some(k);
        }
    }
    let mut hp = pf(w, id, pvo::HEALTH);
    c::turn::approach(0.0, sum, &mut hp);
    set(w, id, pvo::HEALTH, hp);
    let n = pi(w, id, pvo::CARS_N);
    if sum == 0.0 || ((n - 1) as f32) < (hp / HEALTH) * 8.0 { return false; }
    let Some(k) = last else { return false };
    w.svc.help.records.help[super::ship_fighter::HELP_REC].count = 0xffff;
    let n = n - 1;
    seti(w, id, pvo::CARS_N, n);
    let tail = car(w, id, n as usize);
    if let Some(t) = tail { break_off(w, t); }
    let hit = car(w, id, k);
    if let (Some(t), Some(hc)) = (tail, hit) {
        let (p, r) = (w.m(hc).position, w.m(hc).rotation);
        let m = w.mm(t);
        m.position = p;
        m.rotation = r;
    }
    if let Some(hc) = hit { w.play_sound(1, 0, hc); }
    let pc = path(w, pi(w, id, pvo::PATH)).len() as i32;
    let k = k as i32;
    if k < n {
        let (s1, s2) = (car_point(w, id, k as usize), car_point(w, id, k as usize + 1));
        for j in k..n {
            let v = pf(w, id, pvo::CAR_GAP + 4 * (j as usize + 1));
            set(w, id, pvo::CAR_GAP + 4 * j as usize, v);
        }
        let o = pvo::CAR_GAP + 4 * k as usize;
        let g = pf(w, id, o) + (s1 - s2) as f32;
        set(w, id, o, if g < 0.0 { g + pc as f32 } else { g });
        for j in k..n {
            let v = car_point(w, id, j as usize + 1);
            set_car_point(w, id, j as usize, v);
        }
    }
    if n != 0 { return false; }
    let p = w.m(id).position;
    w.play_sound(2, 0, id);
    fx::death_explosion(w, 3.0, 13.0, Some(id), p, -1);
    pieces(w, id, HEAD_PIECES);
    w.delete_moby(id);
    true
}

/// Level11 `0x3180a0(m, path)` (module doc).
fn place_escorts(w: &mut World, id: MobyId, path_i: i32) {
    let Ok(g) = i8::try_from(c::pi16(w, id, pvo::ESCORTS)) else { return };
    let list: Vec<MobyId> = scheduler::group_ids(w, g).into_iter().filter(|&m| m < w.table.mobys.len() && w.m(m).o_class == super::ship_fighter::CLASS).collect();
    if list.is_empty() { return; }
    let p = path(w, path_i);
    let step = (p.len() as i32 / list.len() as i32) as i16 as i32;
    for (j, m) in list.into_iter().enumerate() {
        let i = j as i32 * step;
        let q = at(&p, i);
        story::pvars(w, m, super::ship_fighter::pvo::LEN);
        w.mm(m).position = q;
        seti(w, m, super::ship_fighter::pvo::POINT, i);
        set(w, m, super::ship_fighter::pvo::T, 0.0);
        seti(w, m, super::ship_fighter::pvo::PATH, path_i);
    }
}

/// Level11 `0x318050(m, out)`: the head and its cars (the radar's and the lock's list).
pub fn members(w: &World, id: MobyId) -> Vec<MobyId> {
    let mut out = vec![id];
    let n = pi(w, id, pvo::CARS_N).clamp(0, CARS as i32) as usize;
    out.extend((0..n).filter_map(|k| car(w, id, k)));
    out
}

/// Level11 `0x318488(car)` (module doc).
fn break_off(w: &mut World, m: MobyId) {
    story::pvars(w, m, car_pvo::LEN);
    w.mm(m).state = 1;
    w.mm(m).has_collision = false;
    let v = c::sub(w.m(m).position, c::pv4(w, m, car_pvo::LAST));
    c::set_pv4(w, m, car_pvo::VEL, v);
    let a = w.rng.randf(20.0, 60.0);
    set(w, m, car_pvo::SPIN_A, a * 0.017_453_292 * DT);
    let b = w.rng.randf(20.0, 60.0);
    set(w, m, car_pvo::SPIN_B, b * 0.017_453_292 * DT);
}

/// Level11 `0x3181f0`: a car (module doc).
pub fn car_update(w: &mut World, id: MobyId) {
    story::pvars(w, id, car_pvo::LEN);
    match w.m(id).state {
        0 => {
            let running = c::dec_timer_pvar_i32(w, id, car_pvo::TIMER) == 0;
            if running && w.rng.randi(SLUDGE_CHANCE) == 0 && w.m(id).visible != 0 && 0x46 < w.table.free_slots {
                let r = w.m(id).rows;
                let p = w.m(id).position;
                let o = SLUDGE_AT;
                let at: [f32; 4] = std::array::from_fn(|k| if k < 3 { r[0][k] * o[0] + r[1][k] * o[1] + r[2][k] * o[2] + p[k] } else { 0.0 });
                let j = w.rng.rand_vec(0.0, DT * 4.0);
                let mut v = c::sub(p, c::pv4(w, id, car_pvo::LAST));
                v[2] -= DT + DT;
                let v = add(v, [j[0], j[1], j[2], 0.0]);
                let s = w.rng.randf(0.5, 1.0);
                sludge_spawn(w, s, at, v);
            }
            let p = w.m(id).position;
            c::set_pv4(w, id, car_pvo::LAST, p);
        }
        1 => {
            let p = w.m(id).position;
            fx::death_explosion(w, 5.0, 13.0, Some(id), p, -1);
            pieces(w, id, CAR_PIECES);
            let m = w.mm(id);
            m.state = 2;
            m.visible = 0;
            m.mode |= 1;
        }
        _ => {}
    }
}

/// Level11 `0x31ada0(s, pos, vel, &surface, 0)` (module doc; the surface: the sea, read each tick).
fn sludge_spawn(w: &mut World, s: f32, pos: [f32; 4], vel: [f32; 4]) -> Option<MobyId> {
    let id = w.create_moby(SLUDGE)?;
    story::pvars(w, id, sludge_pvo::LEN);
    let (a, b) = (w.rng.rand_angle(), w.rng.rand_angle());
    let cs = super::class_scale(w, SLUDGE);
    {
        let m = w.mm(id);
        m.state = 1;
        m.position = pos;
        m.rotation[0] = a;
        m.rotation[2] = b;
        m.update_dist = 0xff;
        m.visible = 1;
        m.draw_dist = 0xff;
        m.scale = cs * s;
    }
    c::set_pv4(w, id, sludge_pvo::VEL, vel);
    Some(id)
}

/// The sea's height (the sludge's surface).
fn surface(w: &World) -> f32 { crate::water::sea::ocean_z(&w.svc.water) }

/// Level11 `0x31ab08`: the sludge (module doc).
pub fn sludge_update(w: &mut World, id: MobyId) {
    story::pvars(w, id, sludge_pvo::LEN);
    if w.m(id).visible == 0 {
        let n = c::pi16(w, id, sludge_pvo::UNDRAWN) + 1;
        c::set_pi16(w, id, sludge_pvo::UNDRAWN, n);
        if w.ticks(0xf) < n as i32 {
            w.delete_moby(id);
            return;
        }
    } else {
        c::set_pi16(w, id, sludge_pvo::UNDRAWN, 0);
    }
    let sea = surface(w);
    match w.m(id).state {
        1 => {
            let mut v = c::pv4(w, id, sludge_pvo::VEL);
            v[2] -= DT2 * 15.0;
            c::set_pv4(w, id, sludge_pvo::VEL, v);
            let z = w.m(id).position[2];
            if sea + 3.0 < z && w.rng.randi(3) == 0 { drip(w, id); }
            if w.m(id).position[2] < sea {
                let v = c::pv4(w, id, sludge_pvo::VEL).map(|x| x * 0.25);
                c::set_pv4(w, id, sludge_pvo::VEL, v);
                w.mm(id).state = 2;
                splash(w, id, sea, 8, 4);
            }
        }
        2 => {
            let mut v = c::pv4(w, id, sludge_pvo::VEL);
            v[2] -= DT2 * 5.0;
            c::set_pv4(w, id, sludge_pvo::VEL, v);
            let step = super::class_scale(w, SLUDGE) / w.ticks(0x3c) as f32;
            if w.counter & 3 == 0 {
                splash(w, id, sea, 0, 1);
                let r = w.rng.rand_vec(0.0, DT);
                let vel = [r[0], r[1], r[2].abs()];
                let pos = w.m(id).position;
                *w.svc.fx.part_spawns.entry(crate::particles::type34::TYPE).or_default() += 1;
                let d = crate::particles::type34::Draws::draw(w.rng);
                let f = w.rng.randf(0.0, 1.0);
                if let Some(sys) = w.particles.as_deref_mut() {
                    match crate::particles::type34::spawn(sys, 30000.0, sea, pos, vel, &d) {
                        Some(i) => crate::particles::rec::set_u32(&mut sys.pool.recs[i], 4, crate::particles::tween_color(f.to_bits(), 0x6000_c0c0, 0x6000_c000)),
                        None => w.svc.fx.part_failed += 1,
                    }
                }
            }
            let mut s = w.m(id).scale;
            c::turn::approach(0.0, step, &mut s);
            w.mm(id).scale = s;
            if s == 0.0 {
                w.delete_moby(id);
                return;
            }
        }
        _ => {}
    }
    let v = c::pv4(w, id, sludge_pvo::VEL);
    let p = add(w.m(id).position, v);
    w.mm(id).position = p;
    let mut s = (v[0] * v[0] + v[1] * v[1]).sqrt();
    c::turn::approach(0.0, DT2 * 4.0, &mut s);
    let l = (v[0] * v[0] + v[1] * v[1]).sqrt();
    let k = if l == 0.0 { 0.0 } else { s / l };
    c::set_pv4(w, id, sludge_pvo::VEL, [v[0] * k, v[1] * k, v[2], v[3]]);
}

/// `trunc(multiply_global_scale(randf(a, b)))`.
fn scaled(w: &mut World, a: f32, b: f32) -> i32 {
    let r = w.rng.randf(a, b);
    w.svc.timing.scale(to_pf(r)).to_i32()
}

/// Level11 `0x31ae68(m, &surface, drops, rings)` (module doc).
fn splash(w: &mut World, id: MobyId, sea: f32, drops: i32, rings: i32) {
    let pos = w.m(id).position;
    for _ in 0..drops.max(0) {
        let a = w.rng.rand_angle();
        let s = w.rng.randf(DT * 0.0, DT * 6.0);
        let vz = w.rng.randf(DT * 3.0, DT * 6.5);
        let life = w.rng.rand_range(0x5a, 0x78);
        let kind = w.rng.randi(2);
        let v = [a.cos() * s, a.sin() * s, vz, 0.0];
        *w.svc.fx.part_spawns.entry(crate::particles::type35::TYPE).or_default() += 1;
        if let Some(sys) = w.particles.as_deref_mut() {
            if crate::particles::type35::spawn(sys, w.rng, pos, v, kind, life).is_none() { w.svc.fx.part_failed += 1; }
        }
    }
    for k in 0..rings.max(0) {
        let r = w.rng.rand_vec(1.0, 1.0);
        let p = [r[0] + pos[0], r[1] + pos[1], sea + 0.05, pos[3]];
        let size = w.rng.randf(3.0, 6.0);
        let spin = if k == 0 { 1.0 } else { -1.0 };
        *w.svc.fx.part_spawns.entry(crate::particles::type46::TYPE).or_default() += 1;
        let Some(sys) = w.particles.as_deref_mut() else { continue };
        let Some(i) = crate::particles::type46::spawn(sys, w.rng, size, spin, p, [0.0; 4], sea) else {
            w.svc.fx.part_failed += 1;
            continue;
        };
        sys.pool.recs[i][9] = 0x44;
        let t = scaled(w, 30.0, 90.0);
        let f = w.rng.randf(0.0, 1.0);
        let col = crate::particles::tween_color(f.to_bits(), 0x2000_2080, 0x2000_8080);
        if let Some(sys) = w.particles.as_deref_mut() {
            crate::particles::rec::set_i16(&mut sys.pool.recs[i], 0xa, t as i16);
            crate::particles::rec::set_u32(&mut sys.pool.recs[i], 4, col);
        }
    }
}

/// Level11 `0x31b098(m)`: a drip behind the falling sludge (module doc).
fn drip(w: &mut World, id: MobyId) {
    let r = w.rng.rand_vec(0.0, DT * 2.0);
    let vel = c::pv4(w, id, sludge_pvo::VEL);
    let v1 = [r[0] + vel[0], r[1] + vel[1], r[2] + vel[2], 1.0];
    let z = w.rng.randf(2.0, 4.0) * DT;
    let v2 = [0.0, 0.0, z, 2.0];
    let f = w.rng.randf(0.0, 1.0);
    let c1 = crate::particles::tween_color(f.to_bits(), 0x4010_c040, 0x4010_c080);
    let f = w.rng.randf(0.0, 1.0);
    let c2 = crate::particles::tween_color(f.to_bits(), 0x2010_c080, 0x2010_a0c0);
    let t0 = scaled(w, 10.0 * 0.8, 10.0 * 1.2);
    let t1 = scaled(w, 50.0 * 0.8, 50.0 * 1.2);
    let t2 = scaled(w, 30.0 * 0.8, 30.0 * 1.2);
    let pos = w.m(id).position;
    fx::part02(w, &crate::particles::type02::Spawn { pos, v1, v2, c1, c2, t: [t0, t1, t2], def: -1 });
}
