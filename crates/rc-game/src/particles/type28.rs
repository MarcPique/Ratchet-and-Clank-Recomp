//! Particle type 28, the pulsing drifting mote (level01 spawner `PartType28Spawn` 0x282ef0, update 0x2831b0, read from
//! the decomp and the spawner's disassembly; both the same code on all 19 levels, `overlay-diff`). The census's unit
//! U397 (class 1281, level 12) calls the spawner (G-PRT-001 consumer; its class is not ported).
//!
//! **Spawn** `(spread f12, pos a0, rows a1)` (the decompiler swaps a0 / a1): offset `(randf(±0.03), randf(±0.03), 0)`,
//! turned by the rows when given (`fun_001fa298` + `fun_001f9d20`), position = pos + offset, then z −= 0.05. The colour
//! (three raw `rand()`): `a = rand() % 0x60` (0 → 1), `b = (rand() % a) % 64` (0 → 1), `c = rand() % b`; RGBA =
//! `(a + 0x8f) | (b + 0x8f) << 8 | (c % 0x30 + 0x6f) << 16`, alpha byte 0. byte9 `trunc(4) + 0x40` = 0x44, ALPHA 0x48,
//! byte1 0, rotation 0, texture `def[28][0]`, alpha +0x24 = `randf(0, 0.0224) + 1e−5`, size `randf(10080, 19950)`,
//! +0x28 (the spin phase) 0, timer `ticks(252)`, +0x2c (the pulse direction) 1, velocity +0x30: with rows
//! `rows · (spread, 0, −0.0025)` (`MatrixMulVec3`), without `(randf(±spread), randf(±spread), −0.0025)`. Draws: 2 + 3
//! raw + 2 (+ 2 without rows).
//!
//! **Update**: dir > 0 → alpha += 0.007·speed·dir; else alpha > 0.03 → alpha += 0.007·dir; else alpha += 0.007·dir·0.2
//! and size += 5460 (the tail fades slower and grows twice as fast). alpha ≤ 0.0244, or else the timer → killed (a
//! mote spawned below 0.0174 dies on its first update: the game's numbers). alpha ≥ 0.12 → dir flips and the step is
//! taken back. size += 5460; A = `trunc(alpha·255)`; spin += 0.002 (wraps past 1), rotation byte `trunc(spin·255)`;
//! pos += vel. No RNG. Native `f32`, speed 1.

use super::{fast_dec_timer, rec, Particles};
use crate::rng::Rng;

pub const TYPE: u8 = 28;

/// `PartType28Spawn(spread, pos, rows)` 0x282ef0: 7 draws (9 without rows) with a record; none when the pool is full.
pub fn spawn(sys: &mut Particles, rng: &mut Rng, spread: f32, pos: [f32; 4], rows: Option<[[f32; 3]; 3]>) -> Option<usize> {
    let i = sys.create_part(TYPE)?;
    let def = sys.def_first(TYPE);
    let j = f32::from_bits(0x3cf5_c28f);
    let o = [rng.randf(-j, j), rng.randf(-j, j), 0.0];
    let o = match rows {
        Some(m) => std::array::from_fn(|l| m[0][l] * o[0] + m[1][l] * o[1] + m[2][l] * o[2]),
        None => o,
    };
    let a = rng.rand() % 0x60;
    let a = if a != 0 { a } else { 1 };
    let b = (rng.rand() % a) % 64;
    let b = if b != 0 { b } else { 1 };
    let c = rng.rand() % b;
    let rgba = (a + 0x8f) as u32 | ((b + 0x8f) as u32) << 8 | ((c % 0x30 + 0x6f) as u32) << 16;
    let alpha = rng.randf(0.0, f32::from_bits(0x3cb7_0e1f)) + f32::from_bits(0x3727_c5ac);
    let size = rng.randf(10080.0, 19950.0);
    let t252 = sys.time.ticks(0xfc) as i16;
    let vel = match rows {
        Some(m) => std::array::from_fn(|l| m[0][l] * spread + m[2][l] * f32::from_bits(0xbb23_d70a)),
        None => [rng.randf(-spread, spread), rng.randf(-spread, spread), f32::from_bits(0xbb23_d70a)],
    };
    let r = &mut sys.pool.recs[i];
    rec::set_v4(r, 0x10, [pos[0] + o[0], pos[1] + o[1], pos[2] + o[2] - 0.05, pos[3]]);
    rec::set_u32(r, 4, rgba);
    r[9] = 4 + 0x40;
    r[3] = 0x48;
    r[1] = 0;
    r[8] = 0;
    r[2] = def;
    rec::set_ff(r, 0x24, alpha);
    rec::set_ff(r, 0xc, size);
    rec::set_ff(r, 0x28, 0.0);
    rec::set_i16(r, 10, t252);
    rec::set_ff(r, 0x2c, 1.0);
    rec::set_v3(r, 0x30, vel);
    Some(i)
}

