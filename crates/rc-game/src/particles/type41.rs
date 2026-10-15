//! Particle type 41, a still sprite held by its caller (level01 spawner `PartType41Spawn` 0x285768, update 0x2858a8,
//! read from the decomp; the update is the same code on all 19 levels, `overlay-diff`). The census's unit U134 (class
//! 816, level 3) calls the spawner (G-PRT-001 consumer; its class is not ported).
//!
//! **Spawn** `(size f12, pos a0, tag a1, frame a2, rgba a3, b9 t0, timer t1, blend t2)`: position = pos, RGBA; blend
//! −1 → byte9 = `(b9 >> 5)·16 + trunc(4)` and ALPHA = b9 & 1 ? 0x48 : 0x44 (the class-27 flag byte's rule), else byte9
//! = b9 and ALPHA = blend; byte1 0, size·210000, texture = frame `a2` of type 41's list (`*(def[41] + a2)`), timer,
//! rotation 0xa0, +0x20 = the tag (the caller's word). No RNG.
//!
//! **Update**: a timer of −1 holds the sprite until its caller kills it; else the timer firing kills it. No RNG.

use super::{fast_dec_timer, rec, Particles};
use crate::rng::Rng;

pub const TYPE: u8 = 41;

/// `PartType41Spawn(size, pos, tag, frame, rgba, b9, timer, blend)` 0x285768 (no RNG).
#[allow(clippy::too_many_arguments)]
pub fn spawn(sys: &mut Particles, size: f32, pos: [f32; 4], tag: u32, frame: usize, rgba: u32, b9: u8, timer: i16, blend: i8) -> Option<usize> {
    let i = sys.create_part(TYPE)?;
    let tex = sys.def_frame(TYPE, frame);
    let r = &mut sys.pool.recs[i];
    rec::set_v4(r, 0x10, pos);
    rec::set_u32(r, 4, rgba);
    if blend == -1 {
        r[9] = (b9 >> 5).wrapping_mul(0x10).wrapping_add(4);
        r[3] = if b9 & 1 == 0 { 0x44 } else { 0x48 };
    } else {
        r[9] = b9;
        r[3] = blend as u8;
    }
    r[1] = 0;
    rec::set_ff(r, 0xc, size * 210000.0);
    r[2] = tex;
    rec::set_i16(r, 10, timer);
    r[8] = 0xa0;
    rec::set_u32(r, 0x20, tag);
    Some(i)
}

/// Update 0x2858a8 (no RNG).
pub fn update(sys: &mut Particles, i: usize, _rng: &mut Rng) {
    let r = &mut sys.pool.recs[i];
    if rec::i16(r, 10) != -1 && fast_dec_timer(r, 10) != 0 { sys.kill_part(i); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn held_forever_or_timed() {
        let mut s = Particles::new(None, Vec::new());
        let mut rng = Rng::new();
        let a = spawn(&mut s, 0.5, [1.0, 2.0, 3.0, 1.0], 7, 0, 0x8080_8080, 0x41, -1, -1).unwrap();
        let b = spawn(&mut s, 0.5, [1.0, 2.0, 3.0, 1.0], 7, 0, 0x8080_8080, 0x33, 3, 0x44).unwrap();
        let ra = &s.pool.recs[a];
        assert_eq!((ra[9], ra[3], ra[8], rec::ff(ra, 0xc), rec::u32(ra, 0x20)), (0x24, 0x48, 0xa0, 105000.0, 7));
        assert_eq!((s.pool.recs[b][9], s.pool.recs[b][3]), (0x33, 0x44));
        for _ in 0..3 { s.update_parts(&mut rng); }
        assert_eq!(s.pool.count, 1, "the timed one died on its 3rd update");
        for _ in 0..500 { s.update_parts(&mut rng); }
        assert_eq!(s.pool.count, 1, "−1 holds");
        assert_eq!(rng, Rng::new());
    }
}
