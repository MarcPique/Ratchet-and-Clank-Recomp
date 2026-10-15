//! Collision queries against the level collision mesh: a port of the game's hand-written
//! EE + VU0-macro kernels (level01 overlay addresses; boot copies noted where they exist).
//! Derivation, constants and operation order: `docs/plan/collision_queries.md` ("Port" section).
//!
//! | Kernel | level01 | boot | Port |
//! |---|---|---|---|
//! | `CollLine_Fix` segment query | `0x211870` | `0x1efa68` (identical but for globals) | [`coll_line`] |
//! | sphere query | `0x212960` | - | [`coll_sphere`] |
//! | vertical capsule (hero body) | `0x2135a0` | - | [`coll_capsule`] |
//! | sphere vs mobys only | `0x214468` | - | [`coll_sphere_mobys`] |
//! | sphere vs hero groups | `0x214d70` | - | [`coll_sphere_hero_groups`] |
//! | moby grid insert / remove | `UpdateMobyGrids` 0x265900 | boot 0x20dc20 | [`MobyGrid::update`] |
//! | cell lookup | `0x2117d0` | `0x1ef9c8` | [`Collision::cell_at`] |
//!
//! **Arithmetic.** Every kernel works in "x1024 space" (world * 1024) relative to the centre of
//! the cell being tested, on VU0 floats. The port runs the same operations in the same order on
//! PS2 float bit patterns ([`crate::ps2v`]: round toward zero, no denormals, the adder's guard-bit
//! behaviour), so results are bit-exact to the extent that model is (it does not model the
//! multiplier's rare last-bit deviations). Integer steps (cell mapping, DDA list merge, 16-bit
//! vertex outcodes) are reproduced exactly, including their wrap-around.
//!
//! **Mobys** ([`mobys`]): after the world mesh, the line, sphere and capsule kernels walk the 16-unit moby
//! grid ([`MobyGrid`]) and test each registered moby's class collision blob (triangle mesh and
//! primitives, `rc_formats::moby_collision`) in the moby's frame, sharing the running best with the
//! world pass; [`coll_sphere_mobys`] lists every moby a sphere touches (the wrench, explosions). The
//! `*_m` entry points take a [`MobyScene`] (moby records, grid, class blobs, pose cache) and the moby to
//! ignore; the plain ones are world-only. The per-query stamp (`CollOutput+0x10` / `moby+0x9c`) is a
//! per-query visited list here.

use crate::ps2v::*;
use rc_formats::collision::{Collision, CollisionCell};

pub mod mobys;
pub use mobys::{coll_sphere_mobys, pose_joints, CollMoby, MobyGrid, MobyScene, MobySource, OwnedScene, PoseCache, TableMobys};

/// `0x3f7fdf3b` = 0.99951: the sphere/capsule "best distance" shrink factor. The initial best is
/// `K * r^2`, and each accepted face sets `best = K * d^2`, so a later face must be closer.
pub const BEST_SHRINK: u32 = 0x3f7f_df3b;
/// 2048.0 / 4096.0: half / full cell edge in x1024 units (`0x45000000`, `0x45800000`).
const K2048: F = 0x4500_0000;
const K4096: F = 0x4580_0000;
/// End marker of the per-axis DDA crossing lists (`0x50000000`, a float > 1). Never selected by
/// the merge (it only runs for the counted crossings) and kept for documentation.
pub const DDA_END: u32 = 0x5000_0000;

/// Query flags (the kernels' `a2`/`a1` argument, kept in `s3`; identical in line, sphere and capsule).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct QueryFlags(pub u32);

impl QueryFlags {
    pub const NONE: Self = Self(0);
    /// 0x1: skip the world mesh (only mobys are tested). In `CollLine_Fix`, 0x1 or 0x8 also restricts the
    /// moby pass to mobys with `moby+0x34 & 0x4000`. Without a [`MobyScene`] such a query never hits.
    pub const SKIP_WORLD: Self = Self(0x1);
    /// 0x2: skip moby collision *primitives*; moby triangle meshes are still tested (and a moby with no
    /// mesh whose class needs a pose is skipped before the pose is computed). `coll_sphere_mobys` ignores it.
    pub const SKIP_MOBY_PRIMITIVES: Self = Self(0x2);
    /// 0x4: selects the moby blob's joint count at +0 and primitive mask bit 0 (clear: +2 and bit 1).
    pub const MOBY_SUBMASK: Self = Self(0x4);
    /// 0x8: line only: the moby pass tests only mobys with `moby+0x34 & 0x4000` (as 0x1 does).
    pub const MOBY_FLAGGED_ONLY: Self = Self(0x8);
    /// 0x10: two-sided faces (the front-side test is dropped).
    pub const TWO_SIDED: Self = Self(0x10);
    /// 0x20: exclude faces whose surface id `type & 0x1f` equals `(flags >> 8) & 0x1f`; see
    /// [`QueryFlags::exclude_surface`].
    pub const EXCLUDE_SURFACE: Self = Self(0x20);
    /// 0x80: exclude faces with type bit 7.
    pub const EXCLUDE_HIGH_BIT: Self = Self(0x80);

    /// `0x20 | id << 8`: exclude faces with surface id `id` (0..=0x1f). The hero capsule uses
    /// `exclude_surface(0) | MOBY_SUBMASK` = 0x24 and 0xd24.
    pub const fn exclude_surface(id: u8) -> Self { Self(0x20 | ((id as u32 & 0x1f) << 8)) }
    pub const fn bits(self) -> u32 { self.0 }
    pub const fn contains(self, other: Self) -> bool { self.0 & other.0 == other.0 }
    pub const fn union(self, other: Self) -> Self { Self(self.0 | other.0) }
}

impl std::ops::BitOr for QueryFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self { Self(self.0 | rhs.0) }
}
impl std::ops::BitOrAssign for QueryFlags {
    fn bitor_assign(&mut self, rhs: Self) { self.0 |= rhs.0; }
}

