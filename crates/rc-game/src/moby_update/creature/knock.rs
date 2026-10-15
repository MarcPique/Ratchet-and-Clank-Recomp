//! Knockback, thrown and death flights (level01 `0x271418` start, `0x271558` update, `0x26fa48` aim, `0x26faf0` lob).
//!
//! The knockback record `K` (the pvar record the creature header's +0x10 points at; critter 577 +0x120, amoeboid
//! +0x60):
//!
//! | off | meaning |
//! |---|---|
//! | 0x00 | velocity per tick (xyz, w) |
//! | 0x10 | gravity per tick (subtracted from vz) |
//! | 0x14 | horizontal speed approach step (drag) |
//! | 0x18 / 0x1c | start horizontal speed / vertical speed (also the vz cap) |
//! | 0x20 | collision radius ×1024 (int) |
//! | 0x24 | flags: 2 non-looping anim start, 4 no yaw spring, 0x10 ledge stop, 0x20 set when the health was 0 (no collision) |
//! | 0x28 | height of the collision centre above the origin |
//! | 0x2c | u16 timer, `ticks(300)` at the start |
//! | 0x2e / 0x2f | the burn marker of the hit resolver (kind 4) / spark count |
//! | 0x30 | wall restitution |
//! | 0x3c | byte, 1 at the start |
//! | 0x3e | s16 phase: 0 rising, 1 falling, 2 landed |
//! | 0x40 / 0x44 / 0x48 | yaw target (heading + π) / its spring velocity / the spring's cap |
//! | 0x4c | horizontal speed the drag approaches while airborne |
//! | 0x50 / 0x54 | animation key times to reach at the apex / on landing (−1: no timing) |
//! | 0x5c | water entry count (0 dry) |
//!
//! **Update** result bits: 1 landed this tick, 2 touched something, 0x40 the animation wrapped, 0x100 out of the world
//! box [2, 1021]³ or (with flag 0x20, landed) touched a non-crate moby. Crates the flight touches get a hit
//! (`0x26eaa8(1, crate, self, 0x10000, pos, vel)`), so a knocked creature breaks them.

use super::{add, cs, dot3, len3, scale, set_len2, set_len3, sub, V};
use crate::moby_runtime::MobyId;
use crate::moby_update::services::{pf as to_pf, pv, pvar, sphere_mobys_in, World};
use crate::ps2v::Pf;

/// The record's fields by offset.
pub mod k {
    pub const VEL: usize = 0x00;
    pub const GRAVITY: usize = 0x10;
    pub const DRAG: usize = 0x14;
    pub const SPEED: usize = 0x18;
    pub const UP: usize = 0x1c;
    pub const RADIUS: usize = 0x20;
    pub const FLAGS: usize = 0x24;
    pub const ZOFF: usize = 0x28;
    pub const TIMER: usize = 0x2c;
    pub const BURN: usize = 0x2e;
    pub const SPARKS: usize = 0x2f;
    pub const BOUNCE: usize = 0x30;
    pub const STARTED: usize = 0x3c;
    pub const PHASE: usize = 0x3e;
    pub const YAW: usize = 0x40;
    pub const YAW_VEL: usize = 0x44;
    pub const YAW_MAX: usize = 0x48;
    pub const AIR_SPEED: usize = 0x4c;
    pub const KEY_APEX: usize = 0x50;
    pub const KEY_LAND: usize = 0x54;
    pub const WATER: usize = 0x5c;
}

/// Update result bits.
pub mod res {
    pub const LANDED: u32 = 1;
    pub const TOUCHED: u32 = 2;
    pub const ANIM_WRAPPED: u32 = 0x40;
    pub const OUT: u32 = 0x100;
}

/// `0x26fa48(dir, &angle, &speed, &up)`: the heading of a hit's push vector; with the exact marker w
/// (5627.925) the speeds are also scaled (`speed ·= |dir.xy|`, `up ·= dir.z`).
pub fn aim(dir: V, speed: &mut f32, up: &mut f32) -> f32 {
    let a = super::atan(dir[0], dir[1]);
    if dir[3] == crate::hero::damage::EXACT_PUSH_W {
        *speed *= super::len2(dir);
        *up *= dir[2];
    }
    a
}

/// `0x26faf0(t, g, from, to, &time)`: the vertical speed that lands a lob from `from` on `to` travelling the xy
/// distance at `t` per tick under gravity `g`; `time` = ticks of flight (0 distance → 0).
pub fn lob_up(t: f32, g: f32, from: V, to: V, time: &mut f32) -> f32 {
    let d = super::len2(sub(to, from)) / t;
    *time = d;
    if d == 0.0 { return 0.0; }
    -((from[2] - to[2]) + g * d * d * 0.5) / d
}

