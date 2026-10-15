//! Particle type 53, the bolt-pickup sparkle (`CollectBolt` 0x2bc4f0 spawns two), read from the level01
//! disassembly:
//!
//! * spawner `PartType53Spawn(s12, s13, s14, pos, life, rgba, a3, t0, vel)` 0x287328 — ported in
//!   `moby_update::services::World::part53`: kind-0 sprite, byte9 0x20 (near 0, far 64 u), additive (0x48),
//!   texture `def[53][0]`, z + 0.05, size `s12·210000` at +0xc and +0x20, `s13·210000` at +0x24, rotation 0 /
//!   0x20 / `randi(255)` for a3 = 0 / 1 / other, +0x28 = life (s16), +0x2a spin step t0, +0x2b the start alpha
//!   (`rgba >> 24`), +0x2c..0x34 velocity, +0x38 z deceleration s14;
//! * [`update`] = 0x2874b8.

use super::{fast_dec_timer, lt, rec, Particles};
use crate::ps2v::{self, F};
use crate::rng::Rng;

const HALF: F = 0x3f00_0000;
const FIVE: F = 0x40a0_0000;
const K2: F = 0x4000_0000;
const K1021: F = 0x447f_4000;

/// The record `PartType53Spawn` 0x287328 fills (`b8` = the rotation: 0 / 0x20 / `randi(0xff)` for a3 = 0 / 1 /
/// other, drawn by the caller), for the record `r` of type 53 with texture `def` = `def[53][0]`.
#[allow(clippy::too_many_arguments)]
pub fn fill(r: &mut super::Record, def: u8, s12: F, s13: F, s14: F, pos: [F; 4], life: i32, rgba: u32, b8: u8, t0: i8, vel: [F; 3]) {
    let k: F = 0x484d_1400;
    for (c, &p) in pos.iter().enumerate() { rec::set_f(r, 0x10 + 4 * c, p); }
    rec::set_u32(r, 4, rgba);
    r[3] = 0x48;
    rec::set_f(r, 0x18, ps2v::add(pos[2], 0x3d4c_cccd));
    r[1] = 0;
    r[9] = 0x20;
    r[2] = def;
    rec::set_i16(r, 0xa, life as i16);
    let sz = ps2v::mul(s12, k);
    rec::set_f(r, 0xc, sz);
    r[8] = b8;
    rec::set_i16(r, 0x28, life as i16);
    rec::set_f(r, 0x20, sz);
    r[0x2a] = t0 as u8;
    rec::set_f(r, 0x24, ps2v::mul(s13, k));
    r[0x2b] = (rgba >> 24) as u8;
    rec::set_f(r, 0x38, s14);
    for (c, &v) in vel.iter().enumerate() { rec::set_f(r, 0x2c + 4 * c, v); }
}

/// `PartType53Spawn` 0x287328 with its rotation `b8` already drawn (a3 0 / 1 draw nothing). None when the pool is
/// full.
#[allow(clippy::too_many_arguments)]
pub fn spawn(sys: &mut Particles, s12: F, s13: F, s14: F, pos: [F; 4], life: i32, rgba: u32, b8: u8, t0: i8, vel: [F; 3]) -> Option<usize> {
    let i = sys.create_part(0x35)?;
    let def = sys.def_first(0x35);
    fill(&mut sys.pool.recs[i], def, s12, s13, s14, pos, life, rgba, b8, t0, vel);
    Some(i)
}

