//! Particle type 4, the growing smoke / fire puff (level01 spawner `PartType04Spawn` 0x27e538, update 0x27e650): the
//! Bomb Glove fireballs' smoke (`0x2c4d88`, every tick while the fireball's +0xbc bit 0 is set) and the burn fire.
//!
//! **Record**: position +0x10, velocity +0x20, colour 1 at +0x04 / +0x30, colour 2 at +0x34, byte9 0xa4 (near 1 u, far
//! 320 u), byte3 0x48 when `additive` else 0x44, byte1 0 (sprite), texture `*def[4]`, rotation byte = the low byte of
//! one raw `rand()` (**one draw**, with a record), size 0, +0x0a = +0x3c = life, +0x3a base size, +0x38 growth.
//!
//! **Update**: size = `(growth·(life − t)/life + base)·1000` (integer division); velocity ·= 1 − 0.02·speed; pos +=
//! velocity; rotation byte +1; colour = `FastTweenColor(t/life, c2, c1)`; killed when `FastDecTimer` fires.

use super::{fast_dec_timer, rec, tween_color, Particles};
use crate::rng::Rng;

/// The spawner's arguments.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spawn {
    pub pos: [f32; 4],
    pub vel: [f32; 4],
    pub c1: u32,
    pub c2: u32,
    pub life: i32,
    pub base: i16,
    pub growth: i16,
    pub additive: bool,
}

/// `PartType04Spawn` 0x27e538.
pub fn spawn(sys: &mut Particles, rng: &mut Rng, a: &Spawn) -> Option<usize> {
    let i = sys.create_part(4)?;
    let def = sys.def_first(4);
    let rot = rng.rand() as u8;
    let r = &mut sys.pool.recs[i];
    rec::set_v4(r, 0x10, a.pos);
    rec::set_u32(r, 4, a.c1);
    r[9] = 0xa4;
    r[3] = if a.additive { 0x48 } else { 0x44 };
    r[1] = 0;
    r[2] = def;
    r[8] = rot;
    rec::set_ff(r, 0xc, 0.0);
    rec::set_v4(r, 0x20, a.vel);
    rec::set_i16(r, 0xa, a.life as i16);
    rec::set_i16(r, 0x3a, a.base);
    rec::set_u32(r, 0x3c, a.life as u32);
    rec::set_u32(r, 0x30, a.c1);
    rec::set_i16(r, 0x38, a.growth);
    rec::set_u32(r, 0x34, a.c2);
    Some(i)
}

/// 0x27e650.
pub fn update(sys: &mut Particles, i: usize, _rng: &mut Rng) {
    let speed = f32::from_bits(sys.time.speed);
    let r = &mut sys.pool.recs[i];
    let life = rec::u32(r, 0x3c) as i32;
    if life == 0 { sys.kill_part(i); return; }
    let t = rec::i16(r, 0xa) as i32;
    let size = (rec::i16(r, 0x38) as i32 * (life - t)) / life + rec::i16(r, 0x3a) as i32;
    rec::set_ff(r, 0xc, size as f32 * 1000.0);
    let k = speed * -0.019_999_98 + 1.0;
    let v = rec::v3(r, 0x20).map(|x| x * k);
    rec::set_v3(r, 0x20, v);
    let p = rec::v3(r, 0x10);
    rec::set_v3(r, 0x10, [p[0] + v[0], p[1] + v[1], p[2] + v[2]]);
    r[8] = r[8].wrapping_add(1);
    let f = t as f32 / life as f32;
    rec::set_u32(r, 4, tween_color(f.to_bits(), rec::u32(r, 0x34), rec::u32(r, 0x30)));
    if fast_dec_timer(r, 0xa) != 0 { sys.kill_part(i); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grows_slows_and_fades() {
        let mut sys = Particles::new(None, Vec::new());
        let mut rng = Rng::new();
        let a = Spawn { pos: [5.0, 5.0, 5.0, 0.0], vel: [0.1, 0.0, 0.0, 0.0], c1: 0x2fff_ffff, c2: 0x0000_4fff, life: 45, base: 0x32, growth: 0x78, additive: true };
        let i = spawn(&mut sys, &mut rng, &a).unwrap();
        let s0 = rng.state;
        sys.update_parts(&mut rng);
        let r = &sys.pool.recs[i];
        // t = 45 at the first update: size = base; velocity 0.1·0.98.
        assert_eq!(rec::ff(r, 0xc), 50_000.0);
        assert!((rec::ff(r, 0x20) - 0.098).abs() < 1e-6);
        assert_eq!(rec::u32(r, 4), 0x2fff_ffff);
        let mut n = 1;
        while sys.pool.count > 0 { sys.update_parts(&mut rng); n += 1; }
        assert_eq!(n, 45);
        assert_eq!(rng.state, s0, "no draws in the update");
    }
}