/// A query hit: the fields every kernel writes into `CollOutput` (level01 `0x1742c0`, boot
/// `0x194100`) on success. Offsets are those of the game struct; vec4 lanes w are not meaningful.
///
/// | Off | Game field | Here |
/// |---|---|---|
/// | 0x00..0x17 | mesh ptr, moby transform cache, query stamp, hit-log index | not modelled |
/// | 0x18 | hit moby, 0 for world-mesh / hero-group hits | [`moby`](Self::moby) |
/// | 0x1c | `0x1000 + face type` (triangle) or `-(primitive ptr)` (moby primitive) | [`kind`](Self::kind) |
/// | 0x20 | hit point (line) / closest point (sphere, capsule) | [`point`](Self::point) |
/// | 0x30 | sphere/capsule only: centre (capsule: base) pushed out to touch | [`pushed_centre`](Self::pushed_centre) |
/// | 0x40 | raw face normal `(v2-v0) x (v1-v0)` in x1024 units (not normalised, not rescaled); for a moby primitive the vector from the primitive's axis point to the hit | [`normal`](Self::normal) |
/// | 0x50/0x60/0x70 | triangle v0, v1, v2 (world units); a primitive hit leaves the previous values (here: the previous hit's triangle in the same query, else zero) | [`tri`](Self::tri) |
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CollOutput {
    /// +0x18: index of the hit moby (`None` for world-mesh and hero-group hits, and always without a
    /// [`MobyScene`]).
    pub moby: Option<usize>,
    /// The hit moby primitive (index into the class blob's primitives), for a primitive hit.
    pub primitive: Option<usize>,
    /// +0x1c: `0x1000 | type byte` for a triangle hit (world or moby mesh); for a moby primitive the game
    /// stores `-(record address)`, here `-(blob offset of the record)` = `-(0x10 + 0x20·i)`.
    pub kind: i32,
    /// +0x20: hit point (line) or closest point on the face (sphere/capsule), world units.
    pub point: [f32; 3],
    /// +0x30: sphere/capsule only (`None` for the line, which leaves +0x30 untouched).
    pub pushed_centre: Option<[f32; 3]>,
    /// +0x40: unnormalised `(v2 - v0) x (v1 - v0)` as the kernel computed it (units: 1024^2 world^2).
    pub normal: [f32; 3],
    /// +0x50, +0x60, +0x70: the hit triangle (after the quad split), world units.
    pub tri: [[f32; 3]; 3],
    /// Not stored by the game: the kernel's running best (`vf24.x`) at return. Line: the hit
    /// parameter `t` in [0, 1] along `p0 -> p1`. Sphere/capsule: `0.99951 * d^2` in x1024^2 units
    /// (`d` = distance from the centre / axis point to the closest point).
    pub best: f32,
}

impl CollOutput {
    /// The game's surface-id getter (level01 `0x2151d8`): `kind & 0x1f`, -1 for 0x1f or a negative kind.
    pub fn surface_id(&self) -> i32 {
        if self.kind >= 0 && self.kind & 0x1f != 0x1f { self.kind & 0x1f } else { -1 }
    }
    /// The game's footstep sound class getter (level01 `0x215208`): `(kind & 0x60) >> 5`, 3 or a
    /// negative kind -> 0.
    pub fn sound_class(&self) -> i32 {
        if self.kind >= 0 && self.kind & 0x60 != 0x60 { (self.kind & 0x60) >> 5 } else { 0 }
    }
    /// The face's type byte (`kind & 0xff`) for a triangle hit.
    pub fn face_type(&self) -> Option<u8> { (self.kind >= 0x1000).then_some(self.kind as u8) }
}

// ---------------------------------------------------------------------------------------------
// Shared per-cell machinery (identical code in all three kernels: vertex decode 0x211c98 /
// 0x212c48 / 0x2138a8, face walk, quad split 0x211f48 / 0x212f68 / 0x213da8).

/// A decoded cell vertex, as the kernels leave it in scratchpad (0x70002000 + i*16).
#[derive(Clone, Copy, Debug)]
struct Vert {
    /// Offset from the cell centre in x1024 units: `(x*64, y*64, z*16)` from the packed fields.
    p: V3,
    /// Per-axis outcode bytes (x | y<<8 | z<<16): bit 0 = below the query box, bit 1 = above.
    code: u32,
}

/// Decodes the cell's vertices and their outcodes against the query box `[lo, hi]` (x1024 units
/// relative to the cell centre, already truncated to 16 bits like the game's `ppach`). The
/// compares are 16-bit wrapping subtractions (`psubh`) of the halfword coordinates.
fn decode_vertices(cell: &CollisionCell, lo: [i16; 3], hi: [i16; 3]) -> Vec<Vert> {
    cell.packed
        .iter()
        .map(|pv| {
            let [x, y, z] = pv.fields();
            let h = [(x << 6) as i16, (y << 6) as i16, (z << 4) as i16];
            let mut code = 0u32;
            for k in 0..3 {
                let below = h[k].wrapping_sub(lo[k]) < 0;
                let above = hi[k].wrapping_sub(h[k]) < 0;
                code |= (below as u32 | (above as u32) << 1) << (8 * k);
            }
            Vert { p: h.map(|v| itof0(v as i32)), code }
        })
        .collect()
}

/// The triangles the kernels test, in order: every face record as `(v0, v1, v2)` (the face count
/// is read with `lbu`, i.e. only its low byte), then the first `quad_count` records rewritten in
/// scratch as `(v0, v2, v3)`. So a quad is `(v0,v1,v2) + (v0,v2,v3)`.
fn cell_triangles(cell: &CollisionCell) -> impl Iterator<Item = ([u8; 3], u8)> + '_ {
    let f = (cell.header.face_count as u8 as usize).min(cell.faces.len());
    let q = (cell.header.quad_count as usize).min(cell.faces.len()).min(cell.quad_v3.len());
    let tris = cell.faces[..f].iter().map(|fc| (fc.v, fc.surface));
    let quads = cell.faces[..q].iter().zip(&cell.quad_v3).map(|(fc, &v3)| ([fc.v[0], fc.v[2], v3], fc.surface));
    tris.chain(quads)
}

