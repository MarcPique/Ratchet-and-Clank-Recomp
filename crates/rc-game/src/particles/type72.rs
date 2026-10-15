//! Particle type 72, the Blaster shot's trail sprite (level01 spawner 0x28a3f0, update 0x28a578, read from the decomp).
//! The shot (class 305, `crate::moby_update::classes::blaster_shot`) creates 17 of them and owns them: it places them
//! along its path and sets their sizes and alphas every tick, and kills them when it ends (`0x2e2a18`).
//!
//! **Spawn** `(f, pos, vel, mode)` (the shot passes `(0, position, 0x15f580, 1)`): position +0x10, colour 0x7fffffff,
//! byte9 0x44, ALPHA 0x48 (additive), sprite, size +0x0c `randf(20000, 40000)` (`randf(40000, 80000)` for a negative
//! mode), rotation byte8 `randi(8) + 0x1c`, texture `def[72][0]`, +0x30 mode, velocity +0x20 (its w then = `f`), +0x34
//! 0, spin +0x38 `randi(12) − 6`, +0x3c size·0.1 (·0.05 for mode ≥ 0, then `randi(30)`: 0 → +0x34 = −40, else
//! `randi(30)`: 0 → −1). The draws are the spawner's, in order ([`Draws`]).
//!
//! **Update**: rotation += spin (byte); a size at or below 0 is zeroed and the record killed. Standard `f32`.

use super::{rec, Particles};
use crate::rng::Rng;

pub const TYPE: u8 = 72;

/// The spawner's draws for mode ≥ 0: size `randf(20000, 40000)`, `randi(8)`, `randi(12)`, `randi(30)` (and a second
/// `randi(30)` when the first is not 0).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Draws {
    pub size: f32,
    pub rot: u8,
    pub spin: i32,
    pub b34: i32,
}

impl Draws {
    pub fn draw(rng: &mut Rng) -> Draws {
        let size = rng.randf(20000.0, 40000.0);
        let rot = (rng.randi(8) + 0x1c) as u8;
        let spin = rng.randi(12) - 6;
        let b34 = if rng.randi(30) == 0 { -40 } else if rng.randi(30) == 0 { -1 } else { 0 };
        Draws { size, rot, spin, b34 }
    }
}

/// Spawner 0x28a3f0 (mode 1, velocity 0) with its draws made.
pub fn spawn(sys: &mut Particles, pos: [f32; 4], d: &Draws) -> Option<usize> {
    let i = sys.create_part(TYPE)?;
    let def = sys.def_first(TYPE);
    let r = &mut sys.pool.recs[i];
    rec::set_v4(r, 0x10, pos);
    rec::set_u32(r, 4, 0x7fff_ffff);
    r[9] = 4 + 0x40;
    r[3] = 0x48;
    r[1] = 0;
    rec::set_ff(r, 0xc, d.size);
    r[8] = d.rot;
    r[2] = def;
    rec::set_u32(r, 0x30, 1);
    rec::set_v4(r, 0x20, [0.0; 4]);
    rec::set_u32(r, 0x38, d.spin as u32);
    rec::set_ff(r, 0x3c, d.size * 0.05);
    rec::set_u32(r, 0x34, d.b34 as u32);
    Some(i)
}

/// Update 0x28a578 (no RNG).
pub fn update(sys: &mut Particles, i: usize, _rng: &mut Rng) {
    let r = &mut sys.pool.recs[i];
    r[8] = r[8].wrapping_add(r[0x38]);
    if rec::ff(r, 0xc) <= 0.0 {
        rec::set_ff(r, 0xc, 0.0);
        sys.kill_part(i);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spins_and_lives_while_it_has_a_size() {
        let mut s = Particles::new(None, Vec::new());
        let mut rng = Rng::new();
        let d = Draws { size: 30000.0, rot: 0x1e, spin: -3, b34: 0 };
        let i = spawn(&mut s, [0.0; 4], &d).unwrap();
        s.update_parts(&mut rng);
        assert_eq!(s.pool.recs[i][8], 0x1b);
        rec::set_ff(&mut s.pool.recs[i], 0xc, 0.0);
        s.update_parts(&mut rng);
        assert_ne!(s.pool.recs[i][1] & super::super::FLAG_DEAD, 0);
    }
}