/// Update 0x2874b8 (no RNG). With a = +0x20 (spawn size), b = +0x24, N = +0x2b, L = +0x28 (life), t = timer and
/// d = L − t:
/// * d < 6 (the first five updates): f = d/5, size = `b + ((a + b)·0.5 − b)·f`, A = `N + (N/2 − N)·f` (b → the
///   mean, N → N/2);
/// * else f = t/(L − 6): size = `a + ((a + b)·0.5 − a)·f`, A = `(N >> 1)·f + 0` (the mean → a, N/2 → 0);
///
/// then A = `trunc(A)` into the alpha byte, byte8 += byte 0x2a, pos += vel (the old vel), vel.z −= +0x38, and
/// `KillPart` when any coordinate is outside [2, 1021] (checked first) or on the timer.
pub fn update(sys: &mut Particles, i: usize, _rng: &mut Rng) {
    let r = &mut sys.pool.recs[i];
    r[8] = r[8].wrapping_add(r[0x2a]);
    let life = rec::i16(r, 0x28) as i32;
    let d = life - rec::i16(r, 0xa) as i32;
    let (a, b) = (rec::f(r, 0x20), rec::f(r, 0x24));
    let n = r[0x2b] as i32;
    let alpha = if d < 6 {
        let f = ps2v::div(ps2v::itof0(d), FIVE);
        let m = ps2v::sub(ps2v::mul(ps2v::add(a, b), HALF), b);
        rec::set_f(r, 0xc, ps2v::add(b, ps2v::mul(m, f)));
        ps2v::add(ps2v::itof0(n), ps2v::mul(ps2v::itof0((n >> 1) - n), f))
    } else {
        let f = ps2v::div(ps2v::itof0(rec::i16(r, 0xa) as i32), ps2v::itof0(life - 6));
        let m = ps2v::sub(ps2v::mul(ps2v::add(a, b), HALF), a);
        rec::set_f(r, 0xc, ps2v::add(a, ps2v::mul(m, f)));
        ps2v::add(ps2v::mul(ps2v::itof0(n >> 1), f), 0)
    };
    let al = ps2v::ftoi0(alpha) as u32;
    rec::set_u32(r, 4, al << 24 | (rec::u32(r, 4) & 0xff_ffff));
    let v = [rec::f(r, 0x2c), rec::f(r, 0x30), rec::f(r, 0x34)];
    for (k, vk) in v.iter().enumerate() { rec::set_f(r, 0x10 + 4 * k, ps2v::add(rec::f(r, 0x10 + 4 * k), *vk)); }
    rec::set_f(r, 0x34, ps2v::sub(rec::f(r, 0x34), rec::f(r, 0x38)));
    let p = [rec::f(r, 0x10), rec::f(r, 0x14), rec::f(r, 0x18)];
    let out = p.iter().any(|&x| lt(x, K2) || lt(K1021, x));
    if out || fast_dec_timer(r, 0xa) != 0 { sys.kill_part(i); }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The first sparkle of `CollectBolt` with g = 0.45: size 0.2·g·210000 → g·210000, rgba 0x7f207f7f, spin +1,
    /// life 25, velocity (0.01, −0.005, 0.012).
    fn sparkle(s: &mut Particles) -> usize {
        let i = s.create_part(53).unwrap();
        let r = &mut s.pool.recs[i];
        let k = 210000f32;
        let (a, b) = (0.2 * 0.45 * k, 0.45 * k);
        for (o, v) in [(0x10, 100.0f32), (0x14, 200.0), (0x18, 30.05), (0x1c, 0.0), (0xc, a), (0x20, a), (0x24, b), (0x2c, 0.01), (0x30, -0.005), (0x34, 0.012), (0x38, 0.0)] {
            rec::set_f(r, o, v.to_bits());
        }
        rec::set_u32(r, 4, 0x7f20_7f7f);
        r[3] = 0x48;
        r[9] = 0x20;
        rec::set_i16(r, 0xa, 25);
        rec::set_i16(r, 0x28, 25);
        r[0x2a] = 1;
        r[0x2b] = 0x7f;
        i
    }

    #[test]
    fn sparkle_shrinks_and_fades_over_25_updates() {
        let mut s = Particles::new(None, Vec::new());
        let i = sparkle(&mut s);
        let mut rng = Rng::new();
        rng.srand(1234);
        let (a, b) = (0.2f64 * 0.45 * 210000.0, 0.45f64 * 210000.0);
        let mut n = 0;
        while s.pool.count > 0 {
            s.update_parts(&mut rng);
            n += 1;
            if s.pool.count == 0 { break; }
            let r = &s.pool.recs[i];
            let t = rec::i16(r, 0xa) as f64;
            // The size and alpha use the timer before this update's decrement (d = 25 − (t + 1)).
            let d = 25.0 - (t + 1.0);
            let (size, alpha) = if d < 6.0 {
                (b + ((a + b) / 2.0 - b) * d / 5.0, 127.0 + (63.0 - 127.0) * d / 5.0)
            } else {
                let f = (t + 1.0) / 19.0;
                (a + ((a + b) / 2.0 - a) * f, 63.0 * f)
            };
            let got = f32::from_bits(rec::u32(r, 0xc)) as f64;
            assert!((got - size).abs() < 0.5, "update {n}: size {got} vs {size}");
            assert!((r[7] as f64 - alpha).abs() <= 1.0, "update {n}: alpha {} vs {alpha}", r[7]);
            assert_eq!(rec::u32(r, 4) & 0xff_ffff, 0x20_7f7f);
            assert_eq!(r[8], n as u8);
            let p = rec::pos(r);
            assert!((p[0] - (100.0 + 0.01 * n as f32)).abs() < 1e-3 && (p[2] - (30.05 + 0.012 * n as f32)).abs() < 1e-3);
        }
        assert_eq!(n, 25);
        assert_eq!(rng.state, 1234, "type 53 draws nothing");
    }

    #[test]
    fn sparkle_dies_outside_the_level_box() {
        let mut s = Particles::new(None, Vec::new());
        let i = sparkle(&mut s);
        rec::set_f(&mut s.pool.recs[i], 0x14, 1021.0f32.to_bits());
        rec::set_f(&mut s.pool.recs[i], 0x30, 0.01f32.to_bits());
        s.update_parts(&mut Rng::new());
        assert_eq!(s.pool.count, 0);
    }
}
