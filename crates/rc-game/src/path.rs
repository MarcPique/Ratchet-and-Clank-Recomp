//! The path helpers the level-01 overlay does not link (gaps.md G-CLS-025): engine code over the level's paths
//! (gameplay section 0x70, `0x1b0930[i]` on level 01; the same table sits at 0x1b04b0 on 00, 0x1b0ab0 on 02,
//! 0x1b05b0 on 03, 0x1b0530 on 07, 0x1b0eb0 on 18: the level data area moves per overlay), stored here as
//! [`Services::splines`](crate::moby_update::Services) (`[u32; 4]` points: x, y, z and a w the callers read
//! differently: the roll here, a wall flag in [`push_from_walls`] and `ClampToPath`, a segment length for
//! `crate::spline`).
//!
//! **System or not** (the disassembly of every copy read 2026-09-29; the census clusters them by masked words, so
//! copies that differ only in the data table's address or a helper's `%lo` land in different clusters):
//!
//! | job | copies | finding | here |
//! |---|---|---|---|
//! | position + rotation at t | level00 `0x262e40` (cluster c4c12687d93c: 00, 02 `0x264718`, 04, 05, 06, 07 `0x28b668`, 08, 09, 11, 15, 16 `0x25e338`, 17, 18 `0x265650`), level01 `0x277d40` (level10 `0x2560c0`) | the same source: decompiled instruction for instruction alike, the only difference is `VecSub`'s address (`0x221188`-class boot call vs the overlay's own copy) | [`pose`] (one port) |
//! | keep a point r from the path walls | level00 `0x261d78` (cluster 49a176c9f6de: 00, 05, 06 `0x270d00`, 08, 10, 11, 13, 16, 18) | one source; not on 01 | [`push_from_walls`] |
//! | the point nearest a distance | level03 `0x264558` (03, 04, 14), level05 `0x2a0260` (05, 08, 12, 16), level01 `0x28b510` (the flyer driver's) | the same source three times (identical decompiles; `fabs`'s `%lo` differs); the level-01 copy was already ported as the flyer's `nearest` | [`nearest_at_distance`] (the flyer calls it) |
//! | a segment crosses a path wall | level00 `0x261b48` (00, 08, 13, 16, 18), level02 `0x263710` (02, 03 `0x24ffb0`, 04, 09, 12, 14, 15, 17), level01 `0x276820` `ClampToPath` | the same source: identical decompiles apart from the table address; `ClampToPath` is ported (`World::clamp_to_path`), only its found flag was dropped | `World::clamp_to_path_hit` |
//! | the point at a distance along equal segments | level01 `0x277260` (level10 `0x2554e0`) | one source; consumers class 947 and its sparks 1090 (level 10: `classes::units::orxon_wire`) | [`at_distance`] |
//! | the next path point toward a target | level15 `0x265b38` (cluster dc2339668382: 09 `0x294cb0`, 13 `0x282010`, 15) | one source (the census's "nearest points to a pair"); consumer 193 (09, 15: `classes::units::pack_biter`) | [`toward`] |
//!
//! Standard `f32` (the game's VU0 macro code; no result depends on its last bit). The rotation wraps use the
//! game's `fast_add_rotations` / `fast_subtract_rotations` (`creature::add_rot` / `sub_rot`).

#![allow(clippy::neg_cmp_op_on_partial_ord)] // The game's compares, NaN included, are spelled out on purpose.

use crate::moby_update::creature::{add_rot, atan, sub_rot};

/// One stored path point (`Services::splines`): x, y, z, w as `f32` bits.
pub type Point = [u32; 4];

