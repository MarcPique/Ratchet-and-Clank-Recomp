//! **Rilgar's rising rocks, class 844** (level05 `0x30e7f8` with its bubbles `0x30f218` and sway `0x30f408`, 3 placed;
//! census U200; the name is descriptive [L]). Rocks that sit in the water, swaying (more under Ratchet's weight). When
//! he comes within 24 (xy, outside scenes) a 2 s rumble starts: bubbles from its last second and dust in its last
//! 0.4 s; then it shoots up (the hum, hurting whatever is on it every fourth tick: damage 2) to its height (pvar +0x80
//! above home), floats there bobbing about +0x84 for +0x88 ticks, and falls back with a sound. Standing under it, 2
//! below it within 1.4 (xy), Ratchet is nudged away. It carries what stands on it and keeps a hidden helper moby 0x388
//! at its position. Read from the level05 decomp and its words gp−0x4e54..−0x4e00. Native `f32`.
//!
//! **Pvars** (0xb0): +0x08 the platform block's offset (0x20), +0x60 the velocity (+0x68 its z), +0x70 home (+0x78 its
//! z), +0x7c the float target over home, +0x80 the rise, +0x84 the float height, +0x88 the float time, +0x8c the timer,
//! +0x90 / +0x94 the sway's turn velocities, +0x98 / +0x9c its phases, +0xa0 / +0xa4 their rates, +0xa8 the hum's
//! voice, +0xac the helper (moby + 1 here).
//!
//! | address | what | port |
//! |---|---|---|
//! | every tick | states 0 / 1: the hum alive → released (owner-guarded), voice −1; states 2+: no hum → `PlayClassSound(0, 4)` | [`update`] |
//! | state 0 | → 1, timer `ticks(120)`, voice 0xff, home = position; the phases `random_angle_radians`, the rates `randf_sym(70, 120)`·deg·dt; +0x80 at least +0x84 + 1 | [`update`] ([`rest`]) |
//! | state 1 | the sway (0, 8); outside scenes, within 24 and the timer out → 2, velocity 0, vz = √(2·rise·10·dt²); else, below `ticks(30)`/2: bubbles twice (home z .. + 3); below `ticks(30)`·0.2: twenty dust puffs (type 2, def 0x30, colours 0x40ff7874 / 0x10ff7874, phases `scale(randf(10, 20))`, `scale(randf(10, 20))`, `scale(randf(10, 40))`): at 0.75 out in a random direction, z `randf(z − 0.25, z + 0.5)`, velocity out at `randf(1.5, 4)·dt` and up `randf(0, 3)·dt`, size `randf(0.5, 1)` | [`update`] |
//! | state 2 | every 4th tick `0x283be8(1, 2, 1, m, position, 0x10000, 0, 1, 0)` (L01 `0x26e830`); bubbles (home z .. z + vz·15); the sway (0.3, 1); position += velocity, vz −= 10·dt²; vz ≤ 0 → 3, target = +0x84 − 0.5, timer = `scale(+0x88)` | [`update`] (`attack::sphere_hit`) |
//! | state 3 | bubbles (home z .. z); the sway (0.3, 1); `0x285be8(home z + target, 5·dt², 5·dt², 20·dt, &z, &vz)`; vz = 0 → target = +0x84 ∓ 0.5 (the other side); the timer out → 4 | [`update`] (`turn::spring`) |
//! | state 4 | bubbles (home z .. z + vz·30); the sway (0.3, 1) above home + 1, else (0, 10); position += velocity, vz −= 10·dt²; below home → `PlayClassSound(1, 0)`, the rest (state 1) at home | [`update`] |
//! | every tick | Ratchet below z − 2 within 1.4 → `0x283e60(0, Ratchet, m, 1, Ratchet + 0.75 z, 0.2 away)` (L01 `0x26eaa8`); `CarryRiders(+0x20, position − old, old Euler, Euler)`; the helper 0x388 created once (hidden, no update), else moved here | [`update`] (`attack::hit_moby`) |
//! | `0x30f218(z0, z1, m)` | (z1 − z0)/0.8 type-48 bubbles: at `randf(0, 0.5)` out in a random direction, z = min(`randf(z0, z1)`, `randf((z0 + z1)/2, z1)`) − 2.25 (the drift over its life), size `randf(0.25, 1.25)`, rising 10·dt, life `ticks(30)`, spin `trunc(randf_sym(0, 5))`, colour 0x40ff7874 | [`bubbles`] |
//! | `0x30f408(a, k, m)` | Ratchet on it and grounded: lean (a·d·row 0, −a·d·row 1) from him (d = Ratchet − position); the phases += rates; roll / pitch turn toward the lean + a·0.5·sin(phase) (`0x286078`: 360°·k / 120°·k ·dt², 360°·dt) | [`sway`] |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, attack, turn, DT, DT2};
use crate::moby_update::services::World;
use crate::moby_update::triggers;
use crate::particles::type48;

