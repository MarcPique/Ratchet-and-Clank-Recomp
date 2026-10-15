//! **The flame emitter** of the creatures that spray fire: level02 `0x264e70` (Aridia's flamer 612,
//! `units::aridia_flamer`) and its byte-identical level16 copy `0x25eab8` (Kalebo's 541, `units::kalebo_trooper`), with
//! the same numbers (gp−0x6b30.., 0x1600d0 on both levels: 16, 2, 2, 1, 3, the Pyrocitor's), their clear `0x264e40` /
//! `0x25ea88` and hits `0x2651d0` / `0x25ee18`. The caller keeps a **record** in its pvars: seven slots of kept flames
//! (the particle record + 1 here, a pointer in the game) and the s16 smoke timer at +0x1c.
//!
//! **The emitter** `(range, flames, start, dir)`: the length along `dir` (a `CollLine_Fix` to `range + 0.75`, flags 2:
//! the hit less 0.75, at least 0), the launch speed (`0x25df98` = `0x270830`: `crate::hero::pyrocitor::launch_speed`), 2
//! glow puffs (1 when shorter than 4) and 2 flames of type 12 a tick, each `rand_vec(0, (3 − 2·len/range)·dt)` + the
//! launch step, at `randf(0, 1)` of the step; every 4th tick the third is kept (the first of seven free slots); the smoke
//! timer → `randf(10, 30)` ticks, a type-25 spark from `start` along `step·randf(0.5, 0.1) + rand_vec(0, 2·dt)`. [`emit`].
//! **The hits** `(flames, m, tmpl)`: each kept record still a type-12 particle and alive hits a sphere of
//! `size / 210000 / 4` at its position (`coll_sphere_mobys(…, 0, m, tmpl)`), dropped below life 5; others dropped.
//! [`hits`]. **The clear**: the slots and the timer 0. [`clear`].

use crate::moby_runtime::MobyId;
use crate::moby_update::creature as c;
use crate::moby_update::services::{pf, pv as v4, HitTemplate, World};
use crate::ps2v::Pf;

/// The record: seven slots, then the smoke timer.
pub const SLOTS: usize = 7;
pub const SMOKE: usize = 0x1c;
pub const SIZE: usize = 0x20;
/// The emitter's numbers (0x1600d0: gp−0x6b2c / −0x6b28 glow puffs / flames, −0x6b24 / −0x6b20 the spread).
const GLOWS: i32 = 2;
const FLAMES: i32 = 2;
const SPREAD: [f32; 2] = [1.0, 3.0];

/// `0x264e40(flames)`: the slots and the smoke timer cleared.
pub fn clear(w: &mut World, id: MobyId, rec: usize) {
    for k in 0..SLOTS { c::set_pi32(w, id, rec + 4 * k, 0); }
    c::set_pi16(w, id, rec + SMOKE, 0);
}

