//! Particle type 40, the flame line (Orxon's flame vents 702, `units::orxon_vent_flame`), read from the level01 code
//! (`PartType40Spawn` 0x284d88, `PartType40Update` 0x284f70; the same on every level).
//!
//! **Spawn** `(pos, vel, flags, owner)`: position, velocity, RGBA `0x270ea0(0.7, 0.7, 0.7, 0.08)`, byte1 0, ALPHA 0x44,
//! byte9 `trunc(4) + 0x40`, the phase +0x30 = `rand_angle` (byte8 its ·256), texture `def[12][rand() & 7]`, flags
//! +0x38, the counts +0x34 / +0x36 = 0, the owner +0x3c. Flags below 5 (a fresh flame): size `randf(7000, 10000)`,
//! the split timer +0x0a = n − `rand() % (6n/7)` (n = `trunc(8 / (speed·0.443))` = 18); else size 10000, timer n.
//!
//! **Update**: the split timer running out (+0x38 |= 0x88): the size ·0.8 and a child flame (flags 0x88) at the point
//! jittered by size/900000, its velocity the parent's ·`randf(0.5, 0.7)` ± 20% (z only down), timer `ticks(2)`, the
//! parent's size; the parent's own velocity likewise and its point jittered. Each tick: the texture steps through
//! `def[12]`; the line from the point (the point − velocity after `ticks(3)` of age: +0x34, which nothing advances)
//! to the point + velocity is tested (`CollLine_Fix`): a fresh flame with the hit template (its velocity, zeroed with
//! flag 0x40; the owner, flags 0x10001, damage 1), a spent one (flag 8) with flags 4 and none (killed once its alpha
//! is 0); clear → it moves; a hit: the velocity reflected ·0.02, the point 0.055 off the face, a fresh one's size
//! doubled under 200000, spent. Fresh: the size grows by `260000/n·speed`; spent: +0x36 counts up, the size shrinks
//! by 1.25× that (killed at 0). Alpha `trunc(f·256)` with f = `0.08/n·speed·(age/n) + 0.08` in [0, 0.08], spent ÷
//! `(count/7 + 1)`; v.z += 0.008 (twice when spent); after `ticks(25)` of age or spent the xy velocity ·0.95; the
//! phase turns by ∓0.01·speed (÷ count when spent), byte8 from it.
//!
//! The line tests the world mesh here ([`Particles::coll`]); the mobys it would hit get the hit through
//! [`Particles::lines`], run against the moby scene by the moby loop's next tick (G-PRT-008) [L: the flame passes
//! through them instead of reflecting].

use super::{fast_dec_timer, rec, Particles};
use crate::rng::Rng;

pub const TYPE: u8 = 40;
/// The texture table (`def[12]`, 0x1b2530 on level 01).
const DEF: u8 = 12;

/// A fresh flame's line for the moby loop: `CollLine_Fix(a, b, 0, owner, {dir, owner, 0x10001, 1.0})`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line {
    pub a: [f32; 4],
    pub b: [f32; 4],
    pub dir: [f32; 4],
    /// The owner moby + 1 (0 none).
    pub owner: u32,
}

fn n(sys: &Particles) -> i32 { (8.0 / (f32::from_bits(sys.time.speed) * 0.443)) as i32 }

