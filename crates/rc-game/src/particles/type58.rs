//! Particle type 58, the ground fire (the Snagglebeast's fire sweep, level 7: `units::umbris_beast_fx`), read from
//! the level01 code (the same on every level):
//!
//! * spawner [`spawn`] = `PartType58Spawn(damage, speed, pos, kind, life, emit, count, flame, owner)` 0x287e70: render
//!   kind 0, blend 0x44, byte9 0x70, size 21000, texture `def[58][rand() & 7]`, byte8 `rand()`; +0x30 kind, +0x31
//!   life, +0x32 emit, +0x33 count, +0x35 flame, +0x38 the owner, +0x3c the damage. Kinds 0 / 1: timer = life,
//!   RGBA 0xff808080, phase 0. Kind 2 (a flame): velocity `polar(randf(3, 9)·speed·dt, rand_angle, randf(1.344,
//!   1.798))`, size 42000, alpha `(rand() & 0x1f) + 0x10` (+0x36), timer = flame, phase 2;
//! * [`update`] = 0x288068 by phase (+0x34):
//!   * 0, the glow: colour `FastTweenColor((life − t)/life, 0xff4080ff, 0)`; the timer out → kind 1 killed, else
//!     phase 1 with timer = emit;
//!   * 1, the emitter: `count` flames (kind 2) at the point jittered 0.05 (`0x2747a0`), their speed 1 or t /
//!     `ticks(10)` in the last ten ticks; the timer out → killed (the rest still runs); Ratchet within 0.3 (xy) and
//!     below the point + 1.2: `0x26eaa8(damage, hero, owner, 1, point, unit(hero − point) with z 1)` — queued in
//!     [`Particles::hits`] and delivered by the moby loop's next tick [L: the game writes the record at once; the
//!     victim reads it on its next update either way] (G-PRT-008);
//!   * 2, a flame: pos += v, colour `FastTweenColor(f, alpha << 24 | 0x4060, 0x8fafff)` (f = (flame − t)/flame),
//!     size + 0.043·210000·speed while f < 0.6 else − 0.08·210000·speed, texture `def[58][rand() & 7]`, byte8 ∓ 1 by
//!     the record's slot parity, the timer out → killed.

use super::{fast_dec_timer, rec, tween_color, Particles};
use crate::rng::Rng;

pub const TYPE: u8 = 58;
/// 0x15ed6c (NTSC).
const DT: f32 = 1.0 / 60.0;

/// The spawner's arguments.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spawn {
    pub damage: f32,
    pub speed: f32,
    pub pos: [f32; 4],
    pub kind: u8,
    pub life: u8,
    pub emit: u8,
    pub count: u8,
    pub flame: u8,
    /// The owner moby + 1 (0 none).
    pub owner: u32,
}

/// A hit the emitter phase gives Ratchet (`0x26eaa8(damage, hero, owner, 1, pos, dir)`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub damage: f32,
    /// The owner moby + 1 (0 none).
    pub owner: u32,
    pub pos: [f32; 4],
    pub dir: [f32; 4],
}

/// `PartType58Spawn` 0x287e70 (module doc). None when the pool is full (no draws).
pub fn spawn(sys: &mut Particles, rng: &mut Rng, s: &Spawn) -> Option<usize> {
    let i = sys.create_part(TYPE)?;
    let def = sys.def_frame(TYPE, (rng.rand() & 7) as usize);
    let b8 = rng.rand() as u8;
    let r = &mut sys.pool.recs[i];
    rec::set_v4(r, 0x10, s.pos);
    r[1] = 0;
    r[3] = 0x44;
    r[9] = 0x70;
    rec::set_ff(r, 0xc, 21000.0);
    r[2] = def;
    r[8] = b8;
    r[0x30] = s.kind;
    r[0x32] = s.emit;
    r[0x33] = s.count;
    rec::set_u32(r, 0x38, s.owner);
    rec::set_ff(r, 0x3c, s.damage);
    r[0x31] = s.life;
    r[0x35] = s.flame;
    if s.kind < 2 {
        rec::set_i16(r, 10, s.life as i16);
        rec::set_u32(r, 4, 0xff80_8080);
        r[0x34] = 0;
    } else if s.kind == 2 {
        let len = rng.randf(3.0, 9.0) * s.speed * DT;
        let yaw = rng.rand_angle();
        let pitch = rng.randf(f32::from_bits(0x3fac_0508), f32::from_bits(0x3fe6_1aad));
        let a = (rng.rand() & 0x1f) + 0x10;
        let r = &mut sys.pool.recs[i];
        rec::set_v3(r, 0x20, crate::targeting::polar(len, yaw, pitch));
        rec::set_ff(r, 0xc, 42000.0);
        r[0x36] = a as u8;
        rec::set_i16(r, 10, s.flame as i16);
        rec::set_u32(r, 4, (a as u32) << 24 | 0x80_8080);
        r[0x34] = 2;
    }
    Some(i)
}