/// Flag filters on the face type: 0x80 (type bit 7) and 0x20 (surface id == `(flags>>8)&0x1f`).
#[inline]
fn face_excluded(flags: u32, ty: u8) -> bool {
    (flags & 0x80 & ty as u32) != 0 || (flags & 0x20 != 0 && ((flags >> 8) ^ ty as u32) & 0x1f == 0)
}

/// Cell centre `c << 12 | 2048` in x1024 units.
#[inline]
fn centre_i(c: [i32; 3]) -> [i32; 3] { c.map(|v| (v << 12) + 2048) }

fn cell_of(mesh: &Collision, c: [i32; 3]) -> Option<&CollisionCell> {
    // Cell coordinates are 0..=255 here (queries are confined to [0, 1024)^3).
    let i = mesh.cell_at(c[0] as i16, c[1] as i16, c[2] as i16)?;
    let cell = &mesh.cells[i];
    // A cell whose vertex count is 0 is skipped (it cannot occur on the disc).
    (cell.header.vertex_count != 0).then_some(cell)
}

/// The running best hit, in the kernels' registers `vf24..vf29`, `s6`.
#[derive(Clone, Copy)]
pub(crate) struct Best {
    pub(crate) kind: i32,
    pub(crate) point: V3,
    pub(crate) normal: V3,
    pub(crate) tri: [V3; 3],
    /// `s5`: the moby of a moby hit (the world pass's `s5` is reset to 0 when the moby pass starts).
    pub(crate) moby: Option<usize>,
    /// Index of the hit moby primitive (the game only keeps `-(record address)` in `+0x1c`).
    pub(crate) prim: Option<usize>,
}

/// A triangle hit: `point` is `P - v0`, the vertices are relative to `centre` (the cell centre, or the
/// moby position x1024 for a moby mesh).
#[allow(clippy::too_many_arguments)]
pub(crate) fn record(ty: u8, point: V3, normal: V3, v0: V3, e1: V3, e2: V3, centre: V3, moby: Option<usize>) -> Best {
    let v0abs = vadd(v0, centre);
    Best {
        kind: 0x1000 + ty as i32,
        point: vadd(point, v0abs),
        normal,
        tri: [v0abs, vadd(e1, v0abs), vadd(e2, v0abs)],
        moby,
        prim: None,
    }
}

fn output(b: Best, best: F, pushed: Option<V3>) -> CollOutput {
    output_scaled(b, best, pushed, INV1024)
}

fn output_scaled(b: Best, best: F, pushed: Option<V3>, k: F) -> CollOutput {
    CollOutput {
        moby: b.moby,
        primitive: b.prim,
        kind: b.kind,
        point: v_to_f32(vmuls(b.point, k)),
        pushed_centre: pushed.map(v_to_f32),
        normal: v_to_f32(b.normal),
        tri: b.tri.map(|v| v_to_f32(vmuls(v, k))),
        best: to_f32(best),
    }
}

// ---------------------------------------------------------------------------------------------
// Line (CollLine_Fix)

/// One entry of the line's cell walk: the cell and the segment parameter at which it is entered.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineCell {
    /// Entry parameter along `p0 -> p1` (0 for the start cell), as the float the game compares.
    pub t_enter: f32,
    pub cell: [i32; 3],
}

/// `vmini`/`vmax` of the endpoints, then the `[0, 1024)^3` test the kernel reads from the VU0 MAC
/// sign flags: every `min - 0` must be non-negative and every `max - 1024` negative.
fn in_world(a: V3, b: V3) -> bool {
    (0..3).all(|k| !neg(sub(min(a[k], b[k]), 0)) && neg(sub(max(a[k], b[k]), K1024)))
}

/// The DDA cell list of the line kernel (0x2119b0..0x211b38), `(t bits, cell)` in walk order,
/// without the `t = 1.0` terminator. `None` for a zero-length segment: when both endpoints are in
/// the same cell *and* truncate to the same x1024 integer point, the kernel returns "no hit"
/// straight away (0x211b18), skipping the moby pass too.
fn line_cell_list(a1024: V3, b1024: V3) -> Option<Vec<(F, [i32; 3])>> {
    let ia = a1024.map(ftoi0);
    let ib = b1024.map(ftoi0);
    // psrlw 12: floor(p/4) for p >= 0.
    let s = ia.map(|v| ((v as u32) >> 12) as i32);
    let e = ib.map(|v| ((v as u32) >> 12) as i32);
    if s == e {
        if ia == ib { return None; }
        return Some(vec![(0, s)]);
    }
    let d = vsub(b1024, a1024);
    let mut lists: [Vec<F>; 3] = Default::default();
    let mut dir = [0i32; 3];
    for k in 0..3 {
        let q = div(ONE, d[k]); // vdiv Q, vf0w, vf1x
        let (mut at, end, step) = if e[k] - s[k] >= 0 { (s[k], e[k], 1) } else { (s[k] + 1, e[k] + 1, -1) };
        dir[k] = step;
        while at != end {
            at += step;
            // Plane k*4 in x1024 units, t = (plane - a) * (1/d), float bits clamped to [0, 1.0]
            // as signed integers (pminw/pmaxw): negatives -> +0.
            let t = mul(sub(itof0(at << 12), a1024[k]), q);
            lists[k].push((t as i32).min(ONE as i32).max(0) as u32);
        }
    }
    // Merge on the float bits (non-negative, so integer order = float order); ties x, y, z.
    let total = lists.iter().map(Vec::len).sum::<usize>();
    let mut out = Vec::with_capacity(total + 1);
    out.push((0, s));
    let mut cell = s;
    let mut idx = [0usize; 3];
    let at = |l: &Vec<F>, i: usize| l.get(i).copied().unwrap_or(DDA_END) as i32;
    for _ in 0..total {
        let (x, y, z) = (at(&lists[0], idx[0]), at(&lists[1], idx[1]), at(&lists[2], idx[2]));
        let k = if y - x < 0 { if z - y < 0 { 2 } else { 1 } } else if z - x < 0 { 2 } else { 0 };
        cell[k] += dir[k];
        out.push((at(&lists[k], idx[k]) as u32, cell));
        idx[k] += 1;
    }
    Some(out)
}

