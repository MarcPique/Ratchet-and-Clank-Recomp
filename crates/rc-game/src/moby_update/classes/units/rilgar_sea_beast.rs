//! **Rilgar's sea beasts, class 35** (level05 `0x2d1688` with its wake `0x2d1e30` and splash `0x2d1fd8`, 5 placed;
//! census U183; the name is descriptive [L]). Two kinds by pvar +0x64:
//! * **0, the swimmer**: cruises its spline's points (at random, a new one when within 3 or after 10 s) at 3 a second
//!   just under the bay's surface, leaving a wake, and sinks to 2 under when Ratchet swims within 8 (xy).
//! * **1, the lurker**: hidden below the surface; while Ratchet swims at its height for half a second it lunges up
//!   under him (from the camera's side), grabs and hides him (Ratchet held in state 0x72, carried 20 above it), arcs
//!   over with a splash going in, and drags him down: 8 below its depth he dies. Outside the bay region (its spline as a
//!   polygon) it waits 3 s, inside 10 s.
//!
//! Read from the level05 decomp and its word gp−0x5878 (1.0, the depth under the water). Native `f32`.
//!
//! **Pvars** (0x88): +0x20 the creature record, +0x60 the spline (the swimmer's points, the lurker's region), +0x64 the
//! kind, +0x68 the point, +0x6c the point timer, +0x70 the turn velocity, +0x74 the lurker's depth, +0x78 the vertical
//! speed, +0x7c the lunge timer, +0x84 the wake timer.
//!
//! | address | what | port |
//! |---|---|---|
//! | state 0, kind 0 | no spline → `DeleteMoby`; point `randi(n)`, timer `ticks(600)`, sequence 1 (`ticks(10)`); z = point 0's z − 1; → 1 | [`update`] |
//! | state 0, kind 1 | → 2, hidden, update distance 0xff, +0x74 = z, timer `ticks(30)` | [`update`] |
//! | state 1 | heading at the point (`0x286078` = L01 `0x270cc0`: π·dt², π·dt), position += row 0 · 3·dt; within 3 (`vec_distance`) or the timer out → point = (point + `randi(n)`) mod n, timer `ticks(600)`; Ratchet swimming (group or state 0x12) within 8 (xy) → z toward water (0x1612e8) − 1 − 2; else − 0.5 and the wake; `0x285be8(z, 2·dt², 2·dt², 2·dt, &z, &+0x78)` | [`update`] |
//! | state 2 | the timer counts down; \|Ratchet's water level (0x13f640) − +0x74\| < 0.33 and his group 0x12 → once out: pitch −89°, yaw = camera yaw (0x1671d8) + 90°, rows, position = Ratchet − row 2 · 1, z − 2, vz = 10·dt, shown, sequence 2 (`ticks(5)`), +0xbc = 0, → 3, `CameraScript(the camera, 0, 0, 0)`; else (not in state 0x12) the timer = `ticks(600)` in the region (`0x283a78` = L01 `0x26e6c0`), else `ticks(180)` | [`update`] (`region::point_in_polygon`) |
//! | state 3 | above the water level or falling: Ratchet in 0x72 → his position = the beast's; else a splash at him and `HeroTeleport(him, 0x72, 0)`; 0x1413f5 = 1; his z += 20 | [`update`] (`HeroFields`) |
//! | | z += vz, vz −= 10·dt²; vz < 2·dt: the pitch turns to 89° (4π·dt², 8π·dt) about the point row 2 · 1 from it; falling: below the water and +0xbc clear → +0xbc = 1, `PlayClassSound(0, 0)`, the splash at that point on the water; 8 below +0x74 → his death (`0x23ee98` = L01 `0x2319b0`); wrapped → sequence 1 (`ticks(10)`) | [`update`] (`HeroCall::Death`) |
//! | `0x2d1e30` | the wake timer out: twice, a type-46 ripple at position + row 1 · `randf(−0.5, 0.5)` on the water + 0.05 (a `randf(±45°)` drawn and dropped), size `randf(0.7, 1)`, spin +2 / −2, life `ticks(15)`; timer `trunc(randf(3, 5))` | [`wake`] |
//! | `0x2d1fd8` | the splash moby of size 3 (alpha 0x70) and the 16 drops (`bomb_water::splash_and_drops`), then 16 type-46 ripples at `rand_vec(1, 1)` + the point on the water + 0.05, size `randf(0.7, 1)`, spin +2 then −2, life `trunc(scale(randf(30, 60)))` | [`splash`] |