fn pt(pts: &[Point], i: usize) -> [f32; 4] { pts.get(i).map(|q| q.map(f32::from_bits)).unwrap_or([0.0; 4]) }
fn sub(a: [f32; 4], b: [f32; 4]) -> [f32; 4] { [a[0] - b[0], a[1] - b[1], a[2] - b[2], a[3] - b[3]] }
fn add(a: [f32; 4], b: [f32; 4]) -> [f32; 4] { [a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3]] }
fn len3(v: [f32; 4]) -> f32 { (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt() }
fn len2(v: [f32; 4]) -> f32 { (v[0] * v[0] + v[1] * v[1]).sqrt() }
fn dot3(a: [f32; 4], b: [f32; 4]) -> f32 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }
/// `FastVecNormalize(len, out, v)`: `v` scaled to length `len` (xyz; a zero vector stays zero).
fn set_len(v: [f32; 4], len: f32) -> [f32; 4] {
    let l = len3(v);
    if l == 0.0 { return [0.0, 0.0, 0.0, v[3]]; }
    let k = len / l;
    [v[0] * k, v[1] * k, v[2] * k, v[3]]
}
/// `fun_001f9a40(t, out, a, b)`: `a + (b − a)·t` (all four lanes).
fn lerp(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] { [0, 1, 2, 3].map(|k| a[k] + (b[k] - a[k]) * t) }

/// `0x262e40(t, path, closed, &pos, &rot, flags)` (level01 `0x277d40`): the position `t` segments along the
/// path (`i = trunc(t)`, the fraction `f = t − i` between points i and i + 1) and, unless `flags & 1`, the
/// rotation there. The segment's yaw and pitch come from `p1 − p0`; on an open path `i` is held at `count − 2`
/// with `f ≤ 1` and the rotation is the segment's own; otherwise the yaw and pitch ease with `f` toward the next
/// segment's (`p2 − p1`) and the roll from `p0.w` toward `p1.w` (the points' w is a roll on these paths). The
/// rotation is `(0, −pitch, yaw, −roll)` (the game writes the moby's rotation w too).
///
/// `i + 1` and `i + 2` wrap with `%` once they reach `count`; the game reads `pts[i]` unwrapped (a `t ≥ count` on
/// a closed path reads past the path: its callers wrap `t` first), the port wraps it as well [L]. A negative `t`
/// reads the header before the points in the game; here it is clamped to point 0 [L].
pub fn pose(pts: &[Point], closed: bool, t: f32, want_rot: bool) -> ([f32; 4], [f32; 4]) {
    let n = pts.len() as i32;
    if n == 0 { return ([0.0; 4], [0.0; 4]); }
    let mut i = t as i32;
    let mut clamped = false;
    if !closed && n - 2 <= i {
        clamped = true;
        i = n - 2;
    }
    let mut f = t - i as f32;
    if clamped && 1.0 < f { f = 1.0; }
    let mut i1 = i + 1;
    let mut i2 = i + 2;
    if n <= i2 {
        i1 %= n;
        i2 %= n;
    }
    let idx = |k: i32| k.rem_euclid(n) as usize;
    let (p0, p1) = (pt(pts, idx(i)), pt(pts, idx(i1)));
    let pos = lerp(p0, p1, f);
    if !want_rot { return (pos, [0.0; 4]); }
    let d = sub(p1, p0);
    let yaw0 = atan(d[0], d[1]);
    let pitch0 = atan(len2(d), d[2]);
    let (mut yaw1, mut pitch1, mut roll0, mut roll1) = (yaw0, pitch0, 0.0, 0.0);
    if !clamped {
        let p2 = pt(pts, idx(i2));
        let e = sub(p2, p1);
        yaw1 = atan(e[0], e[1]);
        pitch1 = atan(len2(e), e[2]);
        roll0 = p0[3];
        roll1 = p1[3];
    }
    let ease = |a: f32, b: f32| add_rot(sub_rot(b, a) * f, a);
    (pos, [0.0, -ease(pitch0, pitch1), ease(yaw0, yaw1), -ease(roll0, roll1)])
}

