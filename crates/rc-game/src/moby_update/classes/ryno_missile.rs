//! **The R.Y.N.O.'s missile** (class 457; level01 spawn `0x2e5738` from the R.Y.N.O.'s update, update `0x2e5a48`;
//! read from the decompiler output and the disassembly). The launcher is `crate::hero::ryno`.
//!
//! **Pvars** (the game's layout): +0x00 the target's aim point last tick, +0x10 the missile's centre line (it flies
//! along it and wobbles about it), +0x20 the owner (the item; the port keeps Ratchet's moby + 1), +0x24 its life
//! (`ticks(250)`), +0x28 the target (moby + 1), +0x2c the target's aim height, +0x30 the speed, +0x34 / +0x38 the yaw /
//! pitch turn rates, +0x44 the wobble radius, +0x48 / +0x4c the wobble's two phases, +0x50 / +0x54 their rates,
//! +0x58 the target's class.
//!
//! **Spawn** ([`spawn`]): `CreateMoby(0x1c9)`: distances 0xff, visible, yaw / pitch as aimed, at the barrel, speed 0,
//! a quarter of the class scale; with a target: its class, its record's +0x1e |= 0x80 (the target knows a missile is
//! on it), the aim point; life `ticks(250)`, wobble 0, phases `randf(−π, π)` ×2, rates `randf(±0.00967)`,
//! `randf(±0.02406)`; `MobyBuildMatrix`; class sound 1. A line from Ratchet (at the barrel's height) to the barrel that
//! hits: +0xbc = 2, at the hit, the beam explosion (below) and the delete.
//!
//! **Update** ([`update`]): the scale grows to the class's by 5 % a tick and the speed to 20 u/s by 10 %. After its
//! first 5 ticks it steers: **with its target** (same class, not deleted) toward the intercept of the target's aim
//! point (`super::missile::intercept_time`, from its centre line at its speed) with spring turns of `60π·dt²` (no damping,
//! no cap); the wobble shrinks as `d/6` within 6 of the target, else grows by 0.1 a tick; **without** it the wobble
//! grows, the pitch sways by `sin(a)·0.04` (after 100 ticks of flight) and the yaw by `cos(a)·0.05`. The centre line
//! moves `speed` along the aim; a smoke puff (type 4: 0x6f00afff → 0x100000ff, a random `randf(0.1, 0.2)·dt` drift less
//! `randf(0.1, 1)·dt` along the path, at a random point of this step, `ticks(28..40)` ticks, growth 40..70) trails it;
//! the phases advance (b also bends a by `sin(b)·0.15`), the wobble ≤ 0.9; the missile sits at the centre line plus
//! `(0, w·sin a·cos b, w·cos a·cos b)` in its own frame, the wobble shrinking ×0.7 (while above 0.1) where the world
//! (flags 6) is in the way — and then aims along its actual motion. Out of the positive octant → deleted. Its path is
//! tested (`CollLine_Fix(old, new, 0, missile, tmpl)`: push along the path, 1, 5627.97, flags 0x830000, damage 3, type
//! 3 / 3, class 457): **nothing** — its life out or farther than 200 (2D) from Ratchet → deleted (no explosion);
//! a world face → +0xbc = 1, the beam explosion; Ratchet, its owner, another missile or class 0 → through; another
//! moby → +0xbc = 2, the beam explosion. The beam explosion is `SpawnBeamExplosion(0, 0|1, 4, 2, 9, 1, 15, missile, …,
//! 10 streaks, 3 spark pairs, 9 puffs, sound 0, shake, debris)` (`crate::moby_update::creature::fx::beam_explosion`:
//! no damage sphere, radius 0 — the hit is the path test's). Every `ticks(32)` of its life the phase rates may flip
//! (`rand() & 1`: `×randf(−0.7, −1.5)` and `×randf(−0.6, −1.6)`).
//!
//! Native `f32`; the game's draws in its order. [L]: a missile blocked at its barrel explodes on its first update (the
//! game's spawn does it at once, inside the hero update, where the port has no moby world).

use crate::hero::guns::{add3, len3, sub3, with_len};
use crate::moby_runtime::MobyId;
use crate::moby_update::creature::fx::{self, Beam};
use crate::moby_update::creature::turn::spring_turn;
use crate::moby_update::creature::{add_rot, atan};
use crate::moby_update::services::{self as sv, pvar as p, HitTemplate, World};
use crate::ps2v::Pf;
use crate::targeting::polar;
use std::f32::consts::PI;

