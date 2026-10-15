//! **Gemlik's missile drones, class 101** (level13 `0x2c25b8`, its tick `0x2c2d30`; census U456; 6 placed) and their
//! **missiles, class 1233** (0x4d1, `0x305a58`, created by code). A hovering drone that bobs and drifts about its
//! home (`0x277a00(0.5, 80°·dt)`, `0x277a80(0.105, 18°·dt, 21°·dt)`, x / y = home + the wobble's sines) with jets
//! from joints 2 and 3, humming (sound 1). Two missiles ride its joints 0 and 1. Its target (within 40, 50 while
//! attacking; inside its area path +0xd0; not beyond the range in xy, 8 in z, or — out of state 5 — behind a wall from
//! Ratchet's body) wakes it: it turns, opens (sequences 1, 2, 3) and fires the joints' missiles alternately every
//! `ticks(60)` at 30·dt toward the target (their range 15 at the start, 5 more a shot, at most the distance), a fresh
//! missile replacing each; the target lost, it closes (4, 5, 6) and idles. One with a post (cuboid +0xd4) waits for
//! a target and then flies there (a spring on the distance). Four health; its own missiles do not hurt it; dead:
//! a beam explosion and its missiles and itself deleted.
//!
//! **A missile** held follows its joint (and its drone's hiding); fired, it flies its velocity with a four-puff trail
//! (type 44), hitting along its path (a hit of 1, flags 0x10001, pushing along its heading); a moby hit (not its drone)
//! makes sparks (type 27, 5 when drawn) and a burst (`SpawnBeamExplosion(2, 1, 4, 3, 1, 2, 20, …, 12, 4, 18, 0, 1, 1)`);
//! the Drone Device's drones (0x1df) get a small one (`(0, 0, 0, 0, 0, 1, 0, …, 5, 1, 5, 0, 0, 1)`); past its range
//! or the target: three type-51 puffs and the burst. Outside 2..1021 on any axis: deleted.
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x2c25b8` | the drone's states (module doc) | [`update`] |
//! | `0x2c2d30` | the tick: the hum, the bob and wobble, the jets (type 4: blue / orange on alternate ticks and a hot core), the scale 0.75 × the class's, the hits (0x330000, column 4; not by 0x4d1): health, seq 7 (→ 7) or death (`SetDeathBits`, 8), the flash; the target (`0x274df8` in the area) | [`tick`] |
//! | `0x3058a8` | a missile on joint `j` (draw distance 0x7f, state 1, drawn, the drone's Euler and rows, no collision) | [`new_missile`] |
//! | `0x305988` | the launch (range, velocity, target; state 0, shown, collision) | [`launch`] |
//! | `0x305a58` | the missile | [`missile_update`] |
//!
//! Read from the level13 decomp and disassembly (the explosions' stack arguments, the hit direction). Native `f32`.

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::classes::crate_::set_death_bits;
use crate::moby_update::creature::{self as c, damage, flash, fx, target, turn};
use crate::moby_update::services::{line_hit_in, pv, HitTemplate, World};
use crate::moby_update::story;
use crate::ps2v::Pf;
use std::f32::consts::TAU;

pub const REFERENCE_LEVEL: u32 = 13;
pub const UPDATE_FN: u32 = 0x2c_25b8;
pub const CLASSES: [i16; 1] = [101];
pub const MISSILE_FN: u32 = 0x30_5a58;
pub const MISSILE_CLASSES: [i16; 1] = [MISSILE];

const DT: f32 = c::DT;
const DT2: f32 = c::DT2;
const MISSILE: i16 = 0x4d1;
const DRONES: i16 = 0x1df;

mod pv_ {
    pub const D: usize = 0x20;
    pub const F: usize = 0x60;
    pub const T_POS: usize = 0x70;
    pub const T_MOBY: usize = 0xb0;
    pub const T_KIND: usize = 0xb4;
    pub const HOME: usize = 0xc0;
    pub const AREA: usize = 0xd0;
    pub const POST: usize = 0xd4;
    pub const RANGE: usize = 0xd8;
    pub const TURN_V: usize = 0xdc;
    pub const FIRE_T: usize = 0xe8;
    pub const REACH: usize = 0xec;
    pub const FLY_V: usize = 0xf0;
    pub const SIDE: usize = 0xf4;
    pub const M1: usize = 0xf8;
    pub const M0: usize = 0xfc;
    pub const BOB: usize = 0x100;
    pub const BOB_P: usize = 0x104;
    pub const WOB_A: usize = 0x108;
    pub const WOB_B: usize = 0x10c;
    pub const HUM: usize = 0x110;
    pub const SIZE: usize = 0x114;
}
use pv_ as o;

/// The missile's pvars.
mod mv {
    pub const VEL: usize = 0x00;
    pub const START: usize = 0x10;
    pub const DRONE: usize = 0x20;
    pub const FLAG: usize = 0x24;
    pub const JOINT: usize = 0x26;
    pub const RANGE: usize = 0x28;
    pub const TARGET: usize = 0x2c;
    pub const SIZE: usize = 0x30;
}

const DEATH: fx::Beam = fx::Beam { damage_r: 0.0, damage: 0.0, flash: 4.0, flash2: 2.0, flash_dist: 9.0, scale: 1.0, light: 15.0, streaks: 10, sparks: 3, puffs: 0x10, debris: 1, sound: 2, shake: true };
const BURST: fx::Beam = fx::Beam { damage_r: 2.0, damage: 1.0, flash: 4.0, flash2: 3.0, flash_dist: 1.0, scale: 2.0, light: 20.0, streaks: 12, sparks: 4, puffs: 0x12, debris: 1, sound: 0, shake: true };
const SMALL: fx::Beam = fx::Beam { damage_r: 0.0, damage: 0.0, flash: 0.0, flash2: 0.0, flash_dist: 0.0, scale: 1.0, light: 0.0, streaks: 5, sparks: 1, puffs: 5, debris: 1, sound: 0, shake: false };

fn link(w: &World, id: MobyId, o: usize) -> Option<MobyId> { usize::try_from(c::pi32(w, id, o) - 1).ok().filter(|&m| m < w.table.mobys.len()) }
fn alive(w: &World, m: MobyId) -> bool { let s = w.m(m).state; s != 0xfe && s != 0xfd }
fn blend(w: &mut World, id: MobyId, seq: u8, t: i32) { if w.m(id).anim.seq_b != seq { w.anim_blend(id, seq, 0, t); } }
fn done(w: &World, id: MobyId) -> bool { w.m(id).anim.flags & 2 != 0 }
fn tpos(w: &World, id: MobyId) -> c::V { link(w, id, o::T_MOBY).map_or([0.0; 4], |m| w.m(m).position) }

/// `0x3058a8(drone, j)`: a missile on joint `j` (module doc).
fn new_missile(w: &mut World, id: MobyId, j: u16) -> i32 {
    let Some(m) = w.create_moby(MISSILE) else { return 0 };
    story::pvars(w, m, mv::SIZE);
    let at = w.joint_point(id, j as usize);
    let (rot, rows) = (w.m(id).rotation, w.m(id).rows);
    {
        let mm = w.mm(m);
        mm.draw_dist = 0x7f;
        mm.state = 1;
        mm.visible = 1;
        mm.update_dist = 0xff;
        mm.position = at;
        mm.rotation = rot;
        mm.rows = rows;
        mm.has_collision = false;
    }
    c::set_pi32(w, m, mv::DRONE, id as i32 + 1);
    c::set_pi16(w, m, mv::JOINT, j as i16);
    c::set_pi16(w, m, mv::FLAG, 1);
    c::set_pv4(w, m, mv::START, at);
    w.build_matrix(m);
    m as i32 + 1
}

/// `0x305988(range, missile, vel, target)` (module doc).
fn launch(w: &mut World, m: MobyId, range: f32, vel: c::V, target: Option<MobyId>) {
    if w.m(m).pvars.len() < mv::SIZE { return; }
    c::set_pf(w, m, mv::RANGE, range);
    let Some(d) = link(w, m, mv::DRONE) else { return };
    let (rot, rows) = (w.m(d).rotation, w.m(d).rows);
    let j = c::pi16(w, m, mv::JOINT) as usize;
    let at = w.joint_point(d, j);
    let coll = super::class_collision(w, MISSILE);
    {
        let mm = w.mm(m);
        mm.rotation = rot;
        mm.rows = rows;
        mm.position = at;
        mm.state = 0;
        mm.mode &= !mode::HIDDEN;
        mm.has_collision = coll;
    }
    c::set_pv4(w, m, mv::START, at);
    c::set_pv4(w, m, mv::VEL, vel);
    c::set_pi32(w, m, mv::TARGET, target.map_or(0, |t| t as i32 + 1));
    w.build_matrix(m);
}

/// `0x2c2d30`: the tick (module doc).
fn tick(w: &mut World, id: MobyId) {
    if w.m(id).state == 0 { return; }
    let hum = c::pi32(w, id, o::HUM);
    if hum == -1 || !w.sound_alive(hum, id) {
        let v = w.play_sound(1, 4, id);
        c::set_pi32(w, id, o::HUM, v);
    }
    super::bob(w, id, 0.5, DT * 1.396_263_4, o::BOB, o::BOB_P);
    super::wobble(w, id, f32::from_bits(0x3dd6_7750), DT * 0.314_159_27, DT * 0.366_519_15, o::WOB_A, o::WOB_B);
    let home = c::pv4(w, id, o::HOME);
    w.mm(id).position[0] = home[0] + c::pf(w, id, o::WOB_A).sin();
    w.mm(id).position[1] = home[1] + c::pf(w, id, o::WOB_B).sin();
    let z = -(w.rng.randf(4.5, 5.5) * DT);
    let v = [0.0, 0.0, z, 0.0];
    let (j2, j3) = (w.joint_point(id, 2), w.joint_point(id, 3));
    let puff = |w: &mut World, p: c::V, c1: u32, c2: u32, life: i32, base: i16, growth: i16| {
        fx::part04(w, &crate::particles::type04::Spawn { pos: p, vel: v, c1, c2, life, base, growth, additive: true });
    };
    if w.counter & 1 == 0 {
        for p in [j2, j3] {
            let n = w.rng.rand_range(0x14, 0x18);
            let t = w.ticks(n);
            puff(w, p, 0x6000_ffff, 0x80, t, 100, -10);
        }
    } else {
        for p in [j2, j3] {
            let n = w.rng.rand_range(0x12, 0x16);
            let t = w.ticks(n);
            puff(w, p, 0xcf00_00ff, 0xcf, t, 0x32, -10);
        }
    }
    for p in [j2, j3] {
        let n = w.rng.rand_range(10, 0x19);
        let k = w.rng.rand_range(10, 0xf);
        let t = w.ticks(k);
        puff(w, p, 0xefff_7f4f, 0xff_0000, t, n as i16, -(n as i16));
    }
    let oc = w.m(id).o_class;
    w.mm(id).scale = w.class_scale(oc).to_f32() * 0.75;
    let hit = w.get_hit(id, 0x33_0000, false);
    let res = damage::resolve(w, id, hit, o::D, 0, 4);
    let mine = hit.and_then(|h| h.attacker).is_some_and(|a| w.m(a).o_class == MISSILE);
    let (out5, dmg) = if mine { (1, 0.0) } else { (res.out5, res.damage) };
    if out5 != 1 {
        let hp = c::pf(w, id, o::D) - dmg;
        c::set_pf(w, id, o::D, hp);
        blend(w, id, 7, 5);
        let reaction = if hp <= 0.0 { 1 } else { w.mm(id).state = 7; res.reaction };
        match reaction {
            1 | 2 => {
                set_death_bits(w, id, 0, -1);
                w.mm(id).state = 8;
                c::set_pu8(w, id, o::F + 7, 0xf0);
            }
            3..=8 => c::set_pu8(w, id, o::F + 7, 0x78),
            9 | 10 => c::set_pu8(w, id, o::F + 7, 0xfa),
            _ => {}
        }
        flash::start(w, id, o::F);
    }
    w.mm(id).hit_slot = 0xff;
    flash::update(w, id, o::F);
    let range = if w.m(id).state == 4 { 50.0 } else { 40.0 };
    c::set_pf(w, id, o::RANGE, range);
    let area = usize::try_from(c::pi32(w, id, o::AREA)).ok();
    let t = target::acquire_in(w, id, range, area);
    c::set_pv4(w, id, o::T_POS, t.pos);
    c::set_pi32(w, id, o::T_MOBY, t.moby.map_or(0, |m| m as i32 + 1));
    let mut kind = t.kind;
    if kind != 2 {
        let p = c::pos(w, id);
        if range < c::dist2(p, t.pos) || 8.0 < (p[2] - t.pos[2]).abs() {
            kind = 2;
        } else if w.m(id).state != 5 {
            let b = w.hero.body_point;
            if w.coll_line(pv(p), b, 2, Some(id)).is_some() { kind = 2; }
        }
    }
    c::set_pi32(w, id, o::T_KIND, kind as i32);
    if c::pi32(w, id, o::T_MOBY) == 0 { c::set_pi32(w, id, o::T_MOBY, w.hero_moby.map_or(0, |m| m as i32 + 1)); }
}

fn kind(w: &World, id: MobyId) -> i32 { c::pi32(w, id, o::T_KIND) }

/// State 4 (module doc).
fn attack(w: &mut World, id: MobyId) {
    let t = tpos(w, id);
    let h = c::atan(t[0] - c::pos(w, id)[0], t[1] - c::pos(w, id)[1]);
    turn::turn_toward_pvar(w, id, h, DT2 * 12.566_371, DT2 * 25.132_742, DT * TAU, o::TURN_V);
    let to1 = |w: &mut World, id: MobyId| { w.mm(id).cmd = 1; blend(w, id, 1, 2); };
    match w.m(id).cmd {
        0 => to1(w, id),
        1 => {
            if !done(w, id) { return; }
            w.mm(id).cmd = 2;
            blend(w, id, 2, 2);
            c::set_pi32(w, id, o::FIRE_T, 0);
        }
        2 => {
            if !done(w, id) { return; }
            w.mm(id).cmd = 3;
            blend(w, id, 3, 2);
        }
        3 => {
            if c::dec_timer_pvar_i32(w, id, o::FIRE_T) != 0 {
                let t60 = w.ticks(0x3c);
                c::set_pi32(w, id, o::FIRE_T, t60);
                let side = c::pi32(w, id, o::SIDE);
                let muzzle = w.joint_point(id, (side != 0) as usize);
                let tm = link(w, id, o::T_MOBY);
                let v = c::set_len3(c::sub(tpos(w, id), muzzle), 30.0 * DT);
                let (slot, joint) = if side == 0 { (o::M0, 0) } else { (o::M1, 1) };
                if let Some(m) = link(w, id, slot) {
                    let r = c::pf(w, id, o::REACH);
                    launch(w, m, r, v, tm);
                    w.play_sound(0, 0, id);
                }
                let n = new_missile(w, id, joint);
                c::set_pi32(w, id, slot, n);
                c::set_pi32(w, id, o::SIDE, (side == 0) as i32);
                let r = c::pf(w, id, o::REACH) + 5.0;
                let d = c::dist2(c::pos(w, id), tpos(w, id));
                c::set_pf(w, id, o::REACH, if d < r { d } else { r });
            }
            if kind(w, id) != 2 { return; }
            w.mm(id).cmd = 4;
            blend(w, id, 4, 2);
        }
        4 => {
            if !done(w, id) { return; }
            w.mm(id).cmd = 5;
            blend(w, id, 5, 2);
        }
        5 => {
            if kind(w, id) != 2 {
                w.mm(id).cmd = 2;
                blend(w, id, 2, 2);
                return;
            }
            w.mm(id).cmd = 6;
            blend(w, id, 6, 2);
        }
        6 => {
            if !done(w, id) { return; }
            if kind(w, id) == 2 {
                blend(w, id, 0, 2);
                w.mm(id).state = 2;
                w.mm(id).cmd = 0;
                return;
            }
            to1(w, id);
        }
        _ => {}
    }
}

/// Level13 `0x2c25b8` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    story::pvars(w, id, o::SIZE);
    tick(w, id);
    match w.m(id).state {
        0 => {
            let p = c::pos(w, id);
            c::set_pv4(w, id, o::HOME, p);
            w.mm(id).mode |= 0x5000;
            c::set_pf(w, id, o::REACH, 15.0);
            c::set_pf(w, id, o::D, 4.0);
            c::set_pi16(w, id, o::D + 4, 4);
            c::set_pu8(w, id, o::D + 8, 3);
            c::set_pu8(w, id, 0x58, 0x32);
            c::set_pu8(w, id, 0x5a, 0);
            w.mm(id).draw_dist = 0xff;
            w.mm(id).position[2] += 1.0;
            blend(w, id, 0, 10);
            w.mm(id).cmd = 0;
            let m0 = new_missile(w, id, 0);
            c::set_pi32(w, id, o::M0, m0);
            let m1 = new_missile(w, id, 1);
            c::set_pi32(w, id, o::M1, m1);
            w.mm(id).state = if c::pi32(w, id, o::POST) != -1 { 5 } else { 2 };
            for k in [o::BOB, o::WOB_A, o::WOB_B] {
                let a = w.rng.rand_angle();
                c::set_pf(w, id, k, a);
            }
            c::set_pi32(w, id, o::HUM, -1);
        }
        2 => {
            let t = tpos(w, id);
            let h = c::atan(t[0] - c::pos(w, id)[0], t[1] - c::pos(w, id)[1]);
            turn::turn_toward_pvar(w, id, h, DT2 * 4.712_389, DT2 * 4.712_389, DT * 10.122_91, o::TURN_V);
            if kind(w, id) == 2 { return; }
            c::set_pf(w, id, o::REACH, 15.0);
            w.mm(id).cmd = 1;
            blend(w, id, 1, 0);
            w.mm(id).state = 4;
        }
        4 => attack(w, id),
        5 => {
            if kind(w, id) == 2 { return; }
            w.mm(id).cmd = 0;
            w.mm(id).state = 6;
        }
        6 => {
            let i = c::pi32(w, id, o::POST);
            let post = w.svc.volumes.shape(rc_formats::volumes::ShapeKind::Cuboid, i).map(|s| s.centre()).unwrap_or([0.0; 3]);
            let post = [post[0], post[1], post[2], 1.0];
            let p = c::pos(w, id);
            let d = c::dist3(p, post);
            let (mut x, mut v) = (0.0, c::pf(w, id, o::FLY_V));
            turn::spring(d, DT2 * 10.0, DT2 * 10.0, DT * 20.0, &mut x, &mut v);
            c::set_pf(w, id, o::FLY_V, v);
            let step = c::set_len3(c::sub(post, p), v);
            w.mm(id).position = c::add(p, step);
            if d < 0.1 && v < 0.01 {
                blend(w, id, 0, 10);
                w.mm(id).cmd = 0;
                w.mm(id).state = 2;
            }
        }
        7 => {
            if !done(w, id) { return; }
            let cmd = w.m(id).cmd;
            if w.m(id).anim.seq_b != cmd { w.anim_blend(id, cmd, 0, 10); }
            w.mm(id).state = 4;
        }
        8 => {
            let p = c::pos(w, id);
            fx::beam_explosion(w, &DEATH, Some(id), p);
            for k in [o::M1, o::M0] {
                if let Some(m) = link(w, id, k) { w.delete_moby(m); }
            }
            w.delete_moby(id);
        }
        _ => {}
    }
}

/// Level13 `0x305a58`: a missile (module doc).
pub fn missile_update(w: &mut World, id: MobyId) {
    story::pvars(w, id, mv::SIZE);
    if w.m(id).state == 1 {
        let Some(d) = link(w, id, mv::DRONE).filter(|&d| w.m(d).o_class == 0x65 && alive(w, d)) else {
            w.delete_moby(id);
            return;
        };
        let (rot, rows) = (w.m(d).rotation, w.m(d).rows);
        let j = c::pi16(w, id, mv::JOINT) as usize;
        let at = w.joint_point(d, j);
        let hidden = w.m(d).mode & mode::HIDDEN != 0;
        {
            let m = w.mm(id);
            m.rotation = rot;
            m.rows = rows;
            m.position = at;
            if hidden { m.mode |= mode::HIDDEN; } else { m.mode &= !mode::HIDDEN; }
        }
        c::set_pv4(w, id, mv::START, at);
        bounds(w, id);
        return;
    }
    if w.get_hit(id, 0x23_0000, false).and_then(|h| h.attacker).is_some_and(|a| w.m(a).o_class == DRONES) {
        let p = c::pos(w, id);
        fx::beam_explosion(w, &SMALL, Some(id), p);
        w.delete_moby(id);
        return;
    }
    let old = c::pos(w, id);
    let v = c::pv4(w, id, mv::VEL);
    let p = c::add(old, v);
    w.mm(id).position = p;
    for i in 0..4 {
        let q = c::add(p, c::scale(v, -(i as f32) * 0.25));
        let size = w.rng.randf(25000.0, 35000.0);
        let life = w.ticks(0xf);
        let a = crate::particles::type44::Spawn { size, growth: 500.0, damp: 1.0, fall: f32::from_bits(0xb9d1_b717), w: 0.0, pos: [q[0], q[1], q[2]], vel: [0.0; 3], life, alpha: 0x60, rgb: 0x40_40ff, spin: 3 };
        *w.svc.fx.part_spawns.entry(44).or_default() += 1;
        match w.particles.as_deref_mut() {
            Some(sys) => match crate::particles::type44::spawn_rng(sys, w.rng, &a) {
                Some(r) => sys.pool.recs[r][9] = 0x84,
                None => w.svc.fx.part_failed += 1,
            },
            None => { w.rng.randi(0xff); }
        }
    }
    let mut dir = c::set_len2([v[0], v[1], 0.0, v[3]], 1.0);
    dir[2] = 1.0;
    let tmpl = HitTemplate { dir: dir.map(Pf::f), attacker: Some(id), flags: 0x1_0001, damage: Pf::ONE, w20: 1, ..Default::default() };
    let start = c::pv4(w, id, mv::START);
    let d_self = c::dist2(p, start);
    let d_tgt = link(w, id, mv::TARGET).map_or(0.0, |t| c::dist2(w.m(t).position, start));
    let drawn = w.m(id).visible != 0;
    if d_self <= c::pf(w, id, mv::RANGE) && d_self <= d_tgt {
        let flags = (c::pi16(w, id, mv::FLAG) == 0) as u32;
        let Some(h) = line_hit_in(w.table, w.svc, w.classes, w.coll, pv(old), pv(p), flags, Some(id), &tmpl) else {
            bounds(w, id);
            return;
        };
        if h.moby.is_some() && h.moby == link(w, id, mv::DRONE) {
            bounds(w, id);
            return;
        }
        w.mm(id).position = [h.point[0], h.point[1], h.point[2], p[3]];
        if drawn {
            let n = [h.normal[0], h.normal[1], h.normal[2], 0.0];
            for _ in 0..5 {
                let r = [w.rng.randf(-1.0, 1.0), w.rng.randf(-1.0, 1.0), w.rng.randf(-1.0, 1.0), 0.0];
                let refl = crate::moby_update::services::reflect(pv(v), pv(n)).map(|x| f32::from_bits(x.0));
                let refl = [refl[0], refl[1], refl[2], 0.0];
                let l = c::len3(refl);
                let e = c::add(refl, c::set_len3(r, l * 0.5));
                let s = w.rng.randf(DT * 3.0, DT * 6.0);
                let e = c::set_len3(e, s);
                let (a, b) = (w.ticks(10), w.ticks(0xf));
                let life = w.rng.rand_range(a, b);
                let at = c::pos(w, id);
                fx::part27(w, 30000.0, at, e, 0x7f2f_4f6f, life);
            }
        }
        let at = c::pos(w, id);
        if h.moby.is_some_and(|m| w.m(m).o_class == DRONES) {
            fx::beam_explosion(w, &SMALL, Some(id), at);
        } else {
            fx::beam_explosion(w, &BURST, Some(id), at);
        }
        w.delete_moby(id);
        return;
    }
    if drawn {
        for _ in 0..3 {
            let r = [w.rng.randf(-1.0, 1.0), w.rng.randf(-1.0, 1.0), w.rng.randf(-1.0, 1.0), 0.0];
            let a = c::add(c::set_len3(r, c::len3(r) * 0.1), r);
            let s = w.rng.randf(DT + DT, DT * 4.0);
            let v = c::set_len3(a, s);
            *w.svc.fx.part_spawns.entry(crate::particles::type51::TYPE).or_default() += 1;
            if let Some(sys) = w.particles.as_deref_mut() {
                if crate::particles::type51::spawn(sys, p, v).is_none() { w.svc.fx.part_failed += 1; }
            }
        }
    }
    fx::beam_explosion(w, &BURST, Some(id), p);
    w.delete_moby(id);
}

/// Outside 2..1021 on any axis: deleted.
fn bounds(w: &mut World, id: MobyId) {
    let p = c::pos(w, id);
    if !p[..3].iter().all(|&x| (2.0..=1021.0).contains(&x)) { w.delete_moby(id); }
}
