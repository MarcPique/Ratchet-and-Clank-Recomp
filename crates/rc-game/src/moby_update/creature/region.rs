//! Regions and waypoint graphs on the level's paths (`0x1b0930[i]`, the gameplay splines as the loader copied them;
//! level01 addresses): the amoeboids keep to an arena bounded by one path and move between the points of another.
//!
//! * A path is a polygon of points; an edge (i, i+1) is a wall unless both its end points have w = 0.
//! * A waypoint graph is a path whose point `i` carries in its w word the bit mask of the points visible from it.
//!
//! | fn | game | what |
//! |---|---|---|
//! | [`point_in_polygon`] | `0x26e6c0` | crossing-number test of the xy point against the path's points |
//! | [`crosses`] | `0x276640` | does the segment a → b cross a wall of the path (strictly between the ends)? |
//! | [`clamp`] | `ClampToPath` 0x276820 | the first crossing point along a → b (else b) and whether there was one |
//! | [`visible_nodes`] | `0x276a48` | the graph points reachable from `p` without crossing the walls (a ±r corridor) |
//! | [`meet`] | `0x276c40` | the nodes of mask A reachable from mask B through the graph (up to 32 expansions) |
//! | [`line_of_sight`] | `LineOfSightTest` 0x276fe8 | straight to the target when no wall is in the way (±r), else the graph point nearest the target that both ends reach |
//! | [`graph_init`] | level11 `0x2873b0` (copies on 05, 06, 10; none on 01) | a graph's visibility masks: the point pairs no wall separates |
//! | [`push_out`] | level11 `0x287548` (level00 `0x261d78`, copies on 05, 06, 08, 10, 13, 16, 18) | a point pushed `r` away from the walls of a path it comes within `r` of |
//! | [`push_out_dist`] | level05 `0x304058` | the same, returning the nearest wall line's distance too |

use super::{dist2, V};
use crate::moby_update::services::World;

fn pts<'a>(w: &'a World<'_>, path: usize) -> &'a [[u32; 4]] { w.svc.splines.get(path).map(|v| v.as_slice()).unwrap_or(&[]) }
fn f(p: [u32; 4]) -> V { p.map(f32::from_bits) }

/// `0x26e6c0(p, points, n)`.
pub fn point_in_polygon(w: &World, path: usize, p: V) -> bool {
    let s = pts(w, path);
    let n = s.len();
    let mut inside = false;
    let y = p[1];
    for i in 0..n {
        let a = f(s[i]);
        let b = f(s[(i + 1) % n]);
        if ((a[1] < y && y <= b[1]) || (b[1] < y && y <= a[1])) && a[0] + ((y - a[1]) / (b[1] - a[1])) * (b[0] - a[0]) < p[0] {
            inside = !inside;
        }
    }
    inside
}

/// The crossings of a → b with the walls of `path` in the segment's frame (x along, y across, flattened): the
/// parameters `t` in (0, `limit`) of each wall edge crossing, in path order.
fn crossings(w: &World, path: usize, a: V, b: V, mut on: impl FnMut(f32) -> bool) -> bool {
    let s = pts(w, path);
    let mut d = super::sub(b, a);
    d[2] = 0.0;
    let l = super::len3(d);
    let d = super::scale(d, 1.0 / l / l);
    let x = |p: V| -> (f32, f32, f32) {
        let q = super::sub(p, a);
        (q[0] * d[0] + q[1] * d[1], q[0] * -d[1] + q[1] * d[0], p[3])
    };
    let Some(&first) = s.first() else { return false };
    let (mut x0, mut y0, mut w0) = x(f(first));
    for &pt in s.iter().skip(1) {
        let (x1, y1, w1) = x(f(pt));
        if (w0 != 0.0 || w1 != 0.0) && y1 * y0 < 0.0 {
            let t = ((x0 - x1) / (y0 - y1)) * -y1 + x1;
            if on(t) { return true; }
        }
        (x0, y0, w0) = (x1, y1, w1);
    }
    false
}

/// `0x276640(path, a, b)`: a wall crossing strictly between a and b.
pub fn crosses(w: &World, path: usize, a: V, b: V) -> bool { crossings(w, path, a, b, |t| 0.0 < t && t < 1.0) }

/// `ClampToPath(path, a, b, &out)` 0x276820: `(crossed, lerp(a, b, t_min))` with the nearest crossing (b when none).
pub fn clamp(w: &World, path: usize, a: V, b: V) -> (bool, V) {
    let mut best = 1.0f32;
    let mut hit = false;
    crossings(w, path, a, b, |t| {
        if 0.0 < t && t < best { best = t; hit = true; }
        false
    });
    (hit, super::add(a, super::scale(super::sub(b, a), best)))
}

/// `0x276a48(r, paths, graph, p)`: bit `i` for each graph point reachable from `p` without crossing a wall of any of
/// `paths` (with `r ≠ 0` both edges of a ±r corridor must be clear).
pub fn visible_nodes(w: &World, r: f32, paths: &[usize], graph: usize, p: V) -> u32 {
    let g: Vec<V> = pts(w, graph).iter().map(|&q| f(q)).collect();
    let mut mask = 0u32;
    for (i, node) in g.iter().enumerate() {
        let node = [node[0], node[1], node[2], node[3]];
        let blocked = paths.iter().any(|&path| {
            if r == 0.0 {
                crosses(w, path, p, node)
            } else {
                let v = super::sub(p, node);
                // FastVecCross(v, v, (0, 0, 1)) = (0, 0, 1) × v.
                let c = super::set_len3([-v[1], v[0], 0.0, 0.0], r);
                crosses(w, path, super::sub(p, c), node) || crosses(w, path, super::add(p, c), node)
            }
        });
        if !blocked && i < 32 { mask |= 1 << i; }
    }
    mask
}

