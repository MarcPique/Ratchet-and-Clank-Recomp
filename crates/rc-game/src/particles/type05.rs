//! Particle types 5 and 7, the growing flash / pop puff (level01 spawner `PartType05Spawn` 0x27e750, updates 0x27e850
//! (type 5) and 0x27f200 (type 7), read from the decomp). The two updates are **the same code**: one cluster hash
//! (`tools/ghidra/names/clusters.tsv` 421dbcd1…, 176 bytes, both addresses in every level's overlay, which is why
//! `overlay-diff` cannot pair them: two identical candidates), so type 7 is a table row on this update. No level01
//! code creates type 7 (it has no spawner).
//!
//! **Spawn** `(grow f12, size f13, pos a0, r a1, g a2, b a3, life t0)`: life 0 → nothing (no `CreatePart`). Else
//! position = pos, RGBA = `0x7f << 24 | b << 16 | g << 8 | r` (bytes), byte9 `trunc(4) − 0x60` = 0xa4, ALPHA 0x48
//! (additive), size, byte1 0, rotation 0, +0x20 = life (i32), texture `def[5][0]`, +0x24 = grow / life. No RNG.
//! Callers: the decoy's pop `0x2d90a8` (10), the Glove of Doom canister's pop `0x2ddce0` (20), the morph's flash
//! (`0x2defb0`: `(1e6, 0, pos, 0x7f, 0x7f, 0x7f, ticks(15))`), the census unit U412 (class 170, level 13).
//!
//! **Update**: `FastDecTimer__FRi(+0x20)` fires → killed; else size += step and, below `ticks(6)` left, alpha =
//! `t·0x7f / ticks(6)` (integer). No RNG. Native `f32`.

use super::{rec, Particles};
use crate::rng::Rng;

pub const TYPE: u8 = 5;

/// `PartType05Spawn(grow, size, pos, r, g, b, life)` 0x27e750 (no RNG): the record, or none when `life` is 0 or the
/// pool is full.
#[allow(clippy::too_many_arguments)]
pub fn spawn(sys: &mut Particles, grow: f32, size: f32, pos: [f32; 4], r: u32, g: u32, b: u32, life: i32) -> Option<usize> {
    if life == 0 { return None; }
    let i = sys.create_part(TYPE)?;
    let def = sys.def_first(TYPE);
    let rc = &mut sys.pool.recs[i];
    rec::set_v4(rc, 0x10, pos);
    rec::set_u32(rc, 4, (b & 0xff) << 16 | (g & 0xff) << 8 | 0x7f00_0000 | (r & 0xff));
    rc[9] = 4u8.wrapping_sub(0x60);
    rc[3] = 0x48;
    rec::set_ff(rc, 0xc, size);
    rc[1] = 0;
    rc[8] = 0;
    rec::set_u32(rc, 0x20, life as u32);
    rc[2] = def;
    rec::set_ff(rc, 0x24, grow / life as f32);
    Some(i)
}

/// Updates 0x27e850 / 0x27f200 (no RNG).
pub fn update(sys: &mut Particles, i: usize, _rng: &mut Rng) {
    let t6 = sys.time.ticks(6);
    let r = &mut sys.pool.recs[i];
    // FastDecTimer__FRi 0x220e78: 1 when 0, else t = max(t, 1) − 1 and 2 when it reaches 0.
    let t = rec::u32(r, 0x20) as i32;
    let fired = t == 0 || {
        let n = t.max(1) - 1;
        rec::set_u32(r, 0x20, n as u32);
        n <= 0
    };
    if fired {
        sys.kill_part(i);
        return;
    }
    rec::set_ff(r, 0xc, rec::ff(r, 0xc) + rec::ff(r, 0x24));
    let t = rec::u32(r, 0x20) as i32;
    if t < t6 {
        let a = (t * 0x7f) / t6.max(1);
        rec::set_u32(r, 4, rec::u32(r, 4) & 0xff_ffff | (a as u32) << 24);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The morph's flash `(1e6, 0, pos, 0x7f, 0x7f, 0x7f, ticks(15))`: grows 1e6/15 a tick, fades over its last 6
    /// ticks, lives 14 updates (the timer fires on the 15th); life 0 spawns nothing; type 7 runs the same update.
    #[test]
    fn flash_grows_fades_and_dies() {
        for ty in [5u8, 7] {
            let mut s = Particles::new(None, Vec::new());
            assert!(spawn(&mut s, 1e6, 0.0, [0.0; 4], 1, 2, 3, 0).is_none());
            assert_eq!(s.stats.created, 0);
            let i = spawn(&mut s, 1e6, 0.0, [1.0, 2.0, 3.0, 1.0], 0x7f, 0x7f, 0x7f, 15).unwrap();
            s.pool.recs[i][0] = ty;
            let r = &s.pool.recs[i];
            assert_eq!((rec::u32(r, 4), r[9], r[3]), (0x7f7f_7f7f, 0xa4, 0x48));
            assert_eq!(rec::ff(r, 0x24), 1e6 / 15.0);
            let mut rng = Rng::new();
            let before = rng;
            let mut n = 0;
            let mut alphas = Vec::new();
            while s.pool.count > 0 {
                s.update_parts(&mut rng);
                n += 1;
                if s.pool.count > 0 { alphas.push(rec::u32(&s.pool.recs[i], 4) >> 24); }
            }
            assert_eq!(n, 15);
            assert_eq!(alphas, [0x7f; 9].into_iter().chain([105, 84, 63, 42, 21]).collect::<Vec<_>>(), "t = 5..1 → t·127/6");
            assert_eq!(rng, before);
            assert_eq!(s.stats.unported_kills[ty as usize], 0);
        }
    }
}
