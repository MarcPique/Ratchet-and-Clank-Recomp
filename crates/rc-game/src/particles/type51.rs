//! Particle type 51, the rising dust puff (level01 update `PartType51Update` 0x287060, the same code on all 19 levels,
//! `overlay-diff`; spawner level13 `0x27d8d0`, also on 07 and 14; read from the decomp). Its callers are unported
//! classes (G-PRT-001).
//!
//! **Spawn** `(pos a0, vel a1)` (level13 `0x27d8d0`): position = pos, velocity +0x20 = vel (xyzw), RGBA 0x5e806040,
//! byte9 `trunc(4) + 0x40` = 0x44, ALPHA 0x48 (additive), byte1 0, rotation 0, timer 120 (a literal, not `ticks`),
//! texture `def[51][0]`, size 6300. No RNG.
//!
//! **Update**: vel ·= 0.95 (`0x221228`, all four lanes); vel.z += 5·dt²; pos += vel (`0x2211a0`); alpha byte − 2 (the
//! word + 0xfe000000); alive while the alpha is in 1..=0x5f (`(A − 1) < 0x5f` unsigned) and the timer has not fired:
//! rotation + 1, size += (210000 − size)·0.07; else killed (0x5e fades out in 47 updates). No RNG.

use super::{fast_dec_timer, rec, Particles};
use crate::rng::Rng;

pub const TYPE: u8 = 51;

/// Level13 `0x27d8d0(pos, vel)` (no RNG).
pub fn spawn(sys: &mut Particles, pos: [f32; 4], vel: [f32; 4]) -> Option<usize> {
    let i = sys.create_part(TYPE)?;
    let def = sys.def_first(TYPE);
    let r = &mut sys.pool.recs[i];
    rec::set_v4(r, 0x10, pos);
    rec::set_v4(r, 0x20, vel);
    rec::set_u32(r, 4, 0x5e80_6040);
    r[9] = 4 + 0x40;
    r[3] = 0x48;
    r[1] = 0;
    r[8] = 0;
    rec::set_i16(r, 10, 0x78);
    r[2] = def;
    rec::set_ff(r, 0xc, 6300.0);
    Some(i)
}

/// Update 0x287060 (no RNG).
pub fn update(sys: &mut Particles, i: usize, _rng: &mut Rng) {
    let dt2 = f32::from_bits(sys.time.dt2);
    let r = &mut sys.pool.recs[i];
    let mut v = rec::v4(r, 0x20).map(|x| x * f32::from_bits(0x3f73_3333));
    v[2] += dt2 * 5.0;
    rec::set_v4(r, 0x20, v);
    let p = rec::v4(r, 0x10);
    rec::set_v4(r, 0x10, std::array::from_fn(|k| p[k] + v[k]));
    let c = rec::u32(r, 4).wrapping_add(0xfe00_0000);
    rec::set_u32(r, 4, c);
    if (c & 0xff00_0000).wrapping_sub(1) < 0x5f00_0000 && fast_dec_timer(r, 10) == 0 {
        r[8] = r[8].wrapping_add(1);
        let s = rec::ff(r, 0xc);
        rec::set_ff(r, 0xc, s + (210000.0 - s) * 0.07);
    } else {
        sys.kill_part(i);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The puff slows by 0.95 and rises by 5·dt² a tick, grows toward 210000 and fades 2 a tick: 47 updates.
    #[test]
    fn puff_rises_grows_and_fades() {
        let mut s = Particles::new(None, Vec::new());
        let i = spawn(&mut s, [1.0, 1.0, 1.0, 1.0], [0.1, 0.0, 0.0, 0.0]).unwrap();
        let mut rng = Rng::new();
        s.update_parts(&mut rng);
        let r = &s.pool.recs[i];
        let vz = 5.0 / 3600.0f32;
        assert_eq!(rec::v3(r, 0x20), [0.1 * f32::from_bits(0x3f73_3333), 0.0, 5.0 * f32::from_bits(0x3991_a2b4)]);
        assert!((rec::ff(r, 0x18) - (1.0 + vz)).abs() < 1e-6);
        assert_eq!((rec::u32(r, 4) >> 24, r[8]), (0x5c, 1));
        assert_eq!(rec::ff(r, 0xc), 6300.0 + (210000.0 - 6300.0) * 0.07);
        let mut n = 1;
        while s.pool.count > 0 {
            s.update_parts(&mut rng);
            n += 1;
        }
        assert_eq!(n, 47);
    }
}
