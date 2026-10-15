//! **Umbris' pop-up turrets, class 1126** (level07 `0x31a250`, 6 placed; census U258) and **their shots 880**
//! (`0x30b6a8`, made by the spawner `0x30b538`). The names are descriptive [L]. A turret sweeps back and forth between
//! the bearings of two marker mobys (+0x70 / +0x74; a full turn without the second) at +0x78°/s, firing along its
//! heading every 6 ticks (12 or 18 when Ratchet is far or it is out of view) in bursts: +0x94 shots then +0x98 ticks
//! of rest (with +0x98 = 0, every other turn). A hit puts it down: sound 1, its death bits, untargetable, the red
//! flash; it spins down into the ground (0.7 below home), rests three seconds, rises with sound 2 and sweeps on. A shot
//! flies straight at 15·dt, hurts (damage 1, flags 0x10001) and stops at the first thing it meets (sparks, sound 0),
//! and fizzles 30 out. Read from the level07 decomp. Native `f32`.
//!
//! **Pvars** (turret): +0x20 the damage record, +0x60 the flash record (+0x67 the red), +0x70 / +0x74 the markers,
//! +0x78 the sweep speed (°/s), +0x7c s16 the fire timer, +0x7e s16 the sweep direction, +0x80 / +0x84 the start and
//! end bearings (0..2π, end past start), +0x88 the offset from the start, +0x8c the sweep, +0x90 the home height,
//! +0x94 s16 / +0x98 s32 the burst and rest, +0x9c s16 the burst counter, +0x9e byte the shot's flag. (Shot): +0x00 the hit
//! template, +0x30 the velocity, +0x40 the start, +0x50 the turret, +0x54 the flag (0: the line also meets mobys).
//!
//! | address | what | port |
//! |---|---|---|
//! | 1126 top | the hit (mask 0x230000) through the resolver; up (state < 2) and out > 1: sound 1, `SetDeathBits`, untargetable, red 0xfa, +0x26 `ticks(60)`, the flash, fire timer `ticks(60)`, sequence 2, → 2; the hit slot cleared; the flash update | [`update`] |
//! | 0 | the bearings (+2π when negative; the end +2π when before the start), offset 0, the sweep, yaw = the start (`0x286d30`), home height, +0x29 0, +0xbc 0, the burst counter = +0x94; sequence 1 → 1 | [`update`] |
//! | 1 | sequence 1 once another wraps; the offset ± speed·dt, turning at the ends; yaw = start + offset; the fire timer out: firing (+0xbc 0): with no rest or the counter odd, sound 0 and a shot from joint list 0 at 15·dt along the yaw (`0x30b538`); no rest → the counter toggles, else the counter down → +0xbc 1 and the rest; resting: the counter down → +0xbc 0 and the burst; the timer `ticks(6)`, ×2 beyond 55 (or beyond 35 out of view), ×3 beyond 80 | [`update`], [`fire`] |
//! | 2 | sequence 4; yaw += 4π·dt; the timer out → `ticks(180)`, → 3 | [`update`] |
//! | 3 | down to home − 0.7 at 5·dt; the yaw back to its bearing at 4π·dt; the timer out → sound 2, sequence 3, → 4 | [`update`] |
//! | 4 | up at 5·dt to home; then targetable, sequence 1, → 1 | [`update`] |
//! | `0x30b538` | `CreateMoby(0x370)`: drawn, draw 0x7f, update 0xff, pitch −atan(\|v.xy\|, v.z), yaw atan(v), the start, the velocity, the template (1, flags 0x10001, (v.xy unit, 1)) with type 1 / 1, sequence 1 (`ticks(5)`), mode 0x208, the turret and the flag, alpha 0x10 | [`fire`] |
//! | 880 | position += velocity; 30 (xy) from the start: drawn → three type-51 puffs (a random vector ·1.1, `randf(2·dt, 4·dt)` long); deleted; else drawn or Ratchet within 30: the line from last position (`CollLine_Fix`, flags 1 when +0x54 is 0, the turret ignored, the template): at the hit, drawn → five type-27 sparks (30000, the velocity reflected off the face plus half its length at random, `randf(3·dt, 6·dt)`, 0x7f2f4f6f, `rand_range(ticks 10, 15)`), sound 0, deleted; out of the world → deleted | [`shot_update`] |

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::creature::{self as c, damage, flash, fx, DT};
use crate::moby_update::services::{pv as v4, HitTemplate, World};
use crate::ps2v::Pf;

