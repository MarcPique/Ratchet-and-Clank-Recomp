//! Particle type 11, the TNT spark (`CrateBreakFx` 0x2eb918 spawns up to ten for a TNT crate), read from the
//! level01 disassembly:
//!
//! * [`spawn`] = `PartType11Spawn(size, speed, pos, base, c1, c2, life, t1, t2, t3)` 0x27f8f8 (the crate's calls
//!   go through `moby_update::services::World::part11`, which calls this one; so does the update's split);
//! * [`update`] = 0x27fb80.
//!
//! Record fields:
//!
//! | Off | Type | Use |
//! |---|---|---|
//! | 0x0c | f32 | size: 0 at spawn, then a quadratic in the timer (spark phase) |
//! | 0x20 | vec3 | velocity (units per tick) |
//! | 0x2c | f32 | speed · 0.75 (the children's speed and the flag-2 re-launch speed) |
//! | 0x30 / 0x34 | u32 | colour at the start / end of the phase (tweened by timer / length) |
//! | 0x38 | u8 | t1: byte countdown (`FUN_00220ed8` each spark tick; passed on to the children) |
//! | 0x39 | u8 | t3 flags: 1 smoke phase, 2 "flare" (colour → 0x0fffffff, re-launch at the end), 4 gravity and no size curve, 8 × 5 smoke life |
//! | 0x3a | u8 | t2: generation (children get t2 + 1; only generations 0 and 1 split) |
//! | 0x3b | u8 | phase length N (the life at spawn, then the smoke / flare life) |
//! | 0x3c | f32 | S: the size parameter of the curve |
//!
//! **Life of a crate spark** (t3 = 0): N = life ticks of sparks (pos += vel, vel ·= 0.8, colour c1 → c2, size
//! curve); on the tick the timer equals `trunc(N·0.85)` a generation-0/1 spark splits into five children (life
//! `trunc(N·randf(0.9, 1.1))`, speed `+0x2c·randf(0.85, 1)`, generation + 1); at the end it turns into smoke
//! (flag 1: c2 → 0x0f2f3f3f, or → 0 when additive, life `rand_range(2N, 3N)` (×5 with flag 8) as a byte, rising
//! by 3·dt² per tick, vel ·= 0.98, alpha fading after the colour tween) and dies at the end of that.

use super::{fast_dec_timer, fast_dec_timer_u8, lt, rec, tween_color, Particles};
use crate::hero::physics as ph;
use crate::ps2v::{self, Pf, F};
use crate::rng::Rng;

type V4 = [Pf; 4];

const K085: F = 0x3f59_999a;
const K090: F = 0x3f66_6666;
const K110: F = 0x3f8c_cccd;
const K075: F = 0x3f40_0000;
const K025: F = 0x3e80_0000;
const K098: F = 0x3f7a_e148;
const K08: F = 0x3f4c_cccd;
const K06: F = 0x3f19_999a;
const K3: F = 0x4040_0000;
const K4: F = 0x4080_0000;
const K8: F = 0x4100_0000;
const K10: F = 0x4120_0000;
const K98: F = 0x411c_cccd;
const K0075: F = 0x3d99_999a;
const K16: F = 0x3fcc_cccd;
const K2: F = 0x4000_0000;
/// The smoke colour (alpha 0x0f) a normal-blend spark fades to, and the flare colour.
const SMOKE: u32 = 0x0f2f_3f3f;
const FLARE: u32 = 0x0fff_ffff;

fn qw(r: &super::Record, o: usize) -> V4 { [0, 4, 8, 12].map(|k| Pf(rec::f(r, o + k))) }
fn set_qw(r: &mut super::Record, o: usize, v: V4) { for (k, x) in v.iter().enumerate() { rec::set_f(r, o + 4 * k, x.0); } }

/// The frame-load throttle of [`spawn`] on the loads `[0x15f5d0, 0x15f5d4]`: false (drop the spawn) on a 0 draw.
pub fn throttle(load: [F; 2], rng: &mut Rng) -> bool {
    let [l0, l1] = load;
    for (k, n) in [(K085, 3), (K090, 2), (ps2v::ONE, 1)] {
        if (lt(k, l0) || lt(k, l1)) && rng.randi(n) == 0 { return false; }
    }
    true
}

