//! The spline follower: the game's polyline splines (gameplay sections 0x70 "paths", `0x1b0930[i]`, and 0x74
//! "grind paths", `0x15f70c[i]`) and the three library functions every rider uses — the grind rails and the cable
//! of the hero (`hero::boots`), and the moby classes that ride a path. Level00 addresses (the grind code only
//! exists in the overlays with rails; the same functions are level01 0x272660 / 0x2726c8 / 0x272e28):
//!
//! | game | here | what |
//! |---|---|---|
//! | `0x25d7a0` | [`step_index`] | the next / previous point index (clamped on an open spline, wrapped on a closed one) |
//! | `0x25d808` (doc name `SplineProject`) | [`advance`] | move a cursor `(segment, distance into it)` along the spline by a signed distance and return the point |
//! | `0x25df68` (doc name `SplineSample`) | [`nearest`] | the point of the spline nearest to a position: a coarse walk in steps, then the exact point on the two segments around the nearest vertex |
//! | `0x25da70` | [`band_distance`] | the distance the search uses: horizontal inside a vertical band, 3-D outside it |
//! | `0x25dcd8` / `0x25db00` | [`closest_on_segment`] | the nearest point of one segment (2-D inside the band, the height interpolated; else 3-D) |
//!
//! (The two doc names were coined from the call sites before the functions were read; what they do is the other
//! way round. The port names them by what they do.)
//!
//! **Data.** A spline is `count` points `(x, y, z, w)`; **w is the length of the segment to the next point** (the
//! last point's w is the distance back to the first, used when the spline is closed). The grind-path header's
//! word +0x14 (`rc_formats::volumes::GrindPath::flag`) is the closed flag every call passes (1 = closed loop).
//! Checked on every level's grind paths: w equals the chord to the next point (`tests`, and
//! `tests/hero/hero_boots_grind.rs`).
//!
//! **Quirks kept** (they decide where a rider ends up):
//! * [`advance`] backwards past the first point of an open spline wraps the index to the last point and then
//!   returns the first point with the cursor at `(0, 0)`, reporting the end; forwards it stops at the last point
//!   (cursor on the last point, which has no segment of its own). A closed spline reports the wrap of the last
//!   segment as an end too, but keeps going.
//! * [`nearest`] fails (the caller keeps its old cursor and point) when no coarse sample lies within
//!   `range + step`; the coarse walk keeps the *first* of equally near samples.
//!
//! Standard `f32` arithmetic and `f32::sqrt` (the game's VU0 macro code; no result depends on its last bit).
#![allow(clippy::neg_cmp_op_on_partial_ord)] // The game's compares, NaN included, are spelled out on purpose.

/// One spline point: position and the length of the segment to the next point.
pub type Point = [f32; 4];

/// A position on a spline: segment `seg` (from point `seg` to the next), `t` units along it (0x13f8b4 / 0x13f8b8
/// for the hero's rail).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Cursor {
    pub seg: i32,
    pub t: f32,
}

fn xyz(p: Point) -> [f32; 3] { [p[0], p[1], p[2]] }
fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }
fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0] + b[0], a[1] + b[1], a[2] + b[2]] }
fn scale(a: [f32; 3], k: f32) -> [f32; 3] { [a[0] * k, a[1] * k, a[2] * k] }
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }
/// 3-D length.
pub fn len3(a: [f32; 3]) -> f32 { dot(a, a).sqrt() }
/// 3-D distance.
pub fn dist3(a: [f32; 3], b: [f32; 3]) -> f32 { len3(sub(a, b)) }
/// Horizontal (x, y) distance.
pub fn dist2(a: [f32; 3], b: [f32; 3]) -> f32 { ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt() }
/// `FastVecNormalize(len, v)`: `v` scaled to length `len`, zero for a zero vector.
fn set_len(v: [f32; 3], len: f32) -> [f32; 3] {
    let l = len3(v);
    if l == 0.0 { [0.0; 3] } else { scale(v, len / l) }
}

