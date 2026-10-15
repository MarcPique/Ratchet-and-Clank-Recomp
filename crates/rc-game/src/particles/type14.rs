//! Particle type 14, the muzzle flash on a moby (level01 spawner `PartType14Spawn` 0x280970, update 0x280ac8, read from
//! the decomp; the same code on all 19 levels, `overlay-diff`). Its caller is the update of class 156 (`0x2c7530`, a
//! projectile created by code: not placed on any level, not ported; G-PRT-001).
//!
//! **Spawn** `(spin a0, moby a1)`: position = the moby's + `randf_sym(0, 0.03)` per axis (three draws), RGBA
//! 0xffffffff, byte1 0, ALPHA 0x48, size 29400, byte9 `trunc(4) − 0x60` = 0xa4, rotation `randi(255)`, texture
//! `def[14][0]`, timer `ticks(6)`, +0x28 the moby, +0x24 = 1 (the fading mode), +0x20 = 0 for spin 0, else ±spin
//! (`randi(2)`: 0 → −spin).
//!
//! **Update**: rotation += the spin's low byte. Mode 0 (+0x24 = 0, the caller's): position = the moby's moved 0.01
//! toward the camera (0x167240). Mode 1: the timer fires → killed; else A = `trunc(t·(127/ticks(6)))` over white.
//! No RNG.

use super::{fast_dec_timer, rec, Particles, Record};
use crate::rng::Rng;

pub const TYPE: u8 = 14;

/// `PartType14Spawn(spin, moby)` 0x280970 with the moby's position: 4 or 5 draws with a record; none when the pool is
/// full.
pub fn spawn(sys: &mut Particles, rng: &mut Rng, spin: i32, moby: usize, at: [f32; 4]) -> Option<usize> {
    let i = sys.create_part(TYPE)?;
    let j = f32::from_bits(0x3cf5_c28f);
    let d = [rng.randf_sym(0.0, j), rng.randf_sym(0.0, j), rng.randf_sym(0.0, j)];
    let rot = rng.randi(0xff) as u8;
    let def = sys.def_first(TYPE);
    let t6 = sys.time.ticks(6) as i16;
    let s = if spin == 0 { 0 } else if rng.randi(2) == 0 { -spin } else { spin };
    let r = &mut sys.pool.recs[i];
    rec::set_v4(r, 0x10, [at[0] + d[0], at[1] + d[1], at[2] + d[2], at[3]]);
    rec::set_u32(r, 4, 0xffff_ffff);
    r[1] = 0;
    r[3] = 0x48;
    rec::set_ff(r, 0xc, 29400.0);
    r[9] = 4u8.wrapping_sub(0x60);
    r[8] = rot;
    r[2] = def;
    rec::set_i16(r, 10, t6);
    rec::set_u32(r, 0x28, moby as u32 + 1);
    rec::set_u32(r, 0x24, 1);
    rec::set_u32(r, 0x20, s as u32);
    Some(i)
}

/// The moby a live type-14 record holds.
pub fn moby_of(r: &Record) -> Option<usize> {
    (r[0] == TYPE && r[1] & super::FLAG_DEAD == 0).then(|| (rec::u32(r, 0x28) as usize).checked_sub(1)).flatten()
}

/// Update 0x280ac8 (no RNG).
pub fn update(sys: &mut Particles, i: usize, _rng: &mut Rng) {
    let frame = moby_of(&sys.pool.recs[i]).and_then(|m| sys.moby_frames.get(&m).copied());
    let cam = sys.camera.map(f32::from_bits);
    let t6 = sys.time.ticks(6);
    let r = &mut sys.pool.recs[i];
    r[8] = r[8].wrapping_add(r[0x20]);
    if rec::u32(r, 0x24) == 0 {
        if let Some(f) = frame {
            let d = [cam[0] - f.pos[0], cam[1] - f.pos[1], cam[2] - f.pos[2]];
            let n = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
            let k = if n > 0.0 { 0.01 / n } else { 0.0 };
            rec::set_v3(r, 0x10, [f.pos[0] + d[0] * k, f.pos[1] + d[1] * k, f.pos[2] + d[2] * k]);
        }
        return;
    }
    if fast_dec_timer(r, 10) != 0 {
        sys.kill_part(i);
        return;
    }
    let a = (rec::i16(r, 10) as f32 * (127.0 / t6 as f32)) as i32;
    rec::set_u32(r, 4, (a as u32) << 24 | 0xff_ffff);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The draws (3 jitter, the rotation, the spin's sign), the 6-tick fade from 127.
    #[test]
    fn flash_fades_in_six() {
        let mut s = Particles::new(None, Vec::new());
        let mut rng = Rng::new();
        let mut t = rng;
        let i = spawn(&mut s, &mut rng, 5, 3, [1.0, 2.0, 3.0, 1.0]).unwrap();
        let j = f32::from_bits(0x3cf5_c28f);
        let d = [t.randf_sym(0.0, j), t.randf_sym(0.0, j), t.randf_sym(0.0, j)];
        let rot = t.randi(0xff) as u8;
        let sp = if t.randi(2) == 0 { -5 } else { 5 };
        assert_eq!(rng, t);
        let r = &s.pool.recs[i];
        assert_eq!((rec::pos(r), r[8], rec::u32(r, 0x20) as i32, r[9]), ([1.0 + d[0], 2.0 + d[1], 3.0 + d[2]], rot, sp, 0xa4));
        let mut alphas = Vec::new();
        while s.pool.count > 0 {
            s.update_parts(&mut rng);
            if s.pool.count > 0 { alphas.push(rec::u32(&s.pool.recs[i], 4) >> 24); }
        }
        assert_eq!(alphas, [105, 84, 63, 42, 21]);
        assert_eq!(rng, t);
    }
}
