//! Particle type 39, the glow held on a moby (level01 update `PartType39Update` 0x284c98, the same code on all 19
//! levels, `overlay-diff`; spawner level08 `0x279f00`; read from the decomp). Its caller is level 8's unported class
//! (G-PRT-001).
//!
//! The update and the spawner read four small-data words no code writes (their values in the image, the same on
//! levels 01, 05, 08): size gp−0x6a00 = 140000, RGBA gp−0x69fc = 0x20ffff20, pull gp−0x69f8 = 0, spin gp−0x69f4 = 1.
//!
//! **Spawn** `(moby a0)`: position = the moby's moved `pull` (0) toward the camera, RGBA 0x20ffff20, byte9 `trunc(4) +
//! 0x40` = 0x44, ALPHA 0x48, byte1 0, texture `def[39][0]`, size 140000, timer `ticks(30)`, +0x20 = 255.0, +0x24 the
//! moby, the frame counter +0x28 = 0. No RNG.
//!
//! **Update**: size and RGBA re-read; rotation + 1; position = the moby's moved `pull` toward the camera; texture
//! `def[39][counter / 2]`, counter + 1 (past 30 → 0: a 16-frame loop); the timer is **not** stepped here: the record
//! lives until its caller sets +0x0a to 0 (then killed). No RNG. The moby's position comes from
//! [`Particles::moby_frames`] (a moby missing there: the position stays [L]).

use super::{rec, Particles, Record};
use crate::rng::Rng;

pub const TYPE: u8 = 39;
const SIZE: f32 = 140000.0;
const RGBA: u32 = 0x20ff_ff20;
const PULL: f32 = 0.0;

fn pulled(p: [f32; 3], cam: [f32; 3]) -> [f32; 3] {
    let d = [p[0] - cam[0], p[1] - cam[1], p[2] - cam[2]];
    let n = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    if n > 0.0 { [p[0] + d[0] * PULL / n, p[1] + d[1] * PULL / n, p[2] + d[2] * PULL / n] } else { p }
}

/// Level08 `0x279f00(moby)` with the moby's position (no RNG).
pub fn spawn(sys: &mut Particles, moby: usize, at: [f32; 3]) -> Option<usize> {
    let i = sys.create_part(TYPE)?;
    let def = sys.def_first(TYPE);
    let t30 = sys.time.ticks(0x1e) as i16;
    let pos = pulled(at, sys.camera.map(f32::from_bits));
    let r = &mut sys.pool.recs[i];
    rec::set_v3(r, 0x10, pos);
    rec::set_u32(r, 4, RGBA);
    r[9] = 4 + 0x40;
    r[3] = 0x48;
    r[1] = 0;
    r[2] = def;
    rec::set_ff(r, 0xc, SIZE);
    rec::set_i16(r, 10, t30);
    rec::set_ff(r, 0x20, 255.0);
    rec::set_u32(r, 0x24, moby as u32 + 1);
    rec::set_u32(r, 0x28, 0);
    Some(i)
}

/// The moby a live type-39 record holds.
pub fn moby_of(r: &Record) -> Option<usize> {
    (r[0] == TYPE && r[1] & super::FLAG_DEAD == 0).then(|| (rec::u32(r, 0x24) as usize).checked_sub(1)).flatten()
}

/// Update 0x284c98 (no RNG).
pub fn update(sys: &mut Particles, i: usize, _rng: &mut Rng) {
    let mp = moby_of(&sys.pool.recs[i]).and_then(|m| sys.moby_frames.get(&m)).map(|f| f.pos);
    let cam = sys.camera.map(f32::from_bits);
    let n = rec::u32(&sys.pool.recs[i], 0x28) as i32;
    let tex = sys.def_frame(TYPE, (n / 2) as usize);
    let r = &mut sys.pool.recs[i];
    rec::set_ff(r, 0xc, SIZE);
    rec::set_u32(r, 4, RGBA);
    r[8] = r[8].wrapping_add(1);
    if let Some(p) = mp { rec::set_v3(r, 0x10, pulled(p, cam)); }
    r[2] = tex;
    let n = n + 1;
    rec::set_u32(r, 0x28, if 0x1e < n { 0 } else { n as u32 });
    if rec::i16(r, 10) == 0 { sys.kill_part(i); }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::particles::MobyFrame;

    /// Held on the moby until the caller zeroes the timer; the counter loops over 0..=30.
    #[test]
    fn held_until_the_caller_ends_it() {
        let mut s = Particles::new(None, Vec::new());
        let mut rng = Rng::new();
        let i = spawn(&mut s, 2, [1.0, 2.0, 3.0]).unwrap();
        s.moby_frames.insert(2, MobyFrame { pos: [4.0, 5.0, 6.0], ..Default::default() });
        for _ in 0..100 { s.update_parts(&mut rng); }
        let r = &s.pool.recs[i];
        assert_eq!((rec::pos(r), r[8], rec::u32(r, 0x28)), ([4.0, 5.0, 6.0], 100, 100 % 31));
        rec::set_i16(&mut s.pool.recs[i], 10, 0);
        s.update_parts(&mut rng);
        assert_eq!(s.pool.count, 0);
    }
}
