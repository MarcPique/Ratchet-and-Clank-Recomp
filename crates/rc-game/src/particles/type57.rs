//! Particle type 57, the flat foam ring (751's zone-5 ripple at the foot of the fall), read from the level01 code:
//!
//! * spawner [`spawn`] = `PartType57Spawn(size, spin, pos, vel)` 0x287c80: **kind 1** (a flat quad in the world XY
//!   plane), additive (0x48), byte9 0x2c (near 1 u, far 64 u), texture `def[57][0]`, RGBA `0x808080` with alpha
//!   `randi(16) << 1`, timer 0x80, size `size·210000`, angle `randf(0, 256)` (+0x30, byte8 = trunc), spin +0x34 (its
//!   two draws, in that order, only when the pool gave a record);
//! * [`update`] = 0x287d90 (no RNG): pos += vel (xyz), size + 5880, spin ×0.98, angle += spin, byte8 = trunc(angle),
//!   timer − 1 (killed at ≤ 0), then alpha = t for t ≤ 64, else 128 − t (RGB 0x80).

use super::{rec, Particles, Record};
use crate::rng::Rng;

pub const TYPE: u8 = 57;

fn f(r: &Record, o: usize) -> f32 { f32::from_bits(rec::u32(r, o)) }
fn set(r: &mut Record, o: usize, v: f32) { rec::set_u32(r, o, v.to_bits()); }

/// `PartType57Spawn(size, spin, pos, vel)` 0x287c80 (its draws from `rng`). None when the pool is full (no draws).
pub fn spawn(sys: &mut Particles, rng: &mut Rng, size: f32, spin: f32, pos: [f32; 4], vel: [f32; 4]) -> Option<usize> {
    let i = sys.create_part(TYPE)?;
    let def = sys.def_first(TYPE);
    let a = rng.randi(0x10) as u32;
    let angle = rng.randf(0.0, 256.0);
    let r = &mut sys.pool.recs[i];
    for (k, x) in pos.iter().enumerate() { set(r, 0x10 + 4 * k, *x); }
    rec::set_u32(r, 4, a << 25 | 0x80_8080);
    r[9] = 12 + 0x20;
    r[3] = 0x48;
    r[1] = 1;
    r[0xa..0xc].copy_from_slice(&0x80i16.to_le_bytes());
    r[2] = def;
    set(r, 0xc, size * 210000.0);
    for (k, x) in vel.iter().enumerate() { set(r, 0x20 + 4 * k, *x); }
    set(r, 0x30, angle);
    r[8] = angle as i32 as u8;
    set(r, 0x34, spin);
    Some(i)
}

/// Update 0x287d90.
pub fn update(sys: &mut Particles, i: usize, _rng: &mut Rng) {
    let r = &mut sys.pool.recs[i];
    for k in 0..3 { set(r, 0x10 + 4 * k, f(r, 0x10 + 4 * k) + f(r, 0x20 + 4 * k)); }
    set(r, 0xc, f(r, 0xc) + 5880.0);
    let spin = f(r, 0x34) * 0.98;
    set(r, 0x34, spin);
    let angle = f(r, 0x30) + spin;
    set(r, 0x30, angle);
    r[8] = angle as i32 as u8;
    let t = i16::from_le_bytes([r[0xa], r[0xb]]).wrapping_sub(1);
    r[0xa..0xc].copy_from_slice(&t.to_le_bytes());
    let a = if t < 0x41 { t as i32 } else { 0x80 - t as i32 };
    rec::set_u32(r, 4, (a as u32) << 24 | 0x80_8080);
    if t < 1 { sys.kill_part(i); }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 128 updates, the alpha rising to 64 at t = 64 and falling back to 0.
    #[test]
    fn ring_lives_128_updates() {
        let mut s = Particles::new(None, Vec::new());
        let mut rng = Rng::new();
        let i = spawn(&mut s, &mut rng, 0.45, 0.02, [177.0, 170.0, 39.0, 1.0], [0.0; 4]).unwrap();
        assert_eq!(s.pool.recs[i][1], 1);
        let mut n = 0;
        let mut peak = 0;
        while s.pool.count > 0 {
            s.update_parts(&mut rng);
            n += 1;
            peak = peak.max(rec::u32(&s.pool.recs[i], 4) >> 24);
        }
        assert_eq!((n, peak), (128, 64));
        assert_eq!(f(&s.pool.recs[i], 0xc), 0.45 * 210000.0 + 128.0 * 5880.0);
    }
}