/// The cells [`coll_line`] visits for `p0 -> p1`, in order, with their entry parameters (the walk
/// stops at the first entry whose parameter is exactly 1.0, which is excluded here, or earlier once
/// a hit is closer than the next cell's entry). `None` when the segment leaves `[0, 1024)^3` or has
/// zero length (the query then returns no hit).
pub fn cells_for_line(p0: [f32; 3], p1: [f32; 3]) -> Option<Vec<LineCell>> {
    let (a, b) = (p0.map(f32::to_bits), p1.map(f32::to_bits));
    if !in_world(a, b) { return None; }
    let list = line_cell_list(vmuls(a, K1024), vmuls(b, K1024))?;
    Some(list.into_iter().take_while(|&(t, _)| t != ONE).map(|(t, cell)| LineCell { t_enter: f32::from_bits(t), cell }).collect())
}

/// Segment query `p0 -> p1` against the world mesh: `CollLine_Fix` (level01 `0x211870`, boot
/// `0x1efa68`). Returns the hit nearest `p0`, or `None`.
///
/// Per triangle `(v0, v1, v2)`: `N = (v2-v0) x (v1-v0)`, `s0 = (v0-a).N`, `s1 = (v0-b).N`,
/// `t = s0 / (d.N)`. One-sided unless [`QueryFlags::TWO_SIDED`]: needs `s0 < 0` (sign bit) and
/// `s1 > +0` (`a` on the +N side); the signs must differ in any case. The hit `P = (a - v0) + d*t`
/// must pass three edge tests `((P-v0)x(v1-v0)).N`, `((v2-v0)x(P-v0)).N`, `((P-v1)x(v2-v1)).N`, each
/// with the sign bit clear (inclusive of +0), and be strictly closer: `best - t > +0`, initial best 1.0.
/// Cells are walked in `t` order; once a hit exists the walk stops at the first cell entered at
/// `t >= best`.
pub fn coll_line(mesh: &Collision, p0: [f32; 3], p1: [f32; 3], flags: QueryFlags) -> Option<CollOutput> {
    coll_line_m(mesh, None, p0, p1, flags, None)
}

/// [`coll_line`] with the moby pass: `CollLine_Fix(p0, p1, flags, ignore, 0)` against the world mesh
/// (unless [`QueryFlags::SKIP_WORLD`]) and then the mobys of `scene` (`None`: world only), sharing one
/// running best (see [`mobys`] for the moby pass). The hit-log write of the fifth argument is
/// [`mobys::line_hit_record`].
pub fn coll_line_m(mesh: &Collision, scene: Option<&MobyScene>, p0: [f32; 3], p1: [f32; 3], flags: QueryFlags, ignore: Option<usize>) -> Option<CollOutput> {
    let fl = flags.0;
    let (a, b) = (p0.map(f32::to_bits), p1.map(f32::to_bits));
    if !in_world(a, b) { return None; }
    let a1024 = vmuls(a, K1024);
    let b1024 = vmuls(b, K1024);
    let list = line_cell_list(a1024, b1024)?;
    let d = vsub(b1024, a1024); // vf13
    let mut best_t = ONE; // vf24.x
    let mut best = if fl & 0x1 != 0 { None } else { line_world(mesh, fl, a1024, b1024, d, &list, &mut best_t) };
    if let Some(sc) = scene {
        best = mobys::line_pass(sc, fl, a1024, b1024, d, &list, ignore, &mut best_t, best);
    }
    best.map(|b| output(b, best_t, None))
}

/// The world-mesh walk of `CollLine_Fix` (0x211b58..0x211f94).
fn line_world(mesh: &Collision, fl: u32, a1024: V3, b1024: V3, d: V3, list: &[(F, [i32; 3])], best_t: &mut F) -> Option<Best> {
    let mut best: Option<Best> = None;
    for (i, &(t_enter, c)) in list.iter().enumerate() {
        let t_next = list.get(i + 1).map_or(ONE, |e| e.0);
        if t_enter == ONE { break; }
        if best.is_some() && (*best_t as i32) - (t_enter as i32) <= 0 { break; }
        let Some(cell) = cell_of(mesh, c) else { continue };
        // Sub-segment box of this cell's t-range, integer x1024 relative to the centre, +-1.
        let ci = centre_i(c);
        let pe = vmadds(a1024, d, t_enter).map(ftoi0);
        let pn = vmadds(a1024, d, t_next).map(ftoi0);
        let mut lo = [0i16; 3];
        let mut hi = [0i16; 3];
        for k in 0..3 {
            let (e, n) = (pe[k].wrapping_sub(ci[k]), pn[k].wrapping_sub(ci[k]));
            lo[k] = e.min(n).wrapping_sub(1) as i16;
            hi[k] = e.max(n).wrapping_add(1) as i16;
        }
        let centre = ci.map(itof0); // vf16
        let a_rel = vsub(a1024, centre); // vf14
        let b_rel = vsub(b1024, centre); // vf15
        let verts = decode_vertices(cell, lo, hi);
        for (vi, ty) in cell_triangles(cell) {
            let [v0, v1, v2] = vi.map(|i| verts[i as usize]);
            if v0.code & v1.code & v2.code != 0 || face_excluded(fl, ty) { continue; }
            let Some((q, p, n, e1, e2)) = line_tri(fl, d, a_rel, b_rel, v0.p, v1.p, v2.p, *best_t) else { continue };
            *best_t = q;
            best = Some(record(ty, p, n, v0.p, e1, e2, centre, None));
        }
    }
    best
}

