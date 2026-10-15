//! Particle type 44, the drifting smoke puff (level01 spawner 0x286450, update 0x2865e8, read from the decomp): the
//! Devastator's muzzle smoke (`0x2c7758` / `0x2c7a48`, `crate::hero::devastator`) and its missiles' trail
//! (`0x2c5b70`, `crate::moby_update::classes::devastator_missile`).
//!
//! **Spawn** `(size, growth, damp, fall, w, pos, vel, life, alpha, rgb, spin)`: position +0x10 (its w = `w`), colour
//! `alpha << 24 | rgb`, byte9 0x44 (near 1, far 128), ALPHA 0x48 (additive), sprite, texture `def[44][0]`, size +0x0c,
//! rotation `randi(0xff)` (the one draw, with a record), timer +0x0a = life, velocity +0x20, spin byte +0x30 =
//! `trunc(spin·speed)`, the fall +0x2c, the rgb bytes +0x31..+0x33, life / alpha at +0x3c / +0x3e, growth +0x34 =
//! `growth·speed`, damping +0x38 = `(damp − 1)·speed + 1`. Callers may patch the record after it (the missile's trail
//! takes ALPHA 0x44 and texture `def[23][0]`).
//!
//! **Update**: the timer out → killed; alpha = `trunc(alpha0·t/life)`; position += velocity, and += Ratchet's platform
//! motion 0x13f490 (the puffs ride his platform); velocity = (velocity + gravity-direction·fall)·damping; rotation +=
//! spin; size += growth. Standard `f32`.

use super::{fast_dec_timer, rec, Particles};
use crate::rng::Rng;

pub const TYPE: u8 = 44;

/// The spawner's arguments.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spawn {
    pub size: f32,
    pub growth: f32,
    pub damp: f32,
    pub fall: f32,
    pub w: f32,
    pub pos: [f32; 3],
    pub vel: [f32; 3],
    pub life: i32,
    pub alpha: u8,
    pub rgb: u32,
    pub spin: i32,
}

/// Spawner 0x286450 with its rotation draw `rot` (`randi(0xff)`, made by the caller when the record exists).
pub fn spawn(sys: &mut Particles, a: &Spawn, rot: u8) -> Option<usize> {
    let i = sys.create_part(TYPE)?;
    let def = sys.def_first(TYPE);
    let speed = f32::from_bits(sys.time.speed);
    let r = &mut sys.pool.recs[i];
    rec::set_v4(r, 0x10, [a.pos[0], a.pos[1], a.pos[2], a.w]);
    rec::set_u32(r, 4, (a.alpha as u32) << 24 | (a.rgb & 0xff_ffff));
    rec::set_ff(r, 0xc, a.size);
    r[1] = 0;
    r[3] = 0x48;
    r[9] = 4 + 0x40;
    r[8] = rot;
    rec::set_i16(r, 0xa, a.life as i16);
    r[2] = def;
    rec::set_v3(r, 0x20, a.vel);
    r[0x30] = (a.spin as f32 * speed) as i32 as u8;
    rec::set_ff(r, 0x2c, a.fall);
    r[0x31] = a.rgb as u8;
    r[0x32] = (a.rgb >> 8) as u8;
    r[0x33] = (a.rgb >> 16) as u8;
    rec::set_i16(r, 0x3c, a.life as i16);
    rec::set_i16(r, 0x3e, a.alpha as i16);
    rec::set_ff(r, 0x34, a.growth * speed);
    rec::set_ff(r, 0x38, (a.damp - 1.0) * speed + 1.0);
    Some(i)
}

/// Spawner 0x286450 drawing its own rotation (`randi(0xff)` with a record), from the moby loop.
pub fn spawn_rng(sys: &mut Particles, rng: &mut Rng, a: &Spawn) -> Option<usize> {
    let i = spawn(sys, a, 0)?;
    sys.pool.recs[i][8] = rng.randi(0xff) as u8;
    Some(i)
}

/// Update 0x2865e8 (no RNG).
pub fn update(sys: &mut Particles, i: usize, _rng: &mut Rng) {
    let plat = sys.hero_plat;
    let r = &mut sys.pool.recs[i];
    if fast_dec_timer(r, 0xa) != 0 {
        sys.kill_part(i);
        return;
    }
    let (t, life, a0) = (rec::i16(r, 0xa) as f32, rec::i16(r, 0x3c) as f32, rec::i16(r, 0x3e) as f32);
    let a = if life != 0.0 { (a0 * (t / life)) as i32 } else { 0 };
    rec::set_u32(r, 4, (a as u32) << 24 | (r[0x33] as u32) << 16 | (r[0x32] as u32) << 8 | r[0x31] as u32);
    let v = rec::v3(r, 0x20);
    let p = rec::v3(r, 0x10);
    rec::set_v3(r, 0x10, [p[0] + v[0] + plat[0], p[1] + v[1] + plat[1], p[2] + v[2] + plat[2]]);
    let fall = rec::ff(r, 0x2c);
    let k = rec::ff(r, 0x38);
    // Gravity direction 0x13f5e0 = (0, 0, −1) on foot.
    rec::set_v3(r, 0x20, [v[0] * k, v[1] * k, (v[2] - fall) * k]);
    r[8] = r[8].wrapping_add(r[0x30]);
    let s = rec::ff(r, 0xc) + rec::ff(r, 0x34);
    rec::set_ff(r, 0xc, s);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drifts_grows_fades() {
        let mut s = Particles::new(None, Vec::new());
        let mut rng = Rng::new();
        let a = Spawn { size: 1000.0, growth: 10.0, damp: 0.5, fall: 0.0, w: 0.0, pos: [0.0; 3], vel: [1.0, 0.0, 0.0], life: 4, alpha: 0x40, rgb: 0x10_2030, spin: 2 };
        let i = spawn(&mut s, &a, 5).unwrap();
        s.update_parts(&mut rng);
        let r = &s.pool.recs[i];
        assert_eq!(rec::pos(r), [1.0, 0.0, 0.0]);
        assert_eq!(rec::v3(r, 0x20), [0.5, 0.0, 0.0]);
        assert_eq!(rec::ff(r, 0xc), 1010.0);
        assert_eq!(rec::u32(r, 4), (0x40 * 3 / 4) << 24 | 0x10_2030);
        assert_eq!(r[8], 7);
    }
}
