//! Particle types 48 and 50, level 5's glints and falling sparks (level01 updates 0x286e20 (48) and 0x286f90 (50):
//! no Ghidra function until 2026-09-29, both the same code on all 19 levels, `overlay-diff`; spawners level05
//! `0x29bbe0` / `0x29be70`, read from the decomp). Their callers are level 5's unported classes (G-PRT-001).
//!
//! **48, spawn** `(size f12, pos a0, vel a1, life a2, spin a3, rgba t0)`: RGBA `rgba & 0xffffff` (alpha 0), byte9
//! `trunc(2) + 0x10` = 0x12, ALPHA 0x44, byte1 0, size·210000, texture `def[48][0]`, rotation `randi(256)` (the one
//! draw), velocity +0x20 (xyzw), +0x30 = 2/life, timer = life, +0x38 = rgba, +0x34 = rgba at alpha 0, +0x3c = spin.
//! **48, update**: rotation += spin; pos += vel; colour = `FastTweenColor(|t·(2/life) − 1|, rgba, rgba at alpha 0)`
//! (it fades in to the middle of its life and out again); the timer fires → killed.
//!
//! **50, spawn** `(size f12, floor f13, g f14, pos a0, vel a1, rgba a2, frame a3)`: byte9 `trunc(1) + 0x40` = 0x41,
//! ALPHA 0x44, byte1 0, texture `def[50][frame]`, RGBA, size·210000, rotation `trunc(randf(0, 255))`, timer
//! `ticks(15)`, velocity +0x20 (xyzw), floor +0x30, gravity +0x38, spin +0x34 = `trunc(randf(−3, 3))` (two draws).
//! **50, update**: once the timer has run out (it stays 0 and fires every tick) vel.z −= g; pos += vel; rotation +=
//! spin; frame `def[50][0]` grows by [0x160210]·dt·210000 (0.25 in the image, never written); below the floor →
//! killed. No RNG in either update.

use super::{fast_dec_timer, rec, tween_color, Particles};
use crate::rng::Rng;

pub const TYPE48: u8 = 48;
pub const TYPE50: u8 = 50;

/// Level05 `0x29bbe0(size, pos, vel, life, spin, rgba)`: one `randi(256)` with a record.
#[allow(clippy::too_many_arguments)]
pub fn spawn48(sys: &mut Particles, rng: &mut Rng, size: f32, pos: [f32; 4], vel: [f32; 4], life: i32, spin: i16, rgba: u32) -> Option<usize> {
    let i = sys.create_part(TYPE48)?;
    let def = sys.def_first(TYPE48);
    let rot = rng.randi(0x100) as u8;
    let r = &mut sys.pool.recs[i];
    rec::set_v4(r, 0x10, pos);
    rec::set_u32(r, 4, rgba & 0xff_ffff);
    r[9] = 2 + 0x10;
    r[3] = 0x44;
    r[1] = 0;
    rec::set_ff(r, 0xc, size * 210000.0);
    r[2] = def;
    r[8] = rot;
    rec::set_v4(r, 0x20, vel);
    rec::set_ff(r, 0x30, 2.0 / life as f32);
    rec::set_i16(r, 10, life as i16);
    rec::set_u32(r, 0x38, rgba);
    rec::set_i16(r, 0x3c, spin);
    rec::set_u32(r, 0x34, rgba & 0xff_ffff);
    Some(i)
}

/// Update 0x286e20 (no RNG).
pub fn update48(sys: &mut Particles, i: usize, _rng: &mut Rng) {
    let r = &mut sys.pool.recs[i];
    r[8] = r[8].wrapping_add(r[0x3c]);
    let (p, v) = (rec::v4(r, 0x10), rec::v4(r, 0x20));
    rec::set_v4(r, 0x10, std::array::from_fn(|k| p[k] + v[k]));
    let f = (rec::i16(r, 10) as f32 * rec::ff(r, 0x30) - 1.0).abs();
    rec::set_u32(r, 4, tween_color(f.to_bits(), rec::u32(r, 0x38), rec::u32(r, 0x34)));
    if fast_dec_timer(r, 10) != 0 { sys.kill_part(i); }
}