/// `PartType40Spawn(pos, vel, flags, owner)` 0x284d88 (module doc); `owner` is the moby + 1.
pub fn spawn(sys: &mut Particles, rng: &mut Rng, pos: [f32; 4], vel: [f32; 4], flags: u8, owner: u32) -> Option<usize> {
    let i = sys.create_part(TYPE)?;
    let rgba = super::type25::pack_rgba([0.7, 0.7, 0.7, f32::from_bits(0x3da3_d70a)]);
    let a = rng.rand_angle();
    let frame = sys.def_frame(DEF, (rng.rand() & 7) as usize);
    let nn = n(sys);
    let (size, timer) = if flags < 5 {
        let s = rng.randf(7000.0, 10000.0);
        let k = rng.rand();
        (s, nn - k % ((nn * 6) / 7).max(1))
    } else {
        (10000.0, nn)
    };
    let r = &mut sys.pool.recs[i];
    rec::set_v4(r, 0x10, pos);
    rec::set_v4(r, 0x20, vel);
    rec::set_u32(r, 4, rgba);
    r[1] = 0;
    r[3] = 0x44;
    r[9] = 4 + 0x40;
    rec::set_ff(r, 0x30, a);
    r[8] = (a * 256.0) as i32 as u8;
    r[2] = frame;
    r[0x38] = flags;
    rec::set_i16(r, 0x34, 0);
    rec::set_i16(r, 0x36, 0);
    rec::set_ff(r, 0xc, size);
    rec::set_i16(r, 10, timer as i16);
    rec::set_u32(r, 0x3c, owner);
    Some(i)
}

/// `0x221570`: `v` reflected off the face normal `nrm` (`moby_update::services::reflect`).
fn reflect(v: [f32; 4], nrm: [f32; 3]) -> [f32; 4] {
    use crate::moby_update::services::{pv, reflect};
    reflect(pv(v), pv([nrm[0], nrm[1], nrm[2], 0.0])).map(|x| f32::from_bits(x.0))
}