pub const REFERENCE_LEVEL: u32 = 7;
pub const UPDATE_FN: u32 = 0x31_a250;
pub const CLASSES: [i16; 1] = [1126];
pub const SHOT_FN: u32 = 0x30_b6a8;
pub const SHOT_CLASSES: [i16; 1] = [880];
const SHOT: i16 = 0x370;
const TAU: f32 = std::f32::consts::TAU;
const DEG: f32 = 0.017_453_292;

/// `0x286d30` (L01 `0x2731d0`): an angle wrapped into (−π, π].
fn wrap(a: f32) -> f32 { crate::moby_update::classes::flyer::wrap_frac(a) }
fn blend(w: &mut World, id: MobyId, s: u8, t: i32) { c::blend_to(w, id, s, 0, t); }
fn bearing(w: &World, id: MobyId, m: i32) -> Option<f32> {
    let m = usize::try_from(m).ok().filter(|&m| m < w.table.mobys.len())?;
    let (q, p) = (w.m(m).position, c::pos(w, id));
    let mut a = c::atan(q[0] - p[0], q[1] - p[1]);
    if a < 0.0 { a += TAU; }
    Some(a)
}
fn anim_done(w: &World, id: MobyId) -> bool { w.m(id).anim.flags & 2 != 0 }

/// Level07 `0x31a250` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0xa0 { return; }
    let h = w.get_hit(id, 0x23_0000, false);
    let r = damage::resolve(w, id, h, 0x20, 0, 4);
    if w.m(id).state < 2 && 1 < r.out5 {
        w.play_sound(1, 0, id);
        crate::moby_update::classes::crate_::set_death_bits(w, id, 0, -1);
        w.mm(id).mode &= !mode::TARGETABLE;
        c::set_pu8(w, id, 0x67, 0xfa);
        let t = w.ticks(60);
        c::set_pi16(w, id, 0x26, t as i16);
        flash::start(w, id, 0x60);
        c::set_pi16(w, id, 0x7c, t as i16);
        blend(w, id, 2, 5);
        w.mm(id).state = 2;
    }
    w.mm(id).hit_slot = 0xff;
    flash::update(w, id, 0x60);
    match w.m(id).state {
        0 => {
            if let Some(a) = bearing(w, id, c::pi32(w, id, 0x70)) { c::set_pf(w, id, 0x80, a); }
            let start = c::pf(w, id, 0x80);
            let end = match bearing(w, id, c::pi32(w, id, 0x74)).filter(|_| c::pi32(w, id, 0x74) != -1) {
                Some(e) if start <= e => e,
                Some(e) => e + TAU,
                None => start + TAU,
            };
            c::set_pf(w, id, 0x84, end);
            c::set_pf(w, id, 0x88, 0.0);
            c::set_pf(w, id, 0x8c, end - start);
            w.mm(id).rotation[2] = wrap(start);
            let z = w.m(id).position[2];
            c::set_pf(w, id, 0x90, z);
            c::set_pu8(w, id, 0x29, 0);
            w.mm(id).cmd = 0;
            let n = c::pi16(w, id, 0x94);
            c::set_pi16(w, id, 0x9c, n);
            blend(w, id, 1, 0);
            w.mm(id).state = 1;
        }
        1 => sweep(w, id),
        2 => {
            let a = w.m(id).anim;
            if a.seq_a != 4 && anim_done(w, id) && a.seq_b != 4 { blend(w, id, 4, 1); }
            w.mm(id).rotation[2] = c::add_rot(w.m(id).rotation[2], DT * 2.0 * TAU);
            if c::dec_timer_pvar_s16(w, id, 0x7c) != 0 {
                let t = w.ticks(180);
                c::set_pi16(w, id, 0x7c, t as i16);
                w.mm(id).state = 3;
            }
        }
        3 => {
            if w.m(id).anim.seq_a != 4 && anim_done(w, id) { blend(w, id, 4, 0); }
            let home = c::pf(w, id, 0x90);
            let z = w.m(id).position[2];
            w.mm(id).position[2] = if home - 0.7 < z { z - DT * 5.0 } else { home - 0.7 };
            let target = wrap(c::pf(w, id, 0x80) + c::pf(w, id, 0x88));
            let yaw = w.m(id).rotation[2];
            w.mm(id).rotation[2] = if DT * 2.0 * TAU < c::diff_rots(yaw, target) { c::add_rot(yaw, DT * 2.0 * TAU) } else { target };
            if c::dec_timer_pvar_s16(w, id, 0x7c) != 0 {
                w.play_sound(2, 0, id);
                blend(w, id, 3, 5);
                w.mm(id).state = 4;
            }
        }
        4 => {
            let home = c::pf(w, id, 0x90);
            let z = w.m(id).position[2];
            if z < home {
                w.mm(id).position[2] = z + DT * 5.0;
                return;
            }
            w.mm(id).position[2] = home;
            w.mm(id).mode |= mode::TARGETABLE;
            blend(w, id, 1, 5);
            w.mm(id).state = 1;
        }
        _ => {}
    }
}