/// `PartType11Spawn` 0x27f8f8. Nothing when `life` is 0. The frame-load throttle (0x15f5d0 / 0x15f5d4 against
/// 0.85 / 0.9 / 1.0) draws `randi(3)` / `randi(2)` / `randi(1)` and drops the spawn on a 0. With a record (pool
/// not full): `randi(100) < 20` → normal blend 0x44 (else additive 0x48), byte8 = `rand()`, and velocity
/// `base + (randf(−1, 1) ×3 scaled to speed)`: 5 draws. Byte9 = `trunc(4.0) − 0x60` = 0xa4 (near 1 u, far 320 u).
#[allow(clippy::too_many_arguments)]
pub fn spawn(sys: &mut Particles, rng: &mut Rng, size: F, speed: F, pos: [F; 4], base: [F; 4], c1: u32, c2: u32, life: i32, t1: u8, t2: u8, t3: u8) -> Option<usize> {
    if life == 0 || !throttle(sys.frame_load, rng) { return None; }
    let i = sys.create_part(11)?;
    let def = sys.def_first(11);
    let b3 = if rng.randi(100) < 20 { 0x44 } else { 0x48 };
    let b8 = rng.rand() as u8;
    let rv: V4 = [Pf(rng.randf_bits(0xbf80_0000, ps2v::ONE)), Pf(rng.randf_bits(0xbf80_0000, ps2v::ONE)), Pf(rng.randf_bits(0xbf80_0000, ps2v::ONE)), Pf::ZERO];
    let v = ph::vadd(base.map(Pf), ph::set_len3(rv, Pf(speed)));
    let r = &mut sys.pool.recs[i];
    set_qw(r, 0x10, pos.map(Pf));
    rec::set_u32(r, 4, c1);
    r[9] = (ps2v::ftoi0(K4) - 0x60) as u8;
    r[1] = 0;
    rec::set_u32(r, 0xc, 0);
    r[3] = b3;
    r[8] = b8;
    r[2] = def;
    set_qw(r, 0x20, v);
    rec::set_f(r, 0x2c, ps2v::mul(speed, K075));
    rec::set_i16(r, 0xa, life as i16);
    r[0x39] = t3;
    r[0x3b] = life as u8;
    r[0x38] = t1;
    rec::set_f(r, 0x3c, size);
    rec::set_u32(r, 0x30, c1);
    rec::set_u32(r, 0x34, c2);
    r[0x3a] = t2;
    Some(i)
}

/// The spark-phase size (0x27fd4c..0x27fdf8): with S = +0x3c, N = +0x3b and T the timer,
/// B = (8N − 3N·S)/(8S·S + 8S), A = (3N + 8B·S)/(−8S), and the larger root of A·x² + B·x + (N − T) = 0
/// (`vsqrt` of the discriminant). It rises from 0 at T = N.
fn curve_size(s: F, n: F, t: F) -> F {
    use ps2v::{add, div, mul, sub};
    let f4 = mul(s, K8);
    let f21 = mul(n, K3);
    let f20 = mul(n, K8);
    let f0 = add(mul(f4, s), f4);
    let f2 = mul(f21, s);
    let f3 = mul(s, K8 | ps2v::SIGN);
    let b = div(sub(f20, f2), f0);
    let a = div(add(f21, mul(mul(b, K8), s)), f3);
    let disc = sub(mul(b, b), mul(mul(a, K4), sub(n, t)));
    let root = add(0, ps2v::sqrt(disc));
    let nb = b ^ ps2v::SIGN;
    let a2 = add(a, a);
    let r1 = div(sub(nb, root), a2);
    let r2 = div(add(nb, root), a2);
    if lt(r1, r2) { r2 } else { r1 }
}

