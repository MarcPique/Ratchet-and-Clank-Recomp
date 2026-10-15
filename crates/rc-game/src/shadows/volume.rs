//! The shadow volume of a caster (docs/plan/shadows.md §3.3): the class's proxy shapes posed on its skeleton
//! (*BuildShadowList* 0x29b948), each outlined perpendicular to the shadow direction (*ShadowSetDir* 0x29b1c0,
//! *ShadowSphereOutline* 0x29bf88, *ShadowCapsuleOutline* 0x29c0d8) and swept along it between the planes
//! z = hi and z = lo (*ShadowClipToSlab* 0x29c728), as closed triangle prisms. Plus the per-frame list rules:
//! the size by distance (`MobyProc` 0x26b820) and the 8,064-byte budget of the posed list (0x1acc00..0x1aeb80).
//!
//! Native `f32` with `std` trig (the game's VU0 sine / cosine micro calls, its `asin` polynomial kept as the
//! game's formula in `f32`). The GS-side work of the game (screen projection, the guard-band face drop, the
//! destination-alpha counter) is not here: the engine draws these triangles into a count target
//! (crate `rc-engine` `shadow_render`).
//!
//! **Posing** (0x29b9b0..0x29bbd4). `k = moby+0x2c · (1/1024) · size/4096`; a record point p (joint space, packed
//! model units, w = radius) becomes `P = pos + R·((J·p)·k)` with J the posed joint matrix (`P_j`, the joint's frame
//! in model space: `x·J0 + y·J1 + z·J2 + J3`), R the rotation rows (+0xc0..+0xe0) and the radius `p.w·k` (the
//! joint and row transforms only write xyz). A capsule end with −n segments is then pushed **away** from the other
//! end by `n·16/4096` of the segment (A first, then B from the new A) and becomes flat. Checked against the
//! game's posed list in RAM (`tests/world/shadow_volume_novalis.rs`).
//!
//! **Outlines**, in the plane perpendicular to the unit direction `d`, every ring turning right-handed about `d`:
//! * sphere: `n` points `c + R(d, k·6.28/n)·(e·r)`, k = 1..=n (e = [`DirFrame::perp`]); note 6.28, not 2π;
//! * capsule (`s = d × (B − A)`'s length L, n̂ = (B − A) × d / L): when `L + rB − rA < 0` (A's disc contains B's)
//!   a full circle about A; else the two ends' arcs joined by tangents at `α = 1.57 − asin((rA − rB)/L)`: end A
//!   from angle `1.57 + α` in `nA` steps of `2(3.14 − α)/nA`, end B from `1.57 + α + 2(3.14 − α) − 6.28` in `nB`
//!   steps of `2α/nB` (a flat end: the two points `∓n̂·r`); rA = rB takes α = 1.57 exactly.
//!   (B's disc containing A's is not tested by the game: `asin` then gets |x| > 1 and the outline twists; kept.)
//!
//! **Prism**: each ring point P is moved along d to z = hi (top ring) and to z = lo (bottom ring); side quads between
//! neighbours, fans for the two caps, all wound so the outside is front-facing for a ring turning right-handed
//! about d (d points down). A twisted outline gives inverted faces, as in the game's classification.

use rc_formats::moby_anim::Rows;
use rc_formats::moby_shadow::{ShadowBlock, ShadowPrim};

/// Bytes of the posed list (0x1acc00..0x1aeb80): the per-frame shadow budget.
pub const LIST_BYTES: usize = 0x1f80;
/// A caster's header in the posed list (`{count, guard, lo, hi, dir}`).
pub const HEADER_BYTES: usize = 0x20;
/// Full size (`size` word 0x1000 = 1.0).
pub const FULL_SIZE: u16 = 0x1000;
/// The game's circle constants (0x40c8f5c3, 0x4048f5c3, 0x3fc8f5c3): 6.28, 3.14, 1.57.
pub const TWO_PI: f32 = f32::from_bits(0x40c8_f5c3);
pub const PI: f32 = f32::from_bits(0x4048_f5c3);
pub const HALF_PI: f32 = f32::from_bits(0x3fc8_f5c3);