/// `0x250a78(dist, height, K)` (level03; level04 `0x255150`): the ballistic arc of a flight that rises `height` and covers
/// `dist`: up = √(2·height·g), t = 2·up / g, speed = 2·dist / t, drag = 2·dist / t².
pub fn ballistic(w: &mut World, id: MobyId, dist: f32, height: f32, kr: usize) {
    let g = super::pf(w, id, kr + k::GRAVITY);
    let up = ((height + height) * g).abs().sqrt();
    super::set_pf(w, id, kr + k::UP, up);
    let t = (up + up) / g;
    super::set_pf(w, id, kr + k::SPEED, (dist + dist) / t);
    super::set_pf(w, id, kr + k::DRAG, (dist + dist) / (t * t));
}

/// `0x271418(angle, moby, K, seq, ticks, frame)`: start a flight along `angle` (module doc): the velocity from
/// `K.18` / `K.1c`, yaw target `angle + π`, timer `ticks(300)`, phase 0; a creature with no health left (the header's
/// damage record +0) flies without collision (flag 0x20, +0x94 = 0); then the flight animation (blend, or the
/// non-looping start with flag 2).
pub fn start(w: &mut World, id: MobyId, kr: usize, angle: f32, seq: u8, ticks: i32, frame: i32) {
    let (c, s) = cs(angle);
    let sp = super::pf(w, id, kr + k::SPEED);
    let up = super::pf(w, id, kr + k::UP);
    super::set_pv4(w, id, kr + k::VEL, [c * sp, s * sp, up, 1.0]);
    super::set_pf(w, id, kr + k::YAW_VEL, 0.0);
    super::set_pf(w, id, kr + k::YAW, super::add_rot(angle, std::f32::consts::PI));
    let t = super::ticks(w, 300);
    super::set_pi16(w, id, kr + k::PHASE, 0);
    super::set_pi16(w, id, kr + k::TIMER, t as i16);
    super::set_pu8(w, id, kr + k::STARTED, 1);
    if let Some(h) = super::header(w, id).damage {
        if super::pf(w, id, h) == 0.0 {
            let f = super::pi32(w, id, kr + k::FLAGS) | 0x20;
            super::set_pi32(w, id, kr + k::FLAGS, f);
            w.mm(id).has_collision = false;
        }
    }
    // Flag 2 selects `fun_002130d8(…, 1)` (the non-looping start); the port blends either way (no class uses it here).
    let _ = super::pi32(w, id, kr + k::FLAGS) & 2;
    w.anim_blend(id, seq, frame, ticks);
}

/// `FUN_00271258(moby, vel, prims)`: the burn's fire puffs. Up to 20 points in the moby's collision volume (density 14,
/// the primitives of `prims`: `crate::moby_update::classes::breakables::volume_points`, 0x26fba0), each a type-4 puff
/// flying off at `1.8·dt` in a random direction (two `rand_angle`) plus the flight's velocity and `0.9·dt` up, colours
/// 0x7000a0ff / 0xff (the gold Pyrocitor 0x13e530: 0x7000ffa0 / 0xff00), life `ticks(rand_range(20, 35))`, size 0x1e
/// growing by `rand_range(40, 60)`, additive.
pub fn burn_sparks(w: &mut World, id: MobyId, vel: super::V, prims: u32) {
    let pts = crate::moby_update::classes::breakables::volume_points(w, id, 14.0, 20, prims);
    let gold = w.hero.weapons.gold[crate::hero::pyrocitor::PYROCITOR as usize] != 0;
    let (c1, c2) = if gold { (0x7000_ffa0, 0xff00) } else { (0x7000_a0ff, 0xff) };
    for p in pts {
        let a = w.rng.rand_angle();
        let b = w.rng.rand_angle();
        let mut v = add(super::fx::polar(super::DT * 1.8, a, b), vel);
        v[2] += super::DT * 0.9;
        let n = w.rng.rand_range(20, 35);
        let life = w.ticks(n);
        let growth = w.rng.rand_range(40, 60) as i16;
        let s = crate::particles::type04::Spawn { pos: p, vel: v, c1, c2, life, base: 0x1e, growth, additive: true };
        super::fx::part04(w, &s);
    }
}

