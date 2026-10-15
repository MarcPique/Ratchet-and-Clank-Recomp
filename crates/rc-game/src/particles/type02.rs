//! Particle type 2, the three-phase trail blob (level01 spawner `PartType02Spawn` 0x27dc98, update 0x27de70): the
//! Bomb Glove's bomb trail (every 8th tick of its flight, `0x2c3300`) and the amoeboids' goo bursts (`0x2ef770`).
//!
//! **Record** (spawner): position at +0x10, colour 1 at +0x04 and +0x28, colour 2 at +0x2c, byte9 0x44 (near 1 u,
//! far 128 u), byte3 0x44, or 0x48 when the def word has bit 0x10000, byte1 0 (sprite), texture `*def[def & 0xffff]`
//! (`def` = −1: `*def[23]`), rotation byte `trunc(randf(0, 255))` (**one draw**, made when a record is taken), +0x0c 0,
//! the three phase lengths +0x30 / +0x32 / +0x34 (A, B, C; +0x0a = A), the two sizes `trunc(w·210)` of the two
//! velocity vectors at +0x36 / +0x38, and the two velocities **packed** into one word each at +0x20 / +0x24
//! ([`pack`], `0x276380`: 8 bits per axis around 127 on a shared scale).
//!
//! **Update** (every tick; the rotation byte +1 on odd ticks of 0x15f5cc):
//! * phase A (+0x30 ≠ 0): pos += v1; colour = `FastTweenColor(1 − t/A, c1 & 0xffffff, c1)` (fades in); size = s1·1000;
//! * phase B (+0x32 ≠ 0): v = v2 + (v1 − v2)·t/B; pos += v; size = ((s1 − s2)·t/B + s2)·1000; colour =
//!   `FastTweenColor(t/B, c2, c1)`;
//! * phase C: pos += v2; colour = `FastTweenColor(t/C, c2 & 0xffffff, c2)` (fades out); size = s2·1000.
//!
//! A phase ends when `FastDecTimer` fires: the next phase's length goes to +0x0a; the end of C kills the record.
//! Standard `f32`; no draws in the update.

use super::{fast_dec_timer, rec, tween_color, Particles};
use crate::rng::Rng;

/// `0x276380`: a vector as four bytes: the scale s = `clamp(trunc(max|v|·10000/63), 1, 255)` in the high byte and
/// each axis as `trunc(v/(s·0.0001) + 127)` (low byte of the truncated value).
pub fn pack(v: [f32; 3]) -> u32 {
    let m = v[0].abs().max(v[1].abs()).max(v[2].abs());
    let t = (m * 10000.0 / 63.0) as i32;
    let s = if t < 0x100 { t } else { 0xff };
    let s = if s > 0 { s } else { 1 };
    let unit = s as f32 * 1.0e-4;
    let inv = 1.0 / unit;
    let b = |x: f32| ((x * inv + 127.0) as i32 as u32) & 0xff;
    b(v[0]) | b(v[1]) << 8 | b(v[2]) << 16 | (s as u32) << 24
}

/// `0x2764a8`: the inverse of [`pack`], `(byte − 127)·s·0.0001` per axis.
pub fn unpack(w: u32) -> [f32; 3] {
    let unit = (w >> 24) as f32 * 1.0e-4;
    [0, 8, 16].map(|k| (((w >> k) & 0xff) as f32 - 127.0) * unit)
}

/// The spawner's arguments.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spawn {
    pub pos: [f32; 4],
    /// Velocity 1 (xyz) and size 1 (w).
    pub v1: [f32; 4],
    /// Velocity 2 (xyz) and size 2 (w).
    pub v2: [f32; 4],
    pub c1: u32,
    pub c2: u32,
    /// Phase lengths A, B, C.
    pub t: [i32; 3],
    /// The def word: −1 → `def[23]`; else `def[w & 0xffff]`, bit 0x10000 additive.
    pub def: i32,
}

/// `PartType02Spawn` 0x27dc98. `rng` makes the rotation draw (only with a record). None: the pool is full.
pub fn spawn(sys: &mut Particles, rng: &mut Rng, a: &Spawn) -> Option<usize> {
    let i = sys.create_part(2)?;
    let (def, additive) = if a.def == -1 { (sys.def_first(23), false) } else { (sys.def_first((a.def & 0xffff) as u8), a.def & 0x1_0000 != 0) };
    let rot = rng.randf(0.0, 255.0) as i32 as u8;
    let r = &mut sys.pool.recs[i];
    rec::set_v4(r, 0x10, a.pos);
    rec::set_u32(r, 4, a.c1);
    rec::set_ff(r, 0xc, 0.0);
    r[1] = 0;
    r[9] = 0x44;
    r[3] = if additive { 0x48 } else { 0x44 };
    r[8] = rot;
    r[2] = def;
    rec::set_i16(r, 0xa, a.t[0] as i16);
    rec::set_i16(r, 0x36, (a.v1[3] * 210000.0 / 1000.0) as i32 as i16);
    rec::set_i16(r, 0x38, (a.v2[3] * 210000.0 / 1000.0) as i32 as i16);
    rec::set_u32(r, 0x20, pack([a.v1[0], a.v1[1], a.v1[2]]));
    rec::set_u32(r, 0x24, pack([a.v2[0], a.v2[1], a.v2[2]]));
    rec::set_i16(r, 0x34, a.t[2] as i16);
    rec::set_u32(r, 0x28, a.c1);
    rec::set_u32(r, 0x2c, a.c2);
    rec::set_i16(r, 0x30, a.t[0] as i16);
    rec::set_i16(r, 0x32, a.t[1] as i16);
    Some(i)
}

