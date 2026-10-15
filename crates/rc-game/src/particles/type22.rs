//! Particle type 22, the smoke / exhaust puff that grows and rises (level01 spawner `PartType22Spawn` 0x281f30, update
//! 0x27c6f0, a VU0 macro routine read from the disassembly): the rocks' / walls' / pipe's break bursts
//! (`breakables`), the gunship's and the path enemies' jet exhaust.
//!
//! **Spawn** `(size, pos, vel, c1, c2, life)`: RGBA c1, sprite, ALPHA 0x44, byte9 0x74 (near 1 u, far 224 u), start
//! size `size·0.2` growing to `size` over the life (+0x3c = (size − size·0.2)/life), rotation = the low byte of one raw
//! `rand()`, texture `def[22][0]`, timer +0x0a = +0x38 = life, +0x30 c1, +0x34 c2, velocity +0x20, and the rise
//! +0x2c = 1.24·dt².
//!
//! **Update**: t = timer − 1, killed at t ≤ 0; pos += vel; rotation + 1; vel.z += +0x2c; colour per byte =
//! `ftoi0(c2·(1 − t/N) + c1·t/N)` with both alphas × min(t, 8)/8 (`ITOF12` of `min(8, t) << 9`); size += +0x3c.
//! Standard `f32`.

use super::{rec, Particles};
use crate::rng::Rng;

pub const TYPE: u8 = 22;

/// The spawner's arguments.
#[derive(Clone, Copy, Debug)]
pub struct Spawn {
    pub size: f32,
    pub pos: [f32; 4],
    pub vel: [f32; 4],
    pub c1: u32,
    pub c2: u32,
    pub life: i32,
}

/// `PartType22Spawn` 0x281f30: one `rand()` with a record.
pub fn spawn(sys: &mut Particles, rng: &mut Rng, a: &Spawn) -> Option<usize> {
    let i = sys.create_part(TYPE)?;
    let def = sys.def_first(TYPE);
    let dt2 = f32::from_bits(sys.time.dt2);
    let rot = rng.rand() as u8;
    let r = &mut sys.pool.recs[i];
    rec::set_u32(r, 4, a.c1);
    r[1] = 0;
    r[3] = 0x44;
    r[9] = 4 + 0x70;
    let s0 = a.size * 0.2;
    rec::set_ff(r, 0xc, s0);
    r[8] = rot;
    rec::set_i16(r, 0xa, a.life as i16);
    r[2] = def;
    rec::set_i16(r, 0x38, a.life as i16);
    rec::set_u32(r, 0x30, a.c1);
    rec::set_u32(r, 0x34, a.c2);
    rec::set_ff(r, 0x3c, (a.size - s0) / a.life as f32);
    rec::set_v4(r, 0x20, a.vel);
    rec::set_v4(r, 0x10, a.pos);
    rec::set_ff(r, 0x2c, dt2 * 1.24);
    Some(i)
}

/// Update 0x27c6f0 (no RNG).
pub fn update(sys: &mut Particles, i: usize, _rng: &mut Rng) {
    let r = &mut sys.pool.recs[i];
    let t = rec::i16(r, 0xa) as i32 - 1;
    if t <= 0 {
        sys.kill_part(i);
        return;
    }
    rec::set_i16(r, 0xa, t as i16);
    let n = rec::i16(r, 0x38) as f32;
    let (p, v) = (rec::v3(r, 0x10), rec::v3(r, 0x20));
    rec::set_v3(r, 0x10, [p[0] + v[0], p[1] + v[1], p[2] + v[2]]);
    r[8] = r[8].wrapping_add(1);
    rec::set_ff(r, 0x28, v[2] + rec::ff(r, 0x2c));
    let q = t as f32 / n;
    let fade = t.min(8) as f32 / 8.0;
    let (c1, c2) = (rec::u32(r, 0x30), rec::u32(r, 0x34));
    let mut out = 0u32;
    for k in 0..4 {
        let (mut a, mut b) = ((c1 >> (8 * k) & 0xff) as f32, (c2 >> (8 * k) & 0xff) as f32);
        if k == 3 {
            a *= fade;
            b *= fade;
        }
        let v = b * (1.0 - q) + a * q;
        out |= (v as i32 as u32 & 0xff) << (8 * k);
    }
    rec::set_ff(r, 0xc, rec::ff(r, 0xc) + rec::ff(r, 0x3c));
    rec::set_u32(r, 4, out);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grows_rises_and_fades_over_its_life() {
        let mut s = Particles::new(None, Vec::new());
        let mut rng = Rng::new();
        let a = Spawn { size: 100000.0, pos: [1.0, 2.0, 3.0, 0.0], vel: [0.0; 4], c1: 0x8080_8080, c2: 0x0040_4040, life: 20 };
        let i = spawn(&mut s, &mut rng, &a).unwrap();
        assert_eq!(rec::ff(&s.pool.recs[i], 0xc), 20000.0);
        let mut n = 0;
        while s.pool.count > 0 {
            s.update_parts(&mut rng);
            n += 1;
            if n == 1 {
                // t = 19: colour ≈ c1, alpha 0x80 (t ≥ 8).
                assert_eq!(rec::u32(&s.pool.recs[i], 4) >> 24, (0x80 as f32 * 19.0 / 20.0) as u32);
            }
        }
        assert_eq!(n, 20);
        assert!(rec::ff(&s.pool.recs[i], 0x18) > 3.0, "rises");
    }
}