pub const UPDATE_FN: u32 = 0x2e5a48;
pub const CLASSES: [i16; 1] = [457];
pub const CLASS: i16 = 457;
const DT: f32 = 1.0 / 60.0;

pub mod pv {
    pub const AIM: usize = 0x00;
    pub const LINE: usize = 0x10;
    pub const OWNER: usize = 0x20;
    pub const LIFE: usize = 0x24;
    pub const TARGET: usize = 0x28;
    pub const AIM_H: usize = 0x2c;
    pub const SPEED: usize = 0x30;
    pub const YAW_VEL: usize = 0x34;
    pub const PITCH_VEL: usize = 0x38;
    pub const WOBBLE: usize = 0x44;
    pub const PHASE_A: usize = 0x48;
    pub const PHASE_B: usize = 0x4c;
    pub const RATE_A: usize = 0x50;
    pub const RATE_B: usize = 0x54;
    pub const TARGET_CLASS: usize = 0x58;
    /// The port's flag: blocked at the barrel (explode on the first update).
    pub const BLOCKED: usize = 0x5c;
}

/// The beam explosion's arguments (`0x2e5a48` / `0x2e5738`: `(0, d, 4, 2, 9, 1, 15, …, 10, 3, 9, 0, 1)`, debris 1).
pub const BEAM: Beam = Beam { damage_r: 0.0, damage: 0.0, flash: 4.0, flash2: 2.0, flash_dist: 9.0, scale: 1.0, light: 15.0, streaks: 10, sparks: 3, puffs: 9, debris: 1, sound: 0, shake: true };

/// `0x2e5738(aim height, yaw, pitch, owner, barrel, target)` from the R.Y.N.O.'s update (module doc).
#[allow(clippy::too_many_arguments)]
pub fn spawn(hero: &mut crate::hero::Hero, table: &mut crate::moby_runtime::MobyTable, env: &crate::hero::items::ItemEnv, hits: &mut dyn crate::hero::items::HitSink, rng: &mut crate::rng::Rng, aim_h: f32, yaw: f32, pitch: f32, muzzle: [f32; 3], target: Option<MobyId>) -> Option<MobyId> {
    let id = hits.create_moby(table, CLASS, env.frame as u64)?;
    let tinfo = target.and_then(|t| table.mobys.get(t)).map(|m| (m.o_class, [m.position[0], m.position[1], m.position[2]], crate::targeting::record(m)));
    if let (Some(t), Some((_, _, Some(_)))) = (target, tinfo) { crate::targeting::mark_missile(&mut table.mobys[t]); }
    let (pa, pb) = (rng.randf(-PI, PI), rng.randf(-PI, PI));
    let ra = rng.randf(-0.00967, 0.00967);
    let rb = rng.randf(-0.02406, 0.02406);
    let m = &mut table.mobys[id];
    if m.pvars.len() < 0x80 { m.pvars.resize(0x80, 0); }
    m.update_dist = 0xff;
    m.draw_dist = 0xff;
    m.visible = 1;
    m.rotation = [0.0, pitch, yaw, 0.0];
    m.state = 0;
    p::set_i32(&mut m.pvars, pv::OWNER, env.hero_moby as i32 + 1);
    m.position = [muzzle[0], muzzle[1], muzzle[2], m.position[3]];
    p::set_v4f(&mut m.pvars, pv::LINE, [muzzle[0], muzzle[1], muzzle[2], 0.0]);
    for o in [pv::YAW_VEL, pv::PITCH_VEL, pv::SPEED] { p::set_ff(&mut m.pvars, o, 0.0); }
    p::set_i32(&mut m.pvars, pv::TARGET, target.map_or(0, |t| t as i32 + 1));
    m.scale *= 0.25;
    if let Some((oc, tp, _)) = tinfo {
        p::set_i32(&mut m.pvars, pv::TARGET_CLASS, oc as i32);
        p::set_v4f(&mut m.pvars, pv::AIM, [tp[0], tp[1], tp[2] + aim_h, 0.0]);
        p::set_ff(&mut m.pvars, pv::AIM_H, aim_h);
    }
    p::set_i32(&mut m.pvars, pv::LIFE, crate::hero::physics::ticks(250));
    p::set_ff(&mut m.pvars, pv::WOBBLE, 0.0);
    p::set_ff(&mut m.pvars, pv::PHASE_A, pa);
    p::set_ff(&mut m.pvars, pv::PHASE_B, pb);
    p::set_ff(&mut m.pvars, pv::RATE_A, ra);
    p::set_ff(&mut m.pvars, pv::RATE_B, rb);
    let r = rc_formats::moby_light::rotation_rows([0.0, pitch, yaw]);
    for (row, src) in m.rows.iter_mut().zip(r) { *row = [f32::from_bits(src[0]), f32::from_bits(src[1]), f32::from_bits(src[2]), 0.0]; }
    hero.fx.item_voices.push(crate::hero::packs::SoundCmd::MobySound { moby: id, o_class: CLASS, pos: muzzle, index: 1, flags: 0 });
    // The line from Ratchet at the barrel's height.
    let hp = table.mobys.get(env.hero_moby).map_or(muzzle, |h| [h.position[0], h.position[1], muzzle[2]]);
    let f4 = |a: [f32; 3]| [Pf::f(a[0]), Pf::f(a[1]), Pf::f(a[2]), Pf::ZERO];
    let blocked = hits.probe_moby(table, f4(hp), f4(muzzle), 0, Some(env.hero_moby)).flatten().map(|h| h.point).or_else(|| env.coll.and_then(|c| crate::hero::physics::line_world(c, f4(hp), f4(muzzle), 0)).map(|o| o.point));
    if let Some(h) = blocked {
        let m = &mut table.mobys[id];
        m.cmd = 2;
        m.position = [h[0], h[1], h[2], m.position[3]];
        p::set_i32(&mut m.pvars, pv::BLOCKED, 1);
    }
    Some(id)
}