/// `0x25d7a0(spline, i, delta, closed)`: point index `i + delta`, clamped to the spline on an open one, wrapped
/// on a closed one.
pub fn step_index(pts: &[Point], i: i32, delta: i32, closed: bool) -> i32 {
    let n = pts.len() as i32;
    if n == 0 { return i; }
    if !closed {
        let j = i + delta;
        return if delta < 0 { j.max(0) } else { j.min(n - 1) };
    }
    (i + delta + n).rem_euclid(n)
}

/// `0x25d808(dist, spline, &out, &seg, &t, closed)`: move `c` by `dist` along the spline (negative: backwards)
/// and return the point there and whether an end was reached (module doc for the ends).
pub fn advance(pts: &[Point], closed: bool, dist: f32, c: &mut Cursor) -> ([f32; 3], bool) {
    let n = pts.len() as i32;
    if n == 0 { return ([0.0; 3], true); }
    let (mut d, dir) = if dist < 0.0 { (-dist, -1) } else { (dist, 1) };
    let mut ended = false;
    // A spline of zero-length segments would never end on a closed loop (the game would hang): give up after
    // enough crossings.
    let mut guard = 4 * n as usize + 64;
    loop {
        let i = c.seg.clamp(0, n - 1);
        let at_end = i == n - 1 && !closed;
        if d == 0.0 || at_end || guard == 0 {
            if at_end {
                if dir == -1 {
                    c.seg = 0;
                    c.t = 0.0;
                    return (xyz(pts[0]), true);
                }
                return (xyz(pts[i as usize]), true);
            }
            let j = (i + 1).rem_euclid(n);
            let p = xyz(pts[i as usize]);
            let q = xyz(pts[j as usize]);
            return (add(p, set_len(sub(q, p), c.t)), ended);
        }
        guard -= 1;
        let t = c.t;
        let cross = if dir == 1 { pts[i as usize][3] < t + d } else { t < d };
        if !cross {
            c.t = if dir == 1 { t + d } else { t - d };
            d = 0.0;
            continue;
        }
        let mut j = i + dir;
        if j >= n || j < 0 { j = (j + n).rem_euclid(n); }
        let nd = if dir == 1 {
            let rem = d - (pts[i as usize][3] - t);
            c.t = 0.0;
            if 0.0 <= rem { rem } else { 0.0 }
        } else {
            c.t = pts[j as usize][3];
            d - t
        };
        if (dir == 1 && i == n - 1) || (dir == -1 && i == 0) { ended = true; }
        c.seg = j;
        d = nd;
    }
}

/// `0x25da70(band, a, b)`: the horizontal distance when `band > 0` and the heights differ by less than `band`,
/// else the 3-D distance.
pub fn band_distance(band: f32, a: [f32; 3], b: [f32; 3]) -> f32 {
    if band <= 0.0 || band <= (a[2] - b[2]).abs() { dist3(a, b) } else { dist2(a, b) }
}

