//! The game's one random-number stream and the helpers built on it (docs/plan/particles.md §5).
//!
//! `rand` (boot 0x1160d8) is newlib's: `s = s·0x41c64e6d + 0x3039 (mod 2³²); return s & 0x7fffffff`, with the
//! state word at `*(0x12f76c) + 0x58` (= 0x12f4d8) starting at 1. `srand` (boot 0x1160c8) stores the seed.
//! Level render init (level01 0x255958) calls **srand(1234)** on every level load; the boot star generator
//! calls srand(12345) on its first frame. All game code shares the stream (61 call sites in level01), so a
//! consumer only reproduces the PS2 sequence if every earlier consumer ran in game order.
//!
//! Helpers (level01 addresses, boot in brackets), all taking `r = (rand() >> 16) & 0x7fff` unless noted, with
//! their float operations in the game's order on the PS2 FPU model ([`crate::ps2v`], round toward zero):
//! * [`Rng::randi`] 0x26c930 [0x213260]: `r % n` (MIPS `div`, remainder).
//! * [`Rng::rand_range`] 0x26c970: `r % (b − a + 1) + a`.
//! * [`Rng::randf`] 0x26c9c8 [0x2132a8]: `d = b − a; a + ((f32)r · d) · 2⁻¹⁵` (sub, cvt, mul, mul, add).
//! * [`Rng::randf_sym`] 0x26ca28: `k = (rand() >> 16) & 0xfff; v = a + ((f32)k · (b − a)) · 2⁻¹²`, negated
//!   (`neg.s`) when bit 16 of that `rand()` is set.
//! * [`Rng::rand_angle`] 0x26ca90 [0x213308]: `((f32)(((rand() >> 16) & 0xfff) − 0x800) · π) · 2⁻¹¹`.
//! * [`Rng::rand_vec`] 0x26cb58: `l = randf(lo, hi)` (the caller's f12/f13 pass straight through), then
//!   `a = rand_angle()`, `b = rand_angle()`, result `((cos a · sin b) · l, (sin a · sin b) · l, cos b · l)` with the
//!   VU0 `fast_cos`/`fast_sin` (0x2216f8/0x221710 = `vcallms 0x190/0x192`, the sine polynomial of 28259).

use crate::ps2v::{self, F};
use rc_formats::moby_light::vu0_sin_cos;

/// Level load seed (`srand(1234)` in level01 0x255958).
pub const LEVEL_SEED: u32 = 1234;
/// 0x40490fdb: the π the helpers load with `lui/ori` (not the VU0 program's 0x40490fda).
pub const PI: F = 0x4049_0fdb;
const TWO_POW_M15: F = 0x3800_0000;
const TWO_POW_M12: F = 0x3980_0000;
const TWO_POW_M11: F = 0x3a00_0000;

/// The newlib `rand` state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rng {
    pub state: u32,
}

impl Default for Rng {
    fn default() -> Self { Rng::new() }
}

/// `cvt.s.w` of a value the helpers produce (|v| < 2^24, exact).
fn cvt(v: i32) -> F { ps2v::itof0(v) }

impl Rng {
    /// newlib's static initial state 1.
    pub const fn new() -> Self { Rng { state: 1 } }

    /// `srand` (boot 0x1160c8).
    pub fn srand(&mut self, seed: u32) { self.state = seed; }

    /// `rand` (boot 0x1160d8).
    pub fn rand(&mut self) -> i32 {
        self.state = self.state.wrapping_mul(0x41c6_4e6d).wrapping_add(0x3039);
        (self.state & 0x7fff_ffff) as i32
    }

    /// `sra v0, v0, 16; andi v0, v0, 0x7fff`.
    fn r15(&mut self) -> i32 { (self.rand() >> 16) & 0x7fff }

    /// 0x26c930 `randi(n)`: `r % n`. `n = 0` traps on the PS2 (`break 7`); here it panics.
    pub fn randi(&mut self, n: i32) -> i32 {
        let r = self.r15();
        assert!(n != 0, "randi(0): division by zero (break 7 on the PS2)");
        r.wrapping_rem(n)
    }

    /// 0x26c970 `rand_range(a, b)`: `r % (b − a + 1) + a` (32-bit wrapping like `subu/addiu/addu`).
    pub fn rand_range(&mut self, a: i32, b: i32) -> i32 {
        let r = self.r15();
        let n = b.wrapping_sub(a).wrapping_add(1);
        assert!(n != 0, "rand_range: division by zero (break 7 on the PS2)");
        r.wrapping_rem(n).wrapping_add(a)
    }

    /// 0x26c9c8 `randf(a, b)` on PS2 float bits.
    pub fn randf_bits(&mut self, a: F, b: F) -> F {
        let r = self.r15();
        let d = ps2v::sub(b, a);
        let x = ps2v::mul(ps2v::mul(cvt(r), d), TWO_POW_M15);
        ps2v::add(a, x)
    }

    /// [`Rng::randf_bits`] on `f32` values.
    pub fn randf(&mut self, a: f32, b: f32) -> f32 { f32::from_bits(self.randf_bits(a.to_bits(), b.to_bits())) }

