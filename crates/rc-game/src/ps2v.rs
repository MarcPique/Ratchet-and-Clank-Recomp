//! VU0 macro-mode vector arithmetic on PS2 float bit patterns, built on the FMAC model in
//! [`rc_formats::tfrag_light::ps2`] (round toward zero, no denormals/NaN/Inf, adder guard-bit
//! behaviour). Each helper names the instruction sequence it reproduces, so a kernel written with
//! them follows the game's operation order one to one.

pub use rc_formats::tfrag_light::ps2::{add, div, max, min, mul, sqrt, sub, MAX, ONE, SIGN};

/// A PS2 float as raw bits.
pub type F = u32;
/// The `.xyz` lanes of a VU register.
pub type V3 = [F; 3];

/// 1024.0: world units -> the kernels' fixed "x1024" space.
pub const K1024: F = 0x4480_0000;
/// 2^-10 (0x3a800000): x1024 space -> world units on output.
pub const INV1024: F = 0x3a80_0000;

/// Sign bit set (what the kernels test with `bltz`/`bgez` after `qmfc2`/`dsll32`). `-0.0` counts
/// as negative, exactly like the hardware test.
#[inline]
pub fn neg(x: F) -> bool { x & SIGN != 0 }

/// `vsub.xyz d, a, b`
#[inline]
pub fn vsub(a: V3, b: V3) -> V3 { [sub(a[0], b[0]), sub(a[1], b[1]), sub(a[2], b[2])] }
/// `vadd.xyz d, a, b`
#[inline]
pub fn vadd(a: V3, b: V3) -> V3 { [add(a[0], b[0]), add(a[1], b[1]), add(a[2], b[2])] }
/// `vmul.xyz d, a, b`
#[inline]
pub fn vmul(a: V3, b: V3) -> V3 { [mul(a[0], b[0]), mul(a[1], b[1]), mul(a[2], b[2])] }
/// `vmulx.xyz d, a, s` (or `vmulq`): every lane times one scalar.
#[inline]
pub fn vmuls(a: V3, s: F) -> V3 { [mul(a[0], s), mul(a[1], s), mul(a[2], s)] }
/// `vadda.xyz ACC, vf0, a; vmaddx.xyz d, b, s` (or `vmulax ACC, b, s; vmaddw d, a, vf0`):
/// `a + b * s` per lane, product rounded first.
#[inline]
pub fn vmadds(a: V3, b: V3, s: F) -> V3 { [add(a[0], mul(b[0], s)), add(a[1], mul(b[1], s)), add(a[2], mul(b[2], s))] }

/// `vopmula.xyz ACC, a, b; vopmsub.xyz d, b, a`: the cross product `a x b`, each lane
/// `a.y*b.z - b.y*a.z` with both products rounded before the subtraction.
#[inline]
pub fn cross(a: V3, b: V3) -> V3 {
    [
        sub(mul(a[1], b[2]), mul(b[1], a[2])),
        sub(mul(a[2], b[0]), mul(b[2], a[0])),
        sub(mul(a[0], b[1]), mul(b[0], a[1])),
    ]
}

/// `vmul.xyz t, a, b; vadday.x ACC, t, t; vmaddz.x d, one, t`: `(x + y) + 1*z`, result in lane x.
#[inline]
pub fn dot_x(a: V3, b: V3) -> F {
    let p = vmul(a, b);
    add(add(p[0], p[1]), mul(ONE, p[2]))
}

/// `vmul.xyz t, a, b; vaddax.y ACC, t, t; vmaddz.y d, one, t`: `(y + x) + 1*z`, result in lane y
/// (the kernels use this form when the sign is read with `qmfc2` + `bltz`, i.e. from bit 63).
#[inline]
pub fn dot_y(a: V3, b: V3) -> F {
    let p = vmul(a, b);
    add(add(p[1], p[0]), mul(ONE, p[2]))
}

