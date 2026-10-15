//! **The hover cars**: Kalebo's class 1410 (level16 `0x2e43e0` with its helpers `0x2e4900`..`0x2e4f30`, 2 placed in
//! group 43; census U558) and Rilgar's class 998 (level05 `0x318c78` with `0x319208`..`0x3198e8`, 5 placed in group 63;
//! census U215): the same code compiled per level with its own pvar layout, station count, routes, visibility rule,
//! prompts and boarding test ([`Layout`]; the names are descriptive [L]). A car waits at one of its stations (the
//! cuboids at pvar +0x80..); standing on it (within its boarding box: the local y within 1.2, on Rilgar also −0.75 < x
//! < 1) the prompt names where it goes and △ sends it: the other car, the one already at the destination, hides; the
//! car eases from its station onto its route (a spline), rides it, and eases into the destination station, where the
//! hidden car reappears at the station it left. While riding, the car hums, holds Ratchet's jump and edge brake, and
//! keeps him on through the spline wall (its spline rewritten each tick to seven points round the car). It bobs and
//! sways all the time and carries Ratchet with every move. Read from the level16 / level05 decomp and the tables
//! 0x1d96e0 / 0x215c00 (the same seven points).
//!
//! **Pvars** (the layout's offsets): +0x08 the platform block's offset (0x20), +0x60 the placed position and +0x70 the
//! Euler (the bob is added to the moby's), +0x80 the station cuboids, the routes, the wall spline, the route ridden (a
//! pointer in the game; the spline index here), s16 the hum's voice, s16 the station, s16 the destination, the ride's t
//! and its velocity, the bob's three phases, the route's top speed and acceleration.
//!
//! | address (16 / 05) | what | port |
//! |---|---|---|
//! | every tick first | the old position and Euler kept; `0x261c98` / `0x2907b8(m, shown)` (L01 `0x27b438`: the talked word) | [`update`] (`interact::set_talked`) |
//! | state 0 | voice −1 (Rilgar also the s16 before it); the init (`0x2e4900` / `0x319208`): the station = the cuboid within 1 (`vec_distance`) of the position; Kalebo: item 0x21 (0x13d4e1) not owned and not station 2 → hidden (6); Rilgar: at station 2 with neither mission +0xb8 nor +0xbc done (0x14c050[level·16 + m] = 0xff) → hidden (6); else → 1, the class collision, shown; +0x60 / +0x70 = that cuboid's centre / Euler | [`init`] |
//! | state 1 | Ratchet on it, grounded, inside the boarding box ((Ratchet − position)·rows): the route (`0x2e4a58` / `0x3193a8`: by station, [`Layout::route`]; t = v = 0; acceleration 5·dt²/(d·n), top speed 20·dt/(d·n), d = the route's first segment, n its points); `try_set_help_message(8, msg)` by destination; △ (0x13cae4 & 0x10) with the prompt owner 8 → 3, the car of the group at the destination → 2, no collision, hidden | [`update`] |
//! | state 3 | `0x257268` / `0x285be8(1, dt², dt², dt/2, &t, &v)`; position = lerp(the station's centre, the route's start, t), yaw / roll toward the start's by t; t ≥ 1 → t = v = 0, → 4 | [`update`] |
//! | state 4 | the spring with the route's speeds; the pose = the route at t·(n − 1) (`0x25e338` / `0x28d350` = L01 `0x277d40`); t ≥ 1 → t = v = 0, → 5 | [`update`] (`path::pose`) |
//! | state 5 | as 3 from the route's end to the destination; t ≥ 1 → t = v = 0, → 1, the hidden car put at this car's station and re-placed (the init), the station = the destination | [`update`] |
//! | state 6 | the visibility rule met → 1, the class collision, shown | [`update`] |
//! | every tick | the bob (`0x2e4d88` / `0x319740`): position / Euler = the placed ones; z += 0.25·sin(+120°·dt); pitch += 5°·sin(+97°·dt); roll += 5°·sin(+53°·dt); roll and pitch clamped to ±10° | [`bob`] |
//! | states 3..5 | 0x13f544 = 0x13f542 = 2; the wall (`0x2e4f30` / `0x3198e8`): the wall spline (of 7 points) = the table's points through the rows plus the position, 0x14162a = it; no hum → `PlayClassSound(0, 4)` | [`update`] (`HeroFields`) |
//! | other states | the hum alive → released (owner-guarded), voice −1 | [`update`] |
//! | every tick last | `CarryRiders(+0x20, position − old, old Euler, Euler)` | [`update`] (`triggers::carry_riders`) |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, turn, DT, DT2};
use crate::moby_update::services::World;
use crate::moby_update::triggers;