/// Update 0x27fb80 (see the module doc). RNG: 35 draws on a split tick (5 children × (2 + the spawner's 5), more
/// under frame load), 1 at the end of the spark phase (`rand_range` or, with flag 2, `randf(1.6, 2)`), else 0.
pub fn update(sys: &mut Particles, i: usize, rng: &mut Rng) {
    let (dt2, load0, cam) = (sys.time.dt2, sys.frame_load[0], sys.camera);
    let r = &mut sys.pool.recs[i];
    r[8] = r[8].wrapping_add(1);
    if r[0x39] & 1 != 0 {
        smoke(sys, i, dt2, load0);
        return;
    }
    // pos += vel; gravity (flag 4); vel ·= 0.6 (flag 2) or 0.8.
    set_qw(r, 0x10, ph::vadd(qw(r, 0x10), qw(r, 0x20)));
    if r[0x39] & 4 != 0 { rec::set_f(r, 0x28, ps2v::add(rec::f(r, 0x28), ps2v::mul(dt2, K98))); }
    let k = if r[0x39] & 2 != 0 { K06 } else { K08 };
    set_qw(r, 0x20, ph::vscale(qw(r, 0x20), Pf(k)));
    let t = rec::i16(r, 0xa) as i32;
    let split_at = ps2v::ftoi0(ps2v::mul(ps2v::itof0(r[0x3b] as i32), K085));
    if t == split_at && r[0x3a] < 2 && r[0x39] & 4 == 0 {
        for _ in 0..5 {
            let f = rng.randf_bits(K090, K110);
            let r = &sys.pool.recs[i];
            let life = ps2v::ftoi0(ps2v::mul(ps2v::itof0(r[0x3b] as i32), f));
            let t1 = r[0x38];
            let f = rng.randf_bits(K085, ps2v::ONE);
            let r = &sys.pool.recs[i];
            let speed = ps2v::mul(rec::f(r, 0x2c), f);
            let (size, pos, base) = (rec::f(r, 0x3c), qw(r, 0x10).map(|x| x.0), qw(r, 0x20).map(|x| x.0));
            let (c1, c2, t2, t3) = (rec::u32(r, 0x30), rec::u32(r, 0x34), r[0x3a].wrapping_add(1), r[0x39]);
            spawn(sys, rng, size, speed, pos, base, c1, c2, life, t1, t2, t3);
        }
    }
    let r = &mut sys.pool.recs[i];
    if r[0x39] & 4 == 0 {
        let size = curve_size(rec::f(r, 0x3c), ps2v::itof0(r[0x3b] as i32), ps2v::itof0(rec::i16(r, 0xa) as i32));
        rec::set_f(r, 0xc, size);
        let d = ph::len3(ph::vsub(qw(r, 0x10), [Pf(cam[0]), Pf(cam[1]), Pf(cam[2]), Pf::ZERO]));
        if d < Pf(K10) {
            rec::set_f(r, 0xc, ps2v::mul(rec::f(r, 0xc), ps2v::add(ps2v::mul(d.0, K0075), K025)));
        }
    }
    let f = ps2v::div(ps2v::itof0(rec::i16(r, 0xa) as i32), ps2v::itof0(r[0x3b] as i32));
    let c = if r[0x39] & 2 != 0 { tween_color(f, rec::u32(r, 0x30), FLARE) } else { tween_color(f, rec::u32(r, 0x34), rec::u32(r, 0x30)) };
    rec::set_u32(r, 4, c);
    fast_dec_timer_u8(r, 0x38);
    if fast_dec_timer(r, 0xa) == 0 { return; }
    if r[0x39] & 2 != 0 {
        // Flare end: a new spark phase of 3N ticks (as a byte) with gravity, re-launched at +0x2c·randf(1.6, 2).
        let v = (r[0x3b] as u32) * 3;
        rec::set_i16(r, 0xa, v as i16);
        r[0x3b] = v as u8;
        r[0x39] = (r[0x39] & 0xfd) | 4;
        let f = rng.randf_bits(K16, K2);
        let r = &mut sys.pool.recs[i];
        let len = ps2v::mul(rec::f(r, 0x2c), f);
        set_qw(r, 0x20, ph::set_len3(qw(r, 0x20), Pf(len)));
    } else {
        // Spark end: smoke from the end colour.
        let c2 = rec::u32(r, 0x34);
        rec::set_u32(r, 0x30, c2);
        r[0x39] |= 1;
        rec::set_u32(r, 0x34, if r[3] == 0x48 { 0 } else { SMOKE });
        let n = r[0x3b] as i32;
        let mut v = rng.rand_range(n * 2, n * 3);
        let r = &mut sys.pool.recs[i];
        if r[0x39] & 8 != 0 { v = v.wrapping_mul(5); }
        r[0x3b] = v as u8;
        rec::set_i16(r, 0xa, (v & 0xff) as i16);
    }
}