/// `PartType40Update` 0x284f70 (module doc).
pub fn update(sys: &mut Particles, i: usize, rng: &mut Rng) {
    let speed = f32::from_bits(sys.time.speed);
    let nn = n(sys);
    let t3 = sys.time.ticks(3);
    let t25 = sys.time.ticks(0x19);
    let t2 = sys.time.ticks(2);
    // The split.
    let timer = rec::i16(&sys.pool.recs[i], 10);
    if timer != 0 && fast_dec_timer(&mut sys.pool.recs[i], 10) != 0 {
        let r = &mut sys.pool.recs[i];
        r[0x38] |= 0x88;
        let size = rec::ff(r, 0xc);
        let j = size / 900_000.0;
        rec::set_ff(r, 0xc, size * 0.8);
        let (pos, vel, owner) = (rec::v4(r, 0x10), rec::v4(r, 0x20), rec::u32(r, 0x3c));
        let k = rng.randf(0.5, f32::from_bits(0x3f33_3333));
        let mut cv = vel.map(|x| x * k);
        let mut cp = pos;
        for c in cp.iter_mut().take(3) { *c += rng.randf(-j, j); }
        cv[0] += rng.randf(-vel[0] * 0.2, vel[0] * 0.2);
        cv[1] += rng.randf(-vel[1] * 0.2, vel[1] * 0.2);
        cv[2] += rng.randf(-vel[2] * 0.2, 0.0);
        let child_size = rec::ff(&sys.pool.recs[i], 0xc);
        if let Some(c) = spawn(sys, rng, cp, cv, 0x88, owner) {
            rec::set_i16(&mut sys.pool.recs[c], 10, t2 as i16);
            rec::set_ff(&mut sys.pool.recs[c], 0xc, child_size);
        }
        let k = rng.randf(0.5, f32::from_bits(0x3f33_3333));
        let mut v = vel.map(|x| x * k);
        v[0] += rng.randf(-v[0] * 0.2, v[0] * 0.2);
        v[1] += rng.randf(-v[1] * 0.2, v[1] * 0.2);
        v[2] += rng.randf(-v[2] * 0.2, 0.0);
        let mut p = pos;
        for c in p.iter_mut().take(3) { *c += rng.randf(-j, j); }
        let r = &mut sys.pool.recs[i];
        rec::set_v4(r, 0x20, v);
        rec::set_v4(r, 0x10, p);
    }
    let d0 = sys.def_frame(DEF, 0);
    let (count, timer) = (rec::i16(&sys.pool.recs[i], 0x36) as i32, rec::i16(&sys.pool.recs[i], 10) as i32);
    let k = (timer + count + (sys.pool.recs[i][2] as i32 - d0 as i32)) & 7;
    sys.pool.recs[i][2] = sys.def_frame(DEF, k as usize);
    let r = sys.pool.recs[i];
    let (pos, mut vel) = (rec::v4(&r, 0x10), rec::v4(&r, 0x20));
    let age = rec::i16(&r, 0x34) as i32;
    let new = [pos[0] + vel[0], pos[1] + vel[1], pos[2] + vel[2], pos[3] + vel[3]];
    let start = if t3 < age { [pos[0] - vel[0], pos[1] - vel[1], pos[2] - vel[2], pos[3] - vel[3]] } else { pos };
    let mut flags = r[0x38];
    let fresh = flags & 8 == 0;
    if fresh {
        let dir = if flags & 0x40 != 0 { [0.0; 4] } else { vel };
        sys.lines.push(Line { a: start, b: new, dir, owner: rec::u32(&r, 0x3c) });
    } else if rec::u32(&r, 4) & 0xff00_0000 == 0 {
        sys.kill_part(i);
        return;
    }
    let qf = if fresh { 0 } else { 4 };
    let hit = sys.coll.as_deref().and_then(|c| {
        crate::collision_query::coll_line_m(c, None, [start[0], start[1], start[2]], [new[0], new[1], new[2]], crate::collision_query::QueryFlags(qf), None)
    });
    let mut size = rec::ff(&r, 0xc);
    let mut p = new;
    if let Some(h) = hit {
        let l = (h.normal[0] * h.normal[0] + h.normal[1] * h.normal[1] + h.normal[2] * h.normal[2]).sqrt().max(f32::MIN_POSITIVE);
        let u = h.normal.map(|x| x / l);
        vel = reflect(vel, h.normal).map(|x| x * f32::from_bits(0x3ca3_d70a));
        let off = f32::from_bits(0x3d61_47ae);
        p = [h.point[0] + u[0] * off, h.point[1] + u[1] * off, h.point[2] + u[2] * off, pos[3]];
        if size < 200_000.0 && fresh { size += size; }
        flags |= 8;
    }
    let grow = (260_000.0 / nn as f32) * speed;
    let mut count = count;
    if flags & 8 == 0 {
        size += grow;
    } else {
        count += 1;
        size -= grow * 1.25;
        if size <= 0.0 {
            sys.kill_part(i);
            return;
        }
    }
    let nf = nn as f32;
    let mut f = ((0.08 / nf) * speed * (age as f32 / nf) + 0.08).clamp(0.0, 0.08);
    if flags & 8 != 0 { f /= count as f32 / 7.0 + 1.0; }
    let rgba = (rec::u32(&r, 4) & 0xff_ffff) | ((f * 256.0) as i32 as u32) << 24;
    vel[2] += 0.008;
    if t25 < age || flags & 8 != 0 {
        vel[0] *= 0.95;
        vel[1] *= 0.95;
    }
    let s = if flags & 1 != 0 { 1.0 } else { -1.0 };
    let mut ph = rec::ff(&r, 0x30);
    if flags & 8 == 0 {
        ph += s * speed * -0.01;
    } else {
        vel[2] += 0.008;
        ph += (s * speed * -0.01) / count as f32;
    }
    if 1.0 < ph { ph -= 1.0; }
    if ph < 0.0 { ph += 1.0; }
    let r = &mut sys.pool.recs[i];
    rec::set_v4(r, 0x10, p);
    rec::set_v4(r, 0x20, vel);
    rec::set_ff(r, 0xc, size);
    rec::set_i16(r, 0x36, count as i16);
    rec::set_u32(r, 4, rgba);
    rec::set_ff(r, 0x30, ph);
    r[8] = (ph * 256.0) as i32 as u8;
    r[0x38] = flags;
}