/// `vftoi0`: truncation toward zero, saturating at the i32 range (PS2 treats exponent 255 as an
/// ordinary number, so there is no NaN case).
pub fn ftoi0(x: F) -> i32 {
    let e = ((x >> 23) & 0xff) as i32;
    if e < 127 { return 0; }
    let neg = x & SIGN != 0;
    if e >= 127 + 31 { return if neg { i32::MIN } else { i32::MAX }; }
    let m = ((x & 0x7f_ffff) | 0x80_0000) as i64;
    let v = if e >= 150 { m << (e - 150) } else { m >> (150 - e) };
    if neg { -(v as i32) } else { v as i32 }
}

/// `vitof0`. Every integer the kernels convert is below 2^24 in magnitude, where the
/// conversion is exact (checked in debug builds).
#[inline]
pub fn itof0(i: i32) -> F {
    debug_assert!(i.unsigned_abs() < 1 << 24, "itof0 operand {i} not exactly representable");
    (i as f32).to_bits()
}

/// PS2 float bits -> IEEE `f32` for the public API. Identical bits except exponent 255, which the
/// PS2 treats as a finite number (up to ~6.8e38) and IEEE as Inf/NaN: clamped to `±f32::MAX`.
#[inline]
pub fn to_f32(x: F) -> f32 {
    if (x >> 23) & 0xff == 0xff { if neg(x) { -f32::MAX } else { f32::MAX } } else { f32::from_bits(x) }
}

#[inline]
pub fn v_to_f32(v: V3) -> [f32; 3] { v.map(to_f32) }

#[cfg(test)]
mod tests {
    use super::*;
    fn f(x: f32) -> F { x.to_bits() }

    #[test]
    fn ftoi0_truncates_toward_zero() {
        assert_eq!(ftoi0(f(0.0)), 0);
        assert_eq!(ftoi0(f(0.999)), 0);
        assert_eq!(ftoi0(f(1.0)), 1);
        assert_eq!(ftoi0(f(-1.75)), -1);
        assert_eq!(ftoi0(f(4095.9)), 4095);
        assert_eq!(ftoi0(f(8_388_609.0)), 8_388_609);
        assert_eq!(ftoi0(f(1.5e10)), i32::MAX);
        assert_eq!(ftoi0(f(-1.5e10)), i32::MIN);
    }

    #[test]
    fn cross_and_dots() {
        let x = [f(1.0), 0, 0];
        let y = [0, f(1.0), 0];
        assert_eq!(cross(x, y), [0, 0, f(1.0)]);
        assert_eq!(cross(y, x), [0, 0, f(-1.0)]);
        let a = [f(1.0), f(2.0), f(3.0)];
        let b = [f(4.0), f(-5.0), f(6.0)];
        assert_eq!(dot_x(a, b), f(12.0));
        assert_eq!(dot_y(a, b), f(12.0));
        assert_eq!(vmadds(a, b, f(0.5)), [f(3.0), f(-0.5), f(6.0)]);
    }
}

// ---------------------------------------------------------------------------------------------
// Scalar FPU (COP1) arithmetic with operators, for gameplay code (hero, camera, pad).

/// A PS2 COP1/VU float as raw bits with the [`rc_formats::tfrag_light::ps2`] arithmetic behind the
/// operators (`add.s`, `sub.s`, `mul.s`, `div.s`: round toward zero, no denormals/NaN/Inf). Gameplay
/// code written with it follows the game's FPU op order one to one. Comparisons are the FPU's
/// `c.lt.s` / `c.le.s` / `c.eq.s` on these values (`-0.0 == +0.0`; exponent 255 is an ordinary number).
#[derive(Clone, Copy, Default)]
pub struct Pf(pub F);