/// Update 0x288068 (module doc).
pub fn update(sys: &mut Particles, i: usize, rng: &mut Rng) {
    let speed = f32::from_bits(sys.time.speed);
    match sys.pool.recs[i][0x34] {
        0 => {
            let r = &mut sys.pool.recs[i];
            let life = r[0x31] as f32;
            let t = rec::i16(r, 10) as f32;
            rec::set_u32(r, 4, tween_color(((life - t) / life).to_bits(), 0xff40_80ff, 0));
            if fast_dec_timer(r, 10) != 0 {
                if r[0x30] == 1 {
                    sys.kill_part(i);
                } else {
                    r[0x34] = 1;
                    rec::set_i16(r, 10, r[0x32] as i16);
                }
            }
        }
        1 => {
            let r = sys.pool.recs[i];
            let at = rec::v4(&r, 0x10);
            let t10 = sys.time.ticks(10);
            for _ in 0..r[0x33] {
                let mut p = at;
                for c in p.iter_mut().take(3) { *c += rng.randf(-0.05, 0.05); }
                let t = rec::i16(&r, 10) as i32;
                let f = if t < t10 { t as f32 / t10 as f32 } else { 1.0 };
                let s = Spawn { damage: rec::ff(&r, 0x3c), speed: f, pos: p, kind: 2, life: r[0x31], emit: r[0x32], count: r[0x33], flame: r[0x35], owner: rec::u32(&r, 0x38) };
                spawn(sys, rng, &s);
            }
            if fast_dec_timer(&mut sys.pool.recs[i], 10) != 0 { sys.kill_part(i); }
            let h = sys.hero;
            let (dx, dy) = (h[0] - at[0], h[1] - at[1]);
            if (dx * dx + dy * dy).sqrt() < 0.3 && h[2] < at[2] + 1.2 {
                let d = [dx, dy, h[2] - at[2]];
                let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                let k = if l == 0.0 { 0.0 } else { 1.0 / l };
                let dir = [d[0] * k, d[1] * k, 1.0, 0.0];
                sys.hits.push(Hit { damage: rec::ff(&r, 0x3c), owner: rec::u32(&r, 0x38), pos: at, dir });
            }
        }
        2 => {
            let def = sys.def_frame(TYPE, (rng.rand() & 7) as usize);
            let odd = i & 1 != 0;
            let r = &mut sys.pool.recs[i];
            let (p, v) = (rec::v3(r, 0x10), rec::v3(r, 0x20));
            rec::set_v3(r, 0x10, [p[0] + v[0], p[1] + v[1], p[2] + v[2]]);
            let flame = r[0x35] as f32;
            let f = (flame - rec::i16(r, 10) as f32) / flame;
            rec::set_u32(r, 4, tween_color(f.to_bits(), (r[0x36] as u32) << 24 | 0x4060, 0x8f_afff));
            let size = rec::ff(r, 0xc);
            rec::set_ff(r, 0xc, if f < 0.6 { size + speed * 0.043 * 210000.0 } else { size - speed * 0.08 * 210000.0 });
            r[2] = def;
            r[8] = if odd { r[8].wrapping_add(1) } else { r[8].wrapping_sub(1) };
            if fast_dec_timer(r, 10) != 0 { sys.kill_part(i); }
        }
        _ => {}
    }
}