fn tween(f: f32, a: u32, b: u32) -> u32 { tween_color(f.to_bits(), a, b) }

/// 0x27de70.
pub fn update(sys: &mut Particles, i: usize, _rng: &mut Rng) {
    let odd = sys.counter & 1 != 0;
    let r = &mut sys.pool.recs[i];
    if odd { r[8] = r[8].wrapping_add(1); }
    let t = rec::i16(r, 0xa) as f32;
    let add = |r: &mut super::Record, v: [f32; 3]| {
        let p = rec::v3(r, 0x10);
        rec::set_v3(r, 0x10, [p[0] + v[0], p[1] + v[1], p[2] + v[2]]);
    };
    let (c1, c2) = (rec::u32(r, 0x28), rec::u32(r, 0x2c));
    let (s1, s2) = (rec::i16(r, 0x36) as i32, rec::i16(r, 0x38) as i32);
    if rec::i16(r, 0x30) != 0 {
        add(r, unpack(rec::u32(r, 0x20)));
        let f = t / rec::i16(r, 0x30) as f32;
        rec::set_u32(r, 4, tween(1.0 - f, c1 & 0xff_ffff, c1));
        rec::set_ff(r, 0xc, (s1 * 1000) as f32);
        if fast_dec_timer(r, 0xa) != 0 {
            let b = rec::i16(r, 0x32);
            rec::set_i16(r, 0x30, 0);
            rec::set_i16(r, 0xa, b);
        }
    } else if rec::i16(r, 0x32) != 0 {
        let (v1, v2) = (unpack(rec::u32(r, 0x20)), unpack(rec::u32(r, 0x24)));
        let f = t / rec::i16(r, 0x32) as f32;
        add(r, [0, 1, 2].map(|k| v2[k] + (v1[k] - v2[k]) * f));
        rec::set_ff(r, 0xc, ((s1 - s2) as f32 * f + s2 as f32) * 1000.0);
        rec::set_u32(r, 4, tween(f, c2, c1));
        if fast_dec_timer(r, 0xa) != 0 {
            let c = rec::i16(r, 0x34);
            rec::set_i16(r, 0x32, 0);
            rec::set_i16(r, 0xa, c);
        }
    } else {
        add(r, unpack(rec::u32(r, 0x24)));
        let f = t / rec::i16(r, 0x34) as f32;
        rec::set_u32(r, 4, tween(f, c2 & 0xff_ffff, c2));
        rec::set_ff(r, 0xc, (s2 * 1000) as f32);
        if fast_dec_timer(r, 0xa) != 0 { sys.kill_part(i); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_round_trip_and_scale() {
        let v = [0.02, -0.01, 0.005];
        let w = pack(v);
        // max 0.02 → s = trunc(200/63) = 3, unit 0.0003.
        assert_eq!(w >> 24, 3);
        let u = unpack(w);
        for k in 0..3 { assert!((u[k] - v[k]).abs() <= 0.0003 + 1e-7, "{u:?} vs {v:?}"); }
        // A zero vector keeps scale 1 and unpacks to zero.
        assert_eq!(unpack(pack([0.0; 3])), [0.0; 3]);
    }

    #[test]
    fn three_phases_then_death() {
        let mut sys = Particles::new(None, Vec::new());
        let mut rng = Rng::new();
        let a = Spawn { pos: [10.0, 10.0, 10.0, 0.0], v1: [0.01, 0.0, 0.0, 0.2], v2: [0.0, 0.0, 0.01, 0.1], c1: 0x80ff_ffff, c2: 0x8000_00ff, t: [2, 4, 3], def: 0x1_0019 };
        let before = rng.state;
        let i = spawn(&mut sys, &mut rng, &a).unwrap();
        let mut r2 = Rng { state: before };
        r2.rand();
        assert_eq!(rng.state, r2.state, "one draw at the spawn");
        assert_eq!(sys.pool.recs[i][3], 0x48);
        let mut n = 0;
        while sys.pool.count > 0 {
            sys.counter += 1;
            sys.update_parts(&mut rng);
            n += 1;
            assert!(n < 20);
        }
        // A: 2 updates, B: 4, C: 3 (each phase's last update is the one where the timer fires).
        assert_eq!(n, 2 + 4 + 3);
        let p = rec::pos(&sys.pool.recs[i]);
        assert!(p[0] > 10.0 && p[2] > 10.0, "{p:?}");
        assert_eq!(rng.state, r2.state, "no draws in the update");
    }
}