/// The prompt's owner.
pub const OWNER: i32 = 8;
pub const PROMPT_OTHER: i32 = 0x139b;
const TRIANGLE: u32 = 0x10;
/// The wall's points round the car (0x1d96e0 / 0x215c00: x, y, z, w = 1).
const WALL: [[f32; 3]; 7] = [[1.3, 1.3, 0.0], [1.3, -1.3, 0.0], [0.0, -1.65, 0.0], [-1.3, -1.3, 0.0], [-1.3, 1.3, 0.0], [0.0, 1.65, 0.0], [1.3, 1.3, 0.0]];
const TEN_DEG: f32 = 0.174_532_92;
const BLOCK: usize = 0x20;
const POS: usize = 0x60;
const ROT: usize = 0x70;
const STATIONS: usize = 0x80;

/// When a car shows.
#[derive(Clone, Copy, Debug)]
pub enum Shown {
    /// Kalebo: item `item` owned, or at station `always`.
    Item { item: usize, always: i16 },
    /// Rilgar: at station `at` only once mission `a` or `b` (pvar offsets of the mission bytes) is done.
    Missions { at: i16, a: usize, b: usize },
}

/// A route choice: from a station, the route's pvar and the destination.
#[derive(Clone, Copy, Debug)]
pub enum Route {
    To(usize, i16),
    /// Rilgar's station 2: mission `m` (pvar offset) done → the first, else the second.
    Mission(usize, (usize, i16), (usize, i16)),
}

/// One level's car (module doc).
#[derive(Clone, Copy, Debug)]
pub struct Layout {
    pub level: u32,
    pub update: u32,
    pub class: i16,
    pub stations: usize,
    /// The route by station (None: the route kept).
    pub routes: &'static [Option<Route>],
    pub shown: Shown,
    pub prompts: [i32; 5],
    /// Rilgar's boarding box also bounds the local x (−0.75 < x < 1).
    pub x_box: bool,
    pub wall: usize,
    pub path: usize,
    /// The extra s16 set to −1 at state 0 (Rilgar's +0xb0).
    pub extra_voice: Option<usize>,
    pub voice: usize,
    pub station: usize,
    pub dest: usize,
    pub t: usize,
    pub v: usize,
    pub bob: usize,
    pub vmax: usize,
    pub accel: usize,
}

impl Layout {
    pub const fn size(&self) -> usize { self.accel + 4 }
}

pub const KALEBO: Layout = Layout {
    level: 16,
    update: 0x2e_43e0,
    class: 1410,
    stations: 3,
    routes: &[Some(Route::To(0x90, 1)), Some(Route::To(0x94, 0)), Some(Route::To(0x9c, 1))],
    shown: Shown::Item { item: 0x21, always: 2 },
    prompts: [0x3e89, 0x3e8a, 0x139a, 0x139c, 0x1399],
    x_box: false,
    wall: 0xa0,
    path: 0xa4,
    extra_voice: None,
    voice: 0xaa,
    station: 0xac,
    dest: 0xae,
    t: 0xb0,
    v: 0xb4,
    bob: 0xb8,
    vmax: 0xc4,
    accel: 0xc8,
};

pub const RILGAR: Layout = Layout {
    level: 5,
    update: 0x31_8c78,
    class: 998,
    stations: 5,
    routes: &[Some(Route::To(0x94, 1)), Some(Route::To(0x98, 0)), Some(Route::Mission(0xb8, (0xa4, 4), (0x9c, 3))), Some(Route::To(0xa0, 2)), Some(Route::To(0xa8, 2))],
    shown: Shown::Missions { at: 2, a: 0xb8, b: 0xbc },
    prompts: [0x13a5, 0x1398, 0x139a, 0x139c, 0x1399],
    x_box: true,
    wall: 0xdc,
    path: 0xac,
    extra_voice: Some(0xb0),
    voice: 0xb2,
    station: 0xb4,
    dest: 0xb6,
    t: 0xc0,
    v: 0xc4,
    bob: 0xc8,
    vmax: 0xd4,
    accel: 0xd8,
};

pub const KALEBO_CLASSES: [i16; 1] = [1410];
pub const RILGAR_CLASSES: [i16; 1] = [998];

