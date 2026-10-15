//! Particle type 47, the soft dust puff (the sinking floor's sand, the hero's landings), read from the level01 code:
//!
//! * spawner [`spawn`] = `PartType47Spawn(size, pos, vel)` 0x286cb0: kind-0 sprite, additive (0x48), byte9 0x22
//!   (near 0.5 u, far 64 u), texture `def[47][0]`, RGBA `0x808080` with alpha `0x28 + randi(8)`, rotation
//!   `randi(0x100)` (its two draws, in that order, made by the caller at the game's point:
//!   [`crate::hero::fx::dust`]);
//! * [`update`] = 0x286d80.
//!
//! The spawner leaves +0x30 (the spin step [`update`] adds to the rotation) as the previous occupant of the record
//! left it: the pool's half-zeroing ([`super::PartPool::create_part`]) keeps that quirk.

#![allow(clippy::needless_range_loop)] // record offsets by component, as the game's stores.
use super::{rec, Particles, Record};
use crate::rng::Rng;

pub const TYPE: u8 = 47;

fn f(r: &Record, o: usize) -> f32 { f32::from_bits(rec::u32(r, o)) }
fn set(r: &mut Record, o: usize, v: f32) { rec::set_u32(r, o, v.to_bits()); }

/// `PartType47Spawn(size, pos, vel)` 0x286cb0 with the spawner's draws `alpha` = `randi(8)` and `rot` =
/// `randi(0x100)`. None when the pool is full.
pub fn spawn(sys: &mut Particles, size: f32, pos: [f32; 4], vel: [f32; 4], alpha: u8, rot: u8) -> Option<usize> {
    let i = sys.create_part(TYPE)?;
    let def = sys.def_first(TYPE);
    let r = &mut sys.pool.recs[i];
    for k in 0..4 { set(r, 0x10 + 4 * k, pos[k]); }
    rec::set_u32(r, 4, (alpha as u32 + 0x28) << 24 | 0x80_8080);
    r[9] = 2 + 0x20;
    r[3] = 0x48;
    r[1] = 0;
    set(r, 0xc, size);
    r[2] = def;
    for k in 0..4 { set(r, 0x20 + 4 * k, vel[k]); }
    r[8] = rot;
    Some(i)
}

/// Update 0x286d80 (no RNG): vel ×0.975, pos += vel, the alpha falls by 2 a tick (the RGBA word − 0x2000000), the
/// size grows 14700 a tick, the rotation byte turns by the byte at +0x30; killed once the RGBA word is negative.
pub fn update(sys: &mut Particles, i: usize, _rng: &mut Rng) {
    let r = &mut sys.pool.recs[i];
    for k in 0..4 { set(r, 0x20 + 4 * k, f(r, 0x20 + 4 * k) * f32::from_bits(0x3f79_999a)); }
    for k in 0..3 { set(r, 0x10 + 4 * k, f(r, 0x10 + 4 * k) + f(r, 0x20 + 4 * k)); }
    let c = (rec::u32(r, 4) as i32).wrapping_sub(0x200_0000);
    set(r, 0xc, f(r, 0xc) + 14700.0);
    rec::set_u32(r, 4, c as u32);
    r[8] = r[8].wrapping_add(r[0x30]);
    if c < 0 { sys.kill_part(i); }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Alpha 0x28 + 3 = 0x2b: 22 updates to fall below 0 (0x2b → 0x01 in 21, then negative), growing 14700 a tick.
    #[test]
    fn puff_fades_in_22_updates() {
        let mut s = Particles::new(None, Vec::new());
        let i = spawn(&mut s, 40000.0, [10.0, 10.0, 10.0, 1.0], [0.01, 0.0, 0.02, 0.0], 3, 0x40).unwrap();
        assert_eq!(rec::u32(&s.pool.recs[i], 4), 0x2b80_8080);
        let mut n = 0;
        while s.pool.count > 0 { s.update_parts(&mut Rng::new()); n += 1; }
        assert_eq!(n, 22);
        assert_eq!(f(&s.pool.recs[i], 0xc), 40000.0 + 22.0 * 14700.0);
    }
}