/// `0x271558(moby, K)`: one tick of the flight (module doc). Returns the result bits ([`res`]).
pub fn update(w: &mut World, id: MobyId, kr: usize) -> u32 {
    let mut out = 0u32;
    let g = |w: &World, o: usize| super::pf(w, id, kr + o);
    let mut vel = super::pv4(w, id, kr + k::VEL);
    vel[2] -= g(w, k::GRAVITY);
    let sparks = super::pu8(w, id, kr + k::SPARKS);
    if super::pu8(w, id, kr + k::BURN) == 1 && sparks != 0 {
        // The burning attack kind 4 (the resolver's type 5: the Pyrocitor's and the Tesla Claw's hits) marks the
        // flight: FUN_00271258(moby, K, the primitive mask +0x2f).
        burn_sparks(w, id, vel, sparks as u32);
    }
    let phase = super::pi16(w, id, kr + k::PHASE);
    let mut l = super::len2(vel);
    let target = if phase != 2 { g(w, k::AIR_SPEED) } else { 0.0 };
    super::turn::approach(target, g(w, k::DRAG), &mut l);
    vel = set_len2(vel, l);
    if g(w, k::UP) < vel[2] { vel[2] = g(w, k::UP); }
    let zoff = g(w, k::ZOFF);
    let mut p = super::pos(w, id);
    p[2] += zoff;
    let g0 = super::ground::ground(w, p, 0.5, 0);
    let mut gz = g0.z;
    if g0.surface == 0 {
        // Water under the flight: the entry splash once (`FUN_002ff768(3.0, p)` with p's z the water's: the splash 775,
        // crate::moby_update::classes::splash).
        if g(w, k::WATER) == 0.0 && gz <= p[2] && p[2] + vel[2] < gz {
            super::set_pf(w, id, kr + k::WATER, 1.0);
            crate::moby_update::classes::splash::spawn(w, 3.0, [p[0], p[1], gz, p[3]]);
        }
        gz = super::ground::ground(w, super::pos(w, id), 0.5, 0x20).z;
    } else if super::pi32(w, id, kr + k::FLAGS) & 0x10 != 0 || phase == 2 {
        let t = add(super::pos(w, id), vel);
        let g2 = super::ground::ground(w, t, 0.5, 0x20).z;
        if g2 < gz - 0.25 { vel = set_len2(vel, 0.0); }
    }
    let water = g(w, k::WATER);
    if water != 0.0 {
        if water < 4.0 { vel = scale(vel, 0.7); }
        if water < 7.0 {
            super::set_pf(w, id, kr + k::WATER, water + 1.0);
            let gr = g(w, k::GRAVITY) * 0.75;
            super::set_pf(w, id, kr + k::GRAVITY, gr);
        }
    }
    let m = super::pos(w, id);
    if m[0] < 2.0 || 1021.0 < m[0] || m[1] < 2.0 || 1021.0 < m[1] || m[2] < 2.0 || 1021.0 < m[2] { out = res::OUT; }
    // Animation timing towards the apex / landing key times.
    let (apex, land) = (g(w, k::KEY_APEX), g(w, k::KEY_LAND));
    if land != -1.0 && apex != -1.0 {
        let key = super::ground::key_time(w, id);
        match phase {
            1 => {
                if land <= key {
                    w.mm(id).anim.speed = 0.0;
                } else {
                    let a = g(w, k::GRAVITY) * -0.5;
                    let (n, r) = crate::moby_update::services::quad(to_pf(a), to_pf(vel[2] - a), to_pf(m[2] - gz));
                    let r = f32::from_bits(r.0);
                    let mut t = 60.0;
                    if n > 0 && 0.0 < r && r < 60.0 { t = r; }
                    if t < 0.0001 { t = 0.0001; }
                    let rate = ((land - key) / t).clamp(0.0, 2.0);
                    let mut sp = w.m(id).anim.speed;
                    super::turn::approach(rate, super::SPEED * 0.1, &mut sp);
                    w.mm(id).anim.speed = sp;
                }
            }
            0 => {
                let r = (vel[2] / g(w, k::GRAVITY)).abs();
                if 2.0 < r {
                    let rate = (apex - key) / r;
                    if 0.0 < rate {
                        let mut sp = w.m(id).anim.speed;
                        super::turn::approach(rate, super::SPEED * 0.1, &mut sp);
                        w.mm(id).anim.speed = sp;
                    }
                }
            }
            2 => w.mm(id).anim.speed = 1.0,
            _ => {}
        }
    }
    if w.m(id).anim.flags & 2 != 0 { out |= res::ANIM_WRAPPED; }
    if super::pi32(w, id, kr + k::FLAGS) & 4 == 0 {
        let (t, max) = (g(w, k::YAW), g(w, k::YAW_MAX));
        super::turn::spring_turn2_pvar(w, id, t, 0.02, 0.3, max, kr + k::YAW_VEL);
    }
    let mut p_old = super::pos(w, id);
    p_old[2] += zoff;
    let np = add(super::pos(w, id), vel);
    super::set_pos(w, id, np);
    let mut p_new = np;
    p_new[2] += zoff;
    // Mobys the flight touches next tick: crates get a hit, other mobys end a dead creature's landed flight.
    let q = add(p_new, vel);
    let radius = super::pi32(w, id, kr + k::RADIUS) as f32;
    let touched = sphere_mobys_in(w.table, w.svc, w.classes, to_pf(radius * 0.001_074_218_8), pv(q), 0x10, Some(id), None);
    for t in touched {
        if (500..541).contains(&w.m(t).o_class) {
            super::attack::hit_moby(w, t, id, 1.0, 0x10000, np, vel);
        } else if super::pi32(w, id, kr + k::FLAGS) & 0x20 != 0 && super::pi16(w, id, kr + k::PHASE) == 2 {
            out |= res::OUT;
        }
    }
    if radius * (1.0 / 1024.0) < len3(vel) {
        if let Some(o) = w.coll_line(pv(p_old), pv(p_new), 0x24, Some(id)) {
            // (On a miss the game copies the collision output's stale point: not modelled, the centre stays.)
            p_new = [o.point[0], o.point[1], o.point[2], p_new[3]];
        }
    }
    let mut ret = out;
    if let Some(o) = w.coll_sphere(pv(p_new), to_pf(radius * (1.0 / 1024.0)), 0x24, Some(id)) {
        ret = out | res::TOUCHED;
        if let Some(c) = o.pushed_centre {
            super::set_pos(w, id, [c[0], c[1], c[2] - zoff, np[3]]);
        }
        let n = set_len3([o.normal[0], o.normal[1], o.normal[2], 0.0], 1.0);
        if 0.707 <= n[2] {
            let d = dot3(vel, n);
            if d < 0.0 {
                if o.moby.is_none() || o.kind > 0 {
                    ret = out | 3;
                    super::set_pi16(w, id, kr + k::PHASE, 2);
                }
                vel = sub(vel, set_len3(n, d));
                if super::pi16(w, id, kr + k::PHASE) == 2 {
                    let mut l = len3(vel);
                    super::turn::approach(0.0, g(w, k::GRAVITY), &mut l);
                    vel = set_len3(vel, l);
                }
            }
        } else {
            let d = dot3(vel, n);
            if d < 0.0 {
                vel = add(vel, set_len3(n, d * -(g(w, k::BOUNCE) + 1.0)));
            }
        }
    }
    if super::pi16(w, id, kr + k::PHASE) == 0 && vel[2] < 0.0 { super::set_pi16(w, id, kr + k::PHASE, 1); }
    super::set_pv4(w, id, kr + k::VEL, vel);
    let _ = (pvar::u8, Pf::ZERO);
    ret
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moby_runtime::{ClassInfo, Moby, MobyTable};
    use crate::moby_update::{ClassTable, Services};

    /// A knocked creature crossing a water face (surface 0) makes the splash 775 at the water's height (`FUN_002ff768(3,
    /// p)`), once (the flight's water marker +0x5c).
    #[test]
    fn knocked_into_water_splashes_once() {
        let mut mesh = crate::hero::testkit::floor(10.0, 5, 7, 5, 7);
        for c in &mut mesh.cells { for f in &mut c.faces { f.surface = 0; } }
        let mut classes = ClassTable::default();
        classes.classes.insert(775, (ClassInfo { slot: 1, scale: 1.0, ..Default::default() }, None));
        classes.classes.insert(577, (ClassInfo { slot: 2, scale: 1.0, ..Default::default() }, None));
        let mut m = Moby::init_instance(0, 577, Some(&classes.classes[&577].0));
        m.position = [24.0, 24.0, 10.2, 0.0];
        m.pvars = vec![0; 0x80];
        let set = |p: &mut Vec<u8>, o: usize, x: f32| p[o..o + 4].copy_from_slice(&x.to_le_bytes());
        set(&mut m.pvars, k::VEL, 0.1);
        set(&mut m.pvars, k::VEL + 8, -0.3);
        set(&mut m.pvars, k::AIR_SPEED, 0.1);
        set(&mut m.pvars, k::UP, 1.0);
        let mut table = MobyTable::new(vec![m], 8);
        let hero = crate::hero::Hero::new();
        let mut rng = crate::rng::Rng::new();
        let mut svc = Services::new();
        let mut w = World::new(&mut table, &hero, &mut rng, &classes, &mut svc, 1);
        w.coll = Some(&mesh);
        update(&mut w, 0, 0);
        let splashes: Vec<[f32; 4]> = w.table.mobys.iter().filter(|x| x.o_class == 775).map(|x| x.position).collect();
        assert_eq!(splashes.len(), 1, "one splash");
        assert!((splashes[0][2] - 10.0).abs() < 1e-3 && splashes[0][0] == 24.0, "at the water: {splashes:?}");
        assert_eq!(super::super::pf(&w, 0, k::WATER), 2.0, "marked (1), then counted in the same update");
        update(&mut w, 0, 0);
        assert_eq!(w.table.mobys.iter().filter(|x| x.o_class == 775).count(), 1, "once");
    }
}