/// Line vs one triangle (the block after the face filters in `CollLine_Fix`, shared by the world cells and
/// the moby meshes): `(t, P − v0, N, e1, e2)` when the segment crosses it strictly before `best_t`.
/// `a_rel`/`b_rel` are the endpoints relative to the frame the vertices are in, `d = b − a`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn line_tri(fl: u32, d: V3, a_rel: V3, b_rel: V3, v0: V3, v1: V3, v2: V3, best_t: F) -> Option<(F, V3, V3, V3, V3)> {
    let e1 = vsub(v1, v0);
    let e2 = vsub(v2, v0);
    let n = cross(e2, e1);
    let dn = dot_x(d, n);
    let s0 = dot_x(vsub(v0, a_rel), n);
    let s1 = dot_x(vsub(v0, b_rel), n);
    let q = div(s0, dn);
    if fl & 0x10 == 0 && (!neg(s0) || s1 as i32 <= 0) { return None; }
    if !neg(s0 ^ s1) { return None; }
    let p = vmadds(vsub(a_rel, v0), d, q); // P - v0
    let c0 = dot_x(cross(p, e1), n);
    let c1 = dot_x(cross(e2, p), n);
    let c2 = dot_x(cross(vsub(p, e1), vsub(e2, e1)), n);
    let margin = sub(best_t, q);
    if neg(c0) || neg(c1) || neg(c2) || margin as i32 <= 0 { return None; }
    Some((q, p, n, e1, e2))
}

// ---------------------------------------------------------------------------------------------
// Sphere and capsule: shared cell gathering.

/// Common setup of the sphere (0x212960) and capsule (0x2135a0) kernels.
struct Volume {
    /// `c * 1024` (vf31): the sphere centre / capsule base centre.
    c1024: V3,
    /// Query AABB in world units (vf19/vf20 before scaling) and x1024 units (after).
    lo: V3,
    hi: V3,
    lo1024: V3,
    hi1024: V3,
    /// `r * 1024` (vf30.x), `h * 1024` (vf30.z; capsule only).
    r1024: F,
    h1024: F,
    /// `(r*1024)^2` (vf24.x before the shrink).
    rr: F,
}

impl Volume {
    /// Bounds checks: `c - r >= 0` and `hi - 1024 < 0` on every axis (MAC sign flags),
    /// `r > +0` (and `h > +0` for the capsule) as integers.
    fn new(centre: [f32; 3], radius: f32, height: Option<f32>) -> Option<Self> {
        let c = centre.map(f32::to_bits);
        let r = radius.to_bits();
        let lo = c.map(|v| sub(v, r));
        let mut hi = c.map(|v| add(v, r));
        if let Some(h) = height { hi[2] = add(hi[2], h.to_bits()); }
        let r1024 = mul(r, K1024);
        if lo.iter().any(|&v| neg(sub(v, 0))) { return None; }
        if r as i32 <= 0 { return None; }
        if !hi.iter().all(|&v| neg(sub(v, K1024))) { return None; }
        if let Some(h) = height { if h.to_bits() as i32 <= 0 { return None; } }
        Some(Self {
            c1024: vmuls(c, K1024),
            lo,
            hi,
            lo1024: vmuls(lo, K1024),
            hi1024: vmuls(hi, K1024),
            r1024,
            h1024: height.map_or(0, |h| mul(h.to_bits(), K1024)),
            rr: mul(r1024, r1024),
        })
    }

    /// Every cell of the AABB (`vftoi0(p) >> 2`), z outer, y, x inner, kept when the squared
    /// distance from `c1024` to the cell box is below `r^2`:
    /// `((-r^2 + dx^2) + dy^2) + dz^2 < 0` (unshrunk r^2). For the capsule this is the base sphere
    /// only: the AABB includes the height but the test ignores it.
    fn cells(&self) -> Vec<[i32; 3]> {
        let lo = self.lo.map(|v| ((ftoi0(v) as u32) >> 2) as i32);
        let hi = self.hi.map(|v| ((ftoi0(v) as u32) >> 2) as i32);
        let nrr = sub(0, self.rr); // vf14.y
        let mut out = Vec::new();
        for z in lo[2]..=hi[2] {
            for y in lo[1]..=hi[1] {
                for x in lo[0]..=hi[0] {
                    let cmin = [x, y, z].map(|v| itof0(v << 12));
                    let cmax = cmin.map(|v| add(v, K4096));
                    let p = [0, 1, 2].map(|k| min(max(cmin[k], self.c1024[k]), cmax[k]));
                    let sq = vmul(vsub(p, self.c1024), vsub(p, self.c1024));
                    let s = add(add(add(nrr, sq[0]), mul(ONE, sq[1])), mul(ONE, sq[2]));
                    if neg(s) { out.push([x, y, z]); }
                }
            }
        }
        out
    }

    /// Per-cell setup: centre (vf23), `c - centre` (vf22) and the outcode box (vftoi0 of the
    /// AABB relative to the centre, truncated to 16 bits; no +-1 margin here).
    fn cell_frame(&self, c: [i32; 3]) -> (V3, V3, [i16; 3], [i16; 3]) {
        let centre = c.map(|v| add(itof0(v << 12), K2048));
        let lo = vsub(self.lo1024, centre).map(|v| ftoi0(v) as i16);
        let hi = vsub(self.hi1024, centre).map(|v| ftoi0(v) as i16);
        (centre, vsub(self.c1024, centre), lo, hi)
    }