use crate::moby_runtime::MobyId;
use crate::moby_update::classes::bomb_water::splash_and_drops;
use crate::moby_update::creature::{self as c, region, turn, DT, DT2};
use crate::moby_update::services::{HeroCall, HeroPose, World};
use crate::particles::type46;

pub const REFERENCE_LEVEL: u32 = 5;
pub const UPDATE_FN: u32 = 0x2d_1688;
pub const CLASSES: [i16; 1] = [35];
/// gp−0x5878: the depth under the water.
const DEPTH: f32 = 1.0;
const SWIM: i32 = 0x12;
const HELD: i32 = 0x72;
const PITCH: f32 = f32::from_bits(0x3fc6_d3f2);

/// Pvar offsets (module doc).
pub mod pv {
    pub const SPLINE: usize = 0x60;
    pub const KIND: usize = 0x64;
    pub const POINT: usize = 0x68;
    pub const POINT_T: usize = 0x6c;
    pub const TURN_V: usize = 0x70;
    pub const BASE: usize = 0x74;
    pub const VZ: usize = 0x78;
    pub const TIMER: usize = 0x7c;
    pub const WAKE: usize = 0x84;
    pub const SIZE: usize = 0x88;
}

fn water(w: &World) -> f32 { w.svc.water.plane.z }

fn ripple(w: &mut World, p: c::V, spin: f32) -> Option<usize> {
    let size = w.rng.randf(f32::from_bits(0x3f33_3333), 1.0);
    *w.svc.fx.part_spawns.entry(type46::TYPE).or_default() += 1;
    let wl = water(w);
    let sys = w.particles.as_deref_mut()?;
    let r = type46::spawn(sys, w.rng, size, spin, p, [0.0; 4], wl);
    if r.is_none() { w.svc.fx.part_failed += 1; }
    r
}

fn set_life(w: &mut World, i: usize, life: i32) {
    if let Some(sys) = w.particles.as_deref_mut() { crate::particles::rec::set_i16(&mut sys.pool.recs[i], 0xa, life as i16); }
}

/// `0x2d1e30` (module doc).
pub fn wake(w: &mut World, id: MobyId) {
    if c::dec_timer_pvar_i32(w, id, pv::WAKE) == 0 { return; }
    for k in 0..2 {
        w.rng.randf(f32::from_bits(0xbf49_0fd8), f32::from_bits(0x3f49_0fd8));
        let o = w.rng.randf(-0.5, 0.5);
        let (r1, pos) = (w.m(id).rows[1], w.m(id).position);
        let mut p = c::add(c::scale(r1, o), pos);
        p[2] = water(w) + 0.05;
        let spin = if k == 0 { 2.0 } else { -2.0 };
        if let Some(i) = ripple(w, p, spin) {
            let t = w.ticks(15);
            set_life(w, i, t);
        }
        let t = w.rng.randf(3.0, 5.0) as i32;
        c::set_pi32(w, id, pv::WAKE, t);
    }
}

/// `0x2d1fd8(m, at)` (module doc).
pub fn splash(w: &mut World, at: c::V) {
    splash_and_drops(w, 3.0, at);
    for k in 0..16 {
        let r = w.rng.rand_vec(1.0, 1.0);
        let mut p = [r[0] + at[0], r[1] + at[1], r[2] + at[2], at[3]];
        p[2] = water(w) + 0.05;
        let spin = if k == 0 { 2.0 } else { -2.0 };
        if let Some(i) = ripple(w, p, spin) {
            let f = w.rng.randf(30.0, 60.0);
            let life = w.svc.timing.scale(crate::ps2v::Pf::f(f)).to_f32() as i32;
            set_life(w, i, life);
        }
    }
}

