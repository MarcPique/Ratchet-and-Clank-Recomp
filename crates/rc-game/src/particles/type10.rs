//! Particle type 10, the flyers' engine sparks: a yellow additive puff that drifts, slows, grows and fades (level01
//! spawner `PartType10Spawn` 0x27f660, update 0x27f788, read from the decomp; the update is the same code on all 19
//! levels, `overlay-diff`). The Blarg flyers' path driver (`FlyerPathDriver` 0x2f5168, `classes::flyer`) makes two per
//! engine joint every 7th tick when its flag byte +0x120 has bit 0.
//!
//! **Spawn** `(pos, vel)`: position = pos (xyzw), RGBA `0x270ea0(0.425, 0.425, 0.225, 1)` = 0xff396c6c, byte9
//! `trunc(8) − 0x60` = 0xa8 (near 2, far 320), ALPHA 0x48 (additive), byte1 0, size `randf(60900, 90300)` (the one
//! draw), rotation 0, texture **`def[3][0]`** (`*0x1b250c`: the def table 0x1b2500 + 3·4, not type 10's list), the colour
//! as floats +0x30 / +0x34 / +0x38 = (0.425, 0.425, 0.225), alpha +0x3c = 1, timer `ticks(87)`, velocity +0x20 = vel.
//!
//! **Update**: alpha −= 0.04·speed; alpha ≤ 0, or else the timer firing → killed (the alpha ends it after 25 ticks).
//! Else size += 12000·speed; blue below 0.225 → += 0.0484·speed (it starts at 0.225: never); RGBA = the low 16 bits
//! kept | `trunc(blue·255)` << 16 | `trunc(alpha·255)` << 24; pos += vel; vel ·= 1 − 0.1·speed; rotation byte − 1.
//! No RNG. Native `f32`, speed [0x15ed60] = 1 (NTSC).

use super::{fast_dec_timer, rec, type32::rgba, Particles};
use crate::rng::Rng;

pub const TYPE: u8 = 10;

/// `PartType10Spawn(pos, vel)` 0x27f660: one draw with a record; none when the pool is full.
pub fn spawn(sys: &mut Particles, rng: &mut Rng, pos: [f32; 4], vel: [f32; 4]) -> Option<usize> {
    let i = sys.create_part(TYPE)?;
    let def = sys.def_first(3);
    let t87 = sys.time.ticks(0x57);
    let size = rng.randf(f32::from_bits(0x476d_e400), f32::from_bits(0x47b0_5e00));
    let r = &mut sys.pool.recs[i];
    rec::set_v4(r, 0x10, pos);
    rec::set_u32(r, 4, rgba(f32::from_bits(0x3ed9_999a), f32::from_bits(0x3ed9_999a), f32::from_bits(0x3e66_6666), 1.0));
    r[1] = 0;
    r[9] = 8u8.wrapping_sub(0x60);
    r[3] = 0x48;
    rec::set_ff(r, 0xc, size);
    r[8] = 0;
    r[2] = def;
    rec::set_ff(r, 0x34, f32::from_bits(0x3ed9_999a));
    rec::set_ff(r, 0x38, f32::from_bits(0x3e66_6666));
    rec::set_ff(r, 0x3c, 1.0);
    rec::set_ff(r, 0x30, f32::from_bits(0x3ed9_999a));
    rec::set_i16(r, 10, t87 as i16);
    rec::set_v4(r, 0x20, vel);
    Some(i)
}

/// Update 0x27f788 (no RNG).
pub fn update(sys: &mut Particles, i: usize, _rng: &mut Rng) {
    let r = &mut sys.pool.recs[i];
    let a = rec::ff(r, 0x3c) + -0.04;
    rec::set_ff(r, 0x3c, a);
    if a <= 0.0 || fast_dec_timer(r, 10) != 0 {
        sys.kill_part(i);
        return;
    }
    rec::set_ff(r, 0xc, rec::ff(r, 0xc) + 12000.0);
    let mut b = rec::ff(r, 0x38);
    if b < 0.225 {
        b += 0.0484;
        rec::set_ff(r, 0x38, b);
    }
    let c = (rec::u32(r, 4) & 0xffff) | (((b * 255.0) as i32 as u32) & 0xff) << 16 | ((a * 255.0) as i32 as u32) << 24;
    rec::set_u32(r, 4, c);
    let (p, v) = (rec::v3(r, 0x10), rec::v3(r, 0x20));
    rec::set_v3(r, 0x10, [p[0] + v[0], p[1] + v[1], p[2] + v[2]]);
    let k = -0.100_000_024f32 + 1.0;
    rec::set_v3(r, 0x20, v.map(|x| x * k));
    r[8] = r[8].wrapping_sub(1);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The record, the draw, the 25-tick fade (alpha 1 − 0.04·n), the drift slowing by 0.9 a tick, the size growing.
    #[test]
    fn spawn_fades_drifts_and_dies() {
        let mut s = Particles::new(None, Vec::new());
        let mut rng = Rng::new();
        let mut twin = rng;
        let i = spawn(&mut s, &mut rng, [1.0, 2.0, 3.0, 1.0], [0.02, 0.0, 0.0, 0.0]).unwrap();
        let size = twin.randf(60900.0, 90300.0);
        assert_eq!(rng, twin, "one draw");
        let r = &s.pool.recs[i];
        assert_eq!((r[1], r[3], r[8], r[9]), (0, 0x48, 0, 0xa8));
        assert_eq!(rec::u32(r, 4), 0xff39_6c6c);
        assert_eq!(rec::ff(r, 0xc), size);
        assert_eq!(rec::i16(r, 10), 87);
        s.update_parts(&mut rng);
        let r = &s.pool.recs[i];
        assert_eq!(rec::v3(r, 0x10), [1.02, 2.0, 3.0]);
        assert!((rec::ff(r, 0x20) - 0.018).abs() < 1e-7);
        assert_eq!(rec::ff(r, 0xc), size + 12000.0);
        assert_eq!(rec::u32(r, 4), 0xf439_6c6c, "alpha 0.96 → 0xf4, blue 0.225 → 0x39");
        assert_eq!(r[8], 0xff);
        let mut a = 0.96f32;
        let mut n = 1;
        while s.pool.count > 0 {
            s.update_parts(&mut rng);
            a += -0.04;
            n += 1;
            if s.pool.count > 0 { assert_eq!(rec::u32(&s.pool.recs[i], 4) >> 24, (a * 255.0) as u32); }
        }
        assert_eq!(n, 25, "alpha reaches 0 on the 25th update");
        assert_eq!(rng, twin, "no draws in the update");
        assert_eq!(s.stats.unported_kills[10], 0);
    }
}