/// State 1 (module doc).
fn sweep(w: &mut World, id: MobyId) {
    let a = w.m(id).anim;
    if a.seq_a != 1 && anim_done(w, id) { blend(w, id, 1, 1); }
    let step = c::pf(w, id, 0x78) * DEG * DT;
    let mut off = c::pf(w, id, 0x88);
    if c::pi16(w, id, 0x7e) == 0 {
        off += step;
        if c::pf(w, id, 0x8c) < off {
            off = c::pf(w, id, 0x8c);
            c::set_pi16(w, id, 0x7e, 1);
        }
    } else {
        off -= step;
        if off < 0.0 {
            off = 0.0;
            c::set_pi16(w, id, 0x7e, 0);
        }
    }
    c::set_pf(w, id, 0x88, off);
    w.mm(id).rotation[2] = wrap(c::pf(w, id, 0x80) + off);
    if c::dec_timer_pvar_s16(w, id, 0x7c) == 0 { return; }
    let rest = c::pi32(w, id, 0x98);
    if w.m(id).cmd == 0 {
        if rest == 0 || c::pi16(w, id, 0x9c) & 1 != 0 {
            let p = w.joint_point(id, 0);
            let yaw = w.m(id).rotation[2];
            let v = [yaw.cos() * DT * 15.0, yaw.sin() * DT * 15.0, 0.0, 0.0];
            w.play_sound(0, 0, id);
            let flag = c::pu8(w, id, 0x9e);
            fire(w, p, v, id, flag);
        }
        if rest == 0 {
            let n = c::pi16(w, id, 0x9c) ^ 1;
            c::set_pi16(w, id, 0x9c, n);
        } else if c::dec_timer_pvar_s16(w, id, 0x9c) != 0 {
            w.mm(id).cmd = 1;
            c::set_pi16(w, id, 0x9c, rest as i16);
        }
    } else if c::dec_timer_pvar_s16(w, id, 0x9c) != 0 {
        w.mm(id).cmd = 0;
        let n = c::pi16(w, id, 0x94);
        c::set_pi16(w, id, 0x9c, n);
    }
    let mut t = w.ticks(6) as i16;
    let d = c::dist2(c::pos(w, id), super::hero_pos(w));
    if d <= 55.0 && (d <= 35.0 || w.m(id).visible != 0) {
        c::set_pi16(w, id, 0x7c, t);
        return;
    }
    t *= if 80.0 < d { 3 } else { 2 };
    c::set_pi16(w, id, 0x7c, t);
}

/// `0x30b538(p, v, turret, flag)` (module doc).
pub fn fire(w: &mut World, p: c::V, v: c::V, turret: MobyId, flag: u8) -> Option<MobyId> {
    let s = w.create_moby(SHOT)?;
    if w.m(s).pvars.len() < 0x58 { w.mm(s).pvars.resize(0x58, 0); }
    {
        let m = w.mm(s);
        m.visible = 1;
        m.rotation[0] = 0.0;
        m.draw_dist = 0x7f;
        m.update_dist = 0xff;
        m.rotation[1] = -c::atan(c::len2(v), v[2]);
        m.rotation[2] = c::atan(v[0], v[1]);
        m.position = p;
    }
    c::set_pv4(w, s, 0x40, p);
    c::set_pv4(w, s, 0x30, v);
    let d = c::set_len2([v[0], v[1], 0.0, v[3]], 1.0);
    let dir = [d[0], d[1], 1.0, d[3]];
    let tmpl = HitTemplate { dir: v4(dir), attacker: Some(s), flags: 0x1_0001, b18: 1, b19: 1, damage: Pf::ONE, w20: 1, ..Default::default() };
    write_template(w, s, &tmpl);
    let t = w.ticks(5);
    w.anim_blend(s, 1, 0, t);
    let m = w.mm(s);
    m.mode = 0x208;
    m.alpha = 0x10;
    c::set_pi32(w, s, 0x50, turret as i32 + 1);
    c::set_pi32(w, s, 0x54, flag as i32);
    w.build_matrix(s);
    Some(s)
}

