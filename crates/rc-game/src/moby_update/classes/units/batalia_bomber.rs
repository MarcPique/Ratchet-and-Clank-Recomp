//! **Batalia's bombers and missiles, class 435** (level08 `0x2dd3c0`, census U273; 2 placed bombers, the missiles
//! made by the gunships through `0x2de3e0`). One update, two roles by state:
//!
//! * **The bombers** (placed; states 0 / 3 / 4 / 6): hidden until Ratchet is within +0x8c of the end of their path
//!   (+0x64; +0x90 = 0 also waits `ticks(10)`), then they fly it (25 a second, `SpringTurn2` to the next point,
//!   sparks), and at its end crash into their wall: the story flag 0x13d3b4 + +0x90 is set, the wall's group members
//!   547–552 are flung (`loose_piece::fling`; the larger than 1500 deleted), a ferry 424 in the group explodes and goes,
//!   30 pieces fly out of the cuboid +0x88 (`0x2de528`), sound 2, → 6. Their flag already set at the start: the whole
//!   group is deleted.
//! * **The missiles** (made: states 1 / 2 / 5 / 6): from a gunship's joint, aimed at the turret 440 (+0x4c): they climb
//!   to a point 70 from the turret (5 up) along their launch angles (+0x40 pitch, +0x44 yaw) at the turret's missile
//!   speed (its +0x98, `batalia_turret::missile_speed`), steering by at most 5·dt² a tick, until the turret sees them
//!   at those angles (5°), then dive at it (20·dt², growing to 3× the class scale over `ticks(180)`); within 10 they hit
//!   (60 off the turret's health, `batalia_turret::hit`), sound 0, → 6. The gunships' idle ones (5) fly straight on
//!   (turning to +0x44 / −+0x40 at 30° a second, 20 a second) until they hit something or come within 16 of Ratchet
//!   or leave [5, 500]. Trailing sparks (type 4) on the way; 6: a beam explosion (made ones only), deleted.
//!
//! Any hit on it (0x210000) but in state 4: sound 0, the death explosion (0.5, 13), deleted. Read from the level08
//! decomp; the level data from the overlay (gp 0x166c00). Native `f32`.
//!
//! **Pvars** (bomber): +0x50 the path point, +0x54 (s8) its direction, +0x64 the path, +0x80 the turn speed, +0x88
//! the debris cuboid, +0x8c the reach, +0x90 the flag's index, +0x94 the start timer. (Missile): +0x20 the velocity,
//! +0x40 / +0x44 the launch pitch / yaw, +0x48 70, +0x4c the turret (the port: moby index + 1, 0 none).
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x2dd3c0` | the states (module doc) | [`update`] |
//! | `0x2de3e0(pitch, yaw, p, target, v)` | `CreateMoby(0x1b3)` at p, facing the camera (pitch −atan(\|cam − p\|, Δz)), update 0xff, state 1, drawn, draw 0xff, scale 0.75 of the class, ambient (0x80, 0x40, 0x50); +0x40 pitch, +0x44 yaw, +0x48 70, +0x4c target, +0x20 v; `MobyBuildMatrix` | [`spawn_missile`] |
//! | `0x2de528(m, cuboid, n, k)` | `SpawnBeamExplosion(0, 0, 10 / 6, 6 / 4, 9, 1, 15; 45 / 30, 30 / 20, 30 / 20, shake)` (k 0 / 1); n pieces 0x310 / 0x311 (k 0) or 0x30e / 0x30f (k 1) at random in the cuboid, a random Euler, flung out from the bomber at `randf(12, 15)·dt` (xy), up `randf(6, 18)·dt`, `ticks(randf(60, 90))` | [`debris`] |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::fx::{self, Beam};
use crate::moby_update::creature::{self as c, add, add_rot, atan, diff_rots, dist2, dist3, set_len3, sub, turn, DT, DT2, V};
use crate::moby_update::services::{pf, pv, World};
use crate::moby_update::story;

pub const REFERENCE_LEVEL: u32 = 8;
pub const UPDATE_FN: u32 = 0x2d_d3c0;
pub const CLASSES: [i16; 1] = [435];

const CLASS: i16 = 0x1b3;
const FERRY: i16 = 0x1a8;
const FLAG: u32 = 0x13_d3b4;
/// gp−0x5200..−0x51c0.
const FREE_SPEED: f32 = 10.0;
const STEER: f32 = 5.0;
const BOMB_SPEED: f32 = 25.0;
const HIT_RANGE: f32 = 10.0;
const SPARK_SPEED: f32 = 1.0;
const SPARK_LIFE: i32 = 40;
const SPARK_BASE: f32 = 100.0;
const SPARK_GROWTH: f32 = 200.0;
const SPARK_C1: u32 = 0x4000_80ff;
const SPARK_C2: u32 = 0x40ff;
const SCALE: f32 = 0.75;

mod pvo {
    pub const VEL: usize = 0x20;
    pub const PITCH: usize = 0x40;
    pub const YAW: usize = 0x44;
    pub const DIST: usize = 0x48;
    pub const TARGET: usize = 0x4c;
    pub const NODE: usize = 0x50;
    pub const DIR: usize = 0x54;
    pub const PATH: usize = 0x64;
    pub const TURN_V: usize = 0x80;
    pub const CUBOID: usize = 0x88;
    pub const REACH: usize = 0x8c;
    pub const FLAG: usize = 0x90;
    pub const TIMER: usize = 0x94;
    pub const LEN: usize = 0xa0;
}

/// The wall pieces' fling offsets (classes 547..552).
const OFFSETS: [[f32; 3]; 6] = [
    [f32::from_bits(0xc055_2f1b), f32::from_bits(0x4055_2f1b), f32::from_bits(0x3e8c_cccd)],
    [-6.0, 6.0, f32::from_bits(0xbfe2_d0e5)],
    [-2.0, f32::from_bits(0xbf93_74bc), f32::from_bits(0xbfe2_d0e5)],
    [0.0, 0.0, f32::from_bits(0x3f79_999a)],
    [0.0, f32::from_bits(0xbfd5_1eb8), f32::from_bits(0x3f24_5a1d)],
    [0.0, 0.0, 0.0],
];

fn target(w: &World, id: MobyId) -> Option<MobyId> { usize::try_from(c::pi32(w, id, pvo::TARGET) - 1).ok().filter(|&m| m < w.table.mobys.len()) }
fn path(w: &World, id: MobyId) -> Vec<[f32; 4]> {
    usize::try_from(c::pi32(w, id, pvo::PATH)).ok().and_then(|i| w.svc.splines.get(i)).map(|v| v.iter().map(|q| q.map(f32::from_bits)).collect()).unwrap_or_default()
}
fn flag_index(w: &World, id: MobyId) -> usize { story::flag_index(FLAG) + c::pi32(w, id, pvo::FLAG).max(0) as usize }
fn group(w: &World, id: MobyId) -> Vec<MobyId> {
    usize::try_from(w.m(id).group).ok().and_then(|g| w.svc.groups.lists.get(g)).and_then(|l| l.as_ref()).map(|l| l.iter().map(|&x| x as usize).collect()).unwrap_or_default()
}

/// `0x2de3e0(pitch, yaw, p, target, v)` (module doc).
pub fn spawn_missile(w: &mut World, pitch: f32, yaw: f32, p: V, target: Option<MobyId>, v: V) -> Option<MobyId> {
    let m = w.create_moby(CLASS)?;
    if w.m(m).pvars.len() < pvo::LEN { w.mm(m).pvars.resize(pvo::LEN, 0); }
    let cam = w.camera.map(|x| x.to_f32());
    let scale = super::class_scale(w, CLASS) * SCALE;
    {
        let mo = w.mm(m);
        mo.position = p;
        mo.rotation[1] = -atan(((cam[0] - p[0]).powi(2) + (cam[1] - p[1]).powi(2)).sqrt(), cam[2] - p[2]);
        mo.rotation[2] = atan(cam[0] - p[0], cam[1] - p[1]);
        mo.update_dist = 0xff;
        mo.state = 1;
        mo.visible = 1;
        mo.draw_dist = 0xff;
        mo.scale = scale;
        mo.ambient = [0x80, 0x40, 0x50, mo.ambient[3]];
    }
    c::set_pf(w, m, pvo::PITCH, pitch);
    c::set_pf(w, m, pvo::DIST, 70.0);
    c::set_pf(w, m, pvo::YAW, yaw);
    c::set_pi32(w, m, pvo::TARGET, target.map_or(0, |t| t as i32 + 1));
    c::set_pv4(w, m, pvo::VEL, v);
    w.build_matrix(m);
    Some(m)
}

/// Level08 `0x2dd3c0` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    story::pvars(w, id, pvo::LEN);
    if w.get_hit(id, 0x21_0000, false).is_some() && w.m(id).state != 4 {
        w.play_sound(0, 0, id);
        let p = c::pos(w, id);
        fx::death_explosion(w, 0.5, 13.0, Some(id), p, -1);
        w.delete_moby(id);
        return;
    }
    match w.m(id).state {
        0 => {
            if story::flag(w, flag_index(w, id)) != 0 {
                for m in group(w, id) { w.delete_moby(m); }
                return;
            }
            if c::pi32(w, id, pvo::FLAG) == 0 {
                let t = w.ticks(10);
                c::set_pi32(w, id, pvo::TIMER, t);
            }
            if let Some(&first) = path(w, id).first() { c::set_pos(w, id, first); }
            c::set_pi32(w, id, pvo::NODE, 1);
            let m = w.mm(id);
            m.state = 3;
            m.draw_dist = 0;
            m.has_collision = false;
        }
        1 | 2 => missile(w, id),
        3 => {
            let pts = path(w, id);
            let Some(&last) = pts.last() else { return };
            if w.svc.game_mode == 0 && dist3(super::hero_pos(w), last) < c::pf(w, id, pvo::REACH) && c::dec_timer_pvar_i32(w, id, pvo::TIMER) != 0 {
                let has = super::class_collision(w, CLASS);
                let p = c::pos(w, id);
                let m = w.mm(id);
                m.draw_dist = 0x40;
                m.state = 4;
                m.has_collision = has;
                if let Some(q) = pts.get(1) { w.mm(id).rotation[2] = atan(q[0] - p[0], q[1] - p[1]); }
                w.play_sound(1, 0, id);
            }
        }
        4 => {
            if w.svc.game_mode == 0 { bomb_run(w, id); }
        }
        5 => free_flight(w, id),
        6 => {
            if w.table.first_dynamic <= id {
                const B: Beam = Beam { damage_r: 0.0, damage: 0.0, flash: 3.0, flash2: 2.0, flash_dist: 9.0, scale: 1.0, light: 15.0, streaks: 15, sparks: 10, puffs: 10, debris: 0, sound: -1, shake: true };
                let p = c::pos(w, id);
                fx::beam_explosion(w, &B, Some(id), p);
            }
            w.delete_moby(id);
        }
        _ => {}
    }
}

/// The trail's spark: a random turn about the moby's x axis at `randf(0, 1)·dt`.
fn spark(w: &mut World, id: MobyId) {
    let s = w.rng.randf(0.0, SPARK_SPEED) * DT;
    let a = w.rng.rand_angle();
    let r = w.m(id).rows;
    let v = add(set_len3(r[2], s * a.cos()), set_len3(r[1], s * a.sin()));
    let life = w.ticks(SPARK_LIFE);
    let s = crate::particles::type04::Spawn { pos: c::pos(w, id), vel: v, c1: SPARK_C1, c2: SPARK_C2, life, base: SPARK_BASE as i16, growth: SPARK_GROWTH as i16, additive: true };
    fx::part04(w, &s);
}

/// States 1 / 2 (module doc).
fn missile(w: &mut World, id: MobyId) {
    let Some(t) = target(w, id) else {
        w.mm(id).state = 6;
        return;
    };
    let speed = super::batalia_turret::missile_speed(w, t);
    let spin = add_rot(w.m(id).rotation[0], DT * std::f32::consts::TAU);
    w.mm(id).rotation[0] = spin;
    let diving = w.m(id).state == 2;
    if diving {
        let full = super::class_scale(w, CLASS);
        let mut s = w.m(id).scale;
        let step = full / w.ticks(0xb4) as f32;
        turn::approach(full * 3.0, step, &mut s);
        w.mm(id).scale = s;
    }
    let tp = w.m(t).position;
    let mut aim = [tp[0], tp[1], tp[2] + 5.0, tp[3]];
    let me = c::pos(w, id);
    let bearing = atan(me[0] - aim[0], me[1] - aim[1]);
    let elev = atan(dist2(aim, me), me[2] - aim[2]);
    if !diving {
        let o = fx::polar(c::pf(w, id, pvo::DIST), c::pf(w, id, pvo::YAW), c::pf(w, id, pvo::PITCH));
        aim = add(aim, [o[0], o[1], o[2], 0.0]);
    }
    let d = c::clamp_len3(sub(aim, me), speed * DT);
    let vel = c::pv4(w, id, pvo::VEL);
    let steer = if diving { STEER * 4.0 * DT2 } else { STEER * DT2 };
    let acc = c::clamp_len3(sub(d, vel), steer);
    let vel = add(vel, acc);
    c::set_pv4(w, id, pvo::VEL, vel);
    let p = add(me, vel);
    c::set_pos(w, id, p);
    {
        let m = w.mm(id);
        m.rotation[1] = -atan((vel[0] * vel[0] + vel[1] * vel[1]).sqrt(), vel[2]);
        m.rotation[2] = atan(vel[0], vel[1]);
    }
    if diving {
        if dist3(p, tp) < HIT_RANGE {
            super::batalia_turret::hit(w, t);
            c::set_pv4(w, id, pvo::VEL, [0.0; 4]);
            w.play_sound(0, 0, id);
            w.mm(id).state = 6;
        }
    } else if diff_rots(bearing, c::pf(w, id, pvo::YAW)) < 0.087_266_46 && diff_rots(elev, c::pf(w, id, pvo::PITCH)) < 0.087_266_46 {
        w.mm(id).state = 2;
    }
    spark(w, id);
}

/// State 5 (module doc).
fn free_flight(w: &mut World, id: MobyId) {
    let k = DT * std::f32::consts::FRAC_PI_6;
    let mut yaw = w.m(id).rotation[2];
    turn::approach_rot(c::pf(w, id, pvo::YAW), k, &mut yaw);
    let mut pitch = w.m(id).rotation[1];
    turn::approach_rot(-c::pf(w, id, pvo::PITCH), k, &mut pitch);
    let spin = add_rot(w.m(id).rotation[0], DT * std::f32::consts::TAU);
    {
        let m = w.mm(id);
        m.rotation = [spin, pitch, yaw, m.rotation[3]];
    }
    let v = fx::polar((FREE_SPEED + FREE_SPEED) * DT, yaw, -pitch);
    let vel = [v[0], v[1], v[2], 0.0];
    c::set_pv4(w, id, pvo::VEL, vel);
    let p = add(c::pos(w, id), vel);
    c::set_pos(w, id, p);
    let hit = w.coll_sphere(pv(p), pf(0.25), 2, Some(id)).is_some();
    let inside = |x: f32| (5.0..=500.0).contains(&x);
    if !hit && inside(p[0]) && inside(p[1]) && inside(p[2]) && 16.0 <= dist3(super::hero_pos(w), p) { return; }
    w.mm(id).state = 6;
}

/// State 4: the run along the path and the crash (module doc).
fn bomb_run(w: &mut World, id: MobyId) {
    let pts = path(w, id);
    let n = pts.len() as i32;
    if n == 0 { return; }
    let node = c::pi32(w, id, pvo::NODE);
    let dir = c::pi32(w, id, pvo::DIR) as i8 as i32;
    let i = (node + n + dir).rem_euclid(n);
    let q = pts[i as usize];
    let me = c::pos(w, id);
    turn::spring_turn2_pvar(w, id, atan(q[0] - me[0], q[1] - me[1]), 0.1, f32::from_bits(0x3fa6_6666), 1.0, pvo::TURN_V);
    w.mm(id).rotation[1] = atan(dist2(q, me), me[2] - q[2]);
    if node == n - 1 {
        crash(w, id);
        return;
    }
    let v = set_len3(sub(q, me), BOMB_SPEED * DT);
    let p = add(me, v);
    c::set_pos(w, id, p);
    if dist3(p, q) < 0.5 { c::set_pi32(w, id, pvo::NODE, i); }
    for (c1, c2, life, base, grow, add_) in [(0x6f00_afffu32, 0xffu32, (0x1e, 0x2d), 0x28, (0x28, 0x46), true), (0x1fff_ffff, 0x4f_4f4f, (0x3c, 0x78), 0x28, (100, 0x96), false)] {
        let x = w.rng.randf(-1.0, 1.0);
        let y = w.rng.randf(-1.0, 1.0);
        let z = w.rng.randf(-1.0, 1.0);
        let d = c::clamp_len3([x, y, z, 0.0], DT + DT);
        let l = w.rng.rand_range(life.0, life.1);
        let l = w.ticks(l);
        let g = w.rng.rand_range(grow.0, grow.1);
        let s = crate::particles::type04::Spawn { pos: p, vel: d, c1, c2, life: l, base, growth: g as i16, additive: add_ };
        fx::part04(w, &s);
    }
}

/// The crash at the path's end (module doc).
fn crash(w: &mut World, id: MobyId) {
    let flag = flag_index(w, id);
    story::set_flag(w, flag, 1);
    let me = c::pos(w, id);
    let cam = w.camera.map(|x| x.to_f32());
    let mut big = false;
    for m in group(w, id) {
        if m == id || w.table.mobys.get(m).is_none_or(|x| 0x7f <= x.state) { continue; }
        let class = w.m(m).o_class;
        let k = class - 0x223;
        if !(0..=5).contains(&k) {
            if class == FERRY {
                big = true;
                const B: Beam = Beam { damage_r: 0.0, damage: 0.0, flash: 12.0, flash2: 8.0, flash_dist: 9.0, scale: 1.0, light: 15.0, streaks: 0x32, sparks: 0x3c, puffs: 0x3c, debris: 0, sound: -1, shake: true };
                let p = c::pos(w, m);
                fx::beam_explosion(w, &B, Some(m), p);
                w.delete_moby(m);
            }
            continue;
        }
        let o = OFFSETS[k as usize];
        if 1500.0 < w.m(m).scale {
            w.delete_moby(m);
            continue;
        }
        let mut d = sub(w.m(m).position, me);
        d[2] = 0.0;
        let mut v = set_len3(d, DT * 12.0);
        v[2] = w.rng.randf(DT * 8.0, DT * 12.0);
        w.mm(m).has_collision = false;
        let facing = diff_rots(atan(v[0], v[1]), atan(cam[0] - me[0], cam[1] - me[1])) < std::f32::consts::FRAC_PI_4;
        let life = if facing { w.rng.randf(10.0, 20.0) } else { w.rng.randf(60.0, 90.0) } as i32;
        super::loose_piece::fling(w, m, v, [o[0], o[1], o[2], 0.0], life);
    }
    debris(w, id, c::pi32(w, id, pvo::CUBOID), 0x1e, big);
    w.play_sound(2, 0, id);
    w.mm(id).state = 6;
}

/// `0x2de528(m, cuboid, n, k)` (module doc).
fn debris(w: &mut World, id: MobyId, cuboid: i32, n: i32, big: bool) {
    let me = c::pos(w, id);
    let b = if big {
        Beam { damage_r: 0.0, damage: 0.0, flash: 6.0, flash2: 4.0, flash_dist: 9.0, scale: 1.0, light: 15.0, streaks: 0x1e, sparks: 0x14, puffs: 0x14, debris: 0, sound: -1, shake: true }
    } else {
        Beam { damage_r: 0.0, damage: 0.0, flash: 10.0, flash2: 6.0, flash_dist: 9.0, scale: 1.0, light: 15.0, streaks: 0x2d, sparks: 0x1e, puffs: 0x1e, debris: 0, sound: -1, shake: true }
    };
    fx::beam_explosion(w, &b, Some(id), me);
    let (light, ambient) = (w.m(id).light, w.m(id).ambient);
    for _ in 0..n.max(0) {
        let r = w.rng.randi(0xff) & 1;
        let class = if big { if r == 0 { 0x30f } else { 0x30e } } else if r != 0 { 0x310 } else { 0x311 };
        let Some(m) = w.create_moby(class) else { continue };
        let (a, b2, cc) = (w.rng.rand_angle(), w.rng.rand_angle(), w.rng.rand_angle());
        let (x, y, z) = (w.rng.randf(-1.0, 1.0), w.rng.randf(-1.0, 1.0), w.rng.randf(-1.0, 1.0));
        let at = w.svc.volumes.shape(rc_formats::volumes::ShapeKind::Cuboid, cuboid).map_or([x, y, z], |s| s.world([x, y, z]));
        {
            let mo = w.mm(m);
            mo.draw_dist = 0x40;
            mo.visible = 1;
            mo.update_dist = 0x40;
            mo.light = light;
            mo.ambient = ambient;
            mo.rotation = [a, b2, cc, 0.0];
            mo.position = [at[0], at[1], at[2], 1.0];
        }
        let d = sub(w.m(m).position, me);
        let s = w.rng.randf(DT * 12.0, DT * 15.0);
        let l = (d[0] * d[0] + d[1] * d[1]).sqrt();
        let mut v = if l == 0.0 { [0.0; 4] } else { [d[0] * s / l, d[1] * s / l, d[2], 0.0] };
        v[2] = w.rng.randf(DT * 6.0, DT * 18.0);
        let life = w.rng.randf(60.0, 90.0) as i32;
        super::loose_piece::fling(w, m, v, [0.0; 4], life);
    }
}