pub const REFERENCE_LEVEL: u32 = 5;
pub const UPDATE_FN: u32 = 0x30_e7f8;
pub const CLASSES: [i16; 1] = [844];
pub const HELPER: i16 = 0x388;
/// gp−0x4e54 (gravity ×dt²), −0x4e50 (the float spring ×dt²), −0x4e4c (bubble spacing), −0x4e48 (30: the rumble /
/// bubble ticks), −0x4e44 / −0x4e40 (bubble sizes), −0x4e3c (5: bubble spin), −0x4e38 (bubble rise ×dt), −0x4e34
/// (bubble colour), −0x4e30 (0.5: bubble spread), −0x4e2c (0.3: the sway), −0x4e28 (0.5), −0x4e24 / −0x4e20 / −0x4e1c
/// (360 / 120 / 360: the sway's turn), −0x4e18 (3: the bubbles' height), −0x4e14 / −0x4e10 (dust speeds ×dt),
/// −0x4e0c / −0x4e08 (dust colours), −0x4e04 (3: dust rise ×dt), −0x4e00 (1.4: the push reach).
const G: f32 = 10.0;
const FLOAT_K: f32 = 5.0;
const BUBBLE_STEP: f32 = f32::from_bits(0x3f4c_cccd);
const RUMBLE: i32 = 30;
const BUBBLE_COLOUR: u32 = 0x40ff_7874;
const SWAY: f32 = f32::from_bits(0x3e99_999a);
const DUST_COLOURS: (u32, u32) = (0x40ff_7874, 0x10ff_7874);
const PUSH_REACH: f32 = f32::from_bits(0x3fb3_3333);
const DEG: f32 = 0.017_453_292;

/// Pvar offsets (module doc).
pub mod pv {
    pub const BLOCK: usize = 0x20;
    pub const VEL: usize = 0x60;
    pub const VZ: usize = 0x68;
    pub const HOME: usize = 0x70;
    pub const HOME_Z: usize = 0x78;
    pub const TARGET: usize = 0x7c;
    pub const RISE: usize = 0x80;
    pub const FLOAT: usize = 0x84;
    pub const HOLD: usize = 0x88;
    pub const TIMER: usize = 0x8c;
    pub const TURN_V: usize = 0x90;
    pub const PHASE: usize = 0x98;
    pub const RATE: usize = 0xa0;
    pub const VOICE: usize = 0xa8;
    pub const HELPER: usize = 0xac;
    pub const SIZE: usize = 0xb0;
}

/// `0x30f218(z0, z1, m)` (module doc).
pub fn bubbles(w: &mut World, id: MobyId, z0: f32, z1: f32) {
    let vz = 10.0 * DT;
    let drift = vz * 0.75 * RUMBLE as f32;
    let n = (z1 - z0) / BUBBLE_STEP;
    let mut i = 0;
    while (i as f32) < n {
        let r = w.rng.randf(0.0, 0.5);
        let a = w.rng.rand_angle();
        let pos = w.m(id).position;
        let mut p = [a.cos() * r + pos[0], a.sin() * r + pos[1], pos[2], pos[3]];
        p[2] = w.rng.randf(z0, z1) - drift;
        let z2 = w.rng.randf((z0 + z1) * 0.5, z1);
        if z2 - drift < p[2] { p[2] = z2 - drift; }
        i += 1;
        let size = w.rng.randf(0.25, 1.25);
        let life = w.ticks(RUMBLE);
        let spin = w.rng.randf_sym(0.0, 5.0) as i32 as i16;
        *w.svc.fx.part_spawns.entry(type48::TYPE48).or_default() += 1;
        if let Some(sys) = w.particles.as_deref_mut() {
            if type48::spawn48(sys, w.rng, size, p, [0.0, 0.0, vz, 0.0], life, spin, BUBBLE_COLOUR).is_none() { w.svc.fx.part_failed += 1; }
        }
    }
}