fn point(w: &World, id: MobyId, k: i32) -> Option<c::V> {
    let s = usize::try_from(c::pi32(w, id, pv::SPLINE)).ok().and_then(|i| w.svc.splines.get(i))?;
    s.get(usize::try_from(k).ok()?).map(|q| q.map(f32::from_bits))
}

fn count(w: &World, id: MobyId) -> i32 { usize::try_from(c::pi32(w, id, pv::SPLINE)).ok().and_then(|i| w.svc.splines.get(i)).map_or(0, |s| s.len() as i32) }

fn blend_unless(w: &mut World, id: MobyId, seq: u8, n: i32) {
    if w.m(id).anim.seq_b != seq {
        let t = w.ticks(n);
        c::blend_to(w, id, seq, 0, t);
    }
}

fn swimming(w: &World) -> bool { w.hero.group == SWIM || w.hero.state == SWIM }

fn pitch_pivot(w: &mut World, id: MobyId) {
    let before = c::set_len3(w.m(id).rows[2], DEPTH);
    let (mut a, mut v) = (w.m(id).rotation[1], c::pf(w, id, pv::TURN_V));
    turn::turn_toward(PITCH, DT2 * 12.566_371, DT2 * 12.566_371, DT * 25.132_742, &mut a, &mut v);
    c::set_pf(w, id, pv::TURN_V, v);
    w.mm(id).rotation[1] = a;
    let r = w.m(id).rotation;
    let rows = rc_formats::moby_light::rotation_rows([r[0], r[1], r[2]]);
    let m = w.mm(id);
    for (row, q) in m.rows.iter_mut().zip(rows.iter()) { *row = q.map(f32::from_bits); }
    let after = c::set_len3(m.rows[2], DEPTH);
    m.position = c::sub(m.position, c::sub(after, before));
}

