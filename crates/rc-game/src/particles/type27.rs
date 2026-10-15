//! Particle type 27, the gun spark (level01 spawner `PartType27Spawn` 0x282d80, update 0x282e60, read from the decomp):
//! the Blaster's muzzle sparks and flash and its shots' impact sparks and side sparks (`crate::hero::blaster`,
//! `crate::moby_update::classes::blaster_shot`).
//!
//! **Spawn** `(size, pos, vel, rgba, life)`: position +0x10 and velocity +0x20 as given, colour +0x04, rotation byte8 = the
//! low byte of one raw `rand()` (the one draw), byte9 0x44 (near 1, far 128), ALPHA 0x48 (additive), sprite, texture
//! `def[27][0]`, size +0x0c, the start alpha +0x38 = the colour's alpha byte (signed), timer +0x30 = start +0x34 = life.
//!
//! **Update**: position += velocity; alpha = `a0·t / t0` (integer, the timer before its step); `FastDecTimer` → kill.
//! Standard `f32`.

use super::{rec, Particles};
use crate::rng::Rng;

pub const TYPE: u8 = 27;

/// `PartType27Spawn` 0x282d80 with the spawner's rotation draw `rot` (the low byte of its `rand()`, made by the caller
/// at the game's point).
pub fn spawn(sys: &mut Particles, size: f32, pos: [f32; 4], vel: [f32; 4], rgba: u32, life: i32, rot: u8) -> Option<usize> {
    let i = sys.create_part(TYPE)?;
    fill(sys, i, size, pos, vel, rgba, life, rot);
    Some(i)
}

/// `PartType27Spawn` drawing its own rotation (`rand()`, only when a record was free), from the moby loop.
pub fn spawn_rng(sys: &mut Particles, rng: &mut Rng, size: f32, pos: [f32; 4], vel: [f32; 4], rgba: u32, life: i32) -> Option<usize> {
    let i = sys.create_part(TYPE)?;
    let rot = rng.rand() as u8;
    fill(sys, i, size, pos, vel, rgba, life, rot);
    Some(i)
}

#[allow(clippy::too_many_arguments)]
fn fill(sys: &mut Particles, i: usize, size: f32, pos: [f32; 4], vel: [f32; 4], rgba: u32, life: i32, rot: u8) {
    let def = sys.def_first(TYPE);
    let r = &mut sys.pool.recs[i];
    rec::set_v4(r, 0x10, pos);
    rec::set_u32(r, 4, rgba);
    r[8] = rot;
    r[9] = 4 + 0x40;
    r[3] = 0x48;
    rec::set_ff(r, 0xc, size);
    r[1] = 0;
    r[2] = def;
    rec::set_u32(r, 0x38, (rgba >> 24) as u8 as i8 as i32 as u32);
    rec::set_v4(r, 0x20, vel);
    rec::set_u32(r, 0x34, life as u32);
    rec::set_u32(r, 0x30, life as u32);
}

/// Update 0x282e60 (no RNG).
pub fn update(sys: &mut Particles, i: usize, _rng: &mut Rng) {
    let r = &mut sys.pool.recs[i];
    let (p, v) = (rec::v3(r, 0x10), rec::v3(r, 0x20));
    rec::set_v3(r, 0x10, [p[0] + v[0], p[1] + v[1], p[2] + v[2]]);
    let (a0, t, t0) = (rec::u32(r, 0x38) as i32, rec::u32(r, 0x30) as i32, rec::u32(r, 0x34) as i32);
    let a = if t0 != 0 { a0.wrapping_mul(t) / t0 } else { 0 };
    rec::set_u32(r, 4, (rec::u32(r, 4) & 0xff_ffff).wrapping_add((a as u32) << 24));
    if fast_dec_timer_i32(r, 0x30) { sys.kill_part(i); }
}

/// `FastDecTimer__FRi` on the record's `i32` at `o`.
fn fast_dec_timer_i32(r: &mut super::Record, o: usize) -> bool {
    let t = rec::u32(r, o) as i32;
    if t == 0 { return true; }
    let t = t.max(1) - 1;
    rec::set_u32(r, o, t as u32);
    t <= 0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Moves by its velocity, fades linearly from the colour's alpha, dies with its timer.
    #[test]
    fn moves_fades_and_dies() {
        let mut s = Particles::new(None, Vec::new());
        let mut rng = Rng::new();
        let i = spawn(&mut s, 10000.0, [1.0, 2.0, 3.0, 0.0], [0.5, 0.0, 0.0, 0.0], 0x5f2f_4f6f, 4, 7).unwrap();
        let mut alphas = Vec::new();
        for _ in 0..4 {
            s.update_parts(&mut rng);
            if s.pool.recs[i][1] & super::super::FLAG_DEAD != 0 { break; }
            alphas.push(rec::u32(&s.pool.recs[i], 4) >> 24);
        }
        assert_eq!(alphas, vec![0x5f, 0x5f * 3 / 4, 0x5f * 2 / 4]);
        assert_ne!(s.pool.recs[i][1] & super::super::FLAG_DEAD, 0);
    }
}