/// One posed record of the list at 0x1acc00: world-space points, w = world radius.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PosedPrim {
    pub capsule: bool,
    /// Outline segments (end A, end B; a sphere's in [0]), negative counts already turned into 0.
    pub segments: [i32; 2],
    pub a: [f32; 4],
    /// End B (spheres: unused, zero).
    pub b: [f32; 4],
}

/// A caster's pose inputs.
#[derive(Clone, Copy, Debug)]
pub struct CasterPose<'a> {
    /// Posed joint matrices `P_j` (rows 0..2 rotation, row 3 translation; packed model units).
    pub joints: &'a [Rows],
    /// moby+0xc0 / +0xd0 / +0xe0 (xyz).
    pub rows: [[f32; 3]; 3],
    /// moby+0x10.
    pub position: [f32; 3],
    /// moby+0x2c.
    pub scale: f32,
    /// The size word (0..=0x1000) from [`shadow_size`].
    pub size: u16,
}

/// Bytes a caster takes in the posed list.
pub fn caster_bytes(block: &ShadowBlock) -> usize { HEADER_BYTES + block.prims.iter().map(ShadowPrim::size).sum::<usize>() }

/// How many casters of a list (their byte counts in list order) the game poses: it stops before the first caster
/// that starts past the end of the 8,064-byte area (so the last one may run over it).
pub fn fitting(bytes: impl IntoIterator<Item = usize>) -> usize {
    let mut used = 0usize;
    let mut n = 0;
    for b in bytes {
        if used > LIST_BYTES { break; }
        used += b;
        n += 1;
    }
    n
}

/// The size word of `MobyProc` (0x26b820): `min((+0x7f << 10) − depth) >> 1, 0x1000)` with `depth` =
/// `ftoi0(v.z − r')` of the shadow sphere (integer units, ×1024), the shift **logical** (`srl`). Full size up to
/// 16 units, then linear down towards 0 at 24 (+0x7f = 0x18). The cull before it (depth beyond +0x7f units: no
/// shadow) keeps the difference non-negative, so the logical shift never turns a far caster full-size.
pub fn shadow_size(b7f: u8, depth: i32) -> u16 {
    let v = ((b7f as u32) << 10).wrapping_sub(depth as u32) >> 1;
    (v as i32).min(FULL_SIZE as i32).max(0) as u16
}

fn mul_point(j: &Rows, p: [f32; 3]) -> [f32; 3] { std::array::from_fn(|k| ((j[0][k] * p[0] + j[1][k] * p[1]) + j[2][k] * p[2]) + j[3][k]) }

fn place(pose: &CasterPose, p: [f32; 4], joint: usize, k: f32) -> [f32; 4] {
    let j = pose.joints.get(joint).copied().unwrap_or(rc_formats::moby_anim::IDENTITY);
    let v = mul_point(&j, [p[0], p[1], p[2]]).map(|x| x * k);
    let r = pose.rows;
    let w: [f32; 3] = std::array::from_fn(|c| ((r[0][c] * v[0] + r[1][c] * v[1]) + r[2][c] * v[2]) + pose.position[c]);
    [w[0], w[1], w[2], p[3] * k]
}

/// *BuildShadowList*'s per-caster loop: the block's records posed in world space (module docs).
pub fn pose_prims(block: &ShadowBlock, pose: &CasterPose) -> Vec<PosedPrim> {
    let k = pose.scale * (1.0 / 1024.0) * (pose.size as f32 / 4096.0);
    block
        .prims
        .iter()
        .map(|p| match *p {
            ShadowPrim::Sphere { joint, segments, centre } => {
                PosedPrim { capsule: false, segments: [segments, 0], a: place(pose, centre, joint.max(0) as usize, k), b: [0.0; 4] }
            }
            ShadowPrim::Capsule { joints, segments, a, b } => {
                let mut a = place(pose, a, joints[0] as usize, k);
                let mut b = place(pose, b, joints[1] as usize, k);
                let mut seg = segments;
                if seg[0] < 0 {
                    let f = (-seg[0] * 16) as f32 / 4096.0;
                    for c in 0..3 { a[c] += (a[c] - b[c]) * f; }
                    seg[0] = 0;
                }
                if seg[1] < 0 {
                    let f = (-seg[1] * 16) as f32 / 4096.0;
                    for c in 0..3 { b[c] += (b[c] - a[c]) * f; }
                    seg[1] = 0;
                }
                PosedPrim { capsule: true, segments: seg, a, b }
            }
        })
        .collect()
}