fn station_shape(w: &World, id: MobyId, l: &Layout, k: i16) -> Option<rc_formats::volumes::Shape> {
    if !(0..l.stations as i16).contains(&k) { return None; }
    w.svc.volumes.shape(rc_formats::volumes::ShapeKind::Cuboid, c::pi32(w, id, STATIONS + 4 * k as usize)).copied()
}

fn path_points(w: &World, id: MobyId, l: &Layout) -> Option<Vec<crate::path::Point>> {
    usize::try_from(c::pi32(w, id, l.path)).ok().and_then(|i| w.svc.splines.get(i)).cloned().filter(|p| !p.is_empty())
}

fn mission_done(w: &World, id: MobyId, o: usize) -> bool { super::hints::mission_done(w, c::pi32(w, id, o)) }

fn visible_rule(w: &World, id: MobyId, l: &Layout, station: i16) -> bool {
    match l.shown {
        Shown::Item { item, always } => super::hints::owned(w, item) || station == always,
        Shown::Missions { at, a, b } => station != at || mission_done(w, id, a) || mission_done(w, id, b),
    }
}

/// The init `0x2e4900` / `0x319208`: the station, shown or not, the placed pose (module doc).
pub fn init(w: &mut World, id: MobyId, l: &Layout) {
    c::set_pi16(w, id, l.station, -1);
    let p = w.m(id).position;
    for k in 0..l.stations as i16 {
        let Some(s) = station_shape(w, id, l, k) else { continue };
        let c3 = s.centre();
        if ((p[0] - c3[0]).powi(2) + (p[1] - c3[1]).powi(2) + (p[2] - c3[2]).powi(2)).sqrt() < 1.0 { c::set_pi16(w, id, l.station, k); }
    }
    let station = c::pi16(w, id, l.station);
    if !visible_rule(w, id, l, station) {
        let m = w.mm(id);
        m.state = 6;
        m.has_collision = false;
        m.mode |= 1;
        m.visible = 0;
    } else if matches!(l.shown, Shown::Item { .. }) || (0..l.stations as i16).contains(&station) {
        // (Rilgar's init switch leaves a car off its stations as it is.)
        show(w, id);
    }
    if let Some(s) = station_shape(w, id, l, station) {
        c::set_pv4(w, id, POS, s.matrix[3]);
        c::set_pv4(w, id, ROT, [s.euler[0], s.euler[1], s.euler[2], s.unused_7c]);
    }
}

fn show(w: &mut World, id: MobyId) {
    let coll = super::class_collision(w, w.m(id).o_class);
    let m = w.mm(id);
    m.state = 1;
    m.has_collision = coll;
    m.mode &= !1;
    m.visible = 1;
}

/// The route choice `0x2e4a58` / `0x3193a8` (module doc).
fn choose_route(w: &mut World, id: MobyId, l: &Layout) {
    let station = c::pi16(w, id, l.station);
    let pick = usize::try_from(station).ok().and_then(|k| l.routes.get(k)).copied().flatten().map(|r| match r {
        Route::To(o, d) => (o, d),
        Route::Mission(m, a, b) => if mission_done(w, id, m) { a } else { b },
    });
    if let Some((o, d)) = pick {
        let path = c::pi32(w, id, o);
        c::set_pi32(w, id, l.path, path);
        c::set_pi16(w, id, l.dest, d);
    }
    c::set_pf(w, id, l.v, 0.0);
    c::set_pf(w, id, l.t, 0.0);
    let Some(pts) = path_points(w, id, l) else { return };
    let pt = |k: usize| pts.get(k).copied().unwrap_or_default().map(f32::from_bits);
    let (a, b) = (pt(0), pt(1));
    let d = ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
    let inv = 1.0 / (d * pts.len() as f32);
    c::set_pf(w, id, l.accel, DT2 * 5.0 * inv);
    c::set_pf(w, id, l.vmax, DT * 20.0 * inv);
}

/// `0x2e4be0` / `0x319598(m, station)`: the car of this car's group at `station`.
fn car_at(w: &World, id: MobyId, l: &Layout, station: i16) -> Option<MobyId> {
    let g = usize::from(w.m(id).group as u8);
    let list = w.svc.groups.lists.get(g).cloned().flatten()?;
    list.into_iter().map(usize::from).find(|&k| k < w.table.mobys.len() && w.m(k).pvars.len() >= l.size() && c::pi16(w, k, l.station) == station)
}

