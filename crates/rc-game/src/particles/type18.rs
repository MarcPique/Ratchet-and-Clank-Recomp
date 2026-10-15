//! Particle type 18, the smoke trail behind a moby the Suck Cannon threw (level01 spawner `PartType18Spawn` 0x281430,
//! update 0x2815b8, read from the decomp; both the same code on all 19 levels, `overlay-diff`). Its caller is the
//! thrown moby's flight `0x305260` (`moby_update::creature::react`), once a tick in flight, with the moby's position and
//! rows (+0xc0).
//!
//! **Spawn** `(pos a0, rows a1)`: offset `(randf(±0.1), randf(±0.3), randf(±0.3))` turned by the rows
//! (`fun_001f9d20`: `r0·x + r1·y + r2·z + r3·w`, with a stack row 3 and offset w the spawner never writes; taken as 0
//! [L]), position = pos + offset; RGBA 0x00806040 (alpha 0), byte1 0, ALPHA 0x48 (additive), byte9 `trunc(4) + 0x40` =
//! 0x44, rotation = the low byte of one raw `rand()`, texture `def[18][0]`, alpha +0x24 = 0.08, size `randf(27993,
//! 49980)`, timer `ticks(210)`, direction +0x2c = 1. +0x28 (the spin) is **not written**: the record keeps its previous
//! occupant's word (the allocator only clears 0x00..0x1f), as in the game. Draws: 3 + 1 + 1.
//!
//! **Update**: alpha += 0.013·speed·dir; alpha ≤ 0, or else the timer → killed. alpha ≥ 0.22 → dir flips and the step is
//! taken back (it pulses up to 0.22 and back down: 28 updates). size += 8190·speed; A = `trunc(alpha·255)`; spin +=
//! 0.012 (past 1: − 1); z −= 0.02 (the smoke sinks); rotation byte = `trunc(spin·255)`. No RNG. Native `f32`.

use super::{fast_dec_timer, rec, Particles};
use crate::rng::Rng;

pub const TYPE: u8 = 18;

/// `PartType18Spawn(pos, rows)` 0x281430: five draws with a record; none when the pool is full.
pub fn spawn(sys: &mut Particles, rng: &mut Rng, pos: [f32; 4], rows: [[f32; 3]; 3]) -> Option<usize> {
    let i = sys.create_part(TYPE)?;
    let def = sys.def_first(TYPE);
    let o = [rng.randf(-0.1, 0.1), rng.randf(-0.3, 0.3), rng.randf(-0.3, 0.3)];
    let d: [f32; 3] = std::array::from_fn(|l| rows[0][l] * o[0] + rows[1][l] * o[1] + rows[2][l] * o[2]);
    let rot = rng.rand() as u8;
    let size = rng.randf(f32::from_bits(0x46da_b201), f32::from_bits(0x4743_3c00));
    let t = sys.time.ticks(0xd2) as i16;
    let r = &mut sys.pool.recs[i];
    rec::set_v4(r, 0x10, [d[0] + pos[0], d[1] + pos[1], d[2] + pos[2], pos[3]]);
    rec::set_u32(r, 4, 0x0080_6040);
    r[1] = 0;
    r[3] = 0x48;
    r[9] = 4 + 0x40;
    r[8] = rot;
    r[2] = def;
    rec::set_ff(r, 0x24, f32::from_bits(0x3da3_d70a));
    rec::set_ff(r, 0xc, size);
    rec::set_i16(r, 10, t);
    rec::set_ff(r, 0x2c, 1.0);
    Some(i)
}

/// Update 0x2815b8 (no RNG).
pub fn update(sys: &mut Particles, i: usize, _rng: &mut Rng) {
    let r = &mut sys.pool.recs[i];
    let a = rec::ff(r, 0x24) + 0.013 * rec::ff(r, 0x2c);
    rec::set_ff(r, 0x24, a);
    if a <= 0.0 || fast_dec_timer(r, 10) != 0 {
        sys.kill_part(i);
        return;
    }
    if 0.22 <= a {
        let d = rec::ff(r, 0x2c);
        rec::set_ff(r, 0x2c, -d);
        rec::set_ff(r, 0x24, a + 0.013 * -d);
    }
    rec::set_ff(r, 0xc, rec::ff(r, 0xc) + 8190.0);
    let al = (rec::ff(r, 0x24) * 255.0) as i32;
    rec::set_u32(r, 4, rec::u32(r, 4) & 0xff_ffff | (al as u32) << 24);
    let mut spin = rec::ff(r, 0x28) + 0.012;
    if 1.0 < spin { spin -= 1.0; }
    rec::set_ff(r, 0x28, spin);
    rec::set_ff(r, 0x18, rec::ff(r, 0x18) - 0.02);
    r[8] = (spin * 255.0) as i32 as u8;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The five draws in order, the offset in the moby's frame, the pulse to 0.22 and back (28 updates), the sinking,
    /// the stale spin word kept.
    #[test]
    fn trail_puff_pulses_sinks_and_dies() {
        let mut s = Particles::new(None, Vec::new());
        let mut rng = Rng::new();
        let mut t = rng;
        // A previous occupant left 0.5 at +0x28.
        let k = s.create_part(9).unwrap();
        rec::set_ff(&mut s.pool.recs[k], 0x28, 0.5);
        s.kill_part(k);
        let rows = [[0.0, 1.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]];
        let i = spawn(&mut s, &mut rng, [5.0, 5.0, 5.0, 1.0], rows).unwrap();
        let o = [t.randf(-0.1, 0.1), t.randf(-0.3, 0.3), t.randf(-0.3, 0.3)];
        let rot = t.rand() as u8;
        let size = t.randf(f32::from_bits(0x46da_b201), f32::from_bits(0x4743_3c00));
        assert_eq!(rng, t);
        let r = &s.pool.recs[i];
        assert_eq!(rec::pos(r), [5.0 - o[1], 5.0 + o[0], 5.0 + o[2]]);
        assert_eq!((r[8], rec::ff(r, 0xc), rec::i16(r, 10), rec::u32(r, 4)), (rot, size, 210, 0x0080_6040));
        assert_eq!(rec::ff(r, 0x28), 0.5, "stale spin");
        let mut n = 0;
        let mut peak = 0.0f32;
        let z0 = rec::ff(r, 0x18);
        while s.pool.count > 0 {
            s.update_parts(&mut rng);
            n += 1;
            if s.pool.count > 0 {
                peak = peak.max(rec::ff(&s.pool.recs[i], 0x24));
                assert!((rec::ff(&s.pool.recs[i], 0x18) - (z0 - 0.02 * n as f32)).abs() < 1e-4);
            }
        }
        assert!(peak < 0.22 && peak > 0.2, "peak {peak}");
        assert_eq!(n, 28, "0.08 up to 0.22 in 11 steps, down to 0 in 17");
        assert_eq!(rng, t, "no draws in the update");
    }
}
