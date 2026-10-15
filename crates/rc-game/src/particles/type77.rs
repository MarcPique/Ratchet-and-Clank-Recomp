//! Particle type 77, the drifting puff that fades with its timer (level01 update 0x28ac58, no Ghidra function until
//! 2026-09-29, the same code on all 19 levels, `overlay-diff`; spawner level08 `0x27fe00`; read from the decomp). Its
//! caller is level 8's unported class (G-PRT-001).
//!
//! **Spawn** `(size f12, pos a0, vel a1, life a2)`: position, RGBA 0x50504040, byte9 `trunc(2) + 0x40` = 0x42, ALPHA
//! 0x48, byte1 0, rotation 0xa0, texture `def[77][0]`, size·210000, timer `ticks(life)`, velocity +0x20 with +0x2c =
//! the timer as a float (over vel.w). No RNG.
//!
//! **Update**: f = t/life; RGBA `trunc(38·f) << 24 | 0x808080`, rotation `trunc(33·f)`; pos += (vel, 0); the timer
//! fires → killed. No RNG.

use super::{fast_dec_timer, rec, Particles};
use crate::rng::Rng;

pub const TYPE: u8 = 77;

/// Level08 `0x27fe00(size, pos, vel, life)` (no RNG).
pub fn spawn(sys: &mut Particles, size: f32, pos: [f32; 4], vel: [f32; 3], life: i32) -> Option<usize> {
    let i = sys.create_part(TYPE)?;
    let def = sys.def_first(TYPE);
    let t = sys.time.ticks(life) as i16;
    let r = &mut sys.pool.recs[i];
    rec::set_v4(r, 0x10, pos);
    rec::set_u32(r, 4, 0x5050_4040);
    r[9] = 2 + 0x40;
    r[3] = 0x48;
    r[1] = 0;
    r[8] = 0xa0;
    r[2] = def;
    rec::set_ff(r, 0xc, size * 210000.0);
    rec::set_i16(r, 10, t);
    rec::set_v3(r, 0x20, vel);
    rec::set_ff(r, 0x2c, t as f32);
    Some(i)
}

/// Update 0x28ac58 (no RNG).
pub fn update(sys: &mut Particles, i: usize, _rng: &mut Rng) {
    let r = &mut sys.pool.recs[i];
    let f = rec::i16(r, 10) as f32 / rec::ff(r, 0x2c);
    rec::set_u32(r, 4, ((f * 38.0) as i32 as u32) << 24 | 0x80_8080);
    r[8] = (f * 33.0) as i32 as u8;
    let (p, v) = (rec::v3(r, 0x10), rec::v3(r, 0x20));
    rec::set_v3(r, 0x10, [p[0] + v[0], p[1] + v[1], p[2] + v[2]]);
    if fast_dec_timer(r, 10) != 0 { sys.kill_part(i); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn puff_fades_with_its_timer() {
        let mut s = Particles::new(None, Vec::new());
        let i = spawn(&mut s, 0.2, [1.0; 4], [0.0, 0.0, 0.1], 10).unwrap();
        let mut rng = Rng::new();
        let mut alphas = Vec::new();
        while s.pool.count > 0 {
            s.update_parts(&mut rng);
            alphas.push(rec::u32(&s.pool.recs[i], 4) >> 24);
        }
        assert_eq!(alphas, [38, 34, 30, 26, 22, 19, 15, 11, 7, 3]);
    }
}