    /// 0x26ca28 `randf_sym(a, b)`.
    pub fn randf_sym(&mut self, a: f32, b: f32) -> f32 {
        let v = self.rand() >> 16;
        let k = v & 0xfff;
        let d = ps2v::sub(b.to_bits(), a.to_bits());
        let x = ps2v::mul(ps2v::mul(cvt(k), d), TWO_POW_M12);
        let s = ps2v::add(a.to_bits(), x);
        f32::from_bits(if v & 1 != 0 { s ^ ps2v::SIGN } else { s })
    }

    /// 0x26ca90 `rand_angle()`: radians in [−π, π).
    pub fn rand_angle_bits(&mut self) -> F {
        let k = ((self.rand() >> 16) & 0xfff) - 0x800;
        ps2v::mul(ps2v::mul(cvt(k), PI), TWO_POW_M11)
    }

    pub fn rand_angle(&mut self) -> f32 { f32::from_bits(self.rand_angle_bits()) }

    /// 0x26cb58 `rand_vec(out, lo, hi)`: a random direction scaled by `randf(lo, hi)`.
    pub fn rand_vec(&mut self, lo: f32, hi: f32) -> [f32; 3] {
        let l = self.randf_bits(lo.to_bits(), hi.to_bits());
        let a = self.rand_angle_bits();
        let b = self.rand_angle_bits();
        let (sin_a, cos_a) = vu0_sin_cos(a);
        let (sin_b, cos_b) = vu0_sin_cos(b);
        let x = ps2v::mul(ps2v::mul(cos_a, sin_b), l);
        let y = ps2v::mul(ps2v::mul(sin_a, sin_b), l);
        let z = ps2v::mul(cos_b, l);
        [x, y, z].map(f32::from_bits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The recurrence written out independently (u64 arithmetic, explicit modulus).
    fn reference(seed: u32, n: usize) -> Vec<i32> {
        let mut s = seed as u64;
        (0..n)
            .map(|_| {
                s = (s * 1_103_515_245 + 12_345) % (1u64 << 32);
                (s % (1u64 << 31)) as i32
            })
            .collect()
    }

    #[test]
    fn first_ten_after_level_seed_follow_newlib() {
        let mut r = Rng::new();
        r.srand(LEVEL_SEED);
        let got: Vec<i32> = (0..10).map(|_| r.rand()).collect();
        assert_eq!(got, reference(1234, 10));
        // Unseeded newlib starts at 1: its first output is the well-known 1103527590.
        assert_eq!(Rng::new().rand(), 1_103_527_590);
        assert_eq!(reference(1, 1)[0], 1_103_527_590);
    }

    /// Every helper on seed 1234: the `r` it consumes comes from the reference stream, and the value
    /// follows the documented formula (exact where the float operations are exact, else within 1 ulp of f64).
    #[test]
    fn helpers_on_level_seed() {
        let refs = reference(1234, 16);
        let r15 = |k: usize| (refs[k] >> 16) & 0x7fff;
        let mut r = Rng::new();
        r.srand(LEVEL_SEED);

        assert_eq!(r.randi(100), r15(0) % 100);
        assert_eq!(r.rand_range(-5, 5), r15(1) % 11 - 5);
        // (b − a) = 32768: r · 32768 · 2^-15 = r exactly.
        assert_eq!(r.randf(0.0, 32768.0), r15(2) as f32);
        // randf(−0.3, 0.3): within an ulp of the f64 value, and in range.
        let v = r.randf(-0.3, 0.3);
        let want = -0.3f32 as f64 + r15(3) as f64 * (0.3f32 as f64 - -0.3f32 as f64) / 32768.0;
        assert!((v as f64 - want).abs() <= 2.0 * f32::EPSILON as f64, "{v} vs {want}");
        assert!((-0.3..0.3).contains(&v));
        // randf_sym(1, 3): 1 + k·2·2^-12 exactly, sign from bit 16.
        let raw = refs[4] >> 16;
        let sym = 1.0 + (raw & 0xfff) as f32 * 2.0 / 4096.0;
        assert_eq!(r.randf_sym(1.0, 3.0), if raw & 1 != 0 { -sym } else { sym });
        // rand_angle: k·π·2^-11, k in −2048..2047.
        let k = ((refs[5] >> 16) & 0xfff) - 0x800;
        let a = r.rand_angle();
        assert!((a as f64 - k as f64 * std::f64::consts::PI / 2048.0).abs() < 1e-5, "{a}");
        // rand_vec: randf then two angles (3 draws), unit direction times l.
        let l = 2.0 + r15(6) as f32 / 32768.0;
        let v = r.rand_vec(2.0, 3.0);
        let len = (v[0] as f64).hypot(v[1] as f64).hypot(v[2] as f64);
        assert!((len - l as f64).abs() < 1e-4, "{v:?} len {len} vs {l}");
        assert_eq!(r.state, reference(1234, 9)[8] as u32 | (r.state & 0x8000_0000), "9 draws so far");
    }
}
