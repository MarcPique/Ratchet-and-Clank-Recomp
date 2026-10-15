//! Particle type 52, the flat goo drip (level01 spawner `PartType52Spawn` 0x287158, update 0x287288): the amoeboids'
//! drips (`0x2ef560`), lying flat on the ground (render kind 1).
//!
//! **Record**: position +0x10 with z + 0.05, colour 1 at +0x04 / +0x28, colour 2 at +0x2c, byte3 0x44, byte9 0x24 (near
//! 1 u, far 64 u), byte1 1 (flat quad in the world XY plane), texture `*def[52]`, +0x0a life, size `s1·210000`, rotation
//! byte `randi(255)` (**one draw**, with a record), +0x20 s1, +0x24 s2, +0x30 = 1/life.
//!
//! **Update**: `FastDecTimer` (kill when it fires); else f = t/life: colour = `FastTweenColor(f, c2, c1)`, size =
//! `((s1 − s2)·f + s2)·210000`.

use super::{fast_dec_timer, rec, tween_color, Particles};
use crate::rng::Rng;

/// `PartType52Spawn(s1, s2, pos, c1, c2, life)` 0x287158.
#[allow(clippy::too_many_arguments)]
pub fn spawn(sys: &mut Particles, rng: &mut Rng, s1: f32, s2: f32, pos: [f32; 4], c1: u32, c2: u32, life: i32) -> Option<usize> {
    let i = sys.create_part(52)?;
    let def = sys.def_first(52);
    let rot = rng.randi(0xff) as u8;
    let r = &mut sys.pool.recs[i];
    rec::set_v4(r, 0x10, pos);
    rec::set_u32(r, 4, c1);
    r[3] = 0x44;
    rec::set_ff(r, 0x18, pos[2] + 0.05);
    r[9] = 0x24;
    r[1] = 1;
    rec::set_i16(r, 0xa, life as i16);
    r[2] = def;
    rec::set_ff(r, 0xc, s1 * 210000.0);
    r[8] = rot;
    rec::set_u32(r, 0x28, c1);
    rec::set_u32(r, 0x2c, c2);
    rec::set_ff(r, 0x20, s1);
    rec::set_ff(r, 0x24, s2);
    rec::set_ff(r, 0x30, 1.0 / life as f32);
    Some(i)
}

/// 0x287288.
pub fn update(sys: &mut Particles, i: usize, _rng: &mut Rng) {
    let r = &mut sys.pool.recs[i];
    if fast_dec_timer(r, 0xa) != 0 { sys.kill_part(i); return; }
    let f = rec::i16(r, 0xa) as f32 * rec::ff(r, 0x30);
    rec::set_u32(r, 4, tween_color(f.to_bits(), rec::u32(r, 0x2c), rec::u32(r, 0x28)));
    rec::set_ff(r, 0xc, ((rec::ff(r, 0x20) - rec::ff(r, 0x24)) * f + rec::ff(r, 0x24)) * 210000.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_drip_spreads() {
        let mut sys = Particles::new(None, Vec::new());
        let mut rng = Rng::new();
        let i = spawn(&mut sys, &mut rng, 0.1, 1.5, [1.0, 1.0, 1.0, 0.0], 0x3000_2028, 0x0000_2020, 120).unwrap();
        assert_eq!(sys.pool.recs[i][1], 1, "kind 1 (flat)");
        let mut n = 0;
        while sys.pool.count > 0 { sys.update_parts(&mut rng); n += 1; }
        assert_eq!(n, 120);
        let s = rec::ff(&sys.pool.recs[i], 0xc) / 210000.0;
        assert!((s - (1.5 + (0.1 - 1.5) / 120.0)).abs() < 1e-4, "{s}");
    }
}