/// The ease of states 3 / 5: position `a → b` and yaw / roll `ea → eb` by the spring's t; true when it ends.
fn ease(w: &mut World, id: MobyId, l: &Layout, a: [f32; 4], ea: [f32; 4], b: [f32; 4], eb: [f32; 4]) -> bool {
    let (mut t, mut v) = (c::pf(w, id, l.t), c::pf(w, id, l.v));
    turn::spring(1.0, DT2, DT2, DT * 0.5, &mut t, &mut v);
    c::set_pf(w, id, l.t, t);
    c::set_pf(w, id, l.v, v);
    c::set_pv4(w, id, POS, std::array::from_fn(|k| a[k] + (b[k] - a[k]) * t));
    for k in [1usize, 2] {
        let e = c::add_rot(c::sub_rot(eb[k], ea[k]) * t, ea[k]);
        c::set_pf(w, id, ROT + 4 * k, e);
    }
    if 1.0 <= t {
        c::set_pf(w, id, l.v, 0.0);
        c::set_pf(w, id, l.t, 0.0);
        return true;
    }
    false
}

/// The bob (module doc).
pub fn bob(w: &mut World, id: MobyId, l: &Layout) {
    let (pos, rot) = (c::pv4(w, id, POS), c::pv4(w, id, ROT));
    let ph: [f32; 3] = std::array::from_fn(|k| c::pf(w, id, l.bob + 4 * k));
    let p0 = c::add_rot(ph[0], DT * 2.094_395_2);
    let p1 = c::add_rot(ph[1], DT * 1.692_969_3);
    let p2 = c::add_rot(ph[2], DT * 0.925_024_5);
    for (k, p) in [p0, p1, p2].into_iter().enumerate() { c::set_pf(w, id, l.bob + 4 * k, p); }
    let m = w.mm(id);
    m.position = pos;
    m.rotation = rot;
    m.position[2] += p0.sin() * 0.25;
    m.rotation[1] = c::add_rot(m.rotation[1], p1.sin() * 0.087_266_46);
    m.rotation[0] = c::add_rot(m.rotation[0], p2.sin() * 0.087_266_46);
    m.rotation[0] = m.rotation[0].clamp(-TEN_DEG, TEN_DEG);
    m.rotation[1] = m.rotation[1].clamp(-TEN_DEG, TEN_DEG);
}

/// The wall spline rewritten round the car, and named (module doc).
fn wall(w: &mut World, id: MobyId, l: &Layout) {
    let s = c::pi32(w, id, l.wall);
    let Some(i) = usize::try_from(s).ok().filter(|&i| w.svc.splines.get(i).is_some_and(|p| p.len() == 7)) else { return };
    let (rows, pos) = (w.m(id).rows, w.m(id).position);
    let pts: Vec<[u32; 4]> = WALL
        .iter()
        .map(|q| std::array::from_fn(|k| (q[0] * rows[0][k] + q[1] * rows[1][k] + q[2] * rows[2][k] + if k == 3 { 1.0 } else { 0.0 } + pos[k]).to_bits()))
        .collect();
    w.svc.splines[i] = pts;
    w.hero_fields_mut().wall_spline = Some(s as i16);
}

fn boarding(w: &World, id: MobyId, l: &Layout) -> bool {
    if w.hero.ground_moby != Some(id) || w.hero.air_ticks != 0 { return false; }
    let (h, m) = (super::hero_pos(w), w.m(id));
    let d = [h[0] - m.position[0], h[1] - m.position[1], h[2] - m.position[2]];
    let lx = d[0] * m.rows[0][0] + d[1] * m.rows[0][1] + d[2] * m.rows[0][2];
    let ly = d[0] * m.rows[1][0] + d[1] * m.rows[1][1] + d[2] * m.rows[1][2];
    ly.abs() < 1.2 && (!l.x_box || (lx < 1.0 && -0.75 < lx))
}

/// Kalebo's 1410 (module doc).
pub fn update_kalebo(w: &mut World, id: MobyId) { update(w, id, &KALEBO) }

/// Rilgar's 998 (module doc).
pub fn update_rilgar(w: &mut World, id: MobyId) { update(w, id, &RILGAR) }