/// Level05 `0x2d1688` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pv::SIZE { w.mm(id).pvars.resize(pv::SIZE, 0); }
    let hp = super::hero_pos(w);
    match w.m(id).state {
        0 => {
            if c::pi32(w, id, pv::KIND) == 0 {
                let Some(p0) = point(w, id, 0) else {
                    w.delete_moby(id);
                    return;
                };
                let n = count(w, id);
                let k = w.rng.randi(n);
                c::set_pi32(w, id, pv::POINT, k);
                let t = w.ticks(600);
                c::set_pi32(w, id, pv::POINT_T, t);
                blend_unless(w, id, 1, 10);
                w.mm(id).position[2] = p0[2] - DEPTH;
                c::set_pf(w, id, 0x80, p0[2] - DEPTH);
                w.mm(id).state = 1;
            } else {
                let m = w.mm(id);
                m.state = 2;
                m.mode |= 1;
                m.update_dist = 0xff;
                let z = m.position[2];
                c::set_pf(w, id, pv::BASE, z);
                let t = w.ticks(30);
                c::set_pi32(w, id, pv::TIMER, t);
            }
        }
        1 => {
            let to = point(w, id, c::pi32(w, id, pv::POINT)).unwrap_or(w.m(id).position);
            let pos = w.m(id).position;
            let yaw = c::atan(to[0] - pos[0], to[1] - pos[1]);
            let (mut a, mut v) = (w.m(id).rotation[2], c::pf(w, id, pv::TURN_V));
            let pi = std::f32::consts::PI;
            turn::turn_toward(yaw, DT2 * pi, DT2 * pi, DT * pi, &mut a, &mut v);
            c::set_pf(w, id, pv::TURN_V, v);
            w.mm(id).rotation[2] = a;
            let step = c::set_len3(w.m(id).rows[0], DT * 3.0);
            let pos = c::add(pos, step);
            w.mm(id).position = pos;
            if c::dist3(pos, to) < 3.0 || c::dec_timer_pvar_i32(w, id, pv::POINT_T) != 0 {
                let n = count(w, id).max(1);
                let r = w.rng.randi(n);
                let k = (c::pi32(w, id, pv::POINT) + r) % n;
                c::set_pi32(w, id, pv::POINT, k);
                let t = w.ticks(600);
                c::set_pi32(w, id, pv::POINT_T, t);
            }
            let target = if !swimming(w) || 8.0 <= c::dist2(pos, hp) {
                let t = (water(w) - DEPTH) - 0.5;
                wake(w, id);
                t
            } else {
                (water(w) - DEPTH) - 2.0
            };
            let (mut z, mut vz) = (w.m(id).position[2], c::pf(w, id, pv::VZ));
            turn::spring(target, DT2 + DT2, DT2 + DT2, DT + DT, &mut z, &mut vz);
            w.mm(id).position[2] = z;
            c::set_pf(w, id, pv::VZ, vz);
        }
        2 => {
            c::dec_timer_pvar_i32(w, id, pv::TIMER);
            if (w.hero.water_level.to_f32() - c::pf(w, id, pv::BASE)).abs() < 0.33 && w.hero.group == SWIM {
                if c::pi32(w, id, pv::TIMER) != 0 { return; }
                let yaw = c::add_rot(w.camera_yaw, std::f32::consts::FRAC_PI_2);
                {
                    let m = w.mm(id);
                    m.rotation[1] = -PITCH;
                    m.rotation[2] = yaw;
                    let r = m.rotation;
                    let rows = rc_formats::moby_light::rotation_rows([r[0], r[1], r[2]]);
                    for (row, q) in m.rows.iter_mut().zip(rows.iter()) { *row = q.map(f32::from_bits); }
                    let back = c::set_len3(m.rows[2], DEPTH);
                    m.position = c::sub(hp, back);
                    m.position[2] -= 2.0;
                    m.mode &= !1;
                    m.cmd = 0;
                    m.state = 3;
                }
                c::set_pf(w, id, pv::VZ, DT * 10.0);
                blend_unless(w, id, 2, 5);
                let (cp, ce) = (w.camera.map(|x| f32::from_bits(x.0)), w.hero.loop_in.cam_euler);
                crate::cinematic::camera_script(w, [cp[0], cp[1], cp[2]], ce, 0, 0, false);
                return;
            }
            if w.hero.state == SWIM { return; }
            let inside = usize::try_from(c::pi32(w, id, pv::SPLINE)).ok().is_some_and(|s| region::point_in_polygon(w, s, hp));
            let t = w.ticks(if inside { 600 } else { 180 });
            c::set_pi32(w, id, pv::TIMER, t);
        }
        3 => {
            let wl = w.hero.water_level.to_f32();
            let vz = c::pf(w, id, pv::VZ);
            if wl < w.m(id).position[2] || vz < 0.0 {
                let rot = w.hero.rot.map(|x| x.to_f32());
                if w.hero.state == HELD {
                    let p = w.m(id).position;
                    w.hero_fields_mut().pose = Some(HeroPose { pos: [p[0], p[1], p[2]], yaw: rot[2], target_yaw: w.hero.target_yaw.to_f32() });
                } else {
                    splash(w, [hp[0], hp[1], wl, hp[3]]);
                    crate::cinematic::hero_teleport(w, [hp[0], hp[1], hp[2]], [rot[0], rot[1], rot[2]], HELD, false);
                }
                let f = w.hero_fields_mut();
                f.hero_hidden = Some(1);
                if let Some(p) = f.pose.as_mut() { p.pos[2] += 20.0; }
            }
            let vz = c::pf(w, id, pv::VZ);
            w.mm(id).position[2] += vz;
            let vz = vz - DT2 * 10.0;
            c::set_pf(w, id, pv::VZ, vz);
            if vz < DT + DT { pitch_pivot(w, id); }
            if vz < 0.0 {
                if w.m(id).position[2] < wl && w.m(id).cmd == 0 {
                    w.mm(id).cmd = 1;
                    w.play_sound(0, 0, id);
                    let m = w.m(id);
                    let mut at = c::add(c::set_len3(m.rows[2], DEPTH), m.position);
                    at[2] = wl;
                    splash(w, at);
                }
                if w.m(id).position[2] < c::pf(w, id, pv::BASE) - 8.0 { w.hero_fields_mut().call(HeroCall::Death); }
            }
            if w.m(id).anim.flags & 2 != 0 { blend_unless(w, id, 1, 10); }
        }
        _ => {}
    }
}