/// `0x261d78(r, path index, &pos, &out)`: the position pushed to distance `r` from the path's walls, or None when
/// no wall is within `r`. Every segment i → i + 1 whose two points have a non-zero w is a wall (a segment with
/// both w = 0 is a gap); the test is horizontal (z dropped): with `a = pos − pᵢ`, `n` the unit direction of the
/// segment and `d = |a × n|` the distance from its line, a segment with `d ≤ r` moves the position: when the foot
/// of `a` lies within the segment, to `pᵢ + n·(a·n) + r·(a − n·(a·n))/|…|` (r off the wall on its own side);
/// when it lies beyond an end, to `pᵢ + r·a/|a|` (r from the point) unless `|a| ≥ r`. The z stays the input's;
/// later walls see the moved position.
pub fn push_from_walls(pts: &[Point], r: f32, pos: [f32; 4]) -> Option<[f32; 4]> {
    let n = pts.len();
    let mut p = pos;
    let mut found = false;
    for i in 0..n.saturating_sub(1) {
        let (p0, p1) = (pt(pts, i), pt(pts, i + 1));
        if p0[3] == 0.0 && p1[3] == 0.0 { continue; }
        let mut a = sub(p, p0);
        a[2] = 0.0;
        let mut e = sub(p1, p0);
        e[2] = 0.0;
        let dir = set_len(e, 1.0);
        let d = (a[0] * dir[1] - a[1] * dir[0]).abs();
        if !(d <= r) { continue; }
        let len = len3(e);
        let proj = dot3(a, dir);
        let o = if len < proj || proj < 0.0 {
            if r <= len3(a) { continue; }
            set_len(a, r)
        } else {
            let foot = set_len(dir, proj);
            add(set_len(sub(a, foot), r), foot)
        };
        p = add(o, p0);
        p[2] = pos[2];
        found = true;
    }
    found.then_some(p)
}

/// `0x264558(d, pos, path)` (level05 `0x2a0260`, level01 `0x28b510`): the index of the first point whose
/// distance to `pos` is nearest to `d` (`| |pᵢ − pos| − d |` smallest; a later equal one loses); 0 on an empty
/// path.
pub fn nearest_at_distance(pts: &[Point], d: f32, pos: [f32; 4]) -> i32 {
    let mut best = 1e11f32;
    let mut k = 0;
    for (i, q) in pts.iter().enumerate() {
        let e = (len3(sub(q.map(f32::from_bits), pos)) - d).abs();
        if !(best <= e) {
            best = e;
            k = i as i32;
        }
    }
    k
}

/// Level09 `0x294cb0(moby, path, &target, &out)` (level15 `0x265b38`, level13 `0x282010`): the path point to head for
/// on the way from `me` to `target` along a path of waypoints. Of the points nearer `target` than `me` is (3-D), the one
/// nearest `target` (`b`); the point nearest `me` (`a`, strictly nearer wins, the first on a tie). No such `b` →
/// `target` itself; `a == b` → point `a`; `a < b` → point `a + 1`; else point `a − 1`.
pub fn toward(pts: &[Point], me: [f32; 4], target: [f32; 4]) -> [f32; 4] {
    let d0 = len3(sub(me, target));
    let (mut a, mut b) = (-1i32, -1i32);
    let (mut best_a, mut best_b) = (10000.0f32, 10000.0f32);
    for (k, _) in pts.iter().enumerate() {
        let p = pt(pts, k);
        let dt = len3(sub(p, target));
        let ds = len3(sub(p, me));
        if !(best_a <= ds) {
            best_a = ds;
            a = k as i32;
        }
        if dt < d0 && !(best_b <= dt) {
            best_b = dt;
            b = k as i32;
        }
    }
    if b == -1 { return target; }
    let i = if a == b { a } else if a < b { a + 1 } else { a - 1 };
    pt(pts, i.max(0) as usize)
}