/// *ShadowSetDir*: the unit direction and a unit vector perpendicular to it (the outlines' start), picked from a
/// permutation of the raw direction's components (compared as signed values) crossed with the unit direction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DirFrame {
    pub dir: [f32; 3],
    pub perp: [f32; 3],
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]] }
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }
fn len(a: [f32; 3]) -> f32 { dot(a, a).sqrt() }
fn scale(a: [f32; 3], s: f32) -> [f32; 3] { a.map(|x| x * s) }
fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0] + b[0], a[1] + b[1], a[2] + b[2]] }
fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }

/// [`DirFrame`] of a raw direction; None for a zero or non-finite one.
pub fn dir_frame(raw: [f32; 3]) -> Option<DirFrame> {
    let l = len(raw);
    if !(l > 0.0 && l.is_finite()) { return None; }
    let dir = scale(raw, 1.0 / l);
    let [x, y, z] = raw;
    let t = if x < y {
        if x < z { [x, z, y] } else { [y, x, z] }
    } else if y < z {
        [z, y, x]
    } else {
        [y, x, z]
    };
    // FastVecCross(out, t, dir) = dir × t, normalised.
    let mut p = cross(dir, t);
    // The permutation can be parallel to the direction (e.g. straight down, which no game direction is: direction 0
    // leans 8°, direction 1 stops at 1.5 rad); the game's outline would collapse there. Any perpendicular then.
    if len(p) <= 1e-6 { p = cross(dir, if dir[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] }); }
    let pl = len(p);
    let perp = if pl > 0.0 { scale(p, 1.0 / pl) } else { [0.0; 3] };
    Some(DirFrame { dir, perp })
}

/// `v` turned by θ right-handed about the unit axis `k` (the outlines' VU0 rotation matrix).
pub fn rotate(k: [f32; 3], theta: f32, v: [f32; 3]) -> [f32; 3] {
    let (s, c) = theta.sin_cos();
    let kv = dot(k, v);
    let kx = cross(k, v);
    std::array::from_fn(|i| v[i] * c + kx[i] * s + k[i] * kv * (1.0 - c))
}

/// The game's `asin` 0x221728 in `f32`: `sign·(π/2 − p(|x|)·√|1 − |x||)` (the VU square root takes the absolute
/// value, so |x| > 1 gives a finite, wrong angle instead of NaN).
pub fn asin_game(x: f32) -> f32 {
    const C: [f32; 4] = [1.570_728_8, -0.212_114_4, 0.074_261_01, -0.018_729_3];
    let (a, sgn) = (x.abs(), if x < 0.0 { -1.0 } else { 1.0 });
    let s = (1.0 - a).abs().sqrt();
    let p = ((C[0] + a * C[1]) + (a * a) * C[2]) + (a * a * a) * C[3];
    sgn * (std::f32::consts::FRAC_PI_2 - p * s)
}

/// `n` points `c + R(d, i·step)·v0` for i = 1..=n (the loops rotate before storing).
fn arc(out: &mut Vec<[f32; 3]>, c: [f32; 3], d: [f32; 3], v0: [f32; 3], step: f32, n: i32) {
    let mut v = v0;
    for _ in 0..n.max(0) {
        v = rotate(d, step, v);
        out.push(add(c, v));
    }
}

