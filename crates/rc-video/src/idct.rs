//! The 8×8 inverse DCT (ISO/IEC 13818-2 Annex A): separable, in `f32` with the exact cosine basis, rounded to the
//! nearest integer. Accuracy: IEEE 1180-1990 (checked by the test below with the standard's generator, ranges and
//! limits). Deterministic: the same coefficients always give the same samples.

use std::sync::OnceLock;

/// `BASIS[k][n] = C(k)/2 · cos((2n + 1)·k·π/16)`, C(0) = 1/√2, else 1.
fn basis() -> &'static [[f32; 8]; 8] {
    static B: OnceLock<[[f32; 8]; 8]> = OnceLock::new();
    B.get_or_init(|| {
        let mut b = [[0f32; 8]; 8];
        for (k, row) in b.iter_mut().enumerate() {
            let c = if k == 0 { std::f64::consts::FRAC_1_SQRT_2 } else { 1.0 };
            for (n, v) in row.iter_mut().enumerate() {
                *v = (c / 2.0 * ((2 * n + 1) as f64 * k as f64 * std::f64::consts::PI / 16.0).cos()) as f32;
            }
        }
        b
    })
}

/// Inverse DCT of `block` (coefficients in raster order, `F[v·8 + u]`) in place: sample `f[y·8 + x]`, rounded
/// to the nearest integer (halves upward), not clamped.
pub fn idct(block: &mut [i32; 64]) {
    let b = basis();
    let mut tmp = [0f32; 64];
    // Rows: tmp[v][x] = Σ_u F[v][u]·B[u][x]; all-zero rows stay zero.
    for v in 0..8 {
        let row = &block[v * 8..v * 8 + 8];
        if row.iter().all(|&c| c == 0) { continue; }
        let out = &mut tmp[v * 8..v * 8 + 8];
        for (u, &c) in row.iter().enumerate() {
            if c == 0 { continue; }
            let c = c as f32;
            let bu = &b[u];
            for x in 0..8 { out[x] += c * bu[x]; }
        }
    }
    // Columns: f[y][x] = Σ_v tmp[v][x]·B[v][y].
    let mut out = [0f32; 64];
    for v in 0..8 {
        let t = &tmp[v * 8..v * 8 + 8];
        if t.iter().all(|&c| c == 0.0) { continue; }
        let bv = &b[v];
        for y in 0..8 {
            let w = bv[y];
            let o = &mut out[y * 8..y * 8 + 8];
            for x in 0..8 { o[x] += t[x] * w; }
        }
    }
    for (d, s) in block.iter_mut().zip(out) { *d = (s + 0.5).floor() as i32; }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `T[k][n] = C(k)/2 · cos((2n + 1)kπ/16)` in f64; the 2-D transforms are separable products of it.
    fn table() -> [[f64; 8]; 8] {
        let mut t = [[0.0; 8]; 8];
        for (k, row) in t.iter_mut().enumerate() {
            let c = if k == 0 { std::f64::consts::FRAC_1_SQRT_2 } else { 1.0 };
            for (n, v) in row.iter_mut().enumerate() { *v = c / 2.0 * ((2 * n + 1) as f64 * k as f64 * std::f64::consts::PI / 16.0).cos(); }
        }
        t
    }

    /// Forward DCT: `F[v][u] = Σ_y Σ_x f[y][x]·T[u][x]·T[v][y]`.
    fn fdct_f64(t: &[[f64; 8]; 8], f: &[f64; 64]) -> [f64; 64] {
        let mut rows = [0.0; 64];
        for y in 0..8 { for u in 0..8 { rows[y * 8 + u] = (0..8).map(|x| f[y * 8 + x] * t[u][x]).sum(); } }
        let mut out = [0.0; 64];
        for v in 0..8 { for u in 0..8 { out[v * 8 + u] = (0..8).map(|y| rows[y * 8 + u] * t[v][y]).sum(); } }
        out
    }

    /// Inverse DCT: `f[y][x] = Σ_v Σ_u F[v][u]·T[u][x]·T[v][y]`.
    fn idct_f64(t: &[[f64; 8]; 8], big_f: &[f64; 64]) -> [f64; 64] {
        let mut rows = [0.0; 64];
        for v in 0..8 { for x in 0..8 { rows[v * 8 + x] = (0..8).map(|u| big_f[v * 8 + u] * t[u][x]).sum(); } }
        let mut out = [0.0; 64];
        for y in 0..8 { for x in 0..8 { out[y * 8 + x] = (0..8).map(|v| rows[v * 8 + x] * t[v][y]).sum(); } }
        out
    }

    /// The IEEE 1180 random generator.
    struct Rand(i64);
    impl Rand {
        fn next(&mut self, l: i64, h: i64) -> i64 {
            self.0 = (self.0.wrapping_mul(1103515245).wrapping_add(12345)) & 0xffff_ffff;
            let i = self.0 & 0x7fff_fffe;
            let x = i as f64 / 0x7fff_ffff as f64 * (l + h + 1) as f64;
            x as i64 - l
        }
    }

    fn run(l: i64, h: i64, sign: i64) {
        let mut rng = Rand(1);
        let t = table();
        let n = 10_000;
        let (mut peak, mut sum_err, mut sum_sq) = (0i64, [0i64; 64], [0i64; 64]);
        for _ in 0..n {
            let mut f = [0f64; 64];
            for v in f.iter_mut() { *v = (rng.next(l, h) * sign) as f64; }
            let coef = fdct_f64(&t, &f).map(|c| (c.round() as i64).clamp(-2048, 2047));
            let reference = idct_f64(&t, &coef.map(|c| c as f64)).map(|s| (s.round() as i64).clamp(-256, 255));
            let mut test = coef.map(|c| c as i32);
            idct(&mut test);
            for k in 0..64 {
                let e = (test[k] as i64).clamp(-256, 255) - reference[k];
                peak = peak.max(e.abs());
                sum_err[k] += e;
                sum_sq[k] += e * e;
            }
        }
        let nf = n as f64;
        let pmse = sum_sq.iter().map(|&s| s as f64 / nf).fold(0.0, f64::max);
        let omse = sum_sq.iter().sum::<i64>() as f64 / (64.0 * nf);
        let pme = sum_err.iter().map(|&s| (s as f64 / nf).abs()).fold(0.0, f64::max);
        let ome = (sum_err.iter().sum::<i64>() as f64 / (64.0 * nf)).abs();
        eprintln!("IEEE 1180 L={l} H={h} sign={sign}: peak {peak}, pmse {pmse:.5}, omse {omse:.5}, pme {pme:.5}, ome {ome:.6}");
        assert!(peak <= 1, "peak error {peak}");
        assert!(pmse <= 0.06, "per-pixel mean square error {pmse}");
        assert!(omse <= 0.02, "overall mean square error {omse}");
        assert!(pme <= 0.015, "per-pixel mean error {pme}");
        assert!(ome <= 0.0015, "overall mean error {ome}");
    }

    #[test]
    fn ieee_1180_accuracy() {
        for (l, h) in [(256, 255), (5, 5), (300, 300)] {
            for sign in [1, -1] { run(l, h, sign); }
        }
        let mut z = [0i32; 64];
        idct(&mut z);
        assert!(z.iter().all(|&v| v == 0), "zero in, zero out");
    }
}