/// The smoke phase (0x27ffa0..0x2800f8, flag 1): with N = +0x3b, s0 = `trunc(N·0.75) + 1`, s1 = `trunc(N·0.25) + 1`:
/// vel.z += 3·dt², pos += vel, vel ·= 0.98; while t > s0 the colour tweens (t − s0)/s1 from +0x34 to +0x30; after,
/// additive smoke dies at once (timer 0) and normal smoke fades A = 15t / s0 (integer). An extra timer step when
/// the frame load 0x15f5d0 is above 1.
fn smoke(sys: &mut Particles, i: usize, dt2: F, load0: F) {
    let r = &mut sys.pool.recs[i];
    let n = ps2v::itof0(r[0x3b] as i32);
    let s0 = ps2v::ftoi0(ps2v::mul(n, K075)) + 1;
    let s1 = ps2v::ftoi0(ps2v::mul(n, K025)) + 1;
    rec::set_f(r, 0x28, ps2v::add(rec::f(r, 0x28), ps2v::mul(dt2, K3)));
    set_qw(r, 0x10, ph::vadd(qw(r, 0x10), qw(r, 0x20)));
    set_qw(r, 0x20, ph::vscale(qw(r, 0x20), Pf(K098)));
    let t = rec::i16(r, 0xa) as i32;
    if s0 < t {
        let f = ps2v::div(ps2v::itof0(t - s0), ps2v::itof0(s1));
        rec::set_u32(r, 4, tween_color(f, rec::u32(r, 0x34), rec::u32(r, 0x30)));
    } else if r[3] == 0x48 {
        rec::set_i16(r, 0xa, 0);
    } else {
        let a = (t * 16 - t) / s0;
        rec::set_u32(r, 4, (rec::u32(r, 4) & 0xff_ffff) | (a as u32) << 24);
    }
    if lt(ps2v::ONE, load0) { fast_dec_timer(r, 0xa); }
    if fast_dec_timer(r, 0xa) != 0 { sys.kill_part(i); }
}

#[cfg(test)]
mod tests {
    use super::*;

    const C1: u32 = 0x4f00_8fff;
    const C2: u32 = 0x2f00_5f7f;

    /// A crate spark as `CrateBreakFx` spawns it (size 400000, speed 8.5·dt, life 18, t1 27, t2 = t3 = 0), up
    /// the z axis from (100, 200, 30), the camera 50 units away.
    fn system() -> (Particles, Rng) {
        let mut s = Particles::new(None, Vec::new());
        s.camera = [150f32.to_bits(), 200f32.to_bits(), 30f32.to_bits()];
        let mut rng = Rng::new();
        rng.srand(1234);
        (s, rng)
    }

    fn crate_spark(s: &mut Particles, rng: &mut Rng, life: i32) -> usize {
        let pos = [100f32, 200.0, 30.0, 0.0].map(f32::to_bits);
        spawn(s, rng, 0x48c3_5000, (8.5f32 / 60.0).to_bits(), pos, [0; 4], C1, C2, life, 27, 0, 0).unwrap()
    }

    fn draws(seed: u32, after: u32) -> usize {
        let mut r = Rng::new();
        r.srand(seed);
        (0..1000).find(|_| { let hit = r.state == after; if !hit { r.rand(); } hit }).unwrap_or(usize::MAX)
    }