    /// The push-out on return (0x213510 / 0x2143c8): `Q = r / sqrt(best)` (vrsqrt),
    /// sphere: `pushed = ((c - hit) * Q) / 1024 + hit / 1024`; capsule: the same with the axis point
    /// at height `k = clamp(hit.z - c.z, 0, h)` and `k` subtracted again after scaling.
    fn pushed(&self, b: &Best, best: F, capsule: bool) -> V3 {
        let q = div(self.r1024, sqrt(best));
        let mut v = vsub(self.c1024, b.point);
        let mut k = 0;
        if capsule {
            let up = sub(b.point[2], self.c1024[2]);
            k = max(min(up, self.h1024), 0);
            v[2] = add(v[2], k);
        }
        v = vmuls(v, q);
        if capsule { v[2] = sub(v[2], k); }
        vadd(vmuls(v, INV1024), vmuls(b.point, INV1024))
    }
}

/// The cells [`coll_sphere`] tests, in order (before the mesh lookup drops empty ones), or `None`
/// when the sphere is not inside `[0, 1024)^3` or `radius <= 0`.
pub fn cells_for_sphere(centre: [f32; 3], radius: f32) -> Option<Vec<[i32; 3]>> {
    Some(Volume::new(centre, radius, None)?.cells())
}

/// The cells [`coll_capsule`] tests: those within `radius` of the base centre (the height only
/// widens the enumerated AABB, not the distance test).
pub fn cells_for_capsule(base: [f32; 3], height: f32, radius: f32) -> Option<Vec<[i32; 3]>> {
    Some(Volume::new(base, radius, Some(height))?.cells())
}

/// Closest point on edge `A -> B` to `p` (all relative to v0): `t = ((p-A).(B-A)) / |B-A|^2`
/// clamped to [0, 1] (vmax/vmini), `A + (B-A)*t`.
fn edge_point(p: V3, a: V3, b: V3) -> V3 {
    let ba = vsub(b, a);
    let t = div(dot_x(vsub(p, a), ba), dot_x(ba, ba));
    let t = min(max(add(0, t), 0), ONE);
    vmadds(a, ba, t)
}

/// The shared tail of the sphere/capsule triangle test: `p` is the plane projection (relative to
/// v0) with squared plane distance `d2`; the three edge tests pick the first failing edge
/// (v0v1, then v2v0, then v1v2) and the closest point on it to `p`, whose squared distance to
/// `from` (the centre / axis point, relative to v0) must not exceed `best`.
/// Returns the closest point and its squared distance.
fn closest_on_triangle(p: V3, d2: F, from: V3, e1: V3, e2: V3, n: V3, best: F) -> Option<(V3, F)> {
    if neg(sub(best, d2)) { return None; }
    let c0 = dot_y(cross(p, e1), n);
    let c1 = dot_y(cross(e2, p), n);
    let c2 = dot_y(cross(vsub(p, e1), vsub(e2, e1)), n);
    let (a, b) = if neg(c0) {
        ([0; 3], e1)
    } else if neg(c1) {
        (e2, [0; 3])
    } else if !neg(c2) {
        return Some((p, d2));
    } else {
        (e1, e2)
    };
    let q = edge_point(p, a, b);
    let diff = vsub(q, from);
    let d2 = dot_x(diff, diff);
    if neg(sub(best, d2)) { return None; }
    Some((q, d2))
}

/// Sphere vs one triangle (0x212d88..0x212f34, shared by the world cells, the moby meshes and — with a
/// best that never shrinks — `coll_sphere_mobys`): `(closest point − v0, d², N, e1, e2)` when `d² ≤ best`.
/// `c_rel` is the centre relative to the vertices' frame.
pub(crate) fn sphere_tri(fl: u32, c_rel: V3, v0: V3, v1: V3, v2: V3, best_d: F) -> Option<(V3, F, V3, V3, V3)> {
    let e1 = vsub(v1, v0);
    let e2 = vsub(v2, v0);
    let w = vsub(c_rel, v0); // vf4
    let n = cross(e2, e1);
    let s = dot_x(w, n);
    let nn = dot_x(n, n);
    let q = div(s, nn);
    if fl & 0x10 == 0 && neg(s) { return None; }
    let nq = vmuls(n, q);
    let p = vmadds(w, vsub([0; 3], n), q); // ACC = w; + (-N) * Q
    let sq = vmul(nq, nq);
    let d2 = add(add(sq[0], sq[1]), mul(ONE, sq[2]));
    let (pt, d2) = closest_on_triangle(p, d2, w, e1, e2, n, best_d)?;
    Some((pt, d2, n, e1, e2))
}