/// `0x30f408(a, k, m)` (module doc).
pub fn sway(w: &mut World, id: MobyId, a: f32, k: f32) {
    let (mut lx, mut ly) = (0.0, 0.0);
    if w.hero.ground_moby == Some(id) && w.hero.air_ticks == 0 {
        let (h, m) = (super::hero_pos(w), w.m(id));
        let d = [h[0] - m.position[0], h[1] - m.position[1], h[2] - m.position[2], 0.0];
        lx = a * c::dot3(d, m.rows[0]);
        ly = -a * c::dot3(d, m.rows[1]);
    }
    let p0 = c::add_rot(c::pf(w, id, pv::PHASE), c::pf(w, id, pv::RATE));
    let p1 = c::add_rot(c::pf(w, id, pv::PHASE + 4), c::pf(w, id, pv::RATE + 4));
    c::set_pf(w, id, pv::PHASE, p0);
    c::set_pf(w, id, pv::PHASE + 4, p1);
    let tx = c::add_rot(ly, p0.sin() * a * 0.5);
    let ty = c::add_rot(lx, p1.sin() * a * 0.5);
    let (acc, dec, vmax) = (360.0 * k * DEG * DT2, 120.0 * k * DEG * DT2, 360.0 * DEG * DT);
    for (axis, target) in [(0usize, tx), (1, ty)] {
        let (mut ang, mut v) = (w.m(id).rotation[axis], c::pf(w, id, pv::TURN_V + 4 * axis));
        turn::turn_toward(target, acc, dec, vmax, &mut ang, &mut v);
        w.mm(id).rotation[axis] = ang;
        c::set_pf(w, id, pv::TURN_V + 4 * axis, v);
    }
}

/// State 0 / the end of state 4: at rest at home (module doc).
fn rest(w: &mut World, id: MobyId) {
    w.mm(id).state = 1;
    let t = w.ticks(120);
    c::set_pi32(w, id, pv::TIMER, t);
    for k in 0..2 {
        let a = w.rng.rand_angle();
        c::set_pf(w, id, pv::PHASE + 4 * k, a);
    }
    for k in 0..2 {
        let r = w.rng.randf_sym(70.0, 120.0);
        c::set_pf(w, id, pv::RATE + 4 * k, r * DEG * DT);
    }
}

fn dust(w: &mut World, id: MobyId) {
    let t0 = w.svc.timing.scale(crate::ps2v::Pf::f(w.rng.randf(10.0, 20.0))).to_f32() as i32;
    let t1 = w.svc.timing.scale(crate::ps2v::Pf::f(w.rng.randf(10.0, 20.0))).to_f32() as i32;
    let t2 = w.svc.timing.scale(crate::ps2v::Pf::f(w.rng.randf(10.0, 40.0))).to_f32() as i32;
    for _ in 0..20 {
        let a = w.rng.rand_angle();
        let off = [a.cos() * 0.75, a.sin() * 0.75, 0.0, 0.0];
        let sp = w.rng.randf(1.5, 4.0);
        let mut v1 = c::set_len3(off, sp * DT);
        v1[2] = w.rng.randf(0.0, 3.0) * DT;
        let pos = w.m(id).position;
        let mut p = c::add(pos, off);
        p[2] = w.rng.randf(pos[2] - 0.25, pos[2] + 0.5);
        let size = w.rng.randf(0.5, 1.0);
        v1[3] = size;
        let v2 = [0.0, 0.0, 0.0, size];
        let s = crate::particles::type02::Spawn { pos: p, v1, v2, c1: DUST_COLOURS.0, c2: DUST_COLOURS.1, t: [t0, t1, t2], def: 0x30 };
        crate::moby_update::creature::fx::part02(w, &s);
    }
}

