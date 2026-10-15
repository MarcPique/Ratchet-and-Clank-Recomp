//! Particle type 70, the flat decal that lingers and fades (level01 update `PartType70Update` 0x28a078, read from the
//! decomp; the update is the same code on levels 00–14, 16 and 18 (`overlay-diff`; on 15 and 17 the matcher paired it
//! with another function, not a real difference [L]). Its spawner is level-specific: level00 `0x274cf8` and its copies
//! on 04, 07, 09, 10, 12, 13, 14, 17 (the most-spawned unported type: `CreatePart(0x46)` in 9 overlays; their callers
//! are unported classes, G-PRT-001).
//!
//! **Spawn** `(angle f12, pos a0, frame a1)` (level00 `0x274cf8`): position = pos, RGBA 0x80404040, ALPHA 0x44,
//! **render kind 1** (byte1 = 1: a flat quad in the world XY plane), byte9 `trunc(2) + 0x20` = 0x22, texture = frame
//! `a1` of type 70's list, rotation `trunc(angle·128/π) − 0x20`, size 147000 (0x480f8e00), timer `ticks(120)`. No RNG.
//!
//! **Update**: the timer (s16) steps down by one; the step from 1 sets it back to 2 and takes one off the alpha byte
//! (the word − 0x1000000), so after the timer runs out the decal fades one alpha step every 2 ticks; alpha 0 → killed.
//! No RNG.

use super::{rec, Particles};
use crate::rng::Rng;

pub const TYPE: u8 = 70;

/// Level00 `0x274cf8(angle, pos, frame)` (no RNG).
pub fn spawn(sys: &mut Particles, angle: f32, pos: [f32; 4], frame: usize) -> Option<usize> {
    let i = sys.create_part(TYPE)?;
    let tex = sys.def_frame(TYPE, frame);
    let t = sys.time.ticks(0x78) as i16;
    let r = &mut sys.pool.recs[i];
    rec::set_v4(r, 0x10, pos);
    rec::set_u32(r, 4, 0x8040_4040);
    r[3] = 0x44;
    r[1] = 1;
    r[9] = 2 + 0x20;
    r[2] = tex;
    r[8] = ((angle * 128.0 / std::f32::consts::PI) as i32 as u8).wrapping_sub(0x20);
    rec::set_u32(r, 0xc, 0x480f_8e00);
    rec::set_i16(r, 10, t);
    Some(i)
}

/// Update 0x28a078 (no RNG).
pub fn update(sys: &mut Particles, i: usize, _rng: &mut Rng) {
    let r = &mut sys.pool.recs[i];
    let t = rec::i16(r, 10);
    rec::set_i16(r, 10, t.wrapping_sub(1));
    if t == 1 {
        rec::set_i16(r, 10, 2);
        rec::set_u32(r, 4, rec::u32(r, 4).wrapping_sub(0x100_0000));
    }
    if rec::u32(r, 4) & 0xff00_0000 == 0 { sys.kill_part(i); }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 120 ticks at full alpha, then one alpha step every 2 ticks: gone after 120 + 2·0x7f updates.
    #[test]
    fn decal_lingers_then_fades() {
        let mut s = Particles::new(None, Vec::new());
        let i = spawn(&mut s, std::f32::consts::FRAC_PI_2, [1.0, 2.0, 3.0, 1.0], 0).unwrap();
        let r = &s.pool.recs[i];
        assert_eq!((r[1], r[3], r[8], r[9], rec::u32(r, 4), rec::ff(r, 0xc)), (1, 0x44, 0x20, 0x22, 0x8040_4040, 147000.0));
        let mut rng = Rng::new();
        let mut n = 0;
        let mut first_fade = None;
        while s.pool.count > 0 {
            s.update_parts(&mut rng);
            n += 1;
            if first_fade.is_none() && s.pool.count > 0 && rec::u32(&s.pool.recs[i], 4) >> 24 != 0x80 { first_fade = Some(n); }
        }
        assert_eq!(first_fade, Some(120));
        assert_eq!(n, 120 + 2 * 0x7f);
        assert_eq!(rng, Rng::new());
    }
}