/// Capsule vs one triangle (0x2139a8.., shared by the world cells and the moby meshes): `bot` is the
/// capsule base relative to the vertices' frame, `h` the height (x1024).
pub(crate) fn capsule_tri(fl: u32, c_rel: V3, h: F, v0: V3, v1: V3, v2: V3, best_d: F) -> Option<(V3, F, V3, V3, V3)> {
    let e1 = vsub(v1, v0);
    let e2 = vsub(v2, v0);
    let bot = vsub(c_rel, v0); // vf4: base - v0
    let mut top = bot; // vf5: top - v0
    top[2] = add(top[2], h);
    let n = cross(e2, e1);
    let sb = dot_x(bot, n);
    let st = dot_x(top, n);
    if fl & 0x10 == 0 && neg(sb & st) { return None; }
    let nn = dot_x(n, n);
    // Axis point (vf13) and its plane offset N*Q (vf8).
    let axis_from = |pz: F| {
        // 0x213bd8: k = clamp(pz - base.z, 0, h); axis = (base.x, base.y, k + base.z).
        let k = max(min(sub(pz, bot[2]), h), 0);
        let ax = [bot[0], bot[1], add(k, bot[2])];
        (ax, vmuls(n, div(dot_x(ax, n), nn)))
    };
    let (axis, nq) = if ((n[2] & !SIGN) as i32).wrapping_sub(ONE as i32) < 0 {
        axis_from(0) // vertical face: height of v0 (relative z = 0)
    } else {
        let q = div(sb, mul(h, n[2]));
        let b1z = sub(bot[2], e1[2]);
        let b2z = sub(bot[2], e2[2]);
        let t1z = sub(top[2], e1[2]);
        let t2z = sub(top[2], e2[2]);
        if !neg(bot[2] | b1z | b2z) {
            (bot, vmuls(n, div(sb, nn)))
        } else if neg(top[2] & t1z & t2z) {
            (top, vmuls(n, div(st, nn)))
        } else {
            // Axis/plane intersection X = (base.xy, base.z - h*Q).
            let x = [bot[0], bot[1], sub(bot[2], mul(h, q))];
            let c0 = dot_y(cross(x, e1), n);
            let c1 = dot_y(cross(e2, x), n);
            let c2 = dot_y(cross(vsub(x, e1), vsub(e2, e1)), n);
            let p = if neg(c0) {
                edge_point(x, [0; 3], e1)
            } else if neg(c1) {
                edge_point(x, e2, [0; 3])
            } else if neg(c2) {
                edge_point(x, e1, e2)
            } else {
                x
            };
            axis_from(p[2])
        }
    };
    let sq = vmul(nq, nq);
    let d2 = add(add(sq[0], sq[1]), mul(ONE, sq[2]));
    let p = vsub(axis, nq);
    let (pt, d2) = closest_on_triangle(p, d2, axis, e1, e2, n, best_d)?;
    Some((pt, d2, n, e1, e2))
}

/// Sphere query: level01 `0x212960` (`f12 = r`, `a0 = &centre`, `a1 = flags`). Returns the face
/// closest to `centre` within `radius`, with the centre pushed out to touch it.
///
/// Per triangle: `N = (v2-v0) x (v1-v0)`; one-sided unless [`QueryFlags::TWO_SIDED`]: needs
/// `s = (c-v0).N` with the sign bit clear. Project: `P = (c-v0) - N*(s/N.N)`, `d^2 = |N*(s/N.N)|^2`;
/// if an edge test fails, `P` moves to the closest point of the first failing edge and `d^2` is
/// re-measured from `c`. Accept when `d^2 <= best` (non-strict), then `best = 0.99951 * d^2`
/// (initial `best = 0.99951 * r^2`). All gathered cells are tested (no early out). World mesh only;
/// [`coll_sphere_m`] adds the moby pass.
pub fn coll_sphere(mesh: &Collision, centre: [f32; 3], radius: f32, flags: QueryFlags) -> Option<CollOutput> {
    coll_sphere_m(mesh, None, centre, radius, flags, None)
}

/// [`coll_sphere`] with the moby pass (`a2 = ignore`): the world mesh (unless
/// [`QueryFlags::SKIP_WORLD`]), then the mobys of `scene` sharing the running best.
pub fn coll_sphere_m(mesh: &Collision, scene: Option<&MobyScene>, centre: [f32; 3], radius: f32, flags: QueryFlags, ignore: Option<usize>) -> Option<CollOutput> {
    let fl = flags.0;
    let vol = Volume::new(centre, radius, None)?;
    let mut best_d = mul(vol.rr, BEST_SHRINK);
    let mut best: Option<Best> = None;
    if fl & 0x1 == 0 {
        for c in vol.cells() {
            let Some(cell) = cell_of(mesh, c) else { continue };
            let (centre_f, c_rel, lo, hi) = vol.cell_frame(c);
            let verts = decode_vertices(cell, lo, hi);
            for (vi, ty) in cell_triangles(cell) {
                let [v0, v1, v2] = vi.map(|i| verts[i as usize]);
                if v0.code & v1.code & v2.code != 0 || face_excluded(fl, ty) { continue; }
                let Some((pt, d2, n, e1, e2)) = sphere_tri(fl, c_rel, v0.p, v1.p, v2.p, best_d) else { continue };
                best_d = mul(d2, BEST_SHRINK);
                best = Some(record(ty, pt, n, v0.p, e1, e2, centre_f, None));
            }
        }
    }
    if let Some(sc) = scene {
        best = mobys::volume_pass(sc, fl, &vol, None, ignore, &mut best_d, best);
    }
    best.map(|b| output(b, best_d, Some(vol.pushed(&b, best_d, false))))
}

/// Vertical capsule query (the hero body): level01 `0x2135a0` (`f12 = r`, `f13 = h`,
/// `a0 = &base`, `a1 = flags`). The capsule is the segment `base .. base + (0,0,h)` swept by
/// `radius`. Returns the closest face and the base centre pushed out so the capsule touches it.
///
/// Per triangle (`sb`, `st` = base/top plane sides): one-sided unless [`QueryFlags::TWO_SIDED`]:
/// rejected only when both `sb` and `st` are negative. The axis point is chosen as: vertical face
/// (`|N.z| < 1`): height of v0; base above all three vertices: base; top below all three: top;
/// otherwise the axis/plane intersection `X`, moved to the closest point of the first failing edge,
/// and its height clamped to the axis. That axis point is then treated like the sphere centre
/// (plane projection, edge fallback, `d^2 <= best`, `best = 0.99951 * d^2`).
///
/// Cell gathering tests only the base sphere (see [`cells_for_capsule`]). World mesh only;
/// [`coll_capsule_m`] adds the moby pass.
pub fn coll_capsule(mesh: &Collision, base: [f32; 3], height: f32, radius: f32, flags: QueryFlags) -> Option<CollOutput> {
    coll_capsule_m(mesh, None, base, height, radius, flags, None)
}