/// Level05 `0x30e7f8` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pv::SIZE { w.mm(id).pvars.resize(pv::SIZE, 0); }
    let (old_pos, old_rot) = (w.m(id).position, w.m(id).rotation);
    let v = c::pi32(w, id, pv::VOICE);
    if w.m(id).state < 2 {
        if w.sound_alive(v, id) {
            w.release_sound(v, id);
            c::set_pi32(w, id, pv::VOICE, -1);
        }
    } else if !w.sound_alive(v, id) {
        let s = w.play_sound(0, 4, id);
        c::set_pi32(w, id, pv::VOICE, s);
    }
    let home_z = c::pf(w, id, pv::HOME_Z);
    match w.m(id).state {
        0 => {
            let pos = w.m(id).position;
            c::set_pv4(w, id, pv::HOME, pos);
            rest(w, id);
            c::set_pi32(w, id, pv::VOICE, 0xff);
            let need = c::pf(w, id, pv::FLOAT) + 1.0;
            if c::pf(w, id, pv::RISE) <= need { c::set_pf(w, id, pv::RISE, need); }
        }
        1 => {
            sway(w, id, 0.0, 8.0);
            let near = w.svc.game_mode == 0 && c::dist2(w.m(id).position, super::hero_pos(w)) < 24.0;
            if near && c::dec_timer_pvar_i32(w, id, pv::TIMER) != 0 {
                w.mm(id).state = 2;
                c::set_pv4(w, id, pv::VEL, [0.0; 4]);
                let rise = c::pf(w, id, pv::RISE);
                c::set_pf(w, id, pv::VZ, ((rise + rise) * G * DT2).sqrt());
            } else {
                let timer = c::pi32(w, id, pv::TIMER) as f32;
                let r = w.ticks(RUMBLE) as f32;
                if timer < r * 0.5 {
                    bubbles(w, id, home_z, 3.0 + home_z);
                    bubbles(w, id, home_z, 3.0 + home_z);
                    if timer < r * 0.2 { dust(w, id); }
                }
            }
        }
        2 => {
            if w.counter & 3 == 0 {
                let p = w.m(id).position;
                attack::sphere_hit(w, 1.0, 2.0, 1.0, id, p, 0x1_0000, 0, 1, 0);
            }
            let r = w.ticks(RUMBLE) as f32;
            let top = c::pf(w, id, pv::VZ) * r * 0.5 + w.m(id).position[2];
            bubbles(w, id, home_z, top);
            sway(w, id, SWAY, 1.0);
            let vel = c::pv4(w, id, pv::VEL);
            w.mm(id).position = c::add(w.m(id).position, vel);
            let vz = c::pf(w, id, pv::VZ) - G * DT2;
            c::set_pf(w, id, pv::VZ, vz);
            if vz <= 0.0 {
                w.mm(id).state = 3;
                let f = c::pf(w, id, pv::FLOAT);
                c::set_pf(w, id, pv::TARGET, f - 0.5);
                let t = w.svc.timing.scale(crate::ps2v::Pf::f(c::pf(w, id, pv::HOLD))).to_f32() as i32;
                c::set_pi32(w, id, pv::TIMER, t);
            }
        }
        3 => {
            let z = w.m(id).position[2];
            bubbles(w, id, home_z, z);
            sway(w, id, SWAY, 1.0);
            let (mut z, mut vz) = (z, c::pf(w, id, pv::VZ));
            turn::spring(home_z + c::pf(w, id, pv::TARGET), FLOAT_K * DT2, FLOAT_K * DT2, DT * 20.0, &mut z, &mut vz);
            w.mm(id).position[2] = z;
            c::set_pf(w, id, pv::VZ, vz);
            if vz == 0.0 {
                let f = c::pf(w, id, pv::FLOAT);
                let t = if f < c::pf(w, id, pv::TARGET) { f - 0.5 } else { f + 0.5 };
                c::set_pf(w, id, pv::TARGET, t);
            }
            if c::dec_timer_pvar_i32(w, id, pv::TIMER) != 0 { w.mm(id).state = 4; }
        }
        4 => {
            let r = w.ticks(RUMBLE) as f32;
            let z = w.m(id).position[2];
            bubbles(w, id, home_z, c::pf(w, id, pv::VZ) * r + z);
            let high = home_z + 1.0 < z;
            sway(w, id, if high { SWAY } else { 0.0 }, if high { 1.0 } else { 10.0 });
            let vel = c::pv4(w, id, pv::VEL);
            w.mm(id).position = c::add(w.m(id).position, vel);
            let vz = c::pf(w, id, pv::VZ) - G * DT2;
            c::set_pf(w, id, pv::VZ, vz);
            if w.m(id).position[2] < home_z {
                w.play_sound(1, 0, id);
                let home = c::pv4(w, id, pv::HOME);
                rest(w, id);
                w.mm(id).position = home;
            }
        }
        _ => {}
    }
    let (h, pos) = (super::hero_pos(w), w.m(id).position);
    if h[2] < pos[2] - 2.0 && c::dist2(pos, h) < PUSH_REACH {
        if let Some(hm) = w.hero_moby {
            let a = c::atan(h[0] - pos[0], h[1] - pos[1]);
            attack::hit_moby(w, hm, id, 0.0, 1, [h[0], h[1], h[2] + 0.75, h[3]], [a.cos() * 0.2, a.sin() * 0.2, 0.0, 0.0]);
        }
    }
    let m = w.mm(id);
    let disp = c::sub(m.position, old_pos);
    let rot = m.rotation;
    triggers::carry_riders(&mut m.pvars, pv::BLOCK, disp, old_rot, rot);
    let pos = w.m(id).position;
    match usize::try_from(c::pi32(w, id, pv::HELPER) - 1).ok().filter(|&h| h < w.table.mobys.len()) {
        Some(hm) => {
            w.mm(hm).position = pos;
            w.build_matrix(hm);
        }
        None => {
            if let Some(hm) = w.create_moby(HELPER) {
                c::set_pi32(w, id, pv::HELPER, hm as i32 + 1);
                let m = w.mm(hm);
                m.update_dist = 0;
                m.mode |= 1;
                m.draw_dist = 0;
                m.visible = 0;
                m.position = pos;
                w.build_matrix(hm);
            }
        }
    }
}