    #[test]
    fn spawner_writes_the_record_and_draws_five() {
        let (mut s, mut rng) = system();
        let i = crate_spark(&mut s, &mut rng, 18);
        assert_eq!(draws(1234, rng.state), 5);
        let r = &s.pool.recs[i];
        assert_eq!((r[0], r[1], r[9], r[0x38], r[0x39], r[0x3a], r[0x3b]), (11, 0, 0xa4, 27, 0, 0, 18));
        assert!(r[3] == 0x44 || r[3] == 0x48);
        assert_eq!((rec::u32(r, 4), rec::u32(r, 0x30), rec::u32(r, 0x34), rec::u32(r, 0xc)), (C1, C1, C2, 0));
        let v = [0x20, 0x24, 0x28].map(|o| f32::from_bits(rec::u32(r, o)));
        let len = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((len - 8.5 / 60.0).abs() < 1e-5, "speed {len}");
        assert!((f32::from_bits(rec::u32(r, 0x2c)) - 0.75 * 8.5 / 60.0).abs() < 1e-6);
        // life 0: nothing, no draw.
        let st = rng.state;
        assert!(spawn(&mut s, &mut rng, 0, 0, [0; 4], [0; 4], 0, 0, 0, 0, 0, 0).is_none());
        assert_eq!(rng.state, st);
    }

    #[test]
    fn generation_0_spark_splits_into_five_on_the_fourth_update() {
        let (mut s, mut rng) = system();
        let i = crate_spark(&mut s, &mut rng, 18);
        for tick in 1..=4 {
            let before = rng.state;
            s.update_parts(&mut rng);
            let n = draws(before, rng.state);
            if tick < 4 {
                assert_eq!((n, s.pool.count), (0, 1));
                continue;
            }
            // trunc(18·0.85) = 15: the fourth update (it finds t = 15) splits into five children, 2 + 5 draws each; they
            // sit above the old hw and first run next tick.
            assert_eq!((n, s.pool.count), (35, 6));
            let r = &s.pool.recs[i];
            for c in 1..=5 {
                let cr = &s.pool.recs[i + c];
                assert_eq!((cr[0], cr[0x3a], cr[0x39]), (11, 1, 0), "generation 1, parent flags");
                assert_eq!(cr[0x38], r[0x38] + 1, "t1 as it was before this tick's step");
                assert!((16..=19).contains(&rec::i16(cr, 0xa)), "life trunc(18·randf(0.9, 1.1))");
                assert_eq!(rec::u32(cr, 0x30), C1);
                let v = [0x20, 0x24, 0x28].map(|o| f32::from_bits(rec::u32(cr, o)));
                let pv = [0x20, 0x24, 0x28].map(|o| f32::from_bits(rec::u32(r, o)));
                // base = the parent's velocity (already ·0.8 this tick) plus a random direction of length
                // 0.75·speed·randf(0.85, 1).
                let d = (0..3).map(|k| (v[k] - pv[k]).powi(2)).sum::<f32>().sqrt();
                let sp = 0.75 * 8.5 / 60.0;
                assert!(d >= 0.85 * sp - 1e-5 && d <= sp + 1e-5, "child offset speed {d}");
            }
        }
    }