/// `0x276c40(graph, a, b)`: `a & b`, else `b` grown by the visibility masks (point w words) of its members, up to 32
/// times; 0 when they never meet.
pub fn meet(w: &World, graph: usize, a: u32, mut b: u32) -> u32 {
    let s = pts(w, graph);
    for _ in 0..32 {
        if a & b != 0 { return a & b; }
        let mut nb = b;
        for i in 0..32usize {
            if b >> i & 1 != 0 { nb |= s.get(i).map(|q| q[3]).unwrap_or(0); }
        }
        b = nb;
    }
    0
}

/// `LineOfSightTest(r, paths, graph, from, to, &out)` 0x276fe8: `Some(to)` when no wall of `paths` lies between (with
/// `r ≠ 0`, neither edge of the ±r corridor from `from`); else the graph point, among those both ends reach, nearest
/// (xy) to `to`; `None` when there is none.
pub fn line_of_sight(w: &World, r: f32, paths: &[usize], graph: usize, from: V, to: V) -> Option<V> {
    let blocked = paths.iter().any(|&path| {
        if r == 0.0 {
            crosses(w, path, from, to)
        } else {
            let d = super::set_len3(super::sub(from, to), r);
            let c = [d[1], -d[0], d[2], d[0]];
            crosses(w, path, super::sub(from, c), to) || crosses(w, path, super::add(from, c), to)
        }
    });
    if !blocked { return Some(to); }
    let a = visible_nodes(w, r, paths, graph, from);
    let b = visible_nodes(w, r, paths, graph, to);
    let m = meet(w, graph, a, b);
    if m == 0 { return None; }
    let g = pts(w, graph);
    let mut best: Option<(f32, V)> = None;
    for (i, q) in g.iter().enumerate().take(32) {
        if m >> i & 1 == 0 { continue; }
        let p = f(*q);
        let d = dist2(p, to);
        if best.is_none_or(|(bd, _)| d < bd) { best = Some((d, p)); }
    }
    best.map(|b| b.1)
}

/// `0x2873b0(walls, n, graph)` (level11): every point of `graph` gets the mask of the points it sees: its w = 0, then for
/// each pair i < j that no wall of `walls` crosses ([`crosses`]), bit j in point i's w and bit i in point j's (bits
/// taken mod 32).
pub fn graph_init(w: &mut World, walls: &[usize], graph: usize) {
    let n = pts(w, graph).len();
    for i in 0..n { w.svc.splines[graph][i][3] = 0; }
    for i in 0..n {
        for j in i + 1..n {
            let (a, b) = (f(pts(w, graph)[i]), f(pts(w, graph)[j]));
            if walls.iter().any(|&p| crosses(w, p, a, b)) { continue; }
            w.svc.splines[graph][i][3] |= 1 << (j & 31);
            w.svc.splines[graph][j][3] |= 1 << (i & 31);
        }
    }
}

/// `0x287548(r, path, &p, &out)` (level11; level00 `0x261d78`): `p` (its height kept) pushed out of the walls of
/// `path` it comes within `r` of, edge after edge (an edge is a wall unless both its points' w are 0), the pushed point
/// carried on to the next edge: within `r` of the edge's line (xy) and over the segment → `r` from the line on the
/// point's side; beyond either end and within `r` of the edge's first point → `r` from that point. `None` when no edge
/// moved it (the game leaves `out`).
pub fn push_out(w: &World, r: f32, path: usize, p: V) -> Option<V> { push_out_dist(w, r, path, p).1 }

/// [`push_out`] with the nearest wall line's distance (xy, the smallest `|cross|` over the wall edges, 512 without
/// one): level05 `0x304058(r, path, &p, &out)` (Rilgar's 623 holds off its wall checks for 5 ticks beyond 2).
pub fn push_out_dist(w: &World, r: f32, path: usize, p: V) -> (f32, Option<V>) {
    let s = pts(w, path);
    let mut cur = p;
    let mut found = false;
    let mut near = 512.0f32;
    for i in 0..s.len().saturating_sub(1) {
        let (a, b) = (f(s[i]), f(s[i + 1]));
        if a[3] == 0.0 && b[3] == 0.0 { continue; }
        let d = [cur[0] - a[0], cur[1] - a[1], 0.0, 0.0];
        let seg = [b[0] - a[0], b[1] - a[1], 0.0, 0.0];
        let u = super::set_len3(seg, 1.0);
        let cross_z = (d[0] * u[1] - d[1] * u[0]).abs();
        if cross_z < near { near = cross_z; }
        if r < cross_z { continue; }
        let len = super::len3(seg);
        let proj = super::dot3(d, u);
        let q = if len < proj || proj < 0.0 {
            if r <= super::len3(d) { continue; }
            super::add(super::set_len3(d, r), a)
        } else {
            let e = super::scale(u, proj);
            super::add(super::add(super::set_len3(super::sub(d, e), r), e), a)
        };
        cur = [q[0], q[1], p[2], p[3]];
        found = true;
    }
    (near, found.then_some(cur))
}
