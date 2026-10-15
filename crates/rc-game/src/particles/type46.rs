//! Particle type 46, the flat spreading ring on the water (level01 spawner `PartType46Spawn` 0x286a68, update 0x286b88,
//! read from the decomp): the Plumber's ripples in the Novalis scene 1 (`CutsceneFxUpdate`).
//!
//! **Spawn** `(size, spin, pos, vel, level)`: RGBA `(0x26 + randi(10)) << 24 | 0x808080`, byte9 0x24 (near 1 u, far
//! 64 u), additive (0x48), **kind 1** (flat quad in the world XY plane), texture `def[46][0]`, timer 5, size
//! `size·210000`, velocity, the angle `randf(0, 256)` (+0x30, byte8 = trunc), +0x38 a water-level pointer (the port:
//! the level itself, [`NO_LEVEL`] for none), +0x34 the spin. Draws: `randi(10)`, `randf` (with a record).
//!
//! **Update**: pos += vel; size + 5880; spin ×0.98, angle += spin, byte8 = trunc(angle); with a level, a ring more
//! than 0.2 away from it (|z − level| ≥ 0.2 keeps z) sits 0.02 above it; the timer steps and at 0 restarts at 1 with
//! the alpha one lower; killed at alpha 0. Standard `f32`.

use super::{rec, Particles};
use crate::rng::Rng;

pub const TYPE: u8 = 46;
/// +0x38 without a water level.
pub const NO_LEVEL: f32 = f32::NEG_INFINITY;

/// `PartType46Spawn(size, spin, pos, vel, &0x13f640)` 0x286a68 with the hero's water level as the pointer
/// (`level` = [`super::type45::HERO_WATER_LEVEL`]: the update reads [`Particles::water_z`] through it).
pub fn spawn_on(sys: &mut Particles, rng: &mut Rng, size: f32, spin: f32, pos: [f32; 4], vel: [f32; 4], level: u32) -> Option<usize> {
    let i = spawn(sys, rng, size, spin, pos, vel, NO_LEVEL)?;
    rec::set_u32(&mut sys.pool.recs[i], 0x38, level);
    Some(i)
}

/// `PartType46Spawn(size, spin, pos, vel, level)` 0x286a68.
pub fn spawn(sys: &mut Particles, rng: &mut Rng, size: f32, spin: f32, pos: [f32; 4], vel: [f32; 4], level: f32) -> Option<usize> {
    let i = sys.create_part(TYPE)?;
    let def = sys.def_first(TYPE);
    let a = rng.randi(10);
    let angle = rng.randf(0.0, 256.0);
    let r = &mut sys.pool.recs[i];
    rec::set_v4(r, 0x10, pos);
    rec::set_u32(r, 4, ((a + 0x26) as u32) << 24 | 0x80_8080);
    r[9] = 4 + 0x20;
    r[3] = 0x48;
    r[1] = 1;
    rec::set_i16(r, 0xa, 5);
    r[2] = def;
    rec::set_ff(r, 0xc, size * 210000.0);
    rec::set_v4(r, 0x20, vel);
    rec::set_ff(r, 0x30, angle);
    r[8] = angle as i32 as u8;
    rec::set_ff(r, 0x38, level);
    rec::set_ff(r, 0x34, spin);
    Some(i)
}

/// Update 0x286b88 (no RNG).
pub fn update(sys: &mut Particles, i: usize, _rng: &mut Rng) {
    let r = &mut sys.pool.recs[i];
    let (p, v) = (rec::v3(r, 0x10), rec::v3(r, 0x20));
    rec::set_v3(r, 0x10, [p[0] + v[0], p[1] + v[1], p[2] + v[2]]);
    rec::set_ff(r, 0xc, rec::ff(r, 0xc) + 5880.0);
    let spin = rec::ff(r, 0x34) * 0.98;
    let angle = rec::ff(r, 0x30) + spin;
    rec::set_ff(r, 0x34, spin);
    rec::set_ff(r, 0x30, angle);
    r[8] = angle as i32 as u8;
    let level = if rec::u32(r, 0x38) == super::type45::HERO_WATER_LEVEL { sys.water_z } else { rec::ff(r, 0x38) };
    let r = &mut sys.pool.recs[i];
    if level != NO_LEVEL && (rec::ff(r, 0x18) - level).abs() < 0.2 { rec::set_ff(r, 0x18, level + 0.02); }
    let t = rec::i16(r, 0xa) - 1;
    rec::set_i16(r, 0xa, t);
    if t == 0 {
        rec::set_i16(r, 0xa, 1);
        rec::set_u32(r, 4, rec::u32(r, 4).wrapping_sub(0x0100_0000));
    }
    if rec::u32(r, 4) & 0xff00_0000 == 0 { sys.kill_part(i); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spreads_and_fades_one_step_a_tick_after_five() {
        let mut s = Particles::new(None, Vec::new());
        let mut rng = Rng::new();
        let i = spawn(&mut s, &mut rng, 1.0, 2.0, [5.0, 5.0, 5.1, 0.0], [0.0; 4], 5.0).unwrap();
        let a0 = rec::u32(&s.pool.recs[i], 4) >> 24;
        let mut n = 0;
        while s.pool.count > 0 && n < 200 {
            s.update_parts(&mut rng);
            n += 1;
        }
        assert_eq!(n as u32, 4 + a0);
        assert_eq!(rec::ff(&s.pool.recs[i], 0x18), 5.02);
    }
}