/// The update (module doc).
pub fn update(w: &mut World, id: MobyId, l: &Layout) {
    if w.m(id).pvars.len() < l.size() { w.mm(id).pvars.resize(l.size(), 0); }
    let (old_pos, old_rot) = (w.m(id).position, w.m(id).rotation);
    let shown = (w.m(id).mode ^ 1) & 1;
    crate::moby_update::interact::set_talked(w, id, shown as u32);
    let state = w.m(id).state;
    match state {
        0 => {
            if let Some(o) = l.extra_voice { c::set_pi16(w, id, o, -1); }
            c::set_pi16(w, id, l.voice, -1);
            init(w, id, l);
        }
        1 => {
            if boarding(w, id, l) {
                choose_route(w, id, l);
                let dest = c::pi16(w, id, l.dest);
                let msg = usize::try_from(dest).ok().and_then(|k| l.prompts.get(k)).copied().unwrap_or(PROMPT_OTHER);
                w.svc.interact.try_prompt(OWNER, msg);
                if w.hero.loop_in.pad.pressed & TRIANGLE != 0 && w.svc.interact.prompt.owner == OWNER {
                    w.mm(id).state = 3;
                    if let Some(o) = car_at(w, id, l, dest) {
                        let m = w.mm(o);
                        m.state = 2;
                        m.has_collision = false;
                        m.mode |= 1;
                        m.visible = 0;
                    }
                }
            }
        }
        3 => {
            let st = station_shape(w, id, l, c::pi16(w, id, l.station));
            let start = path_points(w, id, l).map(|p| crate::path::pose(&p, false, 0.0, true));
            if let (Some(s), Some((p, e))) = (st, start) {
                let ea = [s.euler[0], s.euler[1], s.euler[2], s.unused_7c];
                if ease(w, id, l, s.matrix[3], ea, p, e) { w.mm(id).state = 4; }
            }
        }
        4 => {
            let (mut t, mut v) = (c::pf(w, id, l.t), c::pf(w, id, l.v));
            let (acc, vmax) = (c::pf(w, id, l.accel), c::pf(w, id, l.vmax));
            turn::spring(1.0, acc, acc, vmax, &mut t, &mut v);
            c::set_pf(w, id, l.t, t);
            c::set_pf(w, id, l.v, v);
            if let Some(pts) = path_points(w, id, l) {
                let (p, e) = crate::path::pose(&pts, false, t * (pts.len() as f32 - 1.0), true);
                c::set_pv4(w, id, POS, p);
                c::set_pv4(w, id, ROT, e);
            }
            if 1.0 <= t {
                c::set_pf(w, id, l.t, 0.0);
                c::set_pf(w, id, l.v, 0.0);
                w.mm(id).state = 5;
            }
        }
        5 => {
            let st = station_shape(w, id, l, c::pi16(w, id, l.dest));
            let end = path_points(w, id, l).map(|p| crate::path::pose(&p, false, p.len() as f32, true));
            if let (Some(s), Some((p, e))) = (st, end) {
                let eb = [s.euler[0], s.euler[1], s.euler[2], s.unused_7c];
                if ease(w, id, l, p, e, s.matrix[3], eb) {
                    w.mm(id).state = 1;
                    arrive(w, id, l);
                }
            }
        }
        6 if visible_rule(w, id, l, c::pi16(w, id, l.station)) => show(w, id),
        _ => {}
    }
    bob(w, id, l);
    let v = c::pi16(w, id, l.voice) as i32;
    if (3..=5).contains(&w.m(id).state) {
        let f = w.hero_fields_mut();
        f.edge_brake = 2;
        f.jump_lockout = 2;
        wall(w, id, l);
        if !w.sound_alive(v, id) {
            let s = w.play_sound(0, 4, id);
            c::set_pi16(w, id, l.voice, s as i16);
        }
    } else if w.sound_alive(v, id) {
        w.release_sound(v, id);
        c::set_pi16(w, id, l.voice, -1);
    }
    let m = w.mm(id);
    let disp = [m.position[0] - old_pos[0], m.position[1] - old_pos[1], m.position[2] - old_pos[2], m.position[3] - old_pos[3]];
    let rot = m.rotation;
    triggers::carry_riders(&mut m.pvars, BLOCK, disp, old_rot, rot);
}

/// `0x2e4b58` / `0x319510`: the hidden car put at this car's station; the station = the destination.
fn arrive(w: &mut World, id: MobyId, l: &Layout) {
    let (station, dest) = (c::pi16(w, id, l.station), c::pi16(w, id, l.dest));
    if let Some(o) = car_at(w, id, l, dest) {
        if let Some(s) = station_shape(w, id, l, station) {
            let m = w.mm(o);
            m.position = s.matrix[3];
            m.rotation = [s.euler[0], s.euler[1], s.euler[2], s.unused_7c];
        }
        init(w, o, l);
    }
    c::set_pi16(w, id, l.station, dest);
}