/// Level05 `0x29be70(size, floor, g, pos, vel, rgba, frame)`: two draws with a record.
#[allow(clippy::too_many_arguments)]
pub fn spawn50(sys: &mut Particles, rng: &mut Rng, size: f32, floor: f32, g: f32, pos: [f32; 4], vel: [f32; 4], rgba: u32, frame: usize) -> Option<usize> {
    let i = sys.create_part(TYPE50)?;
    let tex = sys.def_frame(TYPE50, frame);
    let rot = rng.randf(0.0, 255.0) as i32 as u8;
    let t15 = sys.time.ticks(0xf) as i16;
    let spin = rng.randf(-3.0, 3.0) as i32;
    let r = &mut sys.pool.recs[i];
    rec::set_v4(r, 0x10, pos);
    r[9] = 1 + 0x40;
    r[3] = 0x44;
    r[1] = 0;
    r[2] = tex;
    rec::set_u32(r, 4, rgba);
    rec::set_ff(r, 0xc, size * 210000.0);
    r[8] = rot;
    rec::set_i16(r, 10, t15);
    rec::set_v4(r, 0x20, vel);
    rec::set_ff(r, 0x30, floor);
    rec::set_ff(r, 0x38, g);
    rec::set_u32(r, 0x34, spin as u32);
    Some(i)
}

/// Update 0x286f90 (no RNG).
pub fn update50(sys: &mut Particles, i: usize, _rng: &mut Rng) {
    let first = sys.def_first(TYPE50);
    let r = &mut sys.pool.recs[i];
    if fast_dec_timer(r, 10) != 0 { rec::set_ff(r, 0x28, rec::ff(r, 0x28) - rec::ff(r, 0x38)); }
    let (p, v) = (rec::v4(r, 0x10), rec::v4(r, 0x20));
    rec::set_v4(r, 0x10, std::array::from_fn(|k| p[k] + v[k]));
    r[8] = r[8].wrapping_add(r[0x34]);
    if r[2] == first { rec::set_ff(r, 0xc, rec::ff(r, 0xc) + 0.25 * (1.0 / 60.0) * 210000.0); }
    if rec::ff(r, 0x18) < rec::ff(r, 0x30) { sys.kill_part(i); }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 48: fades in to mid-life (full colour) and out; one draw.
    #[test]
    fn glint_fades_in_and_out() {
        let mut s = Particles::new(None, Vec::new());
        let mut rng = Rng::new();
        let i = spawn48(&mut s, &mut rng, 0.1, [5.0; 4], [0.0; 4], 20, 3, 0x80ff_8040).unwrap();
        assert_eq!(rec::u32(&s.pool.recs[i], 4), 0x00ff_8040);
        let mut alphas = Vec::new();
        while s.pool.count > 0 {
            s.update_parts(&mut rng);
            if s.pool.count > 0 { alphas.push(rec::u32(&s.pool.recs[i], 4) >> 24); }
        }
        assert_eq!(alphas.len(), 19);
        assert_eq!((alphas[0], *alphas.iter().max().unwrap()), (0, 0x80));
    }

    /// 50: coasts for 15 ticks, then falls under g, dies below the floor; two spawn draws.
    #[test]
    fn spark_coasts_then_falls() {
        let mut s = Particles::new(None, Vec::new());
        let mut rng = Rng::new();
        let mut t = rng;
        let i = spawn50(&mut s, &mut rng, 0.1, 0.0, 0.01, [5.0, 5.0, 1.0, 1.0], [0.0; 4], 0x8080_8080, 0).unwrap();
        let rot = t.randf(0.0, 255.0) as i32 as u8;
        let spin = t.randf(-3.0, 3.0) as i32;
        assert_eq!(rng, t);
        assert_eq!((s.pool.recs[i][8], rec::u32(&s.pool.recs[i], 0x34) as i32), (rot, spin));
        for _ in 0..14 { s.update_parts(&mut rng); }
        assert_eq!(rec::ff(&s.pool.recs[i], 0x18), 1.0, "coasting");
        let mut n = 14;
        while s.pool.count > 0 {
            s.update_parts(&mut rng);
            n += 1;
        }
        // From tick 15: v = −0.01·k, z = 1 − 0.01·k(k+1)/2 < 0 at k = 14.
        assert_eq!(n, 15 + 13);
    }
}