fn v3(a: [f32; 4]) -> [f32; 3] { [a[0], a[1], a[2]] }

/// `0x2e5a48`.
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x80 { w.mm(id).pvars.resize(0x80, 0); }
    if p::i32(&w.m(id).pvars, pv::BLOCKED) != 0 {
        let pos = w.m(id).position;
        fx::beam_explosion(w, &BEAM, Some(id), pos);
        w.delete_moby(id);
        return;
    }
    let speed_k = 1.0f32;
    let class_scale = w.class_scale(CLASS).to_f32();
    {
        let m = w.mm(id);
        m.scale += (class_scale - m.scale) * speed_k * 0.05;
        let s = p::ff(&m.pvars, pv::SPEED);
        p::set_ff(&mut m.pvars, pv::SPEED, s + (DT * 20.0 - s) * (speed_k * 0.1) * 1.0);
    }
    let life = p::i32(&w.m(id).pvars, pv::LIFE);
    let speed = p::ff(&w.m(id).pvars, pv::SPEED);
    if life < w.ticks(250) - w.ticks(5) {
        let t = (p::i32(&w.m(id).pvars, pv::TARGET) as usize).checked_sub(1);
        let tclass = p::i32(&w.m(id).pvars, pv::TARGET_CLASS);
        let live = t.filter(|&t| w.table.mobys.get(t).is_some_and(|m| m.o_class as i32 == tclass && m.state != 0xfe && m.state != 0xfd));
        match live {
            None => {
                let m = w.mm(id);
                p::set_i32(&mut m.pvars, pv::TARGET, 0);
                let wob = p::ff(&m.pvars, pv::WOBBLE) + speed_k * 0.1;
                p::set_ff(&mut m.pvars, pv::WOBBLE, wob);
                let a = p::ff(&m.pvars, pv::PHASE_A);
                if life < w.ticks(250) - w.ticks(100) {
                    let m = w.mm(id);
                    m.rotation[1] = add_rot(m.rotation[1], a.sin() * speed_k * 0.04);
                }
                let m = w.mm(id);
                m.rotation[2] = add_rot(m.rotation[2], a.cos() * speed_k * 0.05);
            }
            Some(t) => {
                let tm = w.m(t).position;
                let aim_h = p::ff(&w.m(id).pvars, pv::AIM_H);
                let tp = [tm[0], tm[1], tm[2] + aim_h];
                let pvs = &w.m(id).pvars;
                let vt = sub3(tp, v3(p::v4f(pvs, pv::AIM)));
                let rel = sub3(tp, v3(p::v4f(pvs, pv::LINE)));
                let tt = super::missile::intercept_time(speed, vt, rel);
                let (mut vy, mut vp) = (p::ff(pvs, pv::YAW_VEL), p::ff(pvs, pv::PITCH_VEL));
                let acc = DT * DT * 188.495_56;
                let m = w.mm(id);
                p::set_v4f(&mut m.pvars, pv::AIM, [tp[0], tp[1], tp[2], 0.0]);
                if 0.0 < tt {
                    let a = super::missile::lead_point(vt, rel, tt);
                    m.rotation[2] = spring_turn(m.rotation[2], atan(a[0], a[1]), acc, 0.0, 0.0, &mut vy);
                    m.rotation[1] = spring_turn(m.rotation[1], -atan((a[0] * a[0] + a[1] * a[1]).sqrt(), a[2]), acc, 0.0, 0.0, &mut vp);
                    p::set_ff(&mut m.pvars, pv::YAW_VEL, vy);
                    p::set_ff(&mut m.pvars, pv::PITCH_VEL, vp);
                }
                let d = len3(rel);
                let wob = p::ff(&m.pvars, pv::WOBBLE);
                p::set_ff(&mut m.pvars, pv::WOBBLE, if d < 6.0 { wob * (d / 6.0) } else { wob + speed_k * 0.1 });
            }
        }
    }
    let (yaw, pitch) = (w.m(id).rotation[2], w.m(id).rotation[1]);
    let dir = polar(speed, yaw, -pitch);
    // The smoke trail (type 4).
    {
        let x = w.rng.randf(-1.0, 1.0);
        let y = w.rng.randf(-1.0, 1.0);
        let z = w.rng.randf(-1.0, 1.0);
        let s = w.rng.randf(0.1, 0.2);
        let mut v = with_len([x, y, z], s * DT);
        let back = w.rng.randf(DT * 0.1, DT);
        v = add3(v, with_len(dir, -back));
        let k = w.rng.randf(0.0, 1.0);
        let pos = w.m(id).position;
        let at = add3(with_len(dir, k * speed), v3(pos));
        let r = w.rng.rand_range(0x1c, 0x28);
        let life = w.ticks(r);
        let growth = w.rng.rand_range(0x28, 0x46);
        let a = crate::particles::type04::Spawn { pos: [at[0], at[1], at[2], 0.0], vel: [v[0], v[1], v[2], 0.0], c1: 0x6f00_afff, c2: 0x1000_00ff, life, base: 0x28, growth: growth as i16, additive: true };
        fx::part04(w, &a);
    }
    let old = v3(w.m(id).position);
    let line = add3(v3(p::v4f(&w.m(id).pvars, pv::LINE)), dir);
    {
        let m = w.mm(id);
        p::set_v4f(&mut m.pvars, pv::LINE, [line[0], line[1], line[2], 0.0]);
        let (ra, rb) = (p::ff(&m.pvars, pv::RATE_A), p::ff(&m.pvars, pv::RATE_B));
        let a = add_rot(p::ff(&m.pvars, pv::PHASE_A), ra);
        let b = add_rot(p::ff(&m.pvars, pv::PHASE_B), rb);
        let a = add_rot(a, b.sin() * 0.15);
        p::set_ff(&mut m.pvars, pv::PHASE_A, a);
        p::set_ff(&mut m.pvars, pv::PHASE_B, b);
        if 0.9 < p::ff(&m.pvars, pv::WOBBLE) { p::set_ff(&mut m.pvars, pv::WOBBLE, f32::from_bits(0x3f66_6666)); }
    }
    let f4 = |a: [f32; 3]| [Pf::f(a[0]), Pf::f(a[1]), Pf::f(a[2]), Pf::ZERO];
    let mut blocked = false;
    let mut pos;
    loop {
        let m = w.m(id);
        let (a, b, wob) = (p::ff(&m.pvars, pv::PHASE_A), p::ff(&m.pvars, pv::PHASE_B), p::ff(&m.pvars, pv::WOBBLE));
        let off = [0.0, wob * a.sin() * b.cos(), wob * a.cos() * b.cos()];
        let r = rc_formats::moby_light::rotation_rows([m.rotation[0], m.rotation[1], m.rotation[2]]).map(|row| [f32::from_bits(row[0]), f32::from_bits(row[1]), f32::from_bits(row[2])]);
        let o: [f32; 3] = std::array::from_fn(|k| off[0] * r[0][k] + off[1] * r[1][k] + off[2] * r[2][k]);
        pos = add3(line, o);
        if w.line(f4(old), f4(pos), 6, Some(id)).is_none() { break; }
        blocked = true;
        let nw = wob * 0.7;
        p::set_ff(&mut w.mm(id).pvars, pv::WOBBLE, nw);
        if nw <= 0.1 || nw.is_nan() { break; }
    }
    {
        let m = w.mm(id);
        m.position = [pos[0], pos[1], pos[2], m.position[3]];
        if blocked {
            let d = sub3(pos, old);
            m.rotation[2] = atan(d[0], d[1]);
            m.rotation[1] = -atan((d[0] * d[0] + d[1] * d[1]).sqrt(), d[2]);
            p::set_ff(&mut m.pvars, pv::YAW_VEL, 0.0);
            p::set_ff(&mut m.pvars, pv::PITCH_VEL, 0.0);
        }
    }
    if pos[0] < 0.0 || pos[1] < 0.0 || pos[2] < 0.0 {
        w.delete_moby(id);
        return;
    }
    let n = with_len(dir, 1.0);
    let tmpl = HitTemplate { dir: [Pf::f(n[0]), Pf::f(n[1]), Pf::ONE, Pf::b(0x45af_df66)], attacker: Some(id), flags: 0x83_0000, b18: 3, b19: 3, h1a: CLASS as u16, damage: Pf::f(3.0), w20: 1 };
    let hit = sv::line_hit_in(w.table, w.svc, w.classes, w.coll, f4(old), f4(pos), 0, Some(id), &tmpl);
    let owner = (p::i32(&w.m(id).pvars, pv::OWNER) as usize).checked_sub(1);
    let explode = match hit {
        None => {
            let mut l = p::i32(&w.m(id).pvars, pv::LIFE);
            let out = crate::hero::guns::dec(&mut l);
            p::set_i32(&mut w.mm(id).pvars, pv::LIFE, l);
            let hero = w.hero_moby.map_or(old, |h| v3(w.m(h).position));
            if !out && ((pos[0] - hero[0]).powi(2) + (pos[1] - hero[1]).powi(2)).sqrt() <= 200.0 {
                flip_rates(w, id);
                return;
            }
            None
        }
        Some(h) => match h.moby {
            None if h.kind < 1 => {
                flip_rates(w, id);
                return;
            }
            None => Some((1u8, h)),
            Some(m) if Some(m) == w.hero_moby || Some(m) == owner || w.m(m).o_class == CLASS || w.m(m).o_class == 0 => {
                flip_rates(w, id);
                return;
            }
            Some(_) => Some((2u8, h)),
        },
    };
    if let Some((k, h)) = explode {
        let m = w.mm(id);
        m.cmd = k;
        m.position = [h.point[0], h.point[1], h.point[2], m.position[3]];
        // The drift (`dir` reflected off the face, 2·dt long) is SpawnBeamExplosion's param_9, which the port's
        // explosion does not read.
        let pos = w.m(id).position;
        let mut b = BEAM;
        if k == 1 { b.damage = 1.0; }
        fx::beam_explosion(w, &b, Some(id), pos);
    }
    w.delete_moby(id);
}

/// Every `ticks(32)` of the life: the wobble's phase rates may flip (`rand() & 1` each).
fn flip_rates(w: &mut World, id: MobyId) {
    let l = p::i32(&w.m(id).pvars, pv::LIFE);
    let t32 = w.ticks(32).max(1);
    if l % t32 != 0 { return; }
    if w.rng.rand() & 1 != 0 {
        let k = w.rng.randf(-0.7, -1.5);
        let m = w.mm(id);
        let r = p::ff(&m.pvars, pv::RATE_A) * k;
        p::set_ff(&mut m.pvars, pv::RATE_A, r);
    }
    if w.rng.rand() & 1 == 0 { return; }
    let k = w.rng.randf(-0.6, -1.6);
    let m = w.mm(id);
    let r = p::ff(&m.pvars, pv::RATE_B) * k;
    p::set_ff(&mut m.pvars, pv::RATE_B, r);
}