impl Pf {
    pub const ZERO: Pf = Pf(0);
    pub const ONE: Pf = Pf(ONE);
    /// A constant from its `lui/ori` bits.
    #[inline]
    pub const fn b(bits: u32) -> Pf { Pf(bits) }
    /// From an IEEE f32 (identical bits; denormals flush to zero like `mtc1` + any PS2 op would read them).
    #[inline]
    pub fn f(x: f32) -> Pf { let b = x.to_bits(); if (b >> 23) & 0xff == 0 { Pf(b & SIGN) } else { Pf(b) } }
    #[inline]
    pub fn to_f32(self) -> f32 { to_f32(self.0) }
    /// `abs.s`
    #[inline]
    pub fn abs(self) -> Pf { Pf(self.0 & !SIGN) }
    /// `sqrt.s` / VU `vsqrt` (of |x|, truncated).
    #[inline]
    pub fn sqrt(self) -> Pf { Pf(sqrt(self.0)) }
    /// `max.s` / `min.s`
    #[inline]
    pub fn max(self, o: Pf) -> Pf { Pf(max(self.0, o.0)) }
    #[inline]
    pub fn min(self, o: Pf) -> Pf { Pf(min(self.0, o.0)) }
    /// `cvt.s.w` (exact below 2^24; larger integers truncate like the hardware).
    #[inline]
    pub fn from_i32(i: i32) -> Pf {
        if i.unsigned_abs() < 1 << 24 { return Pf((i as f32).to_bits()); }
        // Truncate the magnitude to 24 significant bits (round toward zero).
        let m = i.unsigned_abs();
        let drop = 8 - m.leading_zeros() as i32;
        let t = (m >> drop) << drop;
        Pf(if i < 0 { (-(t as f64) as f32).to_bits() } else { (t as f32).to_bits() })
    }
    /// `cvt.w.s` (truncation toward zero, saturating).
    #[inline]
    pub fn to_i32(self) -> i32 { ftoi0(self.0) }
    /// Sign bit set (`-0.0` counts), what `bltz` on the raw word tests.
    #[inline]
    pub fn sign(self) -> bool { neg(self.0) }
    #[inline]
    pub fn is_zero(self) -> bool { (self.0 >> 23) & 0xff == 0 }
    fn key(self) -> i64 { if self.0 & SIGN != 0 { -((self.0 & MAX) as i64) } else { (self.0 & MAX) as i64 } }
}

impl std::fmt::Debug for Pf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{}({:#010x})", self.to_f32(), self.0) }
}
impl PartialEq for Pf {
    fn eq(&self, o: &Pf) -> bool { self.key() == o.key() || (self.is_zero() && o.is_zero()) }
}
impl PartialOrd for Pf {
    fn partial_cmp(&self, o: &Pf) -> Option<std::cmp::Ordering> {
        if self.is_zero() && o.is_zero() { return Some(std::cmp::Ordering::Equal); }
        Some(self.key().cmp(&o.key()))
    }
}
impl std::ops::Add for Pf { type Output = Pf; #[inline] fn add(self, o: Pf) -> Pf { Pf(add(self.0, o.0)) } }
impl std::ops::Sub for Pf { type Output = Pf; #[inline] fn sub(self, o: Pf) -> Pf { Pf(sub(self.0, o.0)) } }
impl std::ops::Mul for Pf { type Output = Pf; #[inline] fn mul(self, o: Pf) -> Pf { Pf(mul(self.0, o.0)) } }
impl std::ops::Div for Pf { type Output = Pf; #[inline] fn div(self, o: Pf) -> Pf { Pf(div(self.0, o.0)) } }
/// `neg.s`: flips the sign bit only.
impl std::ops::Neg for Pf { type Output = Pf; #[inline] fn neg(self) -> Pf { Pf(self.0 ^ SIGN) } }
impl std::ops::AddAssign for Pf { #[inline] fn add_assign(&mut self, o: Pf) { *self = *self + o; } }
impl std::ops::SubAssign for Pf { #[inline] fn sub_assign(&mut self, o: Pf) { *self = *self - o; } }
impl std::ops::MulAssign for Pf { #[inline] fn mul_assign(&mut self, o: Pf) { *self = *self * o; } }
