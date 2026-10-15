//! Particle type 21, the splitting spark (level01 spawner 0x281c10, update 0x281d10, read from the decomp): the
//! Devastator's muzzle burst and smoke (`crate::hero::devastator`) and its missiles' trail sparks.
//!
//! **Spawn** `(size, pos, vel, c1, c2, life, split)` (none for life 0): position +0x10, colour +0x04 = c1, byte9 0xa4
//! (near 1, far 320), ALPHA 0x48 (additive), sprite, texture `def[21][0]`, size +0x0c, rotation = the low byte of one
//! raw `rand()` (with a record), timer +0x0a = life (and +0x38), the start size +0x3c, c1 / c2 at +0x30 / +0x34, split
//! +0x3a, velocity +0x20.
//!
//! **Update**: the timer out → killed; size = `t·size0/life`; colour = `FastTweenColor(t/life, c2, c1)`; position +=
//! velocity; velocity.z −= 0.001·[0x15ed64]; without split (or on even ticks) velocity ×0.96; with split on odd ticks
//! a child spark (no split): a random direction `randf(0.21, 0.25)` of the speed added to it, ×`randf(0.75, 0.95)`,
//! the parent's size and colour, c2, `ticks(rand_range(15, 20))` ticks (draws: 3 + 2 + the spawner's). Standard `f32`.

use super::{fast_dec_timer, rec, tween_color, Particles};
use crate::rng::Rng;

pub const TYPE: u8 = 21;

/// Spawner 0x281c10 with its rotation draw `rot` (made by the caller).
#[allow(clippy::too_many_arguments)]
pub fn spawn(sys: &mut Particles, size: f32, pos: [f32; 4], vel: [f32; 4], c1: u32, c2: u32, life: i32, split: i16, rot: u8) -> Option<usize> {
    if life == 0 { return None; }
    let i = sys.create_part(TYPE)?;
    let def = sys.def_first(TYPE);
    let r = &mut sys.pool.recs[i];
    rec::set_v4(r, 0x10, pos);
    rec::set_u32(r, 4, c1);
    r[1] = 0;
    rec::set_ff(r, 0xc, size);
    r[3] = 0x48;
    r[9] = 4u8.wrapping_sub(0x60);
    r[8] = rot;
    rec::set_i16(r, 0xa, life as i16);
    r[2] = def;
    rec::set_i16(r, 0x38, life as i16);
    rec::set_ff(r, 0x3c, size);
    rec::set_u32(r, 0x30, c1);
    rec::set_u32(r, 0x34, c2);
    rec::set_i16(r, 0x3a, split);
    rec::set_v4(r, 0x20, vel);
    Some(i)
}

/// Spawner 0x281c10 drawing its own rotation (`rand()` with a record), from the moby loop or an update.
#[allow(clippy::too_many_arguments)]
pub fn spawn_rng(sys: &mut Particles, rng: &mut Rng, size: f32, pos: [f32; 4], vel: [f32; 4], c1: u32, c2: u32, life: i32, split: i16) -> Option<usize> {
    let i = spawn(sys, size, pos, vel, c1, c2, life, split, 0)?;
    sys.pool.recs[i][8] = rng.rand() as u8;
    Some(i)
}

/// Update 0x281d10.
pub fn update(sys: &mut Particles, i: usize, rng: &mut Rng) {
    let speed = f32::from_bits(sys.time.speed);
    let odd = sys.counter & 1 != 0;
    let r = &mut sys.pool.recs[i];
    if fast_dec_timer(r, 0xa) != 0 {
        sys.kill_part(i);
        return;
    }
    let (t, life) = (rec::i16(r, 0xa) as f32, rec::i16(r, 0x38) as f32);
    rec::set_ff(r, 0xc, t * rec::ff(r, 0x3c) / life);
    let c = tween_color((t / life).to_bits(), rec::u32(r, 0x34), rec::u32(r, 0x30));
    rec::set_u32(r, 4, c);
    let (p, mut v) = (rec::v3(r, 0x10), rec::v3(r, 0x20));
    rec::set_v3(r, 0x10, [p[0] + v[0], p[1] + v[1], p[2] + v[2]]);
    v[2] -= speed * 0.001;
    let split = rec::i16(r, 0x3a) != 0;
    if !split || !odd {
        rec::set_v3(r, 0x20, v.map(|x| x * f32::from_bits(0x3f75_c28f)));
        return;
    }
    rec::set_v3(r, 0x20, v);
    let (pos, size, c2) = (rec::v3(r, 0x10), rec::ff(r, 0xc), rec::u32(r, 0x34));
    let x = rng.randf(-1.0, 1.0);
    let y = rng.randf(-1.0, 1.0);
    let z = rng.randf(-1.0, 1.0);
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    let k = rng.randf(0.21, 0.25);
    let n = (x * x + y * y + z * z).sqrt();
    let d = if n == 0.0 { [0.0; 3] } else { [x * l * k / n, y * l * k / n, z * l * k / n] };
    let m = rng.randf(0.75, 0.95);
    let nv = [(d[0] + v[0]) * m, (d[1] + v[1]) * m, (d[2] + v[2]) * m, 0.0];
    let rr = rng.rand_range(15, 20);
    let life2 = sys.time.ticks(rr);
    spawn_rng(sys, rng, size, [pos[0], pos[1], pos[2], 0.0], nv, c, c2, life2, 0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shrinks_tweens_and_splits_on_odd_ticks() {
        let mut s = Particles::new(None, Vec::new());
        let mut rng = Rng::new();
        let i = spawn(&mut s, 1000.0, [0.0; 4], [0.1, 0.0, 0.0, 0.0], 0x4f00_7fff, 0x1fff_ffff, 10, 1, 0).unwrap();
        s.counter = 1;
        s.update_parts(&mut rng);
        assert!((rec::ff(&s.pool.recs[i], 0xc) - 900.0).abs() < 1e-3);
        assert_eq!(s.live_by_type()[21], 2, "a child spark");
        s.counter = 2;
        let before = s.live_by_type()[21];
        s.update_parts(&mut rng);
        assert_eq!(s.live_by_type()[21], before, "no split on even ticks");
    }
}