/// `0x277260(d, seg, path, &node, &rem, &out)`: the point `d` along a path of equal segments of length `seg`: node =
/// `trunc(d / seg)`; inside the path the point `rem = d − node·seg` past point `node` toward the next (`VecNormalize`);
/// before the start point 0, past the end the last point (rem 0). Returns (node, rem, point).
pub fn at_distance(pts: &[Point], d: f32, seg: f32) -> (i32, f32, [f32; 4]) {
    let n = pts.len() as i32;
    let node = (d / seg) as i32;
    if node < n - 1 && -1 < node {
        let rem = d - node as f32 * seg;
        let (a, b) = (pt(pts, node as usize), pt(pts, node as usize + 1));
        let v = [b[0] - a[0], b[1] - a[1], b[2] - a[2], b[3] - a[3]];
        let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        let k = if l == 0.0 { 0.0 } else { rem / l };
        return (node, rem, [v[0] * k + a[0], v[1] * k + a[1], v[2] * k + a[2], v[3] * k + a[3]]);
    }
    let node = if node < n - 1 { 0 } else { n - 1 };
    (node, 0.0, pt(pts, node.max(0) as usize))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(xyz: &[[f32; 3]], w: &[f32]) -> Vec<Point> {
        xyz.iter().enumerate().map(|(i, p)| [p[0], p[1], p[2], w.get(i).copied().unwrap_or(0.0)].map(f32::to_bits)).collect()
    }
    fn close(a: [f32; 4], b: [f32; 4]) -> bool { (0..3).all(|k| (a[k] - b[k]).abs() < 1e-4) }

    /// `0x294cb0`: along a row of waypoints the next one toward the target from the nearest one; the target itself when
    /// no waypoint is nearer it than the walker; the nearest one when it is also the one nearest the target.
    #[test]
    fn toward_steps_along_the_waypoints() {
        let pts = path(&[[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [20.0, 0.0, 0.0], [30.0, 0.0, 0.0]], &[]);
        let v = |x: f32| [x, 1.0, 0.0, 1.0];
        assert_eq!(toward(&pts, v(1.0), v(29.0))[0], 10.0, "a = 0 < b = 3: point 1");
        assert_eq!(toward(&pts, v(29.0), v(1.0))[0], 20.0, "a = 3 > b = 0: point 2");
        assert_eq!(toward(&pts, v(20.5), v(21.0)), v(21.0), "no point nearer the target than the walker");
        assert_eq!(toward(&pts, v(12.0), v(9.0))[0], 10.0, "a = b = 1");
        assert_eq!(toward(&[], v(0.0), v(5.0)), v(5.0));
    }

    /// A straight-then-up open path: the position lerps within the segment, the rotation eases toward the next
    /// segment's yaw / pitch, and the last segment is held (t clamped, its own rotation, roll 0).
    #[test]
    fn pose_open_path() {
        let pts = path(&[[0.0, 0.0, 0.0], [4.0, 0.0, 0.0], [4.0, 4.0, 0.0], [4.0, 4.0, 3.0]], &[0.0, 0.5, 1.0, 0.0]);
        let (p, r) = pose(&pts, false, 0.5, true);
        assert!(close(p, [2.0, 0.0, 0.0, 0.25]), "{p:?}");
        // Segment 0 yaw 0, segment 1 yaw π/2: halfway → π/4; pitch 0; roll from 0 toward 0.5 → 0.25 (negated).
        assert!((r[2] - std::f32::consts::FRAC_PI_4).abs() < 1e-5 && r[1] == 0.0 && (r[3] + 0.25).abs() < 1e-6, "{r:?}");
        // t = 1.25: on segment 1 (yaw π/2) easing toward segment 2 (yaw 0 — atan(0, 0) —, pitch π/2).
        let (p, r) = pose(&pts, false, 1.25, true);
        assert!(close(p, [4.0, 1.0, 0.0, 0.0]));
        assert!((r[1] + std::f32::consts::FRAC_PI_2 * 0.25).abs() < 1e-5, "{r:?}");
        // Past count − 2: held on the last segment with f capped at 1.
        let (p, r) = pose(&pts, false, 2.5, true);
        assert!(close(p, [4.0, 4.0, 1.5, 0.5]), "{p:?}");
        let (p, _) = pose(&pts, false, 7.0, true);
        assert!(close(p, [4.0, 4.0, 3.0, 0.0]), "{p:?}");
        assert!((r[1] + std::f32::consts::FRAC_PI_2).abs() < 1e-5 && r[2] == 0.0 && r[3] == 0.0, "{r:?}");
        // No rotation wanted: the rotation is left untouched (zeros here).
        assert_eq!(pose(&pts, false, 0.5, false).1, [0.0; 4]);
    }

    /// A closed square: the last segment eases toward the first, and the indices wrap.
    #[test]
    fn pose_closed_path() {
        let pts = path(&[[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [2.0, 2.0, 0.0], [0.0, 2.0, 0.0]], &[]);
        let (p, r) = pose(&pts, true, 3.5, true);
        assert!(close(p, [0.0, 1.0, 0.0, 0.0]), "{p:?}");
        // Segment 3 yaw −π/2 → segment 0 yaw 0: halfway −π/4.
        assert!((r[2] + std::f32::consts::FRAC_PI_4).abs() < 1e-5, "{r:?}");
        let (p, _) = pose(&pts, true, 2.0, true);
        assert!(close(p, [2.0, 2.0, 0.0, 0.0]));
    }

    /// A wall along x from 0 to 10 (w ≠ 0 on both points) and a gap segment after it.
    #[test]
    fn push_from_walls_moves_a_near_point() {
        let pts = path(&[[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [20.0, 0.0, 0.0]], &[1.0, 1.0, 0.0]);
        // Within the segment, 0.5 off the wall on the +y side, r = 2: pushed to y = 2, z kept.
        let p = push_from_walls(&pts, 2.0, [4.0, 0.5, 7.0, 1.0]).unwrap();
        assert!(close(p, [4.0, 2.0, 7.0, 0.0]), "{p:?}");
        // On the −y side: pushed to −2.
        let p = push_from_walls(&pts, 2.0, [4.0, -0.5, 0.0, 1.0]).unwrap();
        assert!(close(p, [4.0, -2.0, 0.0, 0.0]), "{p:?}");
        // Far from the wall: nothing.
        assert!(push_from_walls(&pts, 2.0, [4.0, 5.0, 0.0, 1.0]).is_none());
        // Beyond the start within r of the point: pushed radially from the point.
        let p = push_from_walls(&pts, 2.0, [-1.0, 0.0, 0.0, 1.0]).unwrap();
        assert!(close(p, [-2.0, 0.0, 0.0, 0.0]), "{p:?}");
        // Beyond the start but r or more from the point: nothing.
        assert!(push_from_walls(&pts, 2.0, [-3.0, 0.0, 0.0, 1.0]).is_none());
        // The gap segment (10 → 20, both w = 0 … the second point's w = 1 makes it a wall): a point near 15 on a
        // path whose second segment has both w = 0 is left alone.
        let gap = path(&[[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [20.0, 0.0, 0.0]], &[1.0, 0.0, 0.0]);
        assert!(push_from_walls(&gap, 2.0, [15.0, 0.5, 0.0, 1.0]).is_none());
        assert!(push_from_walls(&pts, 2.0, [15.0, 0.5, 0.0, 1.0]).is_some());
    }

    #[test]
    fn nearest_at_distance_picks_the_first_best() {
        let pts = path(&[[0.0, 0.0, 0.0], [3.0, 0.0, 0.0], [6.0, 0.0, 0.0], [9.0, 0.0, 0.0]], &[]);
        assert_eq!(nearest_at_distance(&pts, 0.0, [5.0, 0.0, 0.0, 1.0]), 2);
        // Distance 4 from x = 5: points at 1 and 9 tie in |dist − 4| … 0 → |5 − 4| = 1, 9 → |4 − 4| = 0.
        assert_eq!(nearest_at_distance(&pts, 4.0, [5.0, 0.0, 0.0, 1.0]), 3);
        // A tie keeps the first: x = 4.5 is 1.5 from both 3 and 6.
        assert_eq!(nearest_at_distance(&pts, 0.0, [4.5, 0.0, 0.0, 1.0]), 1);
        assert_eq!(nearest_at_distance(&[], 0.0, [0.0; 4]), 0);
    }
}