/// `n` points `c + R(d, θ0 + i·step)·v0` for i = 0..n (the capsule arcs store before rotating).
fn arc_from(out: &mut Vec<[f32; 3]>, c: [f32; 3], d: [f32; 3], v0: [f32; 3], theta0: f32, step: f32, n: i32) {
    let mut v = rotate(d, theta0, v0);
    for _ in 0..n.max(0) {
        out.push(add(c, v));
        v = rotate(d, step, v);
    }
}

/// The outline of one posed record (module docs), as the ring the game builds: the points, then a copy of the
/// first (the closing point). Empty for a degenerate record.
pub fn outline(p: &PosedPrim, f: &DirFrame, out: &mut Vec<[f32; 3]>) {
    let start = out.len();
    let d = f.dir;
    let xyz = |v: [f32; 4]| [v[0], v[1], v[2]];
    if !p.capsule {
        let n = p.segments[0];
        if n <= 0 { return; }
        arc(out, xyz(p.a), d, scale(f.perp, p.a[3]), TWO_PI / n as f32, n);
    } else {
        let (a, b, ra, rb) = (xyz(p.a), xyz(p.b), p.a[3], p.b[3]);
        let side = cross(sub(b, a), d);
        let l = len(side);
        if (l + rb) - ra < 0.0 {
            // A's disc contains B's: a full circle about A.
            let n = p.segments[0];
            if n <= 0 { return; }
            arc(out, a, d, scale(f.perp, ra), TWO_PI / n as f32, n);
        } else {
            if !(l > 0.0 && l.is_finite()) { return; }
            let alpha = if ra - rb == 0.0 { HALF_PI } else { HALF_PI - asin_game((ra - rb) / l) };
            let (span_a, span_b) = ((PI - alpha) * 2.0, alpha * 2.0);
            let n_hat = scale(side, 1.0 / l);
            let (va, vb) = (scale(n_hat, ra), scale(n_hat, rb));
            match p.segments[0] {
                0 => {
                    out.push(sub(a, va));
                    out.push(add(a, va));
                }
                n => arc_from(out, a, d, va, HALF_PI + alpha, span_a / n as f32, n),
            }
            match p.segments[1] {
                0 => {
                    out.push(add(b, vb));
                    out.push(sub(b, vb));
                }
                n => arc_from(out, b, d, vb, (HALF_PI + (alpha + span_a)) - TWO_PI, span_b / n as f32, n),
            }
        }
    }
    if out.len() > start {
        let first = out[start];
        out.push(first);
    }
}

/// The prism of one ring (with its closing point) between z = hi and z = lo along the unit direction `d`
/// (d.z < 0), appended to `tris` as triangles (3 points each), outside front-facing (module docs). Nothing for
/// a ring of fewer than 3 distinct points or a direction without a downward component.
pub fn prism(ring: &[[f32; 3]], d: [f32; 3], lo: f32, hi: f32, tris: &mut Vec<[f32; 3]>) {
    if ring.len() < 4 || d[2] >= -1e-6 { return; }
    let at = |p: [f32; 3], z: f32| add(p, scale(d, (z - p[2]) / d[2]));
    let top: Vec<[f32; 3]> = ring.iter().map(|&p| at(p, hi)).collect();
    let bot: Vec<[f32; 3]> = ring.iter().map(|&p| at(p, lo)).collect();
    for i in 0..ring.len() - 1 {
        tris.extend_from_slice(&[top[i], top[i + 1], bot[i + 1], top[i], bot[i + 1], bot[i]]);
    }
    let n = ring.len() - 1;
    for i in 1..n - 1 {
        tris.extend_from_slice(&[top[0], top[i + 1], top[i]]);
        tris.extend_from_slice(&[bot[0], bot[i], bot[i + 1]]);
    }
}

