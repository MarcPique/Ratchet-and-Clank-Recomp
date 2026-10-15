//! Particle type 13, the crate-break dust puff (`CrateBreakFx` 0x2eb918 spawns six), read from the level01
//! disassembly:
//!
//! * spawner `PartType13Spawn(J, lo, hi, g, size, pos, s, rgba)` 0x280698 — ported in
//!   `moby_update::services::World::part13` (its only caller is the crate break): kind-0 sprite, byte9 0x44 (near
//!   1 u, far 128 u), additive (0x48), texture `def[13][0]`, position jittered by 3 × `randf_sym(0, J)`,
//!   timer `ticks(10)`, +0x24 phase 0, +0x28 spin step ±s, +0x2c growth `randf(lo, hi)`, +0x30 rise g,
//!   +0x34 RGB (`rgba & 0xffffff`);
//! * [`update`] = 0x280818.
//!
//! Record fields: +0x24 phase (u32: 0 fade-in, 1 fade-out), +0x28 low byte = rotation step per tick, +0x2c size
//! factor per tick, +0x30 z step per tick, +0x34 RGB (alpha byte rebuilt every tick).

use super::{fast_dec_timer, rec, Particles};
use crate::ps2v::{self, F};
use crate::rng::Rng;

/// 96.0 and 127.0.
const K96: F = 0x42c0_0000;
const K127: F = 0x42fe_0000;

/// Update 0x280818 (no RNG): byte8 += byte 0x28; z += g; size *= growth; then
/// * phase 0: `FastDecTimer`; when it fires, phase 1, timer `ticks(30)`, RGBA = 0x7f << 24 | RGB; else
///   A = `trunc((ticks(10) − t)·(96 / ticks(10))) + 0x20` (0x20 → 0x7f over the ten ticks);
/// * phase 1: `FastDecTimer` fires → `KillPart`; else A = `trunc(t·(127 / ticks(30)))`.
///
/// A dust puff lives 10 + 30 updates.
pub fn update(sys: &mut Particles, i: usize, _rng: &mut Rng) {
    let time = sys.time;
    let r = &mut sys.pool.recs[i];
    r[8] = r[8].wrapping_add(r[0x28]);
    rec::set_f(r, 0x18, ps2v::add(rec::f(r, 0x18), rec::f(r, 0x30)));
    rec::set_f(r, 0xc, ps2v::mul(rec::f(r, 0xc), rec::f(r, 0x2c)));
    let a: i32 = if rec::u32(r, 0x24) == 0 {
        if fast_dec_timer(r, 0xa) != 0 {
            rec::set_u32(r, 0x24, 1);
            rec::set_i16(r, 0xa, time.ticks(0x1e) as i16);
            rec::set_u32(r, 4, 0x7f00_0000 | rec::u32(r, 0x34));
            return;
        }
        let t10 = time.ticks(10);
        let k = ps2v::div(K96, ps2v::itof0(time.ticks(10)));
        let d = t10.wrapping_sub(rec::i16(r, 0xa) as i32);
        ps2v::ftoi0(ps2v::mul(ps2v::itof0(d), k)).wrapping_add(0x20)
    } else {
        if fast_dec_timer(r, 0xa) != 0 {
            sys.kill_part(i);
            return;
        }
        let k = ps2v::div(K127, ps2v::itof0(time.ticks(0x1e)));
        ps2v::ftoi0(ps2v::mul(ps2v::itof0(rec::i16(r, 0xa) as i32), k))
    };
    rec::set_u32(r, 4, (a as u32) << 24 | rec::u32(r, 0x34));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A record as `PartType13Spawn` leaves it for the crate break (J 0.3, growth 1.07, g 0.05, size 50000,
    /// grey 0x808080, spin +1).
    fn dust(s: &mut Particles) -> usize {
        let i = s.create_part(13).unwrap();
        let r = &mut s.pool.recs[i];
        for (k, v) in [100.0f32, 200.0, 30.0, 0.0].iter().enumerate() { rec::set_f(r, 0x10 + 4 * k, v.to_bits()); }
        rec::set_u32(r, 4, 0x4080_8080);
        r[9] = 0x44;
        r[3] = 0x48;
        rec::set_f(r, 0xc, 50000f32.to_bits());
        rec::set_i16(r, 0xa, 10);
        rec::set_u32(r, 0x24, 0);
        rec::set_u32(r, 0x28, 1);
        rec::set_f(r, 0x2c, 0x3f89_999a);
        rec::set_f(r, 0x30, 0x3d4c_cccd);
        rec::set_u32(r, 0x34, 0x80_8080);
        i
    }

    #[test]
    fn dust_fades_in_then_out_over_40_updates_without_rng() {
        let mut s = Particles::new(None, Vec::new());
        let i = dust(&mut s);
        let mut rng = Rng::new();
        rng.srand(1234);
        let mut n = 0;
        let mut alphas = Vec::new();
        while s.pool.count > 0 {
            s.update_parts(&mut rng);
            n += 1;
            if s.pool.count == 0 { break; }
            let r = &s.pool.recs[i];
            alphas.push(r[7]);
            assert_eq!(rec::u32(r, 4) & 0xff_ffff, 0x80_8080, "RGB kept");
            assert_eq!(r[8], n as u8, "spin +1 per tick");
            let z = f32::from_bits(rec::u32(r, 0x18));
            assert!((z - (30.0 + 0.05 * n as f32)).abs() < 1e-3, "rises by g");
            let size = f32::from_bits(rec::u32(r, 0xc)) as f64;
            let want = 50000.0 * (f32::from_bits(0x3f89_999a) as f64).powi(n);
            assert!((size / want - 1.0).abs() < 1e-5, "size {size} vs {want}");
        }
        assert_eq!(n, 40, "10 fade-in + 30 fade-out updates");
        assert_eq!(rng.state, 1234, "type 13 draws nothing");
        // Fade-in: 0x20 + trunc((10 − t)·9.6) for t = 9..1, then 0x7f on the phase change; fade-out
        // trunc(t·127/30) for t = 29..1.
        // (IEEE f32 here; the PS2's truncating 96/10 = 9.599999 makes k = 5 give 47, not 48.)
        let mut want: Vec<u8> = (1..10).map(|k| 0x20 + (k as f32 * (96.0f32 / 10.0)) as u8).collect();
        want[4] -= 1;
        want.push(0x7f);
        want.extend((1..30).rev().map(|t| (t as f32 * (127.0f32 / 30.0)) as u8));
        assert_eq!(alphas, want);
    }
}
