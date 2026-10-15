//! What the gun missiles share (the Devastator's class 153, `0x2c5b70`, and the R.Y.N.O.'s class 457, `0x2e5a48`: the
//! same block of code in both updates): **the intercept** ([`intercept_time`]) — the missile at speed `s` and the
//! target moving by `vt` a tick, `rel` from the missile's reference point to the target's aim point: the time `t` of
//! `|rel + vt·t| = s·t`, i.e. the roots of `(s² − |vt|²)·t² − 2(vt·rel)·t − |rel|² = 0`; both positive → the larger,
//! one positive → it, none → −1; the missile then turns toward `rel + vt·t` ([`lead_point`]). The turn itself is the
//! engine's `SpringTurn` 0x26cef0 (`crate::moby_update::creature::turn::spring_turn`), with each update's own rates.
//!
//! Native `f32`. The discriminant's square root is taken of its magnitude [L: `fun_001f9988` on the PS2 FPU, whose
//! `sqrt.s` ignores the sign; only reached when the target outruns the missile].

use crate::hero::guns::{add3, dot3, scale3};

/// The intercept time (module doc): `speed` per tick, `vt` the target's motion this tick, `rel` missile → target.
pub fn intercept_time(speed: f32, vt: [f32; 3], rel: [f32; 3]) -> f32 {
    let a = speed * speed - dot3(vt, vt);
    let b = dot3(vt, rel) * -2.0;
    let c = dot3(rel, rel);
    let s = (b * b - a * 4.0 * -c).abs().sqrt();
    let t1 = (-b + s) / (a + a);
    let t2 = (-b - s) / (a + a);
    if t1 <= 0.0 || t2 <= 0.0 {
        if t1 <= 0.0 { if 0.0 < t2 { t2 } else { -1.0 } } else { t1 }
    } else if t1 <= t2 {
        t2
    } else {
        t1
    }
}

/// `rel + vt·t`: where to turn to.
pub fn lead_point(vt: [f32; 3], rel: [f32; 3], t: f32) -> [f32; 3] { add3(scale3(vt, t), rel) }

#[cfg(test)]
mod tests {
    use super::*;

    /// A still target: `t = |rel| / s`; a target crossing at right angles: `s·t = |rel + vt·t|`.
    #[test]
    fn intercepts() {
        let t = intercept_time(0.5, [0.0; 3], [10.0, 0.0, 0.0]);
        assert!((t - 20.0).abs() < 1e-4, "{t}");
        let vt = [0.0, 0.1, 0.0];
        let t = intercept_time(0.5, vt, [10.0, 0.0, 0.0]);
        let p = lead_point(vt, [10.0, 0.0, 0.0], t);
        assert!((dot3(p, p).sqrt() - 0.5 * t).abs() < 1e-3, "{t} {p:?}");
        // A target faster than the missile moving away: no positive root.
        assert_eq!(intercept_time(0.1, [1.0, 0.0, 0.0], [10.0, 0.0, 0.0]), -1.0);
    }
}
