//! U160 (census 2026-10-02): class 86, Eudora's flock spawners (level04 0x29ecf8, the only copy: 3 placed). Read from
//! the level04 decomp and the disassembly of the engine's boids (`0x1f30e0` steering, `0x1f3a80` the move; the
//! decompiler lost their arguments). Native `f32`; the `rand` draws in the game's order.
//!
//! A spawner makes its members (class 85, five each; their own update does nothing) around itself and moves along a
//! path (or around a circle), staying ahead of them; each tick it steers every member: drawn to the spawner, kept apart
//! from the other members and from flyers 494, pushed off the world, with a small wander, at a speed that follows its
//! wing beat (anim speed).
//!
//! **Spawner pvars** (0xb0): +0x00 home; +0x10.. the members (port: index + 1); +0x90 the member count; +0x94 spawned;
//! +0x98 the circle's angle; +0x9c the circle's radius (also the speed scale); +0xa0 the path (−1: the circle); +0xa4
//! its node; +0xa8 the class-85 mobys counted.
//!
//! **Member pvars** (0xa0): +0x00 this tick's steering; +0x10 velocity; +0x30 the wander, +0x40 its range, +0x44 s16
//! its timer; +0x48 the vertical factor (0: flat flight); +0x4c the neighbour range; +0x50 the leader; +0x54 the point
//! drawn to (the spawner's position); +0x5c s16 a path (−1); +0x5e s16 / +0x60.. the classes kept apart from, +0x70..
//! their weights, +0x90.. their distances.
//!
//! **Steering parameters** (`0x166130`, shared): [0] a pull along +0x20 (0), [1] the pull to the point (15·2.073 with
//! fifteen members), [2] (0), [3] the push off the world (2), [4] the contact distance (1), [6] the steering length and
//! [7] the speed (per member, from +0x9c and its anim speed).
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x29ecf8` | once per level: the parameters; not spawned: home = position, angle 0; the class-85 count (live, the run list) when 0; path −1: around the circle at 0.0035 a tick; else `0x29eb20(3 − the nearest member (xy), …)` along the path; spawned: per member the speeds, `0x1f30e0`, `0x1f3a80(0, …)`; not spawned: spawn | [`update`] |
//! | spawn | per member `CreateMoby(85)`: update / draw distance 0x7f, scale ·`randf(0.7, 1.3)`, yaw `rand_angle`, anim speed `randf(0.6, 1.2)`, velocity 0, wander timer 0, range 10, at the spawner + `randf(0.6, 0.9)` along `(cos a sin b, sin a sin b, cos b)`, leader 0, point the spawner's position, weights 1.694, distances 1 / 5, vertical 1.0568, path −1, wander range 0.003, classes 85 / 494 | [`spawn`] |
//! | `0x29eb20` | the path step: the point's projection on its segment plus the step, wrapping to the next segment | [`path_step`] |
//! | `0x1f30e0` | steering (reachable for these members: no pull [0], no leader, no path, vertical): the pull to the point by \|d\|³·[1]; each live member and Ratchet (classes 85 / 494 only) within the range: nearer than [4] → the nearest (stop); else away at weight / (d / distance)²; `coll_sphere([4], pos, 4)`: off the hit by [3] (z + [3]); the sum at [6], z / the vertical factor; the wander (re-drawn every `randf(45, 90)` ticks); the nearest: straight away from it at [6] | [`steer`] |
//! | `0x1f3a80` | velocity += the steering, at [7], z / the vertical factor; the move; up to six `coll_sphere([4]/4, pos, 4)` push-outs; yaw = heading + π | [`fly`] |
//!
//! **Not the game's, noted [L]:** the pull to the point is applied only within the distance from the member to an
//! uninitialised stack vector (the game: leftover stack; the port: always). **Not ported (unreachable for class 86's
//! members):** the boids' pull [0], the leader branch, the path branch (`0x1f3038`, `0x24c7d8`) and the flat-flight
//! branch (`+0x48` 0: the ground line and slope pushes).

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c};
use crate::moby_update::services::{pf, pv, World};
use std::f32::consts::PI;

pub const REFERENCE_LEVEL: u32 = 4;
pub const UPDATE_FN: u32 = 0x29_ecf8;
pub const CLASSES: [i16; 1] = [86];
/// The members.
pub const MEMBER: i16 = 85;

const PARAMS: u32 = 0x16_6130;
const ONCE: u32 = 0x16_13c0;
const SIZE: usize = 0xb0;
const MEMBER_SIZE: usize = 0xa0;

mod sp {
    pub const HOME: usize = 0x00;
    pub const MEMBERS: usize = 0x10;
    pub const COUNT: usize = 0x90;
    pub const SPAWNED: usize = 0x94;
    pub const ANGLE: usize = 0x98;
    pub const RADIUS: usize = 0x9c;
    pub const PATH: usize = 0xa0;
    pub const NODE: usize = 0xa4;
    pub const FOUND: usize = 0xa8;
}

mod mp {
    pub const STEER: usize = 0x00;
    pub const VEL: usize = 0x10;
    pub const WANDER: usize = 0x30;
    pub const WANDER_R: usize = 0x40;
    pub const WANDER_T: usize = 0x44;
    pub const VERTICAL: usize = 0x48;
    pub const RANGE: usize = 0x4c;
    pub const POINT: usize = 0x54;
    pub const CLASS_N: usize = 0x5e;
    pub const CLASS: usize = 0x60;
    pub const WEIGHT: usize = 0x70;
    pub const DIST: usize = 0x90;
}

fn param(w: &World, k: usize) -> f32 { f32::from_bits(w.svc.units.word(PARAMS + 4 * k as u32)) }
fn set_param(w: &mut World, k: usize, x: f32) { w.svc.units.set_word(PARAMS + 4 * k as u32, x.to_bits()); }

fn member(w: &World, id: MobyId, i: usize) -> Option<MobyId> {
    usize::try_from(c::pi32(w, id, sp::MEMBERS + 4 * i) - 1).ok().filter(|&m| m < w.table.mobys.len())
}
fn count(w: &World, id: MobyId) -> usize { c::pi32(w, id, sp::COUNT).clamp(0, 32) as usize }

/// The spawn (module table).
fn spawn(w: &mut World, id: MobyId) {
    c::set_pi32(w, id, sp::SPAWNED, 1);
    c::set_pi32(w, id, sp::FOUND, 0);
    let me = c::pos(w, id);
    for i in 0..count(w, id) {
        c::set_pi32(w, id, sp::MEMBERS + 4 * i, 0);
        let Some(m) = w.create_moby(MEMBER) else { continue };
        c::set_pi32(w, id, sp::MEMBERS + 4 * i, m as i32 + 1);
        {
            let mo = w.mm(m);
            if mo.pvars.len() < MEMBER_SIZE { mo.pvars.resize(MEMBER_SIZE, 0); }
            mo.update_dist = 0x7f;
            mo.draw_dist = 0x7f;
        }
        let s = w.rng.randf(0.7, 1.3);
        w.mm(m).scale *= s;
        let y = w.rng.rand_angle();
        w.mm(m).rotation[2] = y;
        let a = w.rng.randf(0.6, 1.2);
        w.mm(m).anim.speed = a;
        c::set_pv4(w, m, mp::VEL, [0.0; 4]);
        c::set_pi16(w, m, mp::WANDER_T, 0);
        c::set_pf(w, m, mp::RANGE, 10.0);
        let a = w.rng.rand_angle();
        let b = w.rng.rand_angle();
        let r = w.rng.randf(0.6, 0.9);
        let p = [me[0] + a.cos() * b.sin() * r, me[1] + a.sin() * b.sin() * r, me[2] + b.cos() * r, me[3]];
        c::set_pos(w, m, p);
        c::set_pi32(w, m, 0x50, 0);
        c::set_pi32(w, m, mp::POINT, id as i32 + 1);
        c::set_pi32(w, m, 0x58, 0);
        c::set_pf(w, m, mp::WEIGHT, f32::from_bits(0x3fd8_d4fe));
        c::set_pf(w, m, mp::WEIGHT + 4, f32::from_bits(0x3fd8_d4fe));
        c::set_pf(w, m, mp::DIST + 4, 5.0);
        c::set_pf(w, m, mp::VERTICAL, f32::from_bits(0x3f87_2b02));
        c::set_pi16(w, m, 0x5c, -1);
        c::set_pf(w, m, mp::WANDER_R, f32::from_bits(0x3b44_9ba6));
        c::set_pi16(w, m, mp::CLASS_N, 2);
        c::set_pi16(w, m, mp::CLASS, MEMBER);
        c::set_pf(w, m, mp::DIST, 1.0);
        c::set_pi16(w, m, mp::CLASS + 2, 0x1ee);
        w.build_matrix(m);
    }
}

/// `0x29eb20(step, m, path, node, dir)` (module table): returns the new node.
fn path_step(w: &mut World, id: MobyId, path: usize, node: i32, step: f32) -> i32 {
    let pts: Vec<c::V> = w.svc.splines[path].iter().map(|q| q.map(f32::from_bits)).collect();
    let n = pts.len() as i32;
    if n < 2 { return node; }
    let next = |i: i32| if i == n - 1 { 0 } else if i == 0 { 1 } else { i + 1 };
    let at = |i: i32| pts[i.clamp(0, n - 1) as usize];
    let mut node = node;
    let nx = next(node);
    let seg = c::sub(at(nx), at(node));
    let len = c::len3(seg);
    let mut dir = c::set_len3(seg, 1.0);
    let mut t = c::dot3(c::sub(c::pos(w, id), at(node)), dir) + step;
    if len < t {
        let n2 = next(nx);
        dir = c::set_len3(c::sub(at(n2), at(nx)), 1.0);
        t -= len;
        node = nx;
    }
    let p = c::add(at(node), c::set_len3(dir, t));
    c::set_pos(w, id, p);
    node
}

/// `0x1f30e0` for member `m` of spawner `id` (module table).
fn steer(w: &mut World, id: MobyId, m: MobyId) {
    let me = c::pos(w, m);
    let mut f = [0.0f32; 4];
    let point = usize::try_from(c::pi32(w, m, mp::POINT) - 1).ok().filter(|&p| p < w.table.mobys.len()).map(|p| w.m(p).position);
    if let Some(q) = point {
        let v = [q[0] - me[0], q[1] - me[1], q[2] - me[2], 0.0];
        let l = c::len3(v);
        f = c::add(f, c::set_len3(v, l * (l * l * param(w, 1))));
    }
    let n = count(w, id);
    let range = c::pf(w, m, mp::RANGE);
    let classes: Vec<i16> = (0..c::pi16(w, m, mp::CLASS_N).clamp(0, 8) as usize).map(|k| c::pi16(w, m, mp::CLASS + 2 * k)).collect();
    let mut nearest = None;
    for i in 0..=n {
        let o = if i < n { member(w, id, i) } else { w.hero_moby };
        let Some(o) = o else { continue };
        if (w.m(o).state as i8) < 0 || o == m { continue; }
        let q = w.m(o).position;
        let d = c::dist3(me, q);
        if range < d { continue; }
        let Some(k) = classes.iter().position(|&cl| cl == w.m(o).o_class) else { continue };
        if d < param(w, 4) {
            nearest = Some(o);
            break;
        }
        let s = d / c::pf(w, m, mp::DIST + 4 * k);
        let s = if s == 0.0 { 1000.0 } else { c::pf(w, m, mp::WEIGHT + 4 * k) / (s * s) };
        f = c::add(f, c::set_len3(c::sub(me, q), s));
    }
    if let Some(o) = w.coll_sphere(pv(me), pf(param(w, 4)), 4, Some(m)) {
        if let Some(pc) = o.pushed_centre {
            let mut v = [pc[0] - me[0], pc[1] - me[1], pc[2] - me[2], 0.0];
            v[2] += param(w, 3);
            f = c::add(f, c::set_len3(v, param(w, 3)));
        }
    }
    let vf = c::pf(w, m, mp::VERTICAL);
    f = c::set_len3(f, param(w, 6));
    f[2] /= vf;
    if c::dec_timer_pvar_s16(w, m, mp::WANDER_T) != 0 {
        let t = w.rng.randf(45.0, 90.0) as i32;
        c::set_pi16(w, m, mp::WANDER_T, t as i16);
        let r = c::pf(w, m, mp::WANDER_R);
        let x = w.rng.randf(0.0, r) - r * 0.5;
        c::set_pf(w, m, mp::WANDER, x);
        let y = w.rng.randf(0.0, r) - r * 0.5;
        c::set_pf(w, m, mp::WANDER + 4, y);
        let z = if vf == 0.0 { 0.0 } else { w.rng.randf(0.0, r) - r * 0.5 };
        c::set_pf(w, m, mp::WANDER + 8, z);
    }
    f = c::add(f, c::pv4(w, m, mp::WANDER));
    if let Some(o) = nearest {
        f = c::set_len3(c::sub(me, w.m(o).position), param(w, 6));
    }
    c::set_pv4(w, m, mp::STEER, f);
}

/// `0x1f3a80(0, m, …)` (module table).
fn fly(w: &mut World, m: MobyId) {
    let mut v = c::add(c::pv4(w, m, mp::VEL), c::pv4(w, m, mp::STEER));
    v = c::set_len3(v, param(w, 7));
    v[2] /= c::pf(w, m, mp::VERTICAL);
    c::set_pv4(w, m, mp::VEL, v);
    let p = c::add(c::pos(w, m), v);
    c::set_pos(w, m, p);
    for _ in 0..6 {
        let at = c::pos(w, m);
        let Some(o) = w.coll_sphere(pv(at), pf(param(w, 4) * 0.25), 4, Some(m)) else { break };
        let Some(pc) = o.pushed_centre else { break };
        let mo = w.mm(m);
        mo.position[0] = pc[0];
        mo.position[1] = pc[1];
        mo.position[2] = pc[2];
    }
    w.mm(m).rotation[2] = c::add_rot(c::atan(v[0], v[1]), PI);
}

/// Level04 0x29ecf8 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < SIZE { return; }
    if w.svc.units.word(ONCE) == 0 {
        w.svc.units.set_word(ONCE, 1);
        let found = c::pi32(w, id, sp::FOUND) as f32;
        for (k, x) in [0.0, found * 2.073, 0.0, 2.0, 1.0, 1.0].iter().enumerate() { set_param(w, k, *x); }
    }
    let spawned = c::pi32(w, id, sp::SPAWNED) != 0;
    if !spawned {
        let p = c::pos(w, id);
        c::set_pv4(w, id, sp::HOME, p);
        c::set_pf(w, id, sp::ANGLE, 0.0);
    } else if c::pi32(w, id, sp::FOUND) == 0 {
        let n = super::hints::run_list(w).into_iter().filter(|&o| (w.m(o).state as i8) >= 0 && w.m(o).o_class == MEMBER).count();
        c::set_pi32(w, id, sp::FOUND, n as i32);
        set_param(w, 1, n as f32 * 2.073);
    }
    let path = c::pi32(w, id, sp::PATH);
    if path == -1 {
        let a = c::add_rot(c::pf(w, id, sp::ANGLE), 0.003_490_658_7);
        c::set_pf(w, id, sp::ANGLE, a);
        let (home, r) = (c::pv4(w, id, sp::HOME), c::pf(w, id, sp::RADIUS));
        let m = w.mm(id);
        m.position[0] = home[0] + r * a.cos();
        m.position[1] = home[1] + r * a.sin();
    } else if spawned {
        let me = c::pos(w, id);
        let mut near = 10000.0f32;
        for i in 0..count(w, id) {
            if let Some(m) = member(w, id, i) { near = near.min(c::dist2(me, w.m(m).position)); }
        }
        let step = if near < 3.0 { 3.0 - near } else { 0.0 };
        if let Some(p) = usize::try_from(path).ok().filter(|&p| p < w.svc.splines.len()) {
            let node = c::pi32(w, id, sp::NODE);
            let node = path_step(w, id, p, node, step);
            c::set_pi32(w, id, sp::NODE, node);
        }
    }
    if !spawned { return spawn(w, id); }
    let r = c::pf(w, id, sp::RADIUS);
    for i in 0..count(w, id) {
        let Some(m) = member(w, id, i) else { continue };
        if w.m(m).pvars.len() < MEMBER_SIZE { continue; }
        let k = (w.m(m).anim.speed + 1.0) * 0.5;
        let base = (r * 7.753) / 1800.0;
        set_param(w, 7, base * k);
        set_param(w, 6, base * k * 0.05 * k);
        steer(w, id, m);
        fly(w, m);
    }
}
