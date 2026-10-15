//! **Oltanis's ferries, class 712** (level14 `0x2f0538`; census U504, one placed per moby group). Boats that wait at
//! docks (the cuboids +0x80.., stations 0..4) and carry Ratchet along a path to the next dock when he stands on one
//! and presses △; the group's other boat waiting at the far dock is put away while one runs. Read from the level14
//! decomp; native `f32`.
//!
//! * Every tick: its talk word (`0x27b438`) = shown; the pose (+0x60 / +0x70) with a bob (0.25 up and down at
//!   120°/s, ±5° pitch and roll at 97°/s and 53°/s, at most 10°) is its position and rotation (`0x2f1490`); in 3 / 4
//!   with Ratchet aboard his edge brake and jump lock 2 and the boat's wall (`0x2f1638`: 7 points round it, his wall
//!   spline); its hum (sound 0, flags 4) in 3 / 4; its riders carried.
//! * 0: docked at the dock within 1 of it (`0x2f1040`). 1 (docked): Ratchet aboard (not in the air, within 1.2 across
//!   it): the route chosen (`0x2f1140`: from station 0 path +0x94 to 1, 1: +0x98 to 0, 2: +0x9c to 3, 3: +0xa0 to 0;
//!   the speeds from the first chord), the prompt (owner 4: station 0 "0x53ed", 1..3 "0x53ee", 4 "0x1399", else
//!   "0x139b"), △ while holding it → 3, the boat at the far dock put away (state 2, hidden, no collision). Else his
//!   call cuboids (+0x100 → station 0, +0x104 → 1, +0x108 → 2) send it there (in the level's first `ticks(10)` it is
//!   simply placed at that dock).
//! * 3: out of the dock to the path's start (`turn::spring` to 1 at most dt/2; the camera's distance to 7). 4: along
//!   the path (`0x2f1360`, `path::pose`). 5: into the far dock (at most dt/4); docked → 1, the far boat brought to the
//!   dock it left (`0x2f1268`); with Ratchet aboard, △ turns it back (7, a smoothstep to the new route's start). 6:
//!   waits for the mission +0xb8 or +0xbc, then shown with its collision (1).
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x2f0538` | 712 | [`update`] |
//! | `0x2f1040` / `0x2f1140` / `0x2f1268` / `0x2f12f0` / `0x2f1360` / `0x2f13f8` / `0x2f1490` / `0x2f1638` | dock, route, swap, twin, run, prompt, bob, wall | [`update`] |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, turn, DT, DT2};
use crate::moby_update::services::World;
use crate::moby_update::{interact, scheduler, story, triggers};

pub const REFERENCE_LEVEL: u32 = 14;
pub const UPDATE_FN: u32 = 0x2f_0538;
pub const CLASSES: [i16; 1] = [712];

mod o {
    pub const RIDERS: usize = 0x20;
    pub const POS: usize = 0x60;
    pub const ROT: usize = 0x70;
    pub const DOCKS: usize = 0x80;
    pub const ROUTES: usize = 0x94;
    pub const PATH: usize = 0xac;
    pub const SPARE: usize = 0xb0;
    pub const VOICE: usize = 0xb2;
    pub const STATION: usize = 0xb4;
    pub const NEXT: usize = 0xb6;
    pub const MISSIONS: usize = 0xb8;
    pub const T: usize = 0xc0;
    pub const TV: usize = 0xc4;
    pub const BOB: usize = 0xc8;
    pub const VMAX: usize = 0xd4;
    pub const ACCEL: usize = 0xd8;
    pub const WALL: usize = 0xdc;
    pub const SAVED: usize = 0xe0;
    pub const CALLS: usize = 0x100;
    pub const SIZE: usize = 0x110;
}
/// The prompt's owner id.
const OWNER: i32 = 4;
/// Level14 0x1e0460: the wall round the boat.
const WALL: [[f32; 2]; 7] = [[1.3, 1.3], [1.3, -1.3], [0.0, -1.65], [-1.3, -1.3], [-1.3, 1.3], [0.0, 1.65], [1.3, 1.3]];

fn dock(w: &World, id: MobyId, k: i32) -> Option<rc_formats::volumes::Shape> {
    let i = c::pi32(w, id, o::DOCKS + 4 * k.clamp(0, 4) as usize);
    w.svc.volumes.shape(rc_formats::volumes::ShapeKind::Cuboid, i).copied()
}
fn dock_pose(w: &World, id: MobyId, k: i32) -> ([f32; 4], [f32; 4]) {
    dock(w, id, k).map_or(([0.0; 4], [0.0; 4]), |s| {
        let ctr = s.centre();
        ([ctr[0], ctr[1], ctr[2], s.matrix[3][3]], [s.euler[0], s.euler[1], s.euler[2], s.unused_7c])
    })
}
fn route(w: &World, id: MobyId) -> Option<usize> { usize::try_from(c::pi32(w, id, o::PATH)).ok().filter(|&p| p < w.svc.splines.len()) }
fn path_pose(w: &World, id: MobyId, t: f32) -> ([f32; 4], [f32; 4]) {
    route(w, id).map_or(([0.0; 4], [0.0; 4]), |p| crate::path::pose(&w.svc.splines[p], false, t, true))
}
fn set_pose(w: &mut World, id: MobyId, p: [f32; 4], r: [f32; 4]) {
    c::set_pv4(w, id, o::POS, p);
    c::set_pv4(w, id, o::ROT, r);
}
fn aboard(w: &World, id: MobyId) -> bool { w.hero.ground_moby == Some(id) && w.hero.air_ticks == 0 }
/// Ratchet within 1.2 across the boat (its local y).
fn on_deck(w: &World, id: MobyId) -> bool {
    let (h, p, r) = (super::hero_pos(w), c::pos(w, id), w.m(id).rows);
    let d = [h[0] - p[0], h[1] - p[1], h[2] - p[2]];
    (r[1][0] * d[0] + r[1][1] * d[1] + r[1][2] * d[2]).abs() < 1.2
}

/// `0x2f1040(m)`: docked at the first dock within 1, its pose there, state 1.
fn dock_at(w: &mut World, id: MobyId) {
    c::set_pi16(w, id, o::STATION, -1);
    for k in 0..5 {
        if let Some(s) = dock(w, id, k) {
            let ctr = s.centre();
            if c::dist3(c::pos(w, id), [ctr[0], ctr[1], ctr[2], 0.0]) < 1.0 {
                c::set_pi16(w, id, o::STATION, k as i16);
                break;
            }
        }
    }
    w.mm(id).state = 1;
    let st = c::pi16(w, id, o::STATION) as i32;
    let (p, r) = dock_pose(w, id, st);
    set_pose(w, id, p, r);
}

/// `0x2f1140(m)`: the route from the station, its speeds.
fn choose(w: &mut World, id: MobyId) {
    let (path, next) = match c::pi16(w, id, o::STATION) {
        0 => (c::pi32(w, id, o::ROUTES), 1),
        1 => (c::pi32(w, id, o::ROUTES + 4), 0),
        2 => (c::pi32(w, id, o::ROUTES + 8), 3),
        3 => (c::pi32(w, id, o::ROUTES + 12), 0),
        _ => (c::pi32(w, id, o::PATH), c::pi16(w, id, o::NEXT)),
    };
    c::set_pi32(w, id, o::PATH, path);
    c::set_pi16(w, id, o::NEXT, next);
    let pts = route(w, id).map(|p| w.svc.splines[p].iter().map(|q| q.map(f32::from_bits)).collect::<Vec<_>>()).unwrap_or_default();
    let d = if pts.len() >= 2 { c::dist3(pts[0], pts[1]) } else { 0.0 };
    c::set_pf(w, id, o::TV, 0.0);
    c::set_pf(w, id, o::T, 0.0);
    let k = 1.0 / (d * pts.len() as f32);
    c::set_pf(w, id, o::ACCEL, DT2 * 2.5 * k);
    c::set_pf(w, id, o::VMAX, DT * 5.0 * k);
}

/// `0x2f12f0(m, station)`: the group's boat at `station`.
fn twin(w: &World, id: MobyId, station: i16) -> Option<MobyId> {
    let g = w.m(id).group;
    scheduler::group_ids(w, g).into_iter().find(|&m| w.table.mobys.get(m).is_some_and(|b| b.pvars.len() >= o::SIZE) && c::pi16(w, m, o::STATION) == station)
}
fn put_away(w: &mut World, m: MobyId) {
    let b = w.mm(m);
    b.state = 2;
    b.has_collision = false;
    b.mode |= 1;
    b.visible = 0;
}

/// `0x2f13f8(m)`: the prompt (owner 4) by the station (or, docking, the next).
fn prompt(w: &mut World, id: MobyId) {
    let k = if w.m(id).state == 5 { c::pi16(w, id, o::NEXT) } else { c::pi16(w, id, o::STATION) };
    let msg = match k {
        0 => 0x53ed,
        1..=3 => 0x53ee,
        4 => 0x1399,
        _ => 0x139b,
    };
    w.svc.interact.try_prompt(OWNER, msg);
}

fn ease(w: &mut World, id: MobyId, accel: f32, vmax: f32) -> f32 {
    let (mut t, mut v) = (c::pf(w, id, o::T), c::pf(w, id, o::TV));
    turn::spring(1.0, accel, accel, vmax, &mut t, &mut v);
    c::set_pf(w, id, o::T, t);
    c::set_pf(w, id, o::TV, v);
    t
}
/// The pose eased from `(p0, r0)` to `(p1, r1)` by `k` (position lerped; pitch and yaw turned).
fn blend(w: &mut World, id: MobyId, (p0, r0): ([f32; 4], [f32; 4]), (p1, r1): ([f32; 4], [f32; 4]), k: f32) {
    let p: [f32; 4] = std::array::from_fn(|i| p0[i] + (p1[i] - p0[i]) * k);
    c::set_pv4(w, id, o::POS, p);
    c::set_pf(w, id, o::ROT + 4, c::add_rot(c::sub_rot(r1[1], r0[1]) * k, r0[1]));
    c::set_pf(w, id, o::ROT + 8, c::add_rot(c::sub_rot(r1[2], r0[2]) * k, r0[2]));
}
fn done(w: &mut World, id: MobyId) -> bool {
    if 1.0 <= c::pf(w, id, o::T) {
        c::set_pf(w, id, o::TV, 0.0);
        c::set_pf(w, id, o::T, 0.0);
        true
    } else {
        false
    }
}

/// A call cuboid sent the boat to `station` (module doc).
fn call(w: &mut World, id: MobyId, cub: usize, station: i16, depart: i16) {
    let cb = c::pi32(w, id, cub);
    if cb < 0 || !w.in_cuboid(w.hero_point(), cb) { return; }
    if w.ticks(10) <= w.counter as i32 {
        c::set_pi16(w, id, o::STATION, depart);
        choose(w, id);
        w.mm(id).state = 3;
        return;
    }
    c::set_pi16(w, id, o::NEXT, station);
    c::set_pi16(w, id, o::STATION, station);
    let (p, r) = dock_pose(w, id, station as i32);
    w.mm(id).position = p;
    w.mm(id).rotation = r;
    w.mm(id).state = 1;
    set_pose(w, id, p, r);
}

/// Level14 `0x2f0538` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    story::pvars(w, id, o::SIZE);
    let (old, old_rot) = (c::pos(w, id), w.m(id).rotation);
    let shown = (w.m(id).mode ^ 1) & 1;
    interact::set_talked(w, id, shown as u32);
    match w.m(id).state {
        0 => {
            c::set_pi16(w, id, o::VOICE, -1);
            c::set_pi16(w, id, o::SPARE, -1);
            dock_at(w, id);
        }
        1 => {
            if w.hero.ground_moby == Some(id) && w.hero.air_ticks == 0 {
                if on_deck(w, id) {
                    choose(w, id);
                    prompt(w, id);
                    if w.svc.interact.triangle() && w.svc.interact.prompt.owner == OWNER {
                        w.mm(id).state = 3;
                        let n = c::pi16(w, id, o::NEXT);
                        if let Some(t) = twin(w, id, n) { put_away(w, t); }
                        c::set_pi16(w, id, o::SPARE, -1);
                    }
                }
            } else {
                let st = c::pi16(w, id, o::STATION);
                if st != 0 { call(w, id, o::CALLS, 0, if st == 2 { 3 } else { 1 }); }
                if c::pi16(w, id, o::STATION) != 1 { call(w, id, o::CALLS + 4, 1, 0); }
                if c::pi16(w, id, o::STATION) < 2 { call(w, id, o::CALLS + 8, 2, 2); }
            }
        }
        3 => {
            if aboard(w, id) { crate::cinematic::follow_distance(w, 7.0, f32::from_bits(0x3b03_126f), false); }
            let k = ease(w, id, DT2, DT * 0.5);
            let from = dock_pose(w, id, c::pi16(w, id, o::STATION) as i32);
            let to = path_pose(w, id, 0.0);
            blend(w, id, from, to, k);
            if done(w, id) { w.mm(id).state = 4; }
        }
        4 => {
            if aboard(w, id) { crate::cinematic::follow_distance(w, 7.0, f32::from_bits(0x3b44_9ba6), false); }
            let (a, v) = (c::pf(w, id, o::ACCEL), c::pf(w, id, o::VMAX));
            let (mut t, mut tv) = (c::pf(w, id, o::T), c::pf(w, id, o::TV));
            turn::spring(1.0, a, a, v, &mut t, &mut tv);
            c::set_pf(w, id, o::T, t);
            c::set_pf(w, id, o::TV, tv);
            let n = route(w, id).map_or(0, |p| w.svc.splines[p].len()) as f32;
            let (p, r) = path_pose(w, id, t * (n - 1.0));
            set_pose(w, id, p, r);
            if done(w, id) { w.mm(id).state = 5; }
        }
        5 => {
            let k = ease(w, id, DT2, DT * 0.25);
            let n = route(w, id).map_or(0, |p| w.svc.splines[p].len()) as f32;
            let from = path_pose(w, id, n);
            let to = dock_pose(w, id, c::pi16(w, id, o::NEXT) as i32);
            blend(w, id, from, to, k);
            if done(w, id) {
                w.mm(id).state = 1;
                // 0x2f1268: the far boat brought to the dock this one left.
                let n = c::pi16(w, id, o::NEXT);
                if let Some(t) = twin(w, id, n) {
                    let (p, r) = dock_pose(w, id, c::pi16(w, id, o::STATION) as i32);
                    w.mm(t).position = p;
                    w.mm(t).rotation = r;
                    dock_at(w, t);
                }
                c::set_pi16(w, id, o::STATION, n);
            }
            if aboard(w, id) && on_deck(w, id) {
                prompt(w, id);
                if w.svc.interact.triangle() {
                    let (p, r) = (c::pv4(w, id, o::POS), c::pv4(w, id, o::ROT));
                    c::set_pv4(w, id, o::SAVED, p);
                    c::set_pv4(w, id, o::SAVED + 0x10, r);
                    dock_at(w, id);
                    let n = c::pi16(w, id, o::NEXT);
                    c::set_pi16(w, id, o::STATION, n);
                    choose(w, id);
                    w.mm(id).state = 7;
                    set_pose(w, id, p, r);
                    if let Some(t) = twin(w, id, c::pi16(w, id, o::NEXT)) { put_away(w, t); }
                }
            }
        }
        6 => {
            let done = story::mission_done(w, c::pi32(w, id, o::MISSIONS)) || story::mission_done(w, c::pi32(w, id, o::MISSIONS + 4));
            if done {
                let coll = super::class_collision(w, w.m(id).o_class);
                let m = w.mm(id);
                m.state = 1;
                m.has_collision = coll;
                m.mode &= 0xfffe;
                m.visible = 1;
            }
        }
        7 => {
            let t = ease(w, id, DT2, DT * 0.5);
            let k = -2.0 * t * t * t + 3.0 * t * t;
            let from = (c::pv4(w, id, o::SAVED), c::pv4(w, id, o::SAVED + 0x10));
            let to = path_pose(w, id, 0.0);
            blend(w, id, from, to, k);
            if done(w, id) { w.mm(id).state = 4; }
        }
        _ => {}
    }
    bob(w, id);
    let st = w.m(id).state;
    if (3..=4).contains(&st) {
        if aboard(w, id) {
            let f = w.hero_fields_mut();
            f.edge_brake = 2;
            f.jump_lockout = 2;
            wall(w, id);
        }
        if !w.sound_alive(c::pi16(w, id, o::VOICE) as i32, id) {
            let v = w.play_sound(0, 4, id);
            c::set_pi16(w, id, o::VOICE, v as i16);
        }
    } else {
        let v = c::pi16(w, id, o::VOICE) as i32;
        if w.sound_alive(v, id) {
            if v != -1 && w.sound_owner(v) == Some(id) { w.release_sound(v, id); }
            c::set_pi16(w, id, o::VOICE, -1);
        }
    }
    let pos = c::pos(w, id);
    let delta = [pos[0] - old[0], pos[1] - old[1], pos[2] - old[2], pos[3]];
    let rot = w.m(id).rotation;
    triggers::carry_riders(&mut w.mm(id).pvars, o::RIDERS, delta, old_rot, rot);
}

/// `0x2f1490` (module doc).
fn bob(w: &mut World, id: MobyId) {
    let (p, r) = (c::pv4(w, id, o::POS), c::pv4(w, id, o::ROT));
    w.mm(id).position = p;
    w.mm(id).rotation = r;
    let a = c::add_rot(c::pf(w, id, o::BOB), DT * 2.094_395_2);
    c::set_pf(w, id, o::BOB, a);
    w.mm(id).position[2] += a.sin() * 0.25;
    let b = c::add_rot(c::pf(w, id, o::BOB + 4), DT * 1.692_969_3);
    c::set_pf(w, id, o::BOB + 4, b);
    let ry = c::add_rot(w.m(id).rotation[1], b.sin() * 0.087_266_46);
    w.mm(id).rotation[1] = ry;
    let d = c::add_rot(c::pf(w, id, o::BOB + 8), DT * 0.925_024_5);
    c::set_pf(w, id, o::BOB + 8, d);
    let rx = c::add_rot(w.m(id).rotation[0], d.sin() * 0.087_266_46);
    let lim = 0.174_532_92;
    let m = w.mm(id);
    m.rotation[0] = if lim < rx { f32::from_bits(0x3e32_b8c2) } else if rx < -lim { f32::from_bits(0xbe32_b8c2) } else { rx };
    let ry = m.rotation[1];
    if lim < ry { m.rotation[1] = f32::from_bits(0x3e32_b8c2); } else if ry < -lim { m.rotation[1] = f32::from_bits(0xbe32_b8c2); }
}

/// `0x2f1638`: the 7 wall points round the boat, Ratchet's wall spline.
fn wall(w: &mut World, id: MobyId) {
    let Some(k) = usize::try_from(c::pi32(w, id, o::WALL)).ok().filter(|&k| w.svc.splines.get(k).is_some_and(|s| s.len() == 7)) else { return };
    let (r, p) = (w.m(id).rows, c::pos(w, id));
    for (j, [x, y]) in WALL.into_iter().enumerate() {
        let q: [f32; 3] = std::array::from_fn(|i| r[0][i] * x + r[1][i] * y + p[i]);
        let old_w = w.svc.splines[k][j][3];
        w.svc.splines[k][j] = [q[0].to_bits(), q[1].to_bits(), q[2].to_bits(), old_w];
    }
    w.hero_fields_mut().wall_spline = Some(k as i16);
}