/// `0x25dcd8(band, &out, p, a, b)`: the point of segment `a → b` nearest to `p`, and the distance to it. Inside the
/// vertical band (both ends within `band` of `p`'s height) the search is horizontal (`0x25db00`) and the height is
/// interpolated along the segment at the found point; otherwise 3-D. A foot point outside the segment becomes the
/// nearer end.
pub fn closest_on_segment(band: f32, p: [f32; 3], a: [f32; 3], b: [f32; 3]) -> ([f32; 3], f32) {
    let beyond = |x: f32, u: f32, v: f32| (u < x && v < x) || (x < u && x < v);
    if 0.0 < band && (p[2] - a[2]).abs() < band && (p[2] - b[2]).abs() < band {
        // 0x25db00: the line through a, b in the plane, its unit normal n; the foot of p.
        let e = [a[0] - b[0], a[1] - b[1]];
        let l = (e[0] * e[0] + e[1] * e[1]).sqrt();
        let nrm = if l == 0.0 { [0.0, 0.0] } else { [-e[1] / l, e[0] / l] };
        let s = -(nrm[0] * a[0] + nrm[1] * a[1]) + nrm[0] * p[0] + nrm[1] * p[1];
        let mut o = [p[0] - s * nrm[0], p[1] - s * nrm[1], 0.0];
        if (0..2).any(|k| beyond(o[k], a[k], b[k])) {
            let (da, db) = (dist2(o, a), dist2(o, b));
            o = if da < db { [a[0], a[1], 0.0] } else { [b[0], b[1], 0.0] };
        }
        let d = dist2(p, o);
        // 0x25b6b8: a + (b − a)·f with f = |a − o|₂ / |a − b|₂.
        let ab = dist2(a, b);
        let f = if ab == 0.0 { 0.0 } else { dist2(a, o) / ab };
        return (add(a, scale(sub(b, a), f)), d);
    }
    let dir = set_len(sub(b, a), 1.0);
    let dd = dot(dir, dir);
    let k = if dd == 0.0 { 0.0 } else { (dot(p, dir) - dot(a, dir)) / dd };
    let mut o = add(a, scale(dir, k));
    if (0..3).any(|k| beyond(o[k], a[k], b[k])) {
        o = if dist3(o, a) < dist3(o, b) { a } else { b };
    }
    (o, dist3(p, o))
}

/// `0x25df68(range, step, band, spline, p, &out, &seg, &t, closed)`: the point of the spline nearest to `p` and its
/// cursor, or None when no sample of the coarse walk (every `step` units from the first point, distances by
/// [`band_distance`]) lies within `range + step` (the caller keeps its old cursor then). The exact point is the
/// nearer of the two segments around the vertex nearest to `p` among those between one step before and one step
/// after the best sample.
pub fn nearest(pts: &[Point], closed: bool, range: f32, step: f32, band: f32, p: [f32; 3]) -> Option<([f32; 3], Cursor)> {
    let n = pts.len() as i32;
    if n == 0 { return None; }
    let mut best_d = band_distance(band, xyz(pts[0]), p);
    let mut best = Cursor::default();
    let mut c = Cursor::default();
    loop {
        let (q, ended) = advance(pts, closed, step, &mut c);
        let d = band_distance(band, q, p);
        if !(best_d <= d) {
            best_d = d;
            best = c;
        }
        if ended { break; }
    }
    if !(best_d <= step + range) { return None; }
    let mut back = best;
    advance(pts, closed, -step, &mut back);
    let mut fwd = best;
    advance(pts, closed, step, &mut fwd);
    let last = step_index(pts, fwd.seg, 1, closed);
    let mut i = back.seg;
    let (mut v, mut vd) = (best.seg, 1e9f32);
    for _ in 0..=n {
        let d = band_distance(band, xyz(pts[i as usize]), p);
        if !(vd <= d) {
            vd = d;
            v = i;
        }
        if i == last { break; }
        i = (i + 1) % n;
    }
    let mut out = (xyz(pts[v as usize]), Cursor { seg: v, t: 0.0 });
    let mut da = 1e9f32;
    if 0 < v || closed {
        let a = (v + n - 1) % n;
        let (o, d) = closest_on_segment(band, p, xyz(pts[a as usize]), xyz(pts[v as usize]));
        da = d;
        out = (o, Cursor { seg: a, t: dist3(xyz(pts[a as usize]), o) });
    }
    if v < n - 1 || closed {
        let b = (v + 1) % n;
        let (o, d) = closest_on_segment(band, p, xyz(pts[v as usize]), xyz(pts[b as usize]));
        if d < da { out = (o, Cursor { seg: v, t: dist3(xyz(pts[v as usize]), o) }); }
    }
    Some(out)
}