    #[test]
    fn generation_2_spark_turns_to_smoke_and_dies() {
        let (mut s, mut rng) = system();
        let pos = [100f32, 200.0, 30.0, 0.0].map(f32::to_bits);
        let i = spawn(&mut s, &mut rng, 0x48c3_5000, (8.5f32 / 60.0).to_bits(), pos, [0; 4], C1, C2, 18, 27, 2, 0).unwrap();
        let r0 = s.pool.recs[i];
        let mut pos = rec::pos(&r0);
        let mut vel = [0x20, 0x24, 0x28].map(|o| f32::from_bits(rec::u32(&r0, o)));
        let mut ticks = 0;
        let mut smoke_len = 0;
        loop {
            let before = rng.state;
            s.update_parts(&mut rng);
            ticks += 1;
            let r = &s.pool.recs[i];
            let n = draws(before, rng.state);
            if r[1] & crate::particles::FLAG_DEAD != 0 {
                assert_eq!(n, 0);
                break;
            }
            assert_eq!(s.pool.count, 1, "generation 2 never splits");
            let smoking = ticks > 18;
            if ticks == 18 {
                // Spark end: one rand_range(36, 54) draw; colours c2 → smoke (0 when additive).
                assert_eq!(n, 1);
                assert_eq!(r[0x39], 1);
                assert_eq!(rec::u32(r, 0x30), C2);
                assert_eq!(rec::u32(r, 0x34), if r[3] == 0x48 { 0 } else { SMOKE });
                assert!((36..=54).contains(&r[0x3b]));
                assert_eq!(rec::i16(r, 0xa), r[0x3b] as i16);
                smoke_len = r[0x3b] as i32;
            } else {
                assert_eq!(n, 0, "tick {ticks}: no draw");
            }
            // Motion: pos += vel, vel ·= 0.8 (sparks); vel.z += 3·dt², pos += vel, vel ·= 0.98 (smoke).
            if smoking { vel[2] += 3.0 / 3600.0; }
            for k in 0..3 { pos[k] += vel[k]; }
            for v in vel.iter_mut() { *v *= if smoking { 0.98 } else { 0.8 }; }
            let p = rec::pos(r);
            assert!((0..3).all(|k| (p[k] - pos[k]).abs() < 1e-4), "tick {ticks}: {p:?} vs {pos:?}");
            if ticks < 18 {
                // Colour c1 → c2 as t/N falls from 1 (the green byte, 0x8f → 0x5f); t before this tick's step.
                let t = rec::i16(r, 0xa) as f32 + 1.0;
                let g = 0x5f as f32 + (0x8f - 0x5f) as f32 * t / 18.0;
                assert!((r[5] as f32 - g).abs() <= 1.0, "tick {ticks}: green {} vs {g}", r[5]);
                let size = f32::from_bits(rec::u32(r, 0xc));
                assert!((-0.01..=400000.0).contains(&size), "size {size}");
                assert_eq!(r[0x38], 27 - ticks as u8, "t1 counts down");
            }
        }
        // Smoke: additive smoke dies when t reaches s0 = trunc(0.75·N) + 1 (timer zeroed, then the timer test);
        // normal smoke fades out over the whole phase.
        let s0 = (smoke_len as f32 * 0.75) as i32 + 1;
        let want = if r0[3] == 0x48 { 18 + smoke_len - s0 + 1 } else { 18 + smoke_len };
        assert_eq!(ticks, want, "smoke length {smoke_len}");
    }

    #[test]
    fn curve_size_starts_at_zero_and_grows() {
        let s = 0x48c3_5000;
        let n = ps2v::itof0(18);
        // At t = N the roots are 0 and (8 − 3S)/11; the truncated sqrt of B² leaves the larger one a hair below 0.
        let v0 = f32::from_bits(curve_size(s, n, n));
        assert!(v0.abs() < 0.01, "{v0}");
        let mut prev = v0;
        for t in (1..18).rev() {
            let v = f32::from_bits(curve_size(s, n, ps2v::itof0(t)));
            // f64 reference of the same formula.
            let (sf, nf, tf) = (400000.0f64, 18.0f64, t as f64);
            let b = (8.0 * nf - 3.0 * nf * sf) / (8.0 * sf * sf + 8.0 * sf);
            let a = (3.0 * nf + 8.0 * b * sf) / (-8.0 * sf);
            let d = (b * b - 4.0 * a * (nf - tf)).sqrt();
            let want = ((-b - d) / (2.0 * a)).max((-b + d) / (2.0 * a));
            // A = (3N + 8B·S)/(−8S) cancels (8B·S ≈ −3N): in single precision (the game's) it is off by up to
            // ~1% from the exact value, so the f64 reference only bounds it.
            assert!((v as f64 - want).abs() < want.abs() * 0.02 + 1.0, "t {t}: {v} vs {want}");
            assert!(v >= prev);
            prev = v;
        }
    }
}