/// Update 0x2831b0 (no RNG).
pub fn update(sys: &mut Particles, i: usize, _rng: &mut Rng) {
    let r = &mut sys.pool.recs[i];
    let dir = rec::ff(r, 0x2c);
    let a = rec::ff(r, 0x24);
    if 0.0 < dir || 0.03 < a {
        rec::set_ff(r, 0x24, a + 0.007 * dir);
    } else {
        rec::set_ff(r, 0x24, a + 0.007 * dir * 0.2);
        rec::set_ff(r, 0xc, rec::ff(r, 0xc) + 5460.0);
    }
    if rec::ff(r, 0x24) <= 0.0244 || fast_dec_timer(r, 10) != 0 {
        sys.kill_part(i);
        return;
    }
    if 0.12 <= rec::ff(r, 0x24) {
        let d = rec::ff(r, 0x2c);
        rec::set_ff(r, 0x2c, -d);
        rec::set_ff(r, 0x24, rec::ff(r, 0x24) + 0.007 * -d);
    }
    rec::set_ff(r, 0xc, rec::ff(r, 0xc) + 5460.0);
    let al = (rec::ff(r, 0x24) * 255.0) as i32;
    rec::set_u32(r, 4, rec::u32(r, 4) & 0xff_ffff | (al as u32) << 24);
    let mut spin = rec::ff(r, 0x28) + 0.002;
    if 1.0 < spin { spin -= 1.0; }
    rec::set_ff(r, 0x28, spin);
    r[8] = (spin * 255.0) as i32 as u8;
    let (p, v) = (rec::v3(r, 0x10), rec::v3(r, 0x30));
    rec::set_v3(r, 0x10, [p[0] + v[0], p[1] + v[1], p[2] + v[2]]);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The spawn's draws in the game's order (with and without rows) and the colour rule.
    #[test]
    fn spawn_draw_order_and_colour() {
        for rows in [None, Some([[0.0, 1.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]])] {
            let mut s = Particles::new(None, Vec::new());
            let mut rng = Rng::new();
            let mut t = rng;
            let i = spawn(&mut s, &mut rng, 0.01, [5.0, 5.0, 5.0, 1.0], rows).unwrap();
            let j = f32::from_bits(0x3cf5_c28f);
            let (ox, oy) = (t.randf(-j, j), t.randf(-j, j));
            let a = t.rand() % 0x60;
            let a = a.max(1);
            let b = ((t.rand() % a) % 64).max(1);
            let c = t.rand() % b;
            let alpha = t.randf(0.0, f32::from_bits(0x3cb7_0e1f)) + 1e-5;
            let size = t.randf(10080.0, 19950.0);
            let vel = if rows.is_none() { [t.randf(-0.01, 0.01), t.randf(-0.01, 0.01), -0.0025] } else { [0.0, 0.01, -0.0025] };
            assert_eq!(rng, t);
            let r = &s.pool.recs[i];
            assert_eq!(rec::u32(r, 4), (a + 0x8f) as u32 | ((b + 0x8f) as u32) << 8 | ((c % 48 + 0x6f) as u32) << 16);
            assert_eq!((rec::ff(r, 0x24), rec::ff(r, 0xc), rec::i16(r, 10), r[9]), (alpha, size, 252, 0x44));
            assert_eq!(rec::v3(r, 0x30), vel);
            let p = rec::pos(r);
            let o = if rows.is_none() { [ox, oy] } else { [-oy, ox] };
            assert_eq!(p, [5.0 + o[0], 5.0 + o[1], 5.0 - 0.05]);
        }
    }

    /// The pulse: up by 0.007 a tick to 0.12, back down, the slow tail below 0.03, killed at 0.0244; a faint start dies
    /// on the first update.
    #[test]
    fn pulses_and_dies() {
        let mut s = Particles::new(None, Vec::new());
        let mut rng = Rng::new();
        let i = spawn(&mut s, &mut rng, 0.0, [5.0, 5.0, 5.0, 1.0], None).unwrap();
        rec::set_ff(&mut s.pool.recs[i], 0x24, 0.02);
        let size0 = rec::ff(&s.pool.recs[i], 0xc);
        let mut peak = 0.0f32;
        let mut n = 0;
        while s.pool.count > 0 && n < 300 {
            s.update_parts(&mut rng);
            n += 1;
            if s.pool.count > 0 { peak = peak.max(rec::ff(&s.pool.recs[i], 0x24)); }
        }
        assert!(peak < 0.12 && peak > 0.11, "peak {peak}");
        // 0.02 → 0.12 in 15 steps, back to 0.03 in ~13, then 0.0014 a tick down to 0.0244: ~4 more.
        assert!((30..=34).contains(&n), "lived {n}");
        assert!(size0 < 10080.0 + 19950.0);
        let j = spawn(&mut s, &mut rng, 0.0, [5.0, 5.0, 5.0, 1.0], None).unwrap();
        rec::set_ff(&mut s.pool.recs[j], 0x24, 0.01);
        s.update_parts(&mut rng);
        assert_eq!(s.pool.count, 0, "0.017 ≤ 0.0244: killed on the first update");
    }
}