/// [`coll_capsule`] with the moby pass (`a2 = ignore`; the hero passes Ratchet's moby).
pub fn coll_capsule_m(mesh: &Collision, scene: Option<&MobyScene>, base: [f32; 3], height: f32, radius: f32, flags: QueryFlags, ignore: Option<usize>) -> Option<CollOutput> {
    let fl = flags.0;
    let vol = Volume::new(base, radius, Some(height))?;
    let h = vol.h1024;
    let mut best_d = mul(vol.rr, BEST_SHRINK);
    let mut best: Option<Best> = None;
    if fl & 0x1 == 0 {
        for c in vol.cells() {
            let Some(cell) = cell_of(mesh, c) else { continue };
            let (centre_f, c_rel, lo, hi) = vol.cell_frame(c);
            let verts = decode_vertices(cell, lo, hi);
            for (vi, ty) in cell_triangles(cell) {
                let [v0, v1, v2] = vi.map(|i| verts[i as usize]);
                if v0.code & v1.code & v2.code != 0 || face_excluded(fl, ty) { continue; }
                let Some((pt, d2, n, e1, e2)) = capsule_tri(fl, c_rel, h, v0.p, v1.p, v2.p, best_d) else { continue };
                best_d = mul(d2, BEST_SHRINK);
                best = Some(record(ty, pt, n, v0.p, e1, e2, centre_f, None));
            }
        }
    }
    if let Some(sc) = scene {
        best = mobys::volume_pass(sc, fl, &vol, Some(h), ignore, &mut best_d, best);
    }
    best.map(|b| output(b, best_d, Some(vol.pushed(&b, best_d, true))))
}

/// Sphere vs the hero-only collision groups (level01 `0x214d70`, `f12 = r`, `a0 = &centre`), run by hero
/// movement after each capsule pass. Everything at x64 (`0x42800000`), no world-box, radius or stamp checks:
/// 1. groups culled by their u16 bounding sphere: `((dy² − (r+R)²) + dx²) + dz²` kept unless its sign bit is
///    clear and it is not the pair (+0, `dx² = +0`) (the kernel tests the y:x lanes as one 64-bit word);
/// 2. per kept group (in group order): vertices u16 **zero-extended** absolute x64, 16-bit outcodes against
///    `vftoi0(c·64 ∓ r·64)` (no margin, no cell centre); faces `0..lbu(+8)` (the triangle count as a byte);
/// 3. the sphere kernel's triangle test, always one-sided, no type filters, initial best `(r·64)²`
///    **unshrunk**, each accepted `best = 0.99951·d²`.
///
/// Output: `moby = 0`, `kind = 0x1000 | pad byte` (0 on the disc: 0x1000), everything scaled by 1/64,
/// `+0x30 = ((c − hit)·(r/√best))/64 + hit/64`.
pub fn coll_sphere_hero_groups(mesh: &Collision, centre: [f32; 3], radius: f32) -> Option<CollOutput> {
    const K64: F = 0x4280_0000;
    const INV64: F = 0x3c80_0000;
    if mesh.hero_groups.is_empty() { return None; }
    let r64 = mul(radius.to_bits(), K64); // vf30.x
    let c64 = vmuls(centre.map(f32::to_bits), K64); // vf31
    let lo = c64.map(|v| sub(v, r64)); // vf19
    let hi = c64.map(|v| add(v, r64)); // vf20
    let mut best_d = mul(r64, r64); // vf24.x, not shrunk
    let kept: Vec<&rc_formats::collision::HeroGroup> = mesh
        .hero_groups
        .iter()
        .filter(|g| {
            let s = g.header.sphere.map(|v| itof0(v as i32));
            let rr = add(r64, s[3]);
            let d = vsub([s[0], s[1], s[2]], c64);
            let sq = vmul(d, d);
            let v = add(add(sub(sq[1], mul(rr, rr)), mul(ONE, sq[0])), mul(ONE, sq[2]));
            // bgtz on (v.y : dx²) as one 64-bit word.
            !(!neg(v) && !(v == 0 && sq[0] == 0))
        })
        .collect();
    let lo16 = lo.map(|v| ftoi0(v) as i16);
    let hi16 = hi.map(|v| ftoi0(v) as i16);
    let mut best: Option<Best> = None;
    for g in kept {
        let nv = g.header.vertex_count as usize;
        let verts: Vec<Vert> = g
            .vertices
            .iter()
            .take(nv)
            .map(|v| {
                let h = v.xyz.map(|c| c as i16);
                let mut code = 0u32;
                for k in 0..3 {
                    let below = h[k].wrapping_sub(lo16[k]) < 0;
                    let above = hi16[k].wrapping_sub(h[k]) < 0;
                    code |= (below as u32 | (above as u32) << 1) << (8 * k);
                }
                Vert { p: v.xyz.map(|c| itof0(c as i32)), code }
            })
            .collect();
        let nt = (g.header.triangle_count as u8 as usize).min(g.triangles.len());
        for t in &g.triangles[..nt] {
            let [Some(v0), Some(v1), Some(v2)] = t.v.map(|i| verts.get(i as usize).copied()) else { continue };
            if v0.code & v1.code & v2.code != 0 { continue; }
            let Some((pt, d2, n, e1, e2)) = sphere_tri(0, c64, v0.p, v1.p, v2.p, best_d) else { continue };
            best_d = mul(d2, BEST_SHRINK);
            best = Some(record(t.pad, pt, n, v0.p, e1, e2, [0; 3], None));
        }
    }
    let b = best?;
    let q = div(r64, sqrt(best_d)); // vrsqrt Q, vf30.x, vf24.x
    let v = vmuls(vmuls(vsub(c64, b.point), q), INV64);
    let pushed = vadd(v, vmuls(b.point, INV64));
    Some(output_scaled(b, best_d, Some(pushed), INV64))
}

#[cfg(test)]
mod tests;