/// `0x264e70(range, flames, start, dir)` (module doc); `rec` = the pvar offset of the flames' record.
pub fn emit(w: &mut World, id: MobyId, rec: usize, range: f32, start: c::V, dir: c::V) {
    let keep = w.counter & 3 == 0;
    let reach = c::set_len3(dir, range + 0.75);
    let end = c::add(reach, start);
    let len = match w.coll_line(v4(start), v4(end), 2, None) {
        Some(o) => {
            let d = c::dist3(start, [o.point[0], o.point[1], o.point[2], 0.0]) - 0.75;
            if 0.0 <= d { d } else { 0.0 }
        }
        None => range,
    };
    let speed = crate::hero::pyrocitor::launch_speed(len);
    let step = c::set_len3(reach, speed);
    let glows = if len < 4.0 { 1 } else { GLOWS };
    let spread = ((SPREAD[0] - SPREAD[1]) * (len / range) + SPREAD[1]) * c::DT;
    for i in 0..FLAMES + glows {
        let r = w.rng.rand_vec(0.0, spread);
        let vel = [r[0] + step[0], r[1] + step[1], r[2] + step[2], step[3]];
        let k = w.rng.randf(0.0, 1.0);
        let p = c::add(c::scale(step, k), start);
        let flags = if i < glows { 1 } else { 0 };
        let kept = part12(w, len, p, vel, flags);
        if i == 2 && keep {
            if let Some(r) = kept {
                let slot = (0..7).find(|&s| c::pi32(w, id, rec + 4 * s) == 0);
                if let Some(s) = slot { c::set_pi32(w, id, rec + 4 * s, r as i32 + 1); }
            }
        }
    }
    if c::dec_timer_pvar_s16(w, id, rec + SMOKE) != 0 {
        let f = w.rng.randf(10.0, 30.0);
        let t = w.svc.timing.scale(Pf::f(f)).to_i32();
        c::set_pi16(w, id, rec + SMOKE, t as i16);
        let r = w.rng.rand_vec(0.0, c::DT + c::DT);
        let k = w.rng.randf(0.5, 0.1);
        let v = [step[0] * k + r[0], step[1] * k + r[1], step[2] * k + r[2], 0.0];
        part25(w, start, v);
    }
}

/// `PartType12Spawn(len, pos, vel, flags)` (its draws only with a free record). The record index.
fn part12(w: &mut World, len: f32, p: c::V, vel: c::V, flags: u8) -> Option<usize> {
    *w.svc.fx.part_spawns.entry(12).or_default() += 1;
    match w.particles.as_deref_mut() {
        Some(sys) => {
            if 0x800 <= sys.pool.hint { w.svc.fx.part_failed += 1; return None; }
            let d = crate::particles::type12::Draws::draw(w.rng, flags);
            let r = crate::particles::type12::spawn(sys, len, p, vel, flags, 0, &d).map(|x| x.0);
            if r.is_none() { w.svc.fx.part_failed += 1; }
            r
        }
        None => {
            crate::particles::type12::Draws::draw(w.rng, flags);
            None
        }
    }
}

/// `PartType25Spawn(pos, vel, 0)` (its size draw `randf(5000, 30000)` only with a free record).
fn part25(w: &mut World, p: c::V, vel: c::V) {
    *w.svc.fx.part_spawns.entry(25).or_default() += 1;
    match w.particles.as_deref_mut() {
        Some(sys) => {
            if 0x800 <= sys.pool.hint { w.svc.fx.part_failed += 1; return; }
            let size = w.rng.randf(5000.0, 30000.0);
            if crate::particles::type25::spawn(sys, p, vel, false, size).is_none() { w.svc.fx.part_failed += 1; }
        }
        None => { w.rng.randf(5000.0, 30000.0); }
    }
}

/// `0x2651d0(flames, m, tmpl)`: the kept flames' hits (module doc).
pub fn hits(w: &mut World, id: MobyId, rec: usize, t: &HitTemplate) {
    for s in 0..7 {
        let o = rec + 4 * s;
        let v = c::pi32(w, id, o);
        if v == 0 { continue; }
        let i = (v - 1) as usize;
        let rec = w.particles.as_deref().and_then(|p| p.pool.recs.get(i).copied());
        let Some(r) = rec else { c::set_pi32(w, id, o, 0); continue };
        if r[0] != 12 || (r[1] as i8) < 0 {
            c::set_pi32(w, id, o, 0);
            continue;
        }
        let f = |k: usize| f32::from_le_bytes([r[k], r[k + 1], r[k + 2], r[k + 3]]);
        let size = f(0xc);
        let centre = [f(0x10), f(0x14), f(0x18), f(0x1c)];
        w.sphere_mobys(pf(size / 210_000.0 * 0.25), v4(centre), 0, Some(id), Some(t));
        let life = i16::from_le_bytes([r[0xa], r[0xb]]);
        if life < 5 { c::set_pi32(w, id, o, 0); }
    }
}
