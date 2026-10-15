//! Particle type 43, the rising tinted smoke puff that fades in and out (level01 update `PartType43Update` 0x286238,
//! the same code on all 19 levels, `overlay-diff`; spawner level08 `0x27b450`; read from the decomp). Its callers are
//! unported classes (G-PRT-001).
//!
//! **Spawn** `(size f12, g f13, pos a0, vel a1, life a2, r a3, g t0, b t1, alpha t2)` (level08 `0x27b450`): position,
//! velocity +0x20 (xyzw), the colour bytes +0x34..+0x36 and the peak alpha +0x37, RGBA `b << 16 | g << 8 | r` (alpha
//! 0), byte1 0, ALPHA 0x48, byte9 `trunc(4) + 0x40` = 0x44, rotation = the low byte of one raw `rand()`, texture
//! `def[43][0]`, size, the age +0x0a = 0, +0x38 = `ticks(life)`, +0x30 = f13 (the update never reads it).
//!
//! **Update**: age + 1; past +0x38 → killed. Else f = 1 − age/life; each channel `trunc(c − k·f)` with k = 0.001 /
//! 0.0011 / 0.00111 (the integer channels drop by one while f > 0: the game's arithmetic, kept); alpha rises
//! linearly over the first `ticks(30)` (`a·age/ticks(30)`) and then falls to 0 at the end (`a·(1 − (age −
//! ticks(30))/(life − ticks(30)))`); rotation + 1; pos += vel; vel.z += 0.0005 (the puff rises faster). No RNG.

use super::{rec, Particles};
use crate::rng::Rng;

pub const TYPE: u8 = 43;

/// Level08 `0x27b450`'s arguments.
#[derive(Clone, Copy, Debug, Default)]
pub struct Spawn {
    pub size: f32,
    pub g: f32,
    pub pos: [f32; 4],
    pub vel: [f32; 4],
    pub life: i32,
    pub rgb: [u8; 3],
    pub alpha: u8,
}

/// Level08 `0x27b450`: one raw `rand()` with a record; none when the pool is full.
pub fn spawn(sys: &mut Particles, rng: &mut Rng, s: Spawn) -> Option<usize> {
    let i = sys.create_part(TYPE)?;
    let rot = rng.rand() as u8;
    let def = sys.def_first(TYPE);
    let life = sys.time.ticks(s.life) as i16;
    let r = &mut sys.pool.recs[i];
    rec::set_v4(r, 0x10, s.pos);
    rec::set_v4(r, 0x20, s.vel);
    r[0x37] = s.alpha;
    r[0x34..0x37].copy_from_slice(&s.rgb);
    rec::set_u32(r, 4, (s.rgb[2] as u32) << 16 | (s.rgb[1] as u32) << 8 | s.rgb[0] as u32);
    r[1] = 0;
    r[3] = 0x48;
    r[9] = 4 + 0x40;
    r[8] = rot;
    rec::set_ff(r, 0xc, s.size);
    rec::set_i16(r, 10, 0);
    r[2] = def;
    rec::set_i16(r, 0x38, life);
    rec::set_ff(r, 0x30, s.g);
    Some(i)
}

/// Update 0x286238 (no RNG).
pub fn update(sys: &mut Particles, i: usize, _rng: &mut Rng) {
    let t30 = sys.time.ticks(0x1e);
    let r = &mut sys.pool.recs[i];
    let age = rec::i16(r, 10).wrapping_add(1);
    rec::set_i16(r, 10, age);
    let life = rec::i16(r, 0x38);
    if life < age {
        sys.kill_part(i);
        return;
    }
    let f = 1.0 - age as f32 / life as f32;
    let ch = |c: u8, k: f32| (c as f32 + f * -k) as i32 as u32 & 0xff;
    let (cr, cg, cb) = (ch(r[0x34], 0.001), ch(r[0x35], 0.0011), ch(r[0x36], 0.00111));
    let a = r[0x37] as f32;
    let k = if (age as i32) < t30 { age as f32 / t30 as f32 } else { 1.0 - (age as i32 - t30) as f32 / (life as i32 - t30) as f32 };
    let al = (a * k) as i32 as u32;
    rec::set_u32(r, 4, al << 24 | cb << 16 | cg << 8 | cr);
    r[8] = r[8].wrapping_add(1);
    let (p, v) = (rec::v4(r, 0x10), rec::v4(r, 0x20));
    rec::set_v4(r, 0x10, std::array::from_fn(|k| p[k] + v[k]));
    rec::set_ff(r, 0x28, v[2] + 0.0005);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fades in over 30 ticks to the peak and out to 0 at `life`; the channels one below their bytes; rises.
    #[test]
    fn puff_fades_in_and_out() {
        let mut s = Particles::new(None, Vec::new());
        let mut rng = Rng::new();
        let mut t = rng;
        let i = spawn(&mut s, &mut rng, Spawn { size: 50000.0, g: 0.0, pos: [1.0; 4], vel: [0.0; 4], life: 90, rgb: [0x40, 0x50, 0x60], alpha: 0x60 }).unwrap();
        assert_eq!(s.pool.recs[i][8], t.rand() as u8);
        assert_eq!(rec::u32(&s.pool.recs[i], 4), 0x0060_5040);
        let mut alphas = Vec::new();
        while s.pool.count > 0 {
            s.update_parts(&mut rng);
            if s.pool.count > 0 { alphas.push(rec::u32(&s.pool.recs[i], 4) >> 24); }
        }
        assert_eq!(alphas.len(), 90);
        assert_eq!((alphas[0], alphas[29], alphas[89]), (0x60 / 30, 0x60, 0));
        assert!(alphas.iter().all(|&a| a <= 0x60));
        assert_eq!(rng, t);
    }
}