/// The spline's points with each w recomputed as the chord to the next point (the last to the first): what the
/// level data holds. For hand-built splines.
pub fn with_lengths(xyz_pts: &[[f32; 3]]) -> Vec<Point> {
    let n = xyz_pts.len();
    (0..n).map(|i| {
        let a = xyz_pts[i];
        let b = xyz_pts[(i + 1) % n];
        [a[0], a[1], a[2], dist3(a, b)]
    }).collect()
}

/// The direction of the segment the cursor is on, as the grind code reads it (`0x25d7a0` then the next point minus
/// the cursor's point): yaw `atan2(dy, dx)` and pitch `atan2(dz, |dxy|)`; None when the next point coincides with
/// the cursor's point (the end of an open spline).
pub fn heading(pts: &[Point], closed: bool, c: Cursor, at: [f32; 3], dir: i32) -> Option<(f32, f32)> {
    let j = step_index(pts, c.seg, dir, closed);
    let d = sub(xyz(*pts.get(j as usize)?), at);
    if len3(d) < 0.001 { return None; }
    Some((d[1].atan2(d[0]), d[2].atan2((d[0] * d[0] + d[1] * d[1]).sqrt())))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: [f32; 3], b: [f32; 3]) -> bool { dist3(a, b) < 1e-4 }

    #[test]
    fn step_index_clamps_or_wraps() {
        let s = with_lengths(&[[0.0; 3], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]]);
        assert_eq!((step_index(&s, 2, 1, false), step_index(&s, 0, -1, false)), (2, 0));
        assert_eq!((step_index(&s, 2, 1, true), step_index(&s, 0, -1, true)), (0, 2));
    }

    /// A straight open spline: forward, across points, to the end; backward past the start.
    #[test]
    fn advance_straight() {
        let s = with_lengths(&[[0.0; 3], [4.0, 0.0, 0.0], [10.0, 0.0, 0.0]]);
        let mut c = Cursor::default();
        let (p, e) = advance(&s, false, 1.5, &mut c);
        assert!(close(p, [1.5, 0.0, 0.0]) && !e && c == Cursor { seg: 0, t: 1.5 });
        let (p, e) = advance(&s, false, 3.0, &mut c);
        assert!(close(p, [4.5, 0.0, 0.0]) && !e && c.seg == 1 && (c.t - 0.5).abs() < 1e-6);
        // Exactly to the end of a segment stays on it (the crossing test is strict).
        let mut d = Cursor { seg: 0, t: 1.0 };
        let (p, _) = advance(&s, false, 3.0, &mut d);
        assert!(close(p, [4.0, 0.0, 0.0]) && d == Cursor { seg: 0, t: 4.0 });
        // Past the last point: stops on it, reports the end.
        let (p, e) = advance(&s, false, 20.0, &mut c);
        assert!(close(p, [10.0, 0.0, 0.0]) && e && c.seg == 2);
        // Backwards within a segment, across a point, and past the first point (→ point 0, cursor (0, 0)).
        let mut c = Cursor { seg: 1, t: 2.0 };
        let (p, e) = advance(&s, false, -1.0, &mut c);
        assert!(close(p, [5.0, 0.0, 0.0]) && !e);
        let (p, e) = advance(&s, false, -2.0, &mut c);
        assert!(close(p, [3.0, 0.0, 0.0]) && !e && c == Cursor { seg: 0, t: 3.0 });
        let (p, e) = advance(&s, false, -5.0, &mut c);
        assert!(close(p, [0.0; 3]) && e && c == Cursor { seg: 0, t: 0.0 });
    }

    /// A closed square: the wrap of the last segment is reported but the ride goes on.
    #[test]
    fn advance_closed_loop() {
        let s = with_lengths(&[[0.0; 3], [2.0, 0.0, 0.0], [2.0, 2.0, 0.0], [0.0, 2.0, 0.0]]);
        let mut c = Cursor { seg: 3, t: 1.0 };
        let (p, e) = advance(&s, true, 2.0, &mut c);
        assert!(close(p, [1.0, 0.0, 0.0]) && e && c == Cursor { seg: 0, t: 1.0 });
        let (p, e) = advance(&s, true, -3.0, &mut c);
        assert!(close(p, [0.0, 2.0, 0.0]) && e && c.seg == 3);
        // A full lap comes back to the same point.
        let mut c = Cursor { seg: 1, t: 0.5 };
        let (p0, _) = advance(&s, true, 0.0, &mut c);
        let (p1, _) = advance(&s, true, 8.0, &mut c);
        assert!(close(p0, p1));
    }

    /// The nearest point on a straight spline, inside and outside the vertical band.
    #[test]
    fn nearest_straight() {
        let s = with_lengths(&[[0.0; 3], [4.0, 0.0, 0.0], [10.0, 0.0, 1.0]]);
        let (p, c) = nearest(&s, false, 999.0, 2.0, 2.5, [2.0, 1.0, 0.5]).unwrap();
        assert!(close(p, [2.0, 0.0, 0.0]) && c.seg == 0 && (c.t - 2.0).abs() < 1e-5);
        // Inside the band the height comes from the segment (horizontal search).
        let (p, c) = nearest(&s, false, 999.0, 2.0, 2.5, [7.0, -1.0, 2.0]).unwrap();
        assert!(close(p, [7.0, 0.0, 0.5]) && c.seg == 1, "{p:?} {c:?}");
        // Band 0: 3-D foot point.
        let (p, _) = nearest(&s, false, 999.0, 2.0, 0.0, [2.0, 0.0, 3.0]).unwrap();
        assert!(close(p, [2.0, 0.0, 0.0]));
        // Before the start / past the end: the end points.
        let (p, c) = nearest(&s, false, 999.0, 2.0, 2.5, [-3.0, 0.0, 0.0]).unwrap();
        assert!(close(p, [0.0; 3]) && c == Cursor { seg: 0, t: 0.0 });
        let (p, _) = nearest(&s, false, 999.0, 2.0, 2.5, [12.0, 0.0, 1.0]).unwrap();
        assert!(close(p, [10.0, 0.0, 1.0]));
        // Out of range: nothing.
        assert!(nearest(&s, false, 12.0, 10.0, 0.0, [5.0, 40.0, 0.0]).is_none());
    }

    /// A curved (quarter-circle) spline: the nearest point lies on the chord between the two nearest vertices,
    /// and advancing from it follows the curve.
    #[test]
    fn nearest_and_ride_curved() {
        let pts: Vec<[f32; 3]> = (0..=16).map(|i| {
            let a = i as f32 * std::f32::consts::FRAC_PI_2 / 16.0;
            [10.0 * a.cos(), 10.0 * a.sin(), 0.0]
        }).collect();
        let s = with_lengths(&pts);
        let q = [8.0 * 0.7f32.cos(), 8.0 * 0.7f32.sin(), 0.3];
        let (p, c) = nearest(&s, false, 999.0, 2.0, 2.5, q).unwrap();
        assert!((len3([p[0], p[1], 0.0]) - 10.0).abs() < 0.05, "{p:?}");
        assert!(((p[1].atan2(p[0])) - 0.7).abs() < 0.01);
        assert_eq!(c.seg, (0.7 / (std::f32::consts::FRAC_PI_2 / 16.0)) as i32);
        let mut c2 = c;
        let (r, _) = advance(&s, false, 3.0, &mut c2);
        assert!((len3([r[0], r[1], 0.0]) - 10.0).abs() < 0.05);
        assert!((r[1].atan2(r[0]) - (0.7 + 0.3)).abs() < 0.01);
        let (yaw, pitch) = heading(&s, false, c2, r, 1).unwrap();
        assert!((yaw - (1.0 + std::f32::consts::FRAC_PI_2)).abs() < 0.1 && pitch == 0.0);
    }
}