/// The template's words at pvar +0x00 (the shot's line delivers it).
fn write_template(w: &mut World, s: MobyId, t: &HitTemplate) {
    let d = t.dir.map(|x| f32::from_bits(x.0));
    c::set_pv4(w, s, 0, d);
    c::set_pi32(w, s, 0x14, t.flags as i32);
    c::set_pu8(w, s, 0x18, t.b18);
    c::set_pu8(w, s, 0x19, t.b19);
    c::set_pf(w, s, 0x1c, 1.0);
}

fn read_template(w: &World, s: MobyId) -> HitTemplate {
    let d = c::pv4(w, s, 0);
    HitTemplate { dir: v4(d), attacker: Some(s), flags: c::pi32(w, s, 0x14) as u32, b18: c::pu8(w, s, 0x18), b19: c::pu8(w, s, 0x19), damage: Pf::f(c::pf(w, s, 0x1c)), w20: 1, ..Default::default() }
}

/// Level07 `0x30b6a8` (module doc).
pub fn shot_update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x58 { return; }
    let from = c::pos(w, id);
    let p = c::add(from, c::pv4(w, id, 0x30));
    c::set_pos(w, id, p);
    let drawn = w.m(id).visible != 0;
    if 30.0 < c::dist2(p, c::pv4(w, id, 0x40)) {
        if drawn {
            for _ in 0..3 {
                let r = [w.rng.randf(-1.0, 1.0), w.rng.randf(-1.0, 1.0), 0.0, 0.0];
                let z = w.rng.randf(-1.0, 1.0);
                let r = [r[0], r[1], z, 0.0];
                let d = c::add(r, c::set_len3(r, c::len3(r) * 0.1));
                let sp = w.rng.randf(DT + DT, DT * 4.0);
                let d = c::set_len3(d, sp);
                *w.svc.fx.part_spawns.entry(crate::particles::type51::TYPE).or_default() += 1;
                if let Some(sys) = w.particles.as_deref_mut() {
                    if crate::particles::type51::spawn(sys, p, d).is_none() { w.svc.fx.part_failed += 1; }
                }
            }
        }
        w.delete_moby(id);
        return;
    }
    if drawn || c::dist2(p, super::hero_pos(w)) < 30.0 {
        let turret = usize::try_from(c::pi32(w, id, 0x50) - 1).ok().filter(|&m| m < w.table.mobys.len());
        let flags = (c::pi32(w, id, 0x54) == 0) as u32;
        let tmpl = read_template(w, id);
        let h = crate::moby_update::services::line_hit_in(w.table, w.svc, w.classes, w.coll, v4(from), v4(p), flags, turret, &tmpl);
        if let Some(o) = h {
            let at = [o.point[0], o.point[1], o.point[2], p[3]];
            c::set_pos(w, id, at);
            if drawn {
                let vel = c::pv4(w, id, 0x30);
                let n = [o.normal[0], o.normal[1], o.normal[2], 0.0];
                for _ in 0..5 {
                    let r = [w.rng.randf(-1.0, 1.0), w.rng.randf(-1.0, 1.0), 0.0, 0.0];
                    let z = w.rng.randf(-1.0, 1.0);
                    let r = [r[0], r[1], z, 0.0];
                    let refl = crate::moby_update::services::reflect(v4(vel), v4(n)).map(|x| f32::from_bits(x.0));
                    let d = c::add(refl, c::set_len3(r, c::len3(refl) * 0.5));
                    let sp = w.rng.randf(DT * 3.0, DT * 6.0);
                    let d = c::set_len3(d, sp);
                    let (a, b) = (w.ticks(10), w.ticks(15));
                    let life = w.rng.rand_range(a, b);
                    fx::part27(w, 30000.0, at, d, 0x7f2f_4f6f, life);
                }
            }
            w.play_sound(0, 0, id);
            w.delete_moby(id);
            return;
        }
    }
    if !crate::moby_update::creature::projectile::in_world(c::pos(w, id)) { w.delete_moby(id); }
}