/// A caster's whole volume: every posed record outlined along `dir` (raw, from the direction table) and swept
/// between `lo` and `hi`. Returns the number of triangles appended.
pub fn caster_volume(prims: &[PosedPrim], dir: [f32; 3], lo: f32, hi: f32, tris: &mut Vec<[f32; 3]>) -> usize {
    let Some(f) = dir_frame(dir) else { return 0 };
    let before = tris.len();
    let mut ring = Vec::with_capacity(40);
    for p in prims {
        ring.clear();
        outline(p, &f, &mut ring);
        prism(&ring, f.dir, lo, hi, tris);
    }
    (tris.len() - before) / 3
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close3(a: [f32; 3], b: [f32; 3], e: f32) -> bool { (0..3).all(|k| (a[k] - b[k]).abs() < e) }

    const DOWN: [f32; 3] = [0.0, 0.0, -1.0];

    #[test]
    fn size_by_distance() {
        // Full size up to 16 units (depth 16384), 0x800 at 20, towards 0 at 24; +0x7f = 0x16 (amoeboids) is shorter.
        assert_eq!(shadow_size(0x18, 0), FULL_SIZE);
        assert_eq!(shadow_size(0x18, 16 << 10), FULL_SIZE);
        assert_eq!(shadow_size(0x18, 20 << 10), 0x800);
        assert_eq!(shadow_size(0x18, (24 << 10) - 2), 1);
        assert_eq!(shadow_size(0x16, 20 << 10), 0x400);
        // A negative difference (a caster the cull would have dropped): the logical shift gives a huge value → full.
        assert_eq!(shadow_size(0x18, 30 << 10), FULL_SIZE, "srl: the game's arithmetic, never reached after the cull");
    }

    #[test]
    fn budget_stops_after_the_area_is_passed() {
        // Ratchet: 0x20 + 21 capsules = 0x410 bytes; 8 of them start inside the 0x1f80 area (the 9th starts past it).
        let ratchet = HEADER_BYTES + 21 * 0x30;
        assert_eq!(fitting(std::iter::repeat_n(ratchet, 20)), 8);
        assert_eq!(fitting([LIST_BYTES, 0x40, 0x40]), 2, "a caster starting exactly at the end is still posed");
        assert_eq!(fitting(std::iter::empty()), 0);
    }

    fn pose_one(p: ShadowPrim, joints: &[Rows], size: u16) -> PosedPrim {
        let rows = [[0.0, 1.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]]; // 90° about z
        let pose = CasterPose { joints, rows, position: [100.0, 200.0, 50.0], scale: 0.25, size };
        pose_prims(&ShadowBlock { prims: vec![p] }, &pose)[0]
    }

    #[test]
    fn posing_scales_points_and_radius() {
        let mut j = rc_formats::moby_anim::IDENTITY;
        j[3] = [1024.0, 0.0, 0.0, 1.0]; // joint 1 sits 1024 packed units along model x
        let joints = [rc_formats::moby_anim::IDENTITY, j];
        let s = pose_one(ShadowPrim::Sphere { joint: 1, segments: 6, centre: [0.0, 0.0, 2048.0, 512.0] }, &joints, FULL_SIZE);
        // (1024, 0, 2048)·0.25/1024 = (0.25, 0, 0.5); rotated: model x → world y.
        assert!(close3([s.a[0], s.a[1], s.a[2]], [100.0, 200.25, 50.5], 1e-5), "{s:?}");
        assert!((s.a[3] - 0.125).abs() < 1e-7, "radius ×k: {}", s.a[3]);
        let half = pose_one(ShadowPrim::Sphere { joint: 0, segments: 6, centre: [1024.0, 0.0, 0.0, 512.0] }, &joints, 0x800);
        assert!(close3([half.a[0], half.a[1], half.a[2]], [100.0, 200.125, 50.0], 1e-5) && (half.a[3] - 0.0625).abs() < 1e-7);
        // −10 at end A: A moves 160/4096 of the segment away from B, then flat.
        let c = pose_one(ShadowPrim::Capsule { joints: [0, 0], segments: [-10, 4], a: [0.0; 4], b: [4096.0, 0.0, 0.0, 0.0] }, &joints, FULL_SIZE);
        assert_eq!(c.segments, [0, 4]);
        assert!(close3([c.a[0], c.a[1], c.a[2]], [100.0, 200.0 - 0.0390625, 50.0], 1e-5), "{c:?}");
        // −10 at end B only: B moves away from A.
        let c = pose_one(ShadowPrim::Capsule { joints: [0, 0], segments: [4, -10], a: [0.0; 4], b: [4096.0, 0.0, 0.0, 0.0] }, &joints, FULL_SIZE);
        assert!(close3([c.b[0], c.b[1], c.b[2]], [100.0, 201.039_06, 50.0], 1e-5), "{c:?}");
    }

    #[test]
    fn dir_frame_is_orthonormal() {
        for raw in [[-0.12949, 0.05323, -0.99], [-0.52284, 0.21494, -0.82489], [0.0, 0.0, -1.0], [0.3, -0.2, -0.5]] {
            let f = dir_frame(raw).unwrap();
            assert!((len(f.dir) - 1.0).abs() < 1e-6 && (len(f.perp) - 1.0).abs() < 1e-6 && dot(f.dir, f.perp).abs() < 1e-6, "{raw:?} {f:?}");
        }
        assert!(dir_frame([0.0; 3]).is_none());
    }

    fn ring_of(p: PosedPrim, d: [f32; 3]) -> (Vec<[f32; 3]>, DirFrame) {
        let f = dir_frame(d).unwrap();
        let mut r = Vec::new();
        outline(&p, &f, &mut r);
        (r, f)
    }

    fn sphere(c: [f32; 3], r: f32, n: i32) -> PosedPrim { PosedPrim { capsule: false, segments: [n, 0], a: [c[0], c[1], c[2], r], b: [0.0; 4] } }

    #[test]
    fn sphere_outline_is_an_ngon_turning_about_the_direction() {
        let (ring, f) = ring_of(sphere([1.0, 2.0, 3.0], 0.5, 6), DOWN);
        assert_eq!(ring.len(), 7, "6 points and the closing copy");
        assert_eq!(ring[0], ring[6]);
        for p in &ring[..6] {
            let v = sub(*p, [1.0, 2.0, 3.0]);
            assert!((len(v) - 0.5).abs() < 1e-6 && dot(v, f.dir).abs() < 1e-6);
        }
        // Consecutive points turn right-handed about d by 6.28/6.
        let (v0, v1) = (sub(ring[0], [1.0, 2.0, 3.0]), sub(ring[1], [1.0, 2.0, 3.0]));
        assert!(dot(cross(v0, v1), f.dir) > 0.0);
        assert!(((dot(v0, v1) / 0.25).acos() - TWO_PI / 6.0).abs() < 1e-4);
    }

    fn capsule(a: [f32; 4], b: [f32; 4], seg: [i32; 2]) -> PosedPrim { PosedPrim { capsule: true, segments: seg, a, b } }

    #[test]
    fn capsule_outline_arcs_and_tangents() {
        // Horizontal capsule along x, radii 0.5 / 0.25, straight-down direction: tangent angle asin(0.25/2).
        let (ring, f) = ring_of(capsule([0.0, 0.0, 0.0, 0.5], [2.0, 0.0, 0.0, 0.25], [8, 4]), DOWN);
        assert_eq!(ring.len(), 13);
        let alpha = HALF_PI - asin_game(0.25 / 2.0);
        for p in &ring[..8] { assert!((len(*p) - 0.5).abs() < 1e-5); }
        for p in &ring[8..12] { assert!((len(sub(*p, [2.0, 0.0, 0.0])) - 0.25).abs() < 1e-5); }
        // A's arc bulges away from B (its middle point has x < 0), B's away from A.
        assert!(ring[4][0] < -0.45 && ring[10][0] > 2.2);
        // The ring turns right-handed about d throughout (a convex outline: every turn positive).
        for i in 0..ring.len() - 2 {
            let (e0, e1) = (sub(ring[i + 1], ring[i]), sub(ring[i + 2], ring[i + 1]));
            assert!(dot(cross(e0, e1), f.dir) >= -1e-6, "turn {i}");
        }
        // The first A point sits at angle 1.57 + α from n̂ = (B − A) × d / L = (0, 1, 0).
        let first = rotate(f.dir, HALF_PI + alpha, [0.0, 0.5, 0.0]);
        assert!(close3(ring[0], first, 1e-5), "{:?} {first:?}", ring[0]);
        // Flat ends: A−, A+, B+, B− (n̂ = (0, 1, 0)).
        let (flat, _) = ring_of(capsule([0.0, 0.0, 0.0, 0.5], [2.0, 0.0, 0.0, 0.5], [0, 0]), DOWN);
        assert_eq!(flat, vec![[0.0, -0.5, 0.0], [0.0, 0.5, 0.0], [2.0, 0.5, 0.0], [2.0, -0.5, 0.0], [0.0, -0.5, 0.0]]);
        // A's disc contains B's (a capsule along the direction): a full circle about A with A's segments.
        let (full, _) = ring_of(capsule([0.0, 0.0, 1.0, 0.5], [0.0, 0.0, 0.0, 0.25], [5, 4]), DOWN);
        assert_eq!(full.len(), 6);
        for p in &full[..5] { assert!((len(sub(*p, [0.0, 0.0, 1.0])) - 0.5).abs() < 1e-5); }
    }

    fn signed_volume(tris: &[[f32; 3]]) -> f32 { tris.chunks(3).map(|t| dot(t[0], cross(t[1], t[2])) / 6.0).sum() }

    #[test]
    fn prisms_are_closed_outward_and_clipped_to_the_slab() {
        let d = [0.3, -0.1, -0.9];
        let (ring, f) = ring_of(sphere([5.0, 5.0, 11.0], 0.5, 6), d);
        let mut tris = Vec::new();
        prism(&ring, f.dir, 9.8, 10.2, &mut tris);
        // 6 side quads (12 triangles) + 2 caps of 4 triangles.
        assert_eq!(tris.len(), 3 * (12 + 8));
        for p in &tris { assert!((p[2] - 9.8).abs() < 1e-5 || (p[2] - 10.2).abs() < 1e-5, "{p:?}"); }
        // Closed: every directed edge has its reverse; outward: the signed volume is positive and equals the
        // hexagon's area times the slab height (a prism sheared along d keeps its volume).
        let mut edges = std::collections::HashMap::new();
        let key = |p: [f32; 3]| p.map(|x| (x * 1e4).round() as i64);
        for t in tris.chunks(3) {
            for k in 0..3 { *edges.entry((key(t[k]), key(t[(k + 1) % 3]))).or_insert(0) += 1; }
        }
        for (&(a, b), &n) in &edges { assert_eq!(edges.get(&(b, a)), Some(&n), "edge {a:?}→{b:?}"); }
        let hex = 6.0 * 0.5 * 0.25 * (TWO_PI / 6.0).sin();
        // The cross-section perpendicular to d has the hexagon's area; the horizontal slab section is area/|d.z|.
        let want = hex / f.dir[2].abs() * 0.4;
        assert!((signed_volume(&tris) - want).abs() < 5e-4, "{} vs {want}", signed_volume(&tris));
        // Whole caster: every prim appended.
        let prims = [sphere([0.0, 0.0, 1.0], 0.3, 4), capsule([0.0, 0.0, 1.0, 0.2], [1.0, 0.0, 1.0, 0.2], [4, 4])];
        let mut all = Vec::new();
        let n = caster_volume(&prims, DOWN, -0.2, 0.2, &mut all);
        assert_eq!(n, (4 * 2 + 2 * 2) + (8 * 2 + 6 * 2));
        assert!(signed_volume(&all) > 0.0);
    }

    #[test]
    fn game_asin() {
        for x in [-1.0f32, -0.5, 0.0, 0.3, 0.9, 1.0] { assert!((asin_game(x) - x.asin()).abs() < 1e-4, "{x}"); }
        assert!(asin_game(1.5).is_finite(), "|x| > 1 stays finite (the VU square root of |1 − x|)");
    }
}
