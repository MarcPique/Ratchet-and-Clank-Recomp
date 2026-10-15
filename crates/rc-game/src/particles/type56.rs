//! Particle type 56, the waterfall mist puff (751's zone-5 spray at the foot of the fall), read from the level01
//! code:
//!
//! * spawner [`spawn`] = `PartType56Spawn(size, pos, vel)` 0x287b00: kind-0 sprite, additive (0x48), byte9 0x24
//!   (near 1 u, far 64 u), texture `def[56][0]`, grey `0x60 + 2·randi(16)` with alpha 0x40, rotation `randi(0x100)`
//!   (its two draws, in that order, only when the pool gave a record);
//! * [`update`] = 0x287bd8 (no RNG): vel ×0.975 (xyz), pos += vel, size + 6300, grey − 4 a tick with alpha 0x60;
//!   killed once the grey is below 1.

use super::{rec, Particles, Record};
use crate::rng::Rng;

pub const TYPE: u8 = 56;

fn f(r: &Record, o: usize) -> f32 { f32::from_bits(rec::u32(r, o)) }
fn set(r: &mut Record, o: usize, v: f32) { rec::set_u32(r, o, v.to_bits()); }

/// `PartType56Spawn(size, pos, vel)` 0x287b00 (its draws from `rng`). None when the pool is full (no draws).
pub fn spawn(sys: &mut Particles, rng: &mut Rng, size: f32, pos: [f32; 4], vel: [f32; 4]) -> Option<usize> {
    let i = sys.create_part(TYPE)?;
    let def = sys.def_first(TYPE);
    let g = (rng.randi(0x10) * 2 + 0x60) as u32;
    let rot = rng.randi(0x100) as u8;
    let r = &mut sys.pool.recs[i];
    for (k, x) in pos.iter().enumerate() { set(r, 0x10 + 4 * k, *x); }
    rec::set_u32(r, 4, g << 16 | g << 8 | 0x4000_0000 | g);
    r[9] = 4 + 0x20;
    r[3] = 0x48;
    r[1] = 0;
    set(r, 0xc, size);
    r[2] = def;
    for (k, x) in vel.iter().enumerate() { set(r, 0x20 + 4 * k, *x); }
    r[8] = rot;
    Some(i)
}

/// Update 0x287bd8.
pub fn update(sys: &mut Particles, i: usize, _rng: &mut Rng) {
    let r = &mut sys.pool.recs[i];
    for k in 0..3 { set(r, 0x20 + 4 * k, f(r, 0x20 + 4 * k) * f32::from_bits(0x3f79_999a)); }
    for k in 0..3 { set(r, 0x10 + 4 * k, f(r, 0x10 + 4 * k) + f(r, 0x20 + 4 * k)); }
    let g = r[4] as i32 - 4;
    set(r, 0xc, f(r, 0xc) + 6300.0);
    if g < 1 {
        sys.kill_part(i);
    } else {
        let g = g as u32;
        rec::set_u32(r, 4, g << 16 | g << 8 | 0x6000_0000 | g);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Grey 0x60 + 2·k fades by 4 a tick: (0x60 + 2k)/4 updates (rounded up) until below 1.
    #[test]
    fn puff_greys_out() {
        let mut s = Particles::new(None, Vec::new());
        let mut rng = Rng::new();
        let i = spawn(&mut s, &mut rng, 50000.0, [177.0, 170.0, 39.0, 1.0], [0.0, 0.0, 0.0, 0.0]).unwrap();
        let g = s.pool.recs[i][4] as u32;
        assert!((0x60..0x80).contains(&g) && g.is_multiple_of(2));
        assert_eq!(rec::u32(&s.pool.recs[i], 4) >> 24, 0x40);
        let mut n = 0;
        while s.pool.count > 0 { s.update_parts(&mut rng); n += 1; }
        assert_eq!(n, g.div_ceil(4));
        assert_eq!(f(&s.pool.recs[i], 0xc), 50000.0 + n as f32 * 6300.0);
    }
}
